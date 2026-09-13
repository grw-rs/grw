//! On-disk snapshot format (v3), shared by `MGraph::save`/`load` and
//! `VGraph::save`/`load`. All integers little-endian.
//!
//! `header`: magic `GRW\0`, `version: u16` (= 3), `graph_kind: u8` (0 = M,
//! 1 = V), `edge_kind: u8`, `node_count/edge_count: u64`, `nv_hash/ev_hash:
//! u64`, `nv_type_len/ev_type_len: u16`, then the `nv_type`/`ev_type` UTF-8
//! bytes, then `section_count: u16` and that many `(tag: u8, offset: u64,
//! len: u64)` section-table entries (`SectionTag`; always all 7, even when a
//! section is empty).
//!
//! `nodes`/`edges`: fixed-width rows (`slot, gen, ...`) for live nodes/edges
//! only, sorted by slot ascending. `gen` is real for a `VGraph`-sourced file
//! and always 0 for an `MGraph`-sourced one — the format never branches on
//! `graph_kind`, only its meaning does (see `MGraph::load_with`'s doc
//! comment for the M-side of that, `VGraph::from_mgraph`'s for the V-side).
//!
//! `adjacency`: `(neighbor: u32, edge: u32)` pairs, contiguous per node in
//! node order; a node's row carries a byte offset into this section and a
//! pair count.
//!
//! `values`: one heap of bincode-1 bytes (node values in node order, then
//! edge values in edge order), addressed by byte offset + byte length from
//! the node/edge rows above.
//!
//! `free`: one bincode-1 blob (`FreeSectionBody`) — `node_free`/`edge_free`
//! are each an `IdSet` of the *holes* below `next_node`/`next_edge` (every
//! slot `< next` that is not currently live), not the full allocator state;
//! `IdSpace::new_starting_at(next)` plus replaying those holes reconstructs
//! `MGraph`'s allocator exactly, and `VGraph`'s `next_node`/`next_edge` are
//! already exactly this bump-counter shape.
//!
//! `indices`: a catalogue header (`count: u16`, then per index `name` (u16
//! length-prefixed), `cardinality: u8`, `tag: u64`) — sized so `read_header`
//! can stop after it without reading any table — followed, in the same
//! order, by one table per index: `entry_count: u32` (self-delimits the
//! table inside the shared section, since only the whole section has a
//! length in the section table) then that many entries of `key_len: u32` +
//! key bytes + either a raw `u32` node id (Unique) or `len: u32` +
//! bincode-1 `IdSet` bytes (Multi), sorted by key bytes.
//!
//! `trailer`: 32 bytes, SHA-256 of every byte before it (header through
//! `indices`, including the section table) — verified before any section
//! body is parsed, so a flipped byte inside a section always surfaces as a
//! trailer mismatch, never a downstream parse error. The fixed header and
//! the section table are parsed before that check, so corruption there
//! surfaces as a structural error instead ("invalid magic", "unsupported
//! version", "unknown section tag", a truncated or out-of-range section
//! range). Every reader wraps its errors with the file's path.
//!
//! A v2 file (the pre-v3 whole-struct bincode dump) is rejected by `load`/
//! `load_with`/`read_header` with a message naming `convert`, which reads
//! it and writes it out in this format.

use super::layout;
use super::node::Adjacents;
use super::{index, Edge, EdgeRec, Edges, FxHashMap, IdSpace, Node, Nodes};

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::{id, Id};

const MAGIC: [u8; 4] = *b"GRW\0";
const VERSION: u16 = 3;
const VERSION_V2: u16 = 2;
const HEADER_FIXED: usize = 44;
const SECTION_ENTRY_LEN: usize = 1 + 8 + 8;
const NODE_ROW_LEN: usize = 4 + 8 + 8 + 4 + 8 + 4;
const EDGE_ROW_LEN: usize = 4 + 8 + 4 + 4 + 1 + 8 + 4;

type NodeRow = (u32, u64, u64, u32, u64, u32);
type EdgeRow = (u32, u64, u32, u32, u8, u64, u32);
pub(crate) type UniqueEntries = Vec<Vec<(index::KeyBytes, id::N)>>;
pub(crate) type MultiEntries = Vec<Vec<(index::KeyBytes, index::IdSet<id::N>)>>;
type IndexRestoreParts<NV> = (Vec<index::IndexDecl<NV>>, UniqueEntries, MultiEntries);

