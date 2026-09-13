# Persistence

GRW graphs can be saved to and loaded from binary `.grw` files (format
version 3), with layout validation and an integrity check over the whole
file.

## Save and Load

```rust
use std::path::Path;

// save
g.save(Path::new("my_graph.grw")).unwrap();

// load
let g2: graph::MUndir0 = MGraph::load(Path::new("my_graph.grw")).unwrap();
```

`VGraph::save`/`load` write and read exactly the same format — there is one
`.grw` layout, not two; see [Versioned Graphs](./versioned-graph.md) for what
crosses that door and what doesn't.

## Binary Format (v3)

A file is a fixed header, a section table, six content sections, and a
trailer:

1. **Header** — magic `GRW\0`, `version: u16` (= 3), `graph_kind: u8` (0 =
   `MGraph`, 1 = `VGraph`), `edge_kind: u8`, node/edge counts, layout hashes
   for `NV` and `EV`, then the type name strings (for error messages), then a
   `section_count: u16` and that many `(tag, offset, len)` entries — always
   all seven, even when a section is empty.
2. **Nodes** / **Edges** — fixed-width rows for live nodes/edges only, sorted
   by slot ascending. Every row carries a `gen: u64` field; it is real for a
   `VGraph`-sourced file and always `0` for an `MGraph`-sourced one — the byte
   layout never branches on `graph_kind`, only its meaning does.
3. **Adjacency** — `(neighbor, edge)` pairs, contiguous per node in node
   order; a node's row carries a byte offset into this section and a pair
   count.
4. **Values** — one heap of bincode bytes, node values first (in node
   order) then edge values (in edge order), each addressed by byte offset and
   length from the node/edge rows.
5. **Free** — one bincode blob: `node_free`/`edge_free` (each the *holes*
   below `next_node`/`next_edge`, not the full allocator state),
   `next_node: u32`, `next_edge: u32`, and a real `version: u64` (always `0`
   for an `MGraph`-sourced file).
6. **Indices** — see [below](#indices).
7. **Trailer** — 32 bytes, the SHA-256 of every byte before it (header
   through the indices section, including the section table itself).
   Verified before any *section body* is parsed, so a flipped byte inside a
   section surfaces as a trailer-mismatch error, never a downstream parse
   error. The fixed header and the section table are read before the check
   runs, so corruption there surfaces first as a structural error —
   "invalid magic", "unsupported version", "unknown section tag", or a
   truncated/out-of-range section range — rather than as a trailer
   mismatch. Every one of those errors names the file it came from.

```rust
use grw::graph::persist;

let header = persist::read_header(Path::new("my_graph.grw")).unwrap();
assert_eq!(header.version, 3);
assert_eq!(header.sections.len(), 7);
```

`read_header` reads only the header and the section table (and, inside the
indices section, only the catalogue — never a table body), so it can inspect
a file's shape and catalogue without loading the graph at all. Being
header-only, it does **not** verify the trailer: what it reports is what the
bytes claim. `load`/`load_with` verify the trailer before parsing any
section body.

## Indices

If a graph carries indices, `save` writes the whole catalogue and every
table into the indices section: a catalogue header (each declared index's
name, cardinality, and key tag) followed, in the same order, by one table
per index. `load` refuses such a file, naming every index that needs a
declaration — a plain load has nothing to check the file against — so use
`load_with` to both verify and restore them:

```rust
use grw::graph::index::{Cardinality, IndexDecl, IndexName};
use grw::graph::{MGraph, edge};

const BY_VAL: IndexName = IndexName("by_val");

let g: MGraph<u32, edge::Undir<()>> = grw::mgraph![N(0).val(10u32)].unwrap();
let g = g.with_indices(vec![
    IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v)),
]).unwrap();
g.save(Path::new("indexed.grw")).unwrap();

let g2: MGraph<u32, edge::Undir<()>> = MGraph::load_with(Path::new("indexed.grw"), vec![
    IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v)),
]).unwrap();
assert_eq!(g2.catalogue().len(), 1);
```

`load_with` checks the file's catalogue against the declarations you pass —
by name, order-independent — before restoring the tables directly from the
parsed bytes, with no rescan of node values: a missing declaration, an extra
one, or a key-type mismatch (`KeyTag`) are each a distinct, named error.
Plain `load` on such a file is itself an error, naming every index that
needs a declaration. See
[Indices](./indices.md) for the declaration API itself.

## Layout Validation

When loading, GRW validates that the types match:

- **Edge kind** must match (can't load a directed graph as undirected)
- **Layout hash** for node values must match
- **Layout hash** for edge values must match

```rust
// this will fail: type mismatch
let result = MGraph::<i64, edge::Undir<i64>>::load(Path::new("u32_graph.grw"));
// Error: "node value layout mismatch: file has type `u32`, expected `i64`"
```

The layout hash is computed from the type's field names, types, and
structure using FNV hashing. This catches:
- Wrong types (`u32` vs `i64`)
- Reordered fields
- Added/removed fields

## Loading an old file: `convert`

A file written by the pre-v3 format is rejected by `load`/`load_with`/
`read_header` with a message naming `convert`:

```rust
use grw::graph::persist;

let report = persist::convert::<(), edge::Dir<()>>(
    Path::new("old_v2.grw"),
    Path::new("upgraded_v3.grw"),
).unwrap();
let g: graph::MDir0 = MGraph::load(Path::new("upgraded_v3.grw")).unwrap();
assert_eq!(report.node_count, g.node_count() as u64);
```

`convert` reads the old file in full and writes it back out in the current
format at the destination path; the source is untouched.

## Memory-Mapped Loading

`load()` uses `memmap2` for memory-mapped file access: the header and
section table are parsed first, then each section is read directly from the
mmap.

## The Val Trait

`save()` and `load()` require both `NV` and `EV` to implement the `Val`
trait (for layout hash computation). Primitive types (`()`, `bool`, `u8`–`u64`, `i8`–`i64`, `f32`, `f64`, `String`) implement `Val` automatically. For custom structs, use `#[derive(Val)]`:

```rust
use grw::Val;

#[derive(Val, serde::Serialize, serde::Deserialize)]
struct MyNode {
    x: f64,
    y: f64,
    label: String,
}
```
