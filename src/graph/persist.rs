use super::layout;
use super::Edge;

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

const MAGIC: [u8; 4] = *b"GRW\0";
const VERSION: u16 = 2;
const HEADER_SIZE: usize = 44;

pub struct Header {
    pub version: u16,
    pub edge_kind: u8,
    pub node_count: u64,
    pub edge_count: u64,
    pub nv_layout_hash: u64,
    pub ev_layout_hash: u64,
    pub nv_type: String,
    pub ev_type: String,
}

impl<NV, E: Edge> super::Graph<NV, E> {
    pub fn save(&self, path: &Path) -> io::Result<()>
    where
        NV: ::serde::Serialize + layout::Val,
        E::Slot: ::serde::Serialize,
        E::Val: ::serde::Serialize + layout::Val,
    {
        let nv_type = std::any::type_name::<NV>();
        let ev_type = std::any::type_name::<E::Val>();
        let nv_type_bytes = nv_type.as_bytes();
        let ev_type_bytes = ev_type.as_bytes();

        let mut file = File::create(path)?;
        file.write_all(&MAGIC)?;
        file.write_all(&VERSION.to_le_bytes())?;
        file.write_all(&[E::EDGE_KIND])?;
        file.write_all(&[0u8])?;
        file.write_all(&(self.node_count() as u64).to_le_bytes())?;
        file.write_all(&(self.edge_count() as u64).to_le_bytes())?;
        file.write_all(&NV::layout_hash().to_le_bytes())?;
        file.write_all(&<E::Val as layout::Val>::layout_hash().to_le_bytes())?;
        file.write_all(&(nv_type_bytes.len() as u16).to_le_bytes())?;
        file.write_all(&(ev_type_bytes.len() as u16).to_le_bytes())?;
        file.write_all(nv_type_bytes)?;
        file.write_all(ev_type_bytes)?;
        bincode::serialize_into(&mut file, self)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    pub fn load(path: &Path) -> io::Result<Self>
    where
        NV: ::serde::de::DeserializeOwned + layout::Val,
        E::Slot: ::serde::de::DeserializeOwned,
        E::Val: ::serde::de::DeserializeOwned + layout::Val,
    {
        let file = File::open(path)?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        let header = parse_header(&mmap)?;
        if header.edge_kind != E::EDGE_KIND {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "edge kind mismatch: file has {}, expected {}",
                    header.edge_kind, E::EDGE_KIND,
                ),
            ));
        }
        let expected_nv = NV::layout_hash();
        let expected_ev = <E::Val as layout::Val>::layout_hash();
        if header.nv_layout_hash != expected_nv {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "node value layout mismatch: file has type `{}`, expected `{}`",
                    header.nv_type, std::any::type_name::<NV>(),
                ),
            ));
        }
        if header.ev_layout_hash != expected_ev {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "edge value layout mismatch: file has type `{}`, expected `{}`",
                    header.ev_type, std::any::type_name::<E::Val>(),
                ),
            ));
        }
        let data_offset = HEADER_SIZE + header.nv_type.len() + header.ev_type.len();
        let data = &mmap[data_offset..];
        bincode::deserialize(data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

fn parse_header(data: &[u8]) -> io::Result<Header> {
    if data.len() < HEADER_SIZE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file too short"));
    }
    if &data[..4] != &MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic"));
    }
    let version = u16::from_le_bytes([data[4], data[5]]);
    if version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported version: {version} (expected {VERSION})"),
        ));
    }
    let edge_kind = data[6];
    let node_count = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let edge_count = u64::from_le_bytes(data[16..24].try_into().unwrap());
    let nv_layout_hash = u64::from_le_bytes(data[24..32].try_into().unwrap());
    let ev_layout_hash = u64::from_le_bytes(data[32..40].try_into().unwrap());
    let nv_type_len = u16::from_le_bytes([data[40], data[41]]) as usize;
    let ev_type_len = u16::from_le_bytes([data[42], data[43]]) as usize;

    let required = HEADER_SIZE + nv_type_len + ev_type_len;
    if data.len() < required {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file truncated in type strings"));
    }

    let nv_type = std::str::from_utf8(&data[HEADER_SIZE..HEADER_SIZE + nv_type_len])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        .to_owned();
    let ev_type = std::str::from_utf8(&data[HEADER_SIZE + nv_type_len..HEADER_SIZE + nv_type_len + ev_type_len])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        .to_owned();

    Ok(Header {
        version,
        edge_kind,
        node_count,
        edge_count,
        nv_layout_hash,
        ev_layout_hash,
        nv_type,
        ev_type,
    })
}