// `Id` is `u32` under the default `id32` feature (making this conversion a
// no-op clippy would rather see as a plain cast) and `u64` under `id64`,
// where it is a real, checked narrowing — the persisted slot width is fixed
// at u32 regardless of feature.
#[allow(clippy::useless_conversion)]
fn raw_u32(id: Id) -> u32 {
    u32::try_from(id).expect("node/edge id fits in the persisted u32 slot width")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionTag {
    Nodes,
    Edges,
    Adjacency,
    Values,
    Free,
    Indices,
    Trailer,
}

impl SectionTag {
    fn to_u8(self) -> u8 {
        match self {
            SectionTag::Nodes => 0,
            SectionTag::Edges => 1,
            SectionTag::Adjacency => 2,
            SectionTag::Values => 3,
            SectionTag::Free => 4,
            SectionTag::Indices => 5,
            SectionTag::Trailer => 6,
        }
    }

    fn from_u8(byte: u8) -> io::Result<Self> {
        match byte {
            0 => Ok(SectionTag::Nodes),
            1 => Ok(SectionTag::Edges),
            2 => Ok(SectionTag::Adjacency),
            3 => Ok(SectionTag::Values),
            4 => Ok(SectionTag::Free),
            5 => Ok(SectionTag::Indices),
            6 => Ok(SectionTag::Trailer),
            _ => Err(io::Error::new(io::ErrorKind::InvalidData, format!("unknown section tag {byte}"))),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SectionInfo {
    pub tag: SectionTag,
    pub offset: u64,
    pub len: u64,
}

#[derive(Debug, Clone)]
pub struct CatalogueEntry {
    pub name: String,
    pub cardinality: index::Cardinality,
    pub tag: index::KeyTag,
}

pub struct Header {
    pub version: u16,
    pub graph_kind: u8,
    pub edge_kind: u8,
    pub node_count: u64,
    pub edge_count: u64,
    pub nv_layout_hash: u64,
    pub ev_layout_hash: u64,
    pub nv_type: String,
    pub ev_type: String,
    pub sections: Vec<SectionInfo>,
    pub catalogue: Vec<CatalogueEntry>,
}

pub struct ConvertReport {
    pub node_count: u64,
    pub edge_count: u64,
}

fn cardinality_to_u8(c: index::Cardinality) -> u8 {
    match c {
        index::Cardinality::Unique => 0,
        index::Cardinality::Multi => 1,
    }
}

fn cardinality_from_u8(byte: u8) -> io::Result<index::Cardinality> {
    match byte {
        0 => Ok(index::Cardinality::Unique),
        1 => Ok(index::Cardinality::Multi),
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, format!("unknown index cardinality {byte}"))),
    }
}

pub(crate) struct RawNode {
    pub(crate) slot: u32,
    pub(crate) r#gen: u64,
    pub(crate) adj: Vec<(u32, u32)>,
    pub(crate) val_bytes: Vec<u8>,
}

pub(crate) struct RawEdge {
    pub(crate) slot: u32,
    pub(crate) r#gen: u64,
    pub(crate) n1: u32,
    pub(crate) n2: u32,
    pub(crate) slot_kind: u8,
    pub(crate) val_bytes: Vec<u8>,
}

pub(crate) enum RawIndexValue {
    One(u32),
    Many(Vec<u8>),
}

pub(crate) struct RawIndexTable {
    pub(crate) name: String,
    pub(crate) cardinality: u8,
    pub(crate) tag: u64,
    pub(crate) entries: Vec<(Vec<u8>, RawIndexValue)>,
}

pub(crate) struct SnapshotWrite {
    pub(crate) graph_kind: u8,
    pub(crate) edge_kind: u8,
    pub(crate) nv_type: String,
    pub(crate) ev_type: String,
    pub(crate) nv_hash: u64,
    pub(crate) ev_hash: u64,
    pub(crate) nodes: Vec<RawNode>,
    pub(crate) edges: Vec<RawEdge>,
    pub(crate) node_free: Vec<u32>,
    pub(crate) edge_free: Vec<u32>,
    pub(crate) next_node: u32,
    pub(crate) next_edge: u32,
    pub(crate) version: u64,
    pub(crate) indices: Vec<RawIndexTable>,
}

pub(crate) struct SnapshotRead {
    pub(crate) header: Header,
    pub(crate) nodes: Vec<RawNode>,
    pub(crate) edges: Vec<RawEdge>,
    pub(crate) node_free: Vec<u32>,
    pub(crate) edge_free: Vec<u32>,
    pub(crate) next_node: u32,
    pub(crate) next_edge: u32,
    pub(crate) version: u64,
    pub(crate) indices: Vec<RawIndexTable>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FreeSectionBody {
    node_free: index::IdSet<id::N>,
    edge_free: index::IdSet<id::E>,
    next_node: u32,
    next_edge: u32,
    version: u64,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }

    fn need(&self, n: usize) -> io::Result<()> {
        if self.pos + n > self.data.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "file truncated"));
        }
        Ok(())
    }

    fn u8(&mut self) -> io::Result<u8> {
        self.need(1)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn u16(&mut self) -> io::Result<u16> {
        self.need(2)?;
        let v = u16::from_le_bytes(self.data[self.pos..self.pos + 2].try_into().unwrap());
        self.pos += 2;
        Ok(v)
    }

    fn u32(&mut self) -> io::Result<u32> {
        self.need(4)?;
        let v = u32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }

    fn u64(&mut self) -> io::Result<u64> {
        self.need(8)?;
        let v = u64::from_le_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Ok(v)
    }

    fn bytes(&mut self, n: usize) -> io::Result<&'a [u8]> {
        self.need(n)?;
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }
}

fn utf8(bytes: &[u8]) -> io::Result<String> {
    std::str::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)).map(str::to_owned)
}

struct FixedHeader {
    version: u16,
    graph_kind: u8,
    edge_kind: u8,
    node_count: u64,
    edge_count: u64,
    nv_hash: u64,
    ev_hash: u64,
    nv_type: String,
    ev_type: String,
}

fn read_fixed_header(r: &mut Reader) -> io::Result<FixedHeader> {
    let magic = r.bytes(4)?;
    if magic != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic"));
    }
    let version = r.u16()?;
    if version == VERSION_V2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported version: {version} (expected {VERSION}); this is a v2 file, convert it first with `grw::graph::persist::convert`"
            ),
        ));
    }
    if version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported version: {version} (expected {VERSION})"),
        ));
    }
    let graph_kind = r.u8()?;
    let edge_kind = r.u8()?;
    let node_count = r.u64()?;
    let edge_count = r.u64()?;
    let nv_hash = r.u64()?;
    let ev_hash = r.u64()?;
    let nv_len = r.u16()? as usize;
    let ev_len = r.u16()? as usize;
    let nv_type = utf8(r.bytes(nv_len)?)?;
    let ev_type = utf8(r.bytes(ev_len)?)?;
    Ok(FixedHeader { version, graph_kind, edge_kind, node_count, edge_count, nv_hash, ev_hash, nv_type, ev_type })
}

fn read_section_table(r: &mut Reader) -> io::Result<Vec<SectionInfo>> {
    let count = r.u16()? as usize;
    let mut sections = Vec::with_capacity(count);
    for _ in 0..count {
        let tag = SectionTag::from_u8(r.u8()?)?;
        let offset = r.u64()?;
        let len = r.u64()?;
        sections.push(SectionInfo { tag, offset, len });
    }
    Ok(sections)
}

fn section_slice<'a>(data: &'a [u8], sections: &[SectionInfo], tag: SectionTag) -> io::Result<&'a [u8]> {
    let s = sections
        .iter()
        .find(|s| s.tag == tag)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("missing section {tag:?}")))?;
    // `offset`/`len` come straight out of the file's section table: a
    // corrupt table can name a range that overflows `usize` or runs past the
    // end of the mapping, so both are checked before the slice is taken.
    let start = usize::try_from(s.offset)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, format!("section {tag:?} offset out of range")))?;
    let len = usize::try_from(s.len)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, format!("section {tag:?} length out of range")))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("section {tag:?} range overflows")))?;
    if end > data.len() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file truncated"));
    }
    Ok(&data[start..end])
}

fn verify_trailer(data: &[u8], sections: &[SectionInfo]) -> io::Result<()> {
    let trailer = sections
        .iter()
        .find(|s| s.tag == SectionTag::Trailer)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing trailer section"))?;
    // Same untrusted arithmetic as `section_slice`, and this is the check
    // that would otherwise have caught the corruption, so it cannot panic.
    let start = usize::try_from(trailer.offset)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "trailer offset out of range"))?;
    let len = usize::try_from(trailer.len)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "trailer length out of range"))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "trailer range overflows"))?;
    if trailer.len != 32 || end > data.len() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file truncated in trailer"));
    }
    let mut hasher = Sha256::new();
    hasher.update(&data[..start]);
    let actual = hasher.finalize();
    if actual.as_slice() != &data[start..end] {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "trailer hash mismatch: file is corrupted"));
    }
    Ok(())
}

fn read_catalogue_header(r: &mut Reader) -> io::Result<Vec<(String, u8, u64)>> {
    let count = r.u16()? as usize;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let name_len = r.u16()? as usize;
        let name = utf8(r.bytes(name_len)?)?;
        let cardinality = r.u8()?;
        let tag = r.u64()?;
        out.push((name, cardinality, tag));
    }
    Ok(out)
}

fn read_index_tables(r: &mut Reader, headers: Vec<(String, u8, u64)>) -> io::Result<Vec<RawIndexTable>> {
    let mut out = Vec::with_capacity(headers.len());
    for (name, cardinality, tag) in headers {
        let entry_count = r.u32()? as usize;
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let key_len = r.u32()? as usize;
            let key = r.bytes(key_len)?.to_vec();
            let value = if cardinality == 0 {
                RawIndexValue::One(r.u32()?)
            } else {
                let len = r.u32()? as usize;
                RawIndexValue::Many(r.bytes(len)?.to_vec())
            };
            entries.push((key, value));
        }
        out.push(RawIndexTable { name, cardinality, tag, entries });
    }
    Ok(out)
}

fn catalogue_entries(headers: &[(String, u8, u64)]) -> io::Result<Vec<CatalogueEntry>> {
    headers
        .iter()
        .map(|(name, cardinality, tag)| {
            Ok(CatalogueEntry {
                name: name.clone(),
                cardinality: cardinality_from_u8(*cardinality)?,
                tag: index::KeyTag(*tag),
            })
        })
        .collect()
}

fn read_node_rows(data: &[u8], count: u64) -> io::Result<Vec<NodeRow>> {
    let mut r = Reader::new(data);
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let slot = r.u32()?;
        let r#gen = r.u64()?;
        let adj_off = r.u64()?;
        let adj_len = r.u32()?;
        let val_off = r.u64()?;
        let val_len = r.u32()?;
        out.push((slot, r#gen, adj_off, adj_len, val_off, val_len));
    }
    Ok(out)
}

fn resolve_nodes(
    rows: Vec<NodeRow>,
    adjacency: &[u8],
    values: &[u8],
) -> io::Result<Vec<RawNode>> {
    rows.into_iter()
        .map(|(slot, r#gen, adj_off, adj_len, val_off, val_len)| {
            let mut ar = Reader::new(adjacency);
            ar.seek(adj_off as usize);
            let mut adj = Vec::with_capacity(adj_len as usize);
            for _ in 0..adj_len {
                let n = ar.u32()?;
                let e = ar.u32()?;
                adj.push((n, e));
            }
            let start = val_off as usize;
            let end = start + val_len as usize;
            if end > values.len() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "file truncated"));
            }
            Ok(RawNode { slot, r#gen, adj, val_bytes: values[start..end].to_vec() })
        })
        .collect()
}

fn read_edge_rows(data: &[u8], count: u64) -> io::Result<Vec<EdgeRow>> {
    let mut r = Reader::new(data);
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let slot = r.u32()?;
        let r#gen = r.u64()?;
        let n1 = r.u32()?;
        let n2 = r.u32()?;
        let slot_kind = r.u8()?;
        let val_off = r.u64()?;
        let val_len = r.u32()?;
        out.push((slot, r#gen, n1, n2, slot_kind, val_off, val_len));
    }
    Ok(out)
}

fn resolve_edges(rows: Vec<EdgeRow>, values: &[u8]) -> io::Result<Vec<RawEdge>> {
    rows.into_iter()
        .map(|(slot, r#gen, n1, n2, slot_kind, val_off, val_len)| {
            let start = val_off as usize;
            let end = start + val_len as usize;
            if end > values.len() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "file truncated"));
            }
            Ok(RawEdge { slot, r#gen, n1, n2, slot_kind, val_bytes: values[start..end].to_vec() })
        })
        .collect()
}

/// Prefixes an error with the file it came from. Every reader that opens a
/// path wraps its whole body in this once, so no snapshot error reaches a
/// caller without naming the file that produced it.
fn named(path: &Path, e: io::Error) -> io::Error {
    io::Error::new(e.kind(), format!("{}: {e}", path.display()))
}

pub(crate) fn read_snapshot(path: &Path) -> io::Result<SnapshotRead> {
    read_snapshot_inner(path).map_err(|e| named(path, e))
}