pub fn read_header(path: &Path) -> io::Result<Header> {
    let file = File::open(path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    parse_header(&mmap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{self, Graph};
    use crate::edge;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("grw_persist_tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn round_trip_undir0() {
        use edge::undir::E::U;
        let g: graph::Undir0 = vec![U(0, 1), U(1, 2), U(2, 3), U(3, 0)]
            .try_into()
            .unwrap();
        let path = tmp_path("rt_undir0.grw");
        g.save(&path).unwrap();
        let g2: graph::Undir0 = Graph::load(&path).unwrap();
        assert_eq!(g.node_count(), g2.node_count());
        assert_eq!(g.edge_count(), g2.edge_count());
    }

    #[test]
    fn round_trip_dir0() {
        use edge::dir::E::D;
        let g: graph::Dir0 = vec![D(0, 1), D(1, 2), D(2, 0)].try_into().unwrap();
        let path = tmp_path("rt_dir0.grw");
        g.save(&path).unwrap();
        let g2: graph::Dir0 = Graph::load(&path).unwrap();
        assert_eq!(g.node_count(), g2.node_count());
        assert_eq!(g.edge_count(), g2.edge_count());
    }

    #[test]
    fn round_trip_anydir0() {
        use edge::anydir::E::{D, U};
        let g: graph::Anydir0 = vec![U(0, 1), D(1, 2), U(2, 3)].try_into().unwrap();
        let path = tmp_path("rt_anydir0.grw");
        g.save(&path).unwrap();
        let g2: graph::Anydir0 = Graph::load(&path).unwrap();
        assert_eq!(g.node_count(), g2.node_count());
        assert_eq!(g.edge_count(), g2.edge_count());
    }

    #[test]
    fn round_trip_valued() {
        use edge::undir::E::U;
        let g: graph::Undir<u32, u32> = (
            vec![(0, 10u32), (1, 20), (2, 30)],
            vec![(U(0, 1), 100u32), (U(1, 2), 200)],
        )
            .try_into()
            .unwrap();
        let path = tmp_path("rt_valued.grw");
        g.save(&path).unwrap();
        let g2: graph::Undir<u32, u32> = Graph::load(&path).unwrap();
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
        let Err(err) = Graph::<(), edge::Undir<()>>::load(&path) else {
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
        let Err(err) = Graph::<(), edge::Undir<()>>::load(&path) else {
            panic!("expected error");
        };
        let msg = err.to_string();
        assert!(msg.contains("version"), "expected version error, got: {msg}");
    }

    #[test]
    fn edge_kind_mismatch() {
        use edge::undir::E::U;
        let g: graph::Undir0 = vec![U(0, 1)].try_into().unwrap();
        let path = tmp_path("rt_edge_kind_mismatch.grw");
        g.save(&path).unwrap();
        let Err(err) = Graph::<(), edge::Dir<()>>::load(&path) else {
            panic!("expected error");
        };
        let msg = err.to_string();
        assert!(msg.contains("edge kind"), "expected edge kind error, got: {msg}");
    }

    #[test]
    fn layout_mismatch() {
        use edge::undir::E::U;
        let g: graph::Undir<u32, u32> = (
            vec![(0, 10u32), (1, 20)],
            vec![(U(0, 1), 100u32)],
        )
            .try_into()
            .unwrap();
        let path = tmp_path("rt_layout_mismatch.grw");
        g.save(&path).unwrap();
        let Err(err) = Graph::<i64, edge::Undir<i64>>::load(&path) else {
            panic!("expected error");
        };
        let msg = err.to_string();
        assert!(msg.contains("layout mismatch"), "expected layout error, got: {msg}");
    }

    #[test]
    fn read_header_round_trip() {
        use edge::undir::E::U;
        let g: graph::Undir<u32, u32> = (
            vec![(0, 10u32), (1, 20)],
            vec![(U(0, 1), 100u32)],
        )
            .try_into()
            .unwrap();
        let path = tmp_path("rt_read_header.grw");
        g.save(&path).unwrap();
        let header = read_header(&path).unwrap();
        assert_eq!(header.version, 2);
        assert_eq!(header.edge_kind, 0);
        assert_eq!(header.node_count, 2);
        assert_eq!(header.edge_count, 1);
        assert_eq!(header.nv_type, std::any::type_name::<u32>());
        assert_eq!(header.ev_type, std::any::type_name::<u32>());
        assert_ne!(header.nv_layout_hash, 0);
        assert_ne!(header.ev_layout_hash, 0);
    }
}