fn read_snapshot_inner(path: &Path) -> io::Result<SnapshotRead> {
    let file = File::open(path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    let data: &[u8] = &mmap;

    let mut r = Reader::new(data);
    let fixed = read_fixed_header(&mut r)?;
    let sections = read_section_table(&mut r)?;

    verify_trailer(data, &sections)?;

    let nodes_slice = section_slice(data, &sections, SectionTag::Nodes)?;
    let edges_slice = section_slice(data, &sections, SectionTag::Edges)?;
    let adjacency_slice = section_slice(data, &sections, SectionTag::Adjacency)?;
    let values_slice = section_slice(data, &sections, SectionTag::Values)?;
    let free_slice = section_slice(data, &sections, SectionTag::Free)?;
    let indices_slice = section_slice(data, &sections, SectionTag::Indices)?;

    let node_rows = read_node_rows(nodes_slice, fixed.node_count)?;
    let nodes = resolve_nodes(node_rows, adjacency_slice, values_slice)?;
    let edge_rows = read_edge_rows(edges_slice, fixed.edge_count)?;
    let edges = resolve_edges(edge_rows, values_slice)?;

    let free: FreeSectionBody =
        bincode::deserialize(free_slice).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let node_free: Vec<u32> = free.node_free.iter().map(raw_u32).collect();
    let edge_free: Vec<u32> = free.edge_free.iter().map(raw_u32).collect();

    let mut ir = Reader::new(indices_slice);
    let cat_headers = read_catalogue_header(&mut ir)?;
    let catalogue = catalogue_entries(&cat_headers)?;
    let indices = read_index_tables(&mut ir, cat_headers)?;

    let header = Header {
        version: fixed.version,
        graph_kind: fixed.graph_kind,
        edge_kind: fixed.edge_kind,
        node_count: fixed.node_count,
        edge_count: fixed.edge_count,
        nv_layout_hash: fixed.nv_hash,
        ev_layout_hash: fixed.ev_hash,
        nv_type: fixed.nv_type,
        ev_type: fixed.ev_type,
        sections,
        catalogue,
    };

    Ok(SnapshotRead {
        header,
        nodes,
        edges,
        node_free,
        edge_free,
        next_node: free.next_node,
        next_edge: free.next_edge,
        version: free.version,
        indices,
    })
}

/// Reads the fixed header, the section table, and the catalogue header
/// inside the indices section — never a table body, and never the graph.
///
/// Header-only by design: it does **not** verify the trailer, so the
/// catalogue it reports is the catalogue the bytes claim, not one checked
/// against the file's hash. `load`/`load_with` verify the trailer before
/// parsing any section body.
pub fn read_header(path: &Path) -> io::Result<Header> {
    read_header_inner(path).map_err(|e| named(path, e))
}

fn read_header_inner(path: &Path) -> io::Result<Header> {
    let file = File::open(path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    let data: &[u8] = &mmap;

    let mut r = Reader::new(data);
    let fixed = read_fixed_header(&mut r)?;
    let sections = read_section_table(&mut r)?;

    let indices_slice = section_slice(data, &sections, SectionTag::Indices)?;
    let mut ir = Reader::new(indices_slice);
    let cat_headers = read_catalogue_header(&mut ir)?;
    let catalogue = catalogue_entries(&cat_headers)?;

    Ok(Header {
        version: fixed.version,
        graph_kind: fixed.graph_kind,
        edge_kind: fixed.edge_kind,
        node_count: fixed.node_count,
        edge_count: fixed.edge_count,
        nv_layout_hash: fixed.nv_hash,
        ev_layout_hash: fixed.ev_hash,
        nv_type: fixed.nv_type,
        ev_type: fixed.ev_type,
        sections,
        catalogue,
    })
}

pub(crate) fn write_snapshot(path: &Path, mut snap: SnapshotWrite) -> io::Result<()> {
    snap.nodes.sort_by_key(|n| n.slot);
    snap.edges.sort_by_key(|e| e.slot);
    snap.indices.sort_by(|a, b| a.name.cmp(&b.name));
    for table in &mut snap.indices {
        table.entries.sort_by(|a, b| a.0.cmp(&b.0));
    }

    let mut adjacency: Vec<u8> = Vec::new();
    let mut values: Vec<u8> = Vec::new();

    let mut node_rows: Vec<(u32, u64, u64, u32, u64, u32)> = Vec::with_capacity(snap.nodes.len());
    for node in &snap.nodes {
        let adj_off = adjacency.len() as u64;
        for (n, e) in &node.adj {
            adjacency.extend_from_slice(&n.to_le_bytes());
            adjacency.extend_from_slice(&e.to_le_bytes());
        }
        let adj_len = node.adj.len() as u32;
        let val_off = values.len() as u64;
        values.extend_from_slice(&node.val_bytes);
        let val_len = node.val_bytes.len() as u32;
        node_rows.push((node.slot, node.r#gen, adj_off, adj_len, val_off, val_len));
    }

    let mut edge_rows: Vec<(u32, u64, u32, u32, u8, u64, u32)> = Vec::with_capacity(snap.edges.len());
    for edge in &snap.edges {
        let val_off = values.len() as u64;
        values.extend_from_slice(&edge.val_bytes);
        let val_len = edge.val_bytes.len() as u32;
        edge_rows.push((edge.slot, edge.r#gen, edge.n1, edge.n2, edge.slot_kind, val_off, val_len));
    }

    let mut nodes_bytes = Vec::with_capacity(node_rows.len() * NODE_ROW_LEN);
    for (slot, r#gen, adj_off, adj_len, val_off, val_len) in &node_rows {
        nodes_bytes.extend_from_slice(&slot.to_le_bytes());
        nodes_bytes.extend_from_slice(&r#gen.to_le_bytes());
        nodes_bytes.extend_from_slice(&adj_off.to_le_bytes());
        nodes_bytes.extend_from_slice(&adj_len.to_le_bytes());
        nodes_bytes.extend_from_slice(&val_off.to_le_bytes());
        nodes_bytes.extend_from_slice(&val_len.to_le_bytes());
    }

    let mut edges_bytes = Vec::with_capacity(edge_rows.len() * EDGE_ROW_LEN);
    for (slot, r#gen, n1, n2, slot_kind, val_off, val_len) in &edge_rows {
        edges_bytes.extend_from_slice(&slot.to_le_bytes());
        edges_bytes.extend_from_slice(&r#gen.to_le_bytes());
        edges_bytes.extend_from_slice(&n1.to_le_bytes());
        edges_bytes.extend_from_slice(&n2.to_le_bytes());
        edges_bytes.push(*slot_kind);
        edges_bytes.extend_from_slice(&val_off.to_le_bytes());
        edges_bytes.extend_from_slice(&val_len.to_le_bytes());
    }

    let free_body = FreeSectionBody {
        node_free: index::IdSet::from_raw_ids(snap.node_free.iter().map(|&s| s as Id)),
        edge_free: index::IdSet::from_raw_ids(snap.edge_free.iter().map(|&s| s as Id)),
        next_node: snap.next_node,
        next_edge: snap.next_edge,
        version: snap.version,
    };
    let free_bytes =
        bincode::serialize(&free_body).map_err(io::Error::other)?;

    let mut indices_bytes = Vec::new();
    indices_bytes.extend_from_slice(&(snap.indices.len() as u16).to_le_bytes());
    for table in &snap.indices {
        let name_bytes = table.name.as_bytes();
        indices_bytes.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        indices_bytes.extend_from_slice(name_bytes);
        indices_bytes.push(table.cardinality);
        indices_bytes.extend_from_slice(&table.tag.to_le_bytes());
    }
    for table in &snap.indices {
        indices_bytes.extend_from_slice(&(table.entries.len() as u32).to_le_bytes());
        for (key, value) in &table.entries {
            indices_bytes.extend_from_slice(&(key.len() as u32).to_le_bytes());
            indices_bytes.extend_from_slice(key);
            match value {
                RawIndexValue::One(id) => indices_bytes.extend_from_slice(&id.to_le_bytes()),
                RawIndexValue::Many(bytes) => {
                    indices_bytes.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                    indices_bytes.extend_from_slice(bytes);
                }
            }
        }
    }

    let nv_type_bytes = snap.nv_type.as_bytes();
    let ev_type_bytes = snap.ev_type.as_bytes();

    let sections_order = [
        SectionTag::Nodes,
        SectionTag::Edges,
        SectionTag::Adjacency,
        SectionTag::Values,
        SectionTag::Free,
        SectionTag::Indices,
        SectionTag::Trailer,
    ];
    let section_lens: [u64; 7] = [
        nodes_bytes.len() as u64,
        edges_bytes.len() as u64,
        adjacency.len() as u64,
        values.len() as u64,
        free_bytes.len() as u64,
        indices_bytes.len() as u64,
        32,
    ];

    let header_size =
        HEADER_FIXED + nv_type_bytes.len() + ev_type_bytes.len() + 2 + sections_order.len() * SECTION_ENTRY_LEN;

    let mut offset = header_size as u64;
    let mut section_infos = Vec::with_capacity(sections_order.len());
    for (tag, len) in sections_order.iter().zip(section_lens.iter()) {
        section_infos.push(SectionInfo { tag: *tag, offset, len: *len });
        offset += *len;
    }

    let mut buf = Vec::with_capacity(offset as usize);
    buf.extend_from_slice(&MAGIC);
    buf.extend_from_slice(&VERSION.to_le_bytes());
    buf.push(snap.graph_kind);
    buf.push(snap.edge_kind);
    buf.extend_from_slice(&(snap.nodes.len() as u64).to_le_bytes());
    buf.extend_from_slice(&(snap.edges.len() as u64).to_le_bytes());
    buf.extend_from_slice(&snap.nv_hash.to_le_bytes());
    buf.extend_from_slice(&snap.ev_hash.to_le_bytes());
    buf.extend_from_slice(&(nv_type_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(&(ev_type_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(nv_type_bytes);
    buf.extend_from_slice(ev_type_bytes);
    buf.extend_from_slice(&(section_infos.len() as u16).to_le_bytes());
    for s in &section_infos {
        buf.push(s.tag.to_u8());
        buf.extend_from_slice(&s.offset.to_le_bytes());
        buf.extend_from_slice(&s.len.to_le_bytes());
    }

    buf.extend_from_slice(&nodes_bytes);
    buf.extend_from_slice(&edges_bytes);
    buf.extend_from_slice(&adjacency);
    buf.extend_from_slice(&values);
    buf.extend_from_slice(&free_bytes);
    buf.extend_from_slice(&indices_bytes);

    let mut hasher = Sha256::new();
    hasher.update(&buf);
    let digest = hasher.finalize();
    buf.extend_from_slice(&digest);

    let mut file = File::create(path)?;
    file.write_all(&buf)
}

/// Verifies `decls` against the file's catalogue (order independent) and
/// splits the file's parsed index tables into unique/multi entry lists in
/// `decls`' own order, so `Indices::from_parts`/the VGraph equivalent can
/// restore the tables without rescanning node values through the
/// extractors.
pub(crate) fn verify_and_split_indices<NV>(
    file_catalogue: &[RawIndexTable],
    decls: Vec<index::IndexDecl<NV>>,
) -> io::Result<IndexRestoreParts<NV>> {
    let missing: Vec<&str> =
        file_catalogue.iter().map(|t| t.name.as_str()).filter(|n| !decls.iter().any(|d| d.name().0 == *n)).collect();
    if !missing.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing declarations for indices: {}", missing.join(", ")),
        ));
    }
    let extra: Vec<&str> = decls
        .iter()
        .map(|d| d.name().0)
        .filter(|n| !file_catalogue.iter().any(|t| t.name == *n))
        .collect();
    if !extra.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("declarations for indices not present in the file: {}", extra.join(", ")),
        ));
    }

    let mut unique = Vec::new();
    let mut multi = Vec::new();
    for decl in &decls {
        let table = file_catalogue
            .iter()
            .find(|t| t.name == decl.name().0)
            .expect("name presence verified by the missing-declarations check above");
        let file_cardinality = cardinality_from_u8(table.cardinality)?;
        if file_cardinality != decl.cardinality() || table.tag != decl.tag().0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "index `{}` declaration mismatch: file has cardinality {:?} tag {}, declared cardinality {:?} tag {}",
                    decl.name().0,
                    file_cardinality,
                    table.tag,
                    decl.cardinality(),
                    decl.tag().0,
                ),
            ));
        }
        match file_cardinality {
            index::Cardinality::Unique => {
                let entries = table
                    .entries
                    .iter()
                    .map(|(k, v)| {
                        let RawIndexValue::One(raw) = v else { unreachable!("unique table entry must be One") };
                        (index::KeyBytes::from_bytes(k.clone()), id::N(*raw as Id))
                    })
                    .collect();
                unique.push(entries);
            }
            index::Cardinality::Multi => {
                let entries: Vec<(index::KeyBytes, index::IdSet<id::N>)> = table
                    .entries
                    .iter()
                    .map(|(k, v)| {
                        let RawIndexValue::Many(bytes) = v else { unreachable!("multi table entry must be Many") };
                        let set: index::IdSet<id::N> =
                            bincode::deserialize(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                        Ok((index::KeyBytes::from_bytes(k.clone()), set))
                    })
                    .collect::<io::Result<Vec<_>>>()?;
                multi.push(entries);
            }
        }
    }
    Ok((decls, unique, multi))
}

/// Builds the file's index tables from a graph's declarations and already
/// scanned/collected entries, in `decls`' own order (matching `unique`
/// and `multi`'s parallel per-cardinality ordering).
pub(crate) fn build_index_tables<NV>(
    decls: &[index::IndexDecl<NV>],
    unique: UniqueEntries,
    multi: MultiEntries,
) -> io::Result<Vec<RawIndexTable>> {
    let mut unique_iter = unique.into_iter();
    let mut multi_iter = multi.into_iter();
    decls
        .iter()
        .map(|decl| {
            let (cardinality, entries) = match decl.cardinality() {
                index::Cardinality::Unique => {
                    let entries = unique_iter
                        .next()
                        .expect("unique table count matches decls")
                        .into_iter()
                        .map(|(k, v)| (k.as_bytes().to_vec(), RawIndexValue::One(raw_u32(v.0))))
                        .collect();
                    (index::Cardinality::Unique, entries)
                }
                index::Cardinality::Multi => {
                    let entries = multi_iter
                        .next()
                        .expect("multi table count matches decls")
                        .into_iter()
                        .map(|(k, set)| {
                            let bytes = bincode::serialize(&set).map_err(io::Error::other)?;
                            Ok((k.as_bytes().to_vec(), RawIndexValue::Many(bytes)))
                        })
                        .collect::<io::Result<Vec<_>>>()?;
                    (index::Cardinality::Multi, entries)
                }
            };
            Ok(RawIndexTable {
                name: decl.name().0.to_string(),
                cardinality: cardinality_to_u8(cardinality),
                tag: decl.tag().0,
                entries,
            })
        })
        .collect()
}

fn mgraph_index_tables<NV>(indices: &index::Indices<NV>) -> io::Result<Vec<RawIndexTable>> {
    let unique: UniqueEntries = indices
        .unique_tables()
        .iter()
        .map(|t| t.iter().map(|(k, v)| (k.clone(), *v)).collect())
        .collect();
    let multi: MultiEntries = indices
        .multi_tables()
        .iter()
        .map(|t| t.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .collect();
    build_index_tables(indices.decls(), unique, multi)
}

fn mgraph_free<V>(nodes: &Nodes<V>) -> (Vec<u32>, u32) {
    let next = nodes.store.len() as u32;
    let mut holes = Vec::new();
    for slot in 0..next {
        if nodes.free_ids.0.contains(slot as Id) {
            holes.push(slot);
        }
    }
    (holes, next)
}

fn mgraph_edge_free<E: Edge>(edges: &Edges<E>) -> (Vec<u32>, u32) {
    let next = edges.store.len() as u32;
    let mut holes = Vec::new();
    for slot in 0..next {
        if edges.free_ids.0.contains(slot as Id) {
            holes.push(slot);
        }
    }
    (holes, next)
}

/// Writes any `Graph` read surface in the on-disk format.
///
/// The format serialises `MGraph`'s internal stores, which a trait-generic
/// writer cannot observe, so the graph is first materialised through
/// `MGraph::from_graph`. Node ids survive that step; edge ids do not, and edge
/// tombstones are compacted away — output is therefore byte-identical to
/// `MGraph::save` only for sources whose edge store already has no gaps.
/// `MGraph::save` keeps the direct path so previously written files stay
/// byte-reproducible.
pub fn save_graph<NV, E, G>(g: &G, path: &Path) -> io::Result<()>
where
    NV: Clone + ::serde::Serialize + layout::Val,
    E: Edge,
    E::Slot: ::serde::Serialize,
    E::Val: Clone + ::serde::Serialize + layout::Val,
    G: super::Graph<NV, E>,
{
    super::MGraph::<NV, E>::from_graph(g).save(path)
}

impl<NV, E: Edge> super::MGraph<NV, E> {
    /// Serialises this graph's own stores verbatim, tombstones and free lists
    /// included. `save_graph` is the trait-generic counterpart.
    pub fn save(&self, path: &Path) -> io::Result<()>
    where
        NV: ::serde::Serialize + layout::Val,
        E::Slot: ::serde::Serialize,
        E::Val: ::serde::Serialize + layout::Val,
    {
        let mut nodes = Vec::with_capacity(self.nodes.store.len());
        for (i, opt) in self.nodes.store.iter().enumerate() {
            let Some(node) = opt else { continue };
            let val_bytes = bincode::serialize(&node.val).map_err(io::Error::other)?;
            let adj = node.adj.entries_iter().map(|(n, e)| (raw_u32(n.0), raw_u32(e.0))).collect();
            nodes.push(RawNode { slot: i as u32, r#gen: 0, adj, val_bytes });
        }

        let mut edges = Vec::with_capacity(self.edges.store.len());
        for (i, opt) in self.edges.store.iter().enumerate() {
            let Some(rec) = opt else { continue };
            let val_bytes = bincode::serialize(&rec.val).map_err(io::Error::other)?;
            edges.push(RawEdge {
                slot: i as u32,
                r#gen: 0,
                n1: raw_u32(rec.n1.0),
                n2: raw_u32(rec.n2.0),
                slot_kind: E::slot_to_byte(rec.slot),
                val_bytes,
            });
        }

        let (node_free, next_node) = mgraph_free(&self.nodes);
        let (edge_free, next_edge) = mgraph_edge_free(&self.edges);
        let indices = mgraph_index_tables(&self.indices)?;

        write_snapshot(
            path,
            SnapshotWrite {
                graph_kind: 0,
                edge_kind: E::EDGE_KIND,
                nv_type: std::any::type_name::<NV>().to_string(),
                ev_type: std::any::type_name::<E::Val>().to_string(),
                nv_hash: NV::layout_hash(),
                ev_hash: <E::Val as layout::Val>::layout_hash(),
                nodes,
                edges,
                node_free,
                edge_free,
                next_node,
                next_edge,
                version: 0,
                indices,
            },
        )
    }

    /// Loads a file whose catalogue is empty. A file that carries a
    /// catalogue is **refused**, with an error naming every index that needs
    /// a declaration — use `load_with(path, decls)`, which verifies the
    /// catalogue against `decls` and restores the tables from the file.
    pub fn load(path: &Path) -> io::Result<Self>
    where
        NV: ::serde::de::DeserializeOwned + layout::Val,
        E::Slot: ::serde::de::DeserializeOwned,
        E::Val: ::serde::de::DeserializeOwned + layout::Val,
    {
        Self::load_with(path, Vec::new())
    }

    /// V-as-M: a file whose node/edge rows carry real generations and whose
    /// free section carries a real version (e.g. one written by
    /// `VGraph::save`) has both silently dropped, since `MGraph` has no
    /// generation or version fields to hold them.
    pub fn load_with(path: &Path, decls: Vec<index::IndexDecl<NV>>) -> io::Result<Self>
    where
        NV: ::serde::de::DeserializeOwned + layout::Val,
        E::Slot: ::serde::de::DeserializeOwned,
        E::Val: ::serde::de::DeserializeOwned + layout::Val,
    {
        let snap = read_snapshot(path)?;
        check_edge_kind::<E>(&snap.header)?;
        check_layout_hashes::<NV, E>(&snap.header)?;
        let (decls, unique, multi) = verify_and_split_indices(&snap.indices, decls)?;

        let mut store: Vec<Option<Node<NV>>> = Vec::new();
        for raw in snap.nodes {
            let idx = raw.slot as usize;
            if store.len() <= idx {
                store.resize_with(idx + 1, || None);
            }
            let val: NV =
                bincode::deserialize(&raw.val_bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let mut adj = Adjacents::new();
            for (n, e) in raw.adj {
                adj.insert(id::N(n as Id), id::E(e as Id));
            }
            store[idx] = Some(Node { val, adj });
        }
        if store.len() < snap.next_node as usize {
            store.resize_with(snap.next_node as usize, || None);
        }
        let node_count = store.iter().filter(|o| o.is_some()).count();
        let mut free_ids = IdSpace::new_starting_at(snap.next_node as Id);
        for slot in &snap.node_free {
            free_ids.push_id(*slot as Id);
        }
        let nodes = Nodes { store, free_ids, count: node_count };

        let mut edge_store: Vec<Option<EdgeRec<E::Slot, E::Val>>> = Vec::new();
        for raw in snap.edges {
            let idx = raw.slot as usize;
            if edge_store.len() <= idx {
                edge_store.resize_with(idx + 1, || None);
            }
            let val: E::Val =
                bincode::deserialize(&raw.val_bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            edge_store[idx] = Some(EdgeRec {
                n1: id::N(raw.n1 as Id),
                n2: id::N(raw.n2 as Id),
                slot: E::slot_from_byte(raw.slot_kind),
                val,
            });
        }
        if edge_store.len() < snap.next_edge as usize {
            edge_store.resize_with(snap.next_edge as usize, || None);
        }
        let edge_count = edge_store.iter().filter(|o| o.is_some()).count();
        let mut edge_free_ids = IdSpace::new_starting_at(snap.next_edge as Id);
        for slot in &snap.edge_free {
            edge_free_ids.push_id(*slot as Id);
        }
        let edges = Edges { store: edge_store, free_ids: edge_free_ids, count: edge_count };

        let unique_tables: Vec<FxHashMap<index::KeyBytes, id::N>> =
            unique.into_iter().map(|v| v.into_iter().collect()).collect();
        let multi_tables: Vec<FxHashMap<index::KeyBytes, index::IdSet<id::N>>> =
            multi.into_iter().map(|v| v.into_iter().collect()).collect();
        let indices = index::Indices::from_parts(decls, unique_tables, multi_tables);

        let mut g = super::MGraph { nodes, edges, degrees: Vec::new(), indices };
        g.build_degrees();
        Ok(g)
    }
}

pub(crate) fn check_edge_kind<E: Edge>(header: &Header) -> io::Result<()> {
    if header.edge_kind != E::EDGE_KIND {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("edge kind mismatch: file has {}, expected {}", header.edge_kind, E::EDGE_KIND),
        ));
    }
    Ok(())
}

pub(crate) fn check_layout_hashes<NV, E: Edge>(header: &Header) -> io::Result<()>
where
    NV: layout::Val,
    E::Val: layout::Val,
{
    let expected_nv = NV::layout_hash();
    let expected_ev = <E::Val as layout::Val>::layout_hash();
    if header.nv_layout_hash != expected_nv {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "node value layout mismatch: file has type `{}`, expected `{}`",
                header.nv_type,
                std::any::type_name::<NV>(),
            ),
        ));
    }
    if header.ev_layout_hash != expected_ev {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "edge value layout mismatch: file has type `{}`, expected `{}`",
                header.ev_type,
                std::any::type_name::<E::Val>(),
            ),
        ));
    }
    Ok(())
}

const HEADER_SIZE_V2: usize = 44;

struct HeaderV2 {
    edge_kind: u8,
    nv_type: String,
    ev_type: String,
    nv_layout_hash: u64,
    ev_layout_hash: u64,
}

fn parse_header_v2(data: &[u8]) -> io::Result<HeaderV2> {
    if data.len() < HEADER_SIZE_V2 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file too short"));
    }
    if data[..4] != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic"));
    }
    let version = u16::from_le_bytes([data[4], data[5]]);
    if version != VERSION_V2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported version: {version} (expected {VERSION_V2})"),
        ));
    }
    let edge_kind = data[6];
    let nv_layout_hash = u64::from_le_bytes(data[24..32].try_into().unwrap());
    let ev_layout_hash = u64::from_le_bytes(data[32..40].try_into().unwrap());
    let nv_type_len = u16::from_le_bytes([data[40], data[41]]) as usize;
    let ev_type_len = u16::from_le_bytes([data[42], data[43]]) as usize;

    let required = HEADER_SIZE_V2 + nv_type_len + ev_type_len;
    if data.len() < required {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file truncated in type strings"));
    }
    let nv_type = utf8(&data[HEADER_SIZE_V2..HEADER_SIZE_V2 + nv_type_len])?;
    let ev_type = utf8(&data[HEADER_SIZE_V2 + nv_type_len..HEADER_SIZE_V2 + nv_type_len + ev_type_len])?;

    Ok(HeaderV2 { edge_kind, nv_type, ev_type, nv_layout_hash, ev_layout_hash })
}

/// The v2 body: the pre-v3 whole-struct bincode dump of `MGraph`, which had
/// exactly these three fields (its `indices` field was `#[serde(skip)]`, so
/// no index bytes were ever written). `MGraph` itself carries no serde
/// derive; this record is the only shape v2 bytes are read into, and the
/// `MGraph` is built from it field by field.
#[derive(::serde::Deserialize)]
#[serde(bound(
    deserialize = "NV: ::serde::de::DeserializeOwned, E::Slot: ::serde::de::DeserializeOwned, E::Val: ::serde::de::DeserializeOwned",
))]
struct MGraphV2<NV, E: Edge> {
    nodes: Nodes<NV>,
    edges: Edges<E>,
    degrees: Vec<(Id, index::IdSet<id::N>)>,
}

fn load_v2<NV, E: Edge>(path: &Path) -> io::Result<super::MGraph<NV, E>>
where
    NV: ::serde::de::DeserializeOwned + layout::Val,
    E::Slot: ::serde::de::DeserializeOwned,
    E::Val: ::serde::de::DeserializeOwned + layout::Val,
{
    load_v2_inner(path).map_err(|e| named(path, e))
}

fn load_v2_inner<NV, E: Edge>(path: &Path) -> io::Result<super::MGraph<NV, E>>
where
    NV: ::serde::de::DeserializeOwned + layout::Val,
    E::Slot: ::serde::de::DeserializeOwned,
    E::Val: ::serde::de::DeserializeOwned + layout::Val,
{
    let file = File::open(path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    let header = parse_header_v2(&mmap)?;
    if header.edge_kind != E::EDGE_KIND {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("edge kind mismatch: file has {}, expected {}", header.edge_kind, E::EDGE_KIND),
        ));
    }
    let expected_nv = NV::layout_hash();
    let expected_ev = <E::Val as layout::Val>::layout_hash();
    if header.nv_layout_hash != expected_nv {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "node value layout mismatch: file has type `{}`, expected `{}`",
                header.nv_type,
                std::any::type_name::<NV>(),
            ),
        ));
    }
    if header.ev_layout_hash != expected_ev {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "edge value layout mismatch: file has type `{}`, expected `{}`",
                header.ev_type,
                std::any::type_name::<E::Val>(),
            ),
        ));
    }
    let data_offset = HEADER_SIZE_V2 + header.nv_type.len() + header.ev_type.len();
    let data = &mmap[data_offset..];
    let v2: MGraphV2<NV, E> =
        bincode::deserialize(data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(super::MGraph {
        nodes: v2.nodes,
        edges: v2.edges,
        degrees: v2.degrees,
        indices: index::Indices::empty(),
    })
}

/// Reads a v2 file (today's whole-struct bincode dump, `MGraph` only) and
/// writes it out in the v3 format at `to`.
pub fn convert<NV, E: Edge>(from: &Path, to: &Path) -> io::Result<ConvertReport>
where
    NV: ::serde::Serialize + ::serde::de::DeserializeOwned + layout::Val,
    E::Slot: ::serde::Serialize + ::serde::de::DeserializeOwned,
    E::Val: ::serde::Serialize + ::serde::de::DeserializeOwned + layout::Val,
{
    let g: super::MGraph<NV, E> = load_v2(from)?;
    let report = ConvertReport { node_count: g.node_count() as u64, edge_count: g.edge_count() as u64 };
    g.save(to)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{self, MGraph};
    use crate::edge;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("grw_persist_tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn round_trip_undir0() {
        use edge::undir::E::U;
        let g: graph::MUndir0 = vec![U(0, 1), U(1, 2), U(2, 3), U(3, 0)]
            .try_into()
            .unwrap();
        let path = tmp_path("rt_undir0.grw");
        g.save(&path).unwrap();
        let g2: graph::MUndir0 = MGraph::load(&path).unwrap();
        assert_eq!(g.node_count(), g2.node_count());
        assert_eq!(g.edge_count(), g2.edge_count());
    }

    #[test]
    fn round_trip_dir0() {
        use edge::dir::E::D;
        let g: graph::MDir0 = vec![D(0, 1), D(1, 2), D(2, 0)].try_into().unwrap();
        let path = tmp_path("rt_dir0.grw");
        g.save(&path).unwrap();
        let g2: graph::MDir0 = MGraph::load(&path).unwrap();
        assert_eq!(g.node_count(), g2.node_count());
        assert_eq!(g.edge_count(), g2.edge_count());
    }

    #[test]
    fn round_trip_anydir0() {
        use edge::anydir::E::{D, U};
        let g: graph::MAnydir0 = vec![U(0, 1), D(1, 2), U(2, 3)].try_into().unwrap();
        let path = tmp_path("rt_anydir0.grw");
        g.save(&path).unwrap();
        let g2: graph::MAnydir0 = MGraph::load(&path).unwrap();
        assert_eq!(g.node_count(), g2.node_count());
        assert_eq!(g.edge_count(), g2.edge_count());
    }

    #[test]
    fn round_trip_valued() {
        use edge::undir::E::U;
        let g: graph::MUndir<u32, u32> = (
            vec![(0, 10u32), (1, 20), (2, 30)],
            vec![(U(0, 1), 100u32), (U(1, 2), 200)],
        )
            .try_into()
            .unwrap();
        let path = tmp_path("rt_valued.grw");
        g.save(&path).unwrap();
        let g2: graph::MUndir<u32, u32> = MGraph::load(&path).unwrap();
        assert_eq!(g.node_count(), g2.node_count());
        assert_eq!(g.edge_count(), g2.edge_count());
        assert_eq!(g.get(0u32), g2.get(0u32));
        assert_eq!(g.get(1u32), g2.get(1u32));
        assert_eq!(g.get(2u32), g2.get(2u32));
    }

    #[test]
    fn header_validation_bad_magic() {
        let path = tmp_path("rt_bad_magic.grw");
        std::fs::write(&path, b"BADMxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx").unwrap();
        let Err(err) = MGraph::<(), edge::Undir<()>>::load(&path) else {
            panic!("expected error");
        };
        let msg = err.to_string();
        assert!(msg.contains("magic"), "expected magic error, got: {msg}");
    }

    #[test]
    fn header_validation_bad_version() {
        let path = tmp_path("rt_bad_version.grw");
        let mut data = vec![0u8; 48];
        data[..4].copy_from_slice(b"GRW\0");
        data[4..6].copy_from_slice(&99u16.to_le_bytes());
        std::fs::write(&path, &data).unwrap();
        let Err(err) = MGraph::<(), edge::Undir<()>>::load(&path) else {
            panic!("expected error");
        };
        let msg = err.to_string();
        assert!(msg.contains("version"), "expected version error, got: {msg}");
    }

    #[test]
    fn edge_kind_mismatch() {
        use edge::undir::E::U;
        let g: graph::MUndir0 = vec![U(0, 1)].try_into().unwrap();
        let path = tmp_path("rt_edge_kind_mismatch.grw");
        g.save(&path).unwrap();
        let Err(err) = MGraph::<(), edge::Dir<()>>::load(&path) else {
            panic!("expected error");
        };
        let msg = err.to_string();
        assert!(msg.contains("edge kind"), "expected edge kind error, got: {msg}");
    }

    #[test]
    fn layout_mismatch() {
        use edge::undir::E::U;
        let g: graph::MUndir<u32, u32> = (
            vec![(0, 10u32), (1, 20)],
            vec![(U(0, 1), 100u32)],
        )
            .try_into()
            .unwrap();
        let path = tmp_path("rt_layout_mismatch.grw");
        g.save(&path).unwrap();
        let Err(err) = MGraph::<i64, edge::Undir<i64>>::load(&path) else {
            panic!("expected error");
        };
        let msg = err.to_string();
        assert!(msg.contains("layout mismatch"), "expected layout error, got: {msg}");
    }

    #[test]
    fn read_header_round_trip() {
        use edge::undir::E::U;
        let g: graph::MUndir<u32, u32> = (
            vec![(0, 10u32), (1, 20)],
            vec![(U(0, 1), 100u32)],
        )
            .try_into()
            .unwrap();
        let path = tmp_path("rt_read_header.grw");
        g.save(&path).unwrap();
        let header = read_header(&path).unwrap();
        assert_eq!(header.version, 3);
        assert_eq!(header.graph_kind, 0);
        assert_eq!(header.edge_kind, 0);
        assert_eq!(header.node_count, 2);
        assert_eq!(header.edge_count, 1);
        assert_eq!(header.nv_type, std::any::type_name::<u32>());
        assert_eq!(header.ev_type, std::any::type_name::<u32>());
        assert_ne!(header.nv_layout_hash, 0);
        assert_ne!(header.ev_layout_hash, 0);
        assert!(header.catalogue.is_empty());
    }
}
