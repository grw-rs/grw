# `.grw` File Format Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `.shgr` binary format with a new `.grw` format that embeds type metadata (edge kind, node/edge value type names) for REPL plugin auto-detection.

**Architecture:** Add `const EDGE_KIND: u8` to the `Edge` trait and its three impls. Rewrite `persist::save` to write a 28-byte fixed header + variable-length type strings + bincode payload. Rewrite `persist::load` to validate magic/version/edge_kind then deserialize. Add `persist::read_header` for introspection. Update all callers and test data to use `.grw` extension.

**Tech Stack:** Rust, bincode, memmap2, serde, std::any::type_name

**Spec:** `docs/plans/2026-03-13-grw-file-format-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/graph/edge.rs` | Modify | Add `const EDGE_KIND: u8` to `Edge` trait + 3 impls |
| `src/persist/mod.rs` | Rewrite | New header format, save/load/read_header, Header struct |
| `src/bin/crosscheck.rs` | Modify | `.shgr` → `.grw` in `cell_path()` |
| `src/bin/gen/valued.rs` | Modify | `.shgr` → `.grw` in output path |
| `src/bin/verify_patterns.rs` | Modify | `"shgr"` → `"grw"` in file extension lookup |

---

## Chunk 1: Core Format

### Task 1: Add `EDGE_KIND` to Edge trait

**Files:**
- Modify: `src/graph/edge.rs:15-37` (Edge trait definition)
- Modify: `src/graph/edge.rs:121-160` (Undir impl)
- Modify: `src/graph/edge.rs:257-313` (Dir impl)
- Modify: `src/graph/edge.rs:458-521` (Anydir impl)

- [ ] **Step 1: Add `const EDGE_KIND: u8` to the Edge trait**

In `src/graph/edge.rs`, inside the `Edge` trait (line 15), add after `const SLOT_COUNT: usize`:

```rust
const EDGE_KIND: u8;
```

- [ ] **Step 2: Add `EDGE_KIND = 0` to Undir impl**

In `src/graph/edge.rs`, inside `impl<V> Edge for super::Undir<V>` (the `undir` module, around line 121), add:

```rust
const EDGE_KIND: u8 = 0;
```

- [ ] **Step 3: Add `EDGE_KIND = 1` to Dir impl**

In `src/graph/edge.rs`, inside `impl<V> Edge for super::Dir<V>` (the `dir` module, around line 257), add:

```rust
const EDGE_KIND: u8 = 1;
```

- [ ] **Step 4: Add `EDGE_KIND = 2` to Anydir impl**

In `src/graph/edge.rs`, inside `impl<V> Edge for super::Anydir<V>` (the `anydir` module, around line 458), add:

```rust
const EDGE_KIND: u8 = 2;
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check --features persist`
Expected: PASS (no errors)

---

### Task 2: Rewrite persist module — Header struct and constants

**Files:**
- Modify: `src/persist/mod.rs:1-9` (imports and constants)

- [ ] **Step 1: Replace magic and version constants, add Header struct**

Replace the top of `src/persist/mod.rs` (lines 1-8) with:

```rust
use crate::graph;

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

const MAGIC: [u8; 4] = *b"GRW\0";
const VERSION: u16 = 1;
const FIXED_HEADER_SIZE: usize = 28;

pub struct Header {
    pub version: u16,
    pub edge_kind: u8,
    pub node_count: u64,
    pub edge_count: u64,
    pub nv_type: String,
    pub ev_type: String,
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --features persist`
Expected: FAIL — save/load still reference old VERSION type (u32). This is expected; we fix them next.

---

### Task 3: Rewrite `save` function

**Files:**
- Modify: `src/persist/mod.rs:10-22` (save function)

- [ ] **Step 1: Replace the save function**

Replace the entire `save` function with:

```rust
pub fn save<NV, E>(graph: &graph::Graph<NV, E>, path: &Path) -> io::Result<()>
where
    NV: serde::Serialize,
    E: graph::Edge,
    E::Slot: serde::Serialize,
    E::Val: serde::Serialize,
{
    let nv_type = std::any::type_name::<NV>();
    let ev_type = std::any::type_name::<E::Val>();
    let nv_type_bytes = nv_type.as_bytes();
    let ev_type_bytes = ev_type.as_bytes();

    let mut file = File::create(path)?;
    file.write_all(&MAGIC)?;
    file.write_all(&VERSION.to_le_bytes())?;
    file.write_all(&[E::EDGE_KIND])?;
    file.write_all(&[0u8])?; // reserved
    file.write_all(&(graph.node_count() as u64).to_le_bytes())?;
    file.write_all(&(graph.edge_count() as u64).to_le_bytes())?;
    file.write_all(&(nv_type_bytes.len() as u16).to_le_bytes())?;
    file.write_all(&(ev_type_bytes.len() as u16).to_le_bytes())?;
    file.write_all(nv_type_bytes)?;
    file.write_all(ev_type_bytes)?;
    bincode::serialize_into(&mut file, graph)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}
```

`std::any::type_name::<T>()` has no `'static` bound on `T` — the signature is identical to the current `save`.

- [ ] **Step 2: Verify compilation (save only)**

Run: `cargo check --features persist`
Expected: FAIL — `load` still references old format. Expected; we fix it next.

---

### Task 4: Rewrite `load` function

**Files:**
- Modify: `src/persist/mod.rs:24-49` (load function)

- [ ] **Step 1: Replace the load function**

Replace the entire `load` function with:

```rust
pub fn load<NV, E>(path: &Path) -> io::Result<graph::Graph<NV, E>>
where
    NV: serde::de::DeserializeOwned,
    E: graph::Edge,
    E::Slot: serde::de::DeserializeOwned,
    E::Val: serde::de::DeserializeOwned,
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
    let data_offset = FIXED_HEADER_SIZE + header.nv_type.len() + header.ev_type.len();
    let data = &mmap[data_offset..];
    bincode::deserialize(data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --features persist`
Expected: FAIL — `parse_header` doesn't exist yet. Expected; next step.

---

### Task 5: Add `read_header` and internal `parse_header`

**Files:**
- Modify: `src/persist/mod.rs` (add after load function)

- [ ] **Step 1: Add `parse_header` (internal) and `read_header` (public)**

Add after the `load` function:

```rust
fn parse_header(data: &[u8]) -> io::Result<Header> {
    if data.len() < FIXED_HEADER_SIZE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file too short"));
    }
    if &data[..4] != &MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic"));
    }
    let version = u16::from_le_bytes([data[4], data[5]]);
    if version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported version: {version}"),
        ));
    }
    let edge_kind = data[6];
    let node_count = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let edge_count = u64::from_le_bytes(data[16..24].try_into().unwrap());
    let nv_type_len = u16::from_le_bytes([data[24], data[25]]) as usize;
    let ev_type_len = u16::from_le_bytes([data[26], data[27]]) as usize;

    let required = FIXED_HEADER_SIZE + nv_type_len + ev_type_len;
    if data.len() < required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file truncated in type strings",
        ));
    }

    let nv_type = std::str::from_utf8(&data[FIXED_HEADER_SIZE..FIXED_HEADER_SIZE + nv_type_len])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        .to_owned();
    let ev_type = std::str::from_utf8(
        &data[FIXED_HEADER_SIZE + nv_type_len..FIXED_HEADER_SIZE + nv_type_len + ev_type_len],
    )
    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
    .to_owned();

    Ok(Header {
        version,
        edge_kind,
        node_count,
        edge_count,
        nv_type,
        ev_type,
    })
}

pub fn read_header(path: &Path) -> io::Result<Header> {
    let file = File::open(path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    parse_header(&mmap)
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --features persist`
Expected: PASS (all three functions compile, tests will fail due to format change — that's expected)

---

### Task 6: Update persist tests

**Files:**
- Modify: `src/persist/mod.rs:51-140` (test module)

- [ ] **Step 1: Rewrite tests for new format**

Replace the entire `#[cfg(test)] mod tests` block with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
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
        save(&g, &path).unwrap();
        let g2: graph::Undir0 = load(&path).unwrap();
        assert_eq!(g.node_count(), g2.node_count());
        assert_eq!(g.edge_count(), g2.edge_count());
    }

    #[test]
    fn round_trip_dir0() {
        use edge::dir::E::D;
        let g: graph::Dir0 = vec![D(0, 1), D(1, 2), D(2, 0)].try_into().unwrap();
        let path = tmp_path("rt_dir0.grw");
        save(&g, &path).unwrap();
        let g2: graph::Dir0 = load(&path).unwrap();
        assert_eq!(g.node_count(), g2.node_count());
        assert_eq!(g.edge_count(), g2.edge_count());
    }

    #[test]
    fn round_trip_anydir0() {
        use edge::anydir::E::{D, U};
        let g: graph::Anydir0 = vec![U(0, 1), D(1, 2), U(2, 3)].try_into().unwrap();
        let path = tmp_path("rt_anydir0.grw");
        save(&g, &path).unwrap();
        let g2: graph::Anydir0 = load(&path).unwrap();
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
        save(&g, &path).unwrap();
        let g2: graph::Undir<u32, u32> = load(&path).unwrap();
        assert_eq!(g.node_count(), g2.node_count());
        assert_eq!(g.edge_count(), g2.edge_count());
        assert_eq!(g.get(0u32), g2.get(0u32));
        assert_eq!(g.get(1u32), g2.get(1u32));
        assert_eq!(g.get(2u32), g2.get(2u32));
    }

    #[test]
    fn roundtrip_structure_preserved() {
        use edge::undir::E::U;
        let g: graph::Undir0 = vec![
            U(0, 1), U(1, 2), U(2, 0),
            U(0, 3), U(0, 4), U(0, 5),
            U(5, 6), U(6, 7),
        ]
            .try_into()
            .unwrap();
        let path = tmp_path("rt_structure.grw");
        save(&g, &path).unwrap();
        let g2: graph::Undir0 = load(&path).unwrap();
        assert_eq!(g.node_count(), g2.node_count());
        assert_eq!(g.edge_count(), g2.edge_count());
    }

    #[test]
    fn header_validation_bad_magic() {
        let path = tmp_path("rt_bad_magic.grw");
        std::fs::write(&path, b"BADMxxxxxxxxxxxxxxxxxxxxxxxxxxxx").unwrap();
        let result = load::<(), edge::Undir<()>>(&path);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("magic"), "expected magic error, got: {msg}");
    }

    #[test]
    fn header_validation_bad_version() {
        let path = tmp_path("rt_bad_version.grw");
        let mut data = vec![0u8; 32];
        data[..4].copy_from_slice(b"GRW\0");
        data[4..6].copy_from_slice(&99u16.to_le_bytes()); // bad version
        std::fs::write(&path, &data).unwrap();
        let result = load::<(), edge::Undir<()>>(&path);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("version"), "expected version error, got: {msg}");
    }

    #[test]
    fn edge_kind_mismatch() {
        use edge::undir::E::U;
        let g: graph::Undir0 = vec![U(0, 1)].try_into().unwrap();
        let path = tmp_path("rt_edge_kind_mismatch.grw");
        save(&g, &path).unwrap();
        let result = load::<(), edge::Dir<()>>(&path);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("edge kind"), "expected edge kind error, got: {msg}");
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
        save(&g, &path).unwrap();
        let header = read_header(&path).unwrap();
        assert_eq!(header.version, 1);
        assert_eq!(header.edge_kind, 0); // Undir
        assert_eq!(header.node_count, 2);
        assert_eq!(header.edge_count, 1);
        assert_eq!(header.nv_type, std::any::type_name::<u32>());
        assert_eq!(header.ev_type, std::any::type_name::<u32>());
    }

    #[test]
    fn read_header_dir() {
        use edge::dir::E::D;
        let g: graph::Dir0 = vec![D(0, 1)].try_into().unwrap();
        let path = tmp_path("rt_read_header_dir.grw");
        save(&g, &path).unwrap();
        let header = read_header(&path).unwrap();
        assert_eq!(header.edge_kind, 1); // Dir
        assert_eq!(header.nv_type, std::any::type_name::<()>());
        assert_eq!(header.ev_type, std::any::type_name::<()>());
    }

    #[test]
    fn read_header_anydir() {
        use edge::anydir::E::{D, U};
        let g: graph::Anydir0 = vec![U(0, 1), D(1, 2)].try_into().unwrap();
        let path = tmp_path("rt_read_header_anydir.grw");
        save(&g, &path).unwrap();
        let header = read_header(&path).unwrap();
        assert_eq!(header.edge_kind, 2); // Anydir
    }
}
```

- [ ] **Step 2: Run persist tests**

Run: `cargo test --features persist persist`
Expected: all 11 tests PASS

---

## Chunk 2: Caller Migration

### Task 7: Update crosscheck.rs

**Files:**
- Modify: `src/bin/crosscheck.rs:491`

- [ ] **Step 1: Change `.shgr` to `.grw` in `cell_path`**

In `src/bin/crosscheck.rs`, line 491, change:

```rust
PathBuf::from(dir).join(format!("{}.shgr", cell_filename(cell)))
```

to:

```rust
PathBuf::from(dir).join(format!("{}.grw", cell_filename(cell)))
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --features crosscheck`
Expected: PASS

---

### Task 8: Update gen/valued.rs

**Files:**
- Modify: `src/bin/gen/valued.rs:204-209`

- [ ] **Step 1: Rename `.shgr` to `.grw` and update variable name**

In `src/bin/gen/valued.rs`, replace lines 204-209:

```rust
        let valued_shgr_path = graph_dir.join(format!("{density_label}_valued.shgr"));
        grw::persist::save(&valued_grw, &valued_shgr_path).unwrap_or_else(|e| {
            eprintln!("failed to save {}: {e}", valued_shgr_path.display());
            std::process::exit(1);
        });
        println!("  saved: {}", valued_shgr_path.file_name().unwrap().to_string_lossy());
```

with:

```rust
        let valued_grw_path = graph_dir.join(format!("{density_label}_valued.grw"));
        grw::persist::save(&valued_grw, &valued_grw_path).unwrap_or_else(|e| {
            eprintln!("failed to save {}: {e}", valued_grw_path.display());
            std::process::exit(1);
        });
        println!("  saved: {}", valued_grw_path.file_name().unwrap().to_string_lossy());
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --features crosscheck`
Expected: PASS

---

### Task 9: Update verify_patterns.rs

**Files:**
- Modify: `src/bin/verify_patterns.rs:516-520`

- [ ] **Step 1: Rename `shgr` to `grw` and update variable name**

In `src/bin/verify_patterns.rs`, replace lines 516-520:

```rust
    let shgr_path = find_file_by_ext(graph_dir, "shgr", Some("_valued"));
    let grb_path = find_file_by_ext(graph_dir, "grb", Some("_valued"));

    let grw_g: grw::graph::Undir<u32, u32> = grw::persist::load(&shgr_path).unwrap_or_else(|e| {
        eprintln!("failed to load {}: {e}", shgr_path.display());
```

with:

```rust
    let grw_path = find_file_by_ext(graph_dir, "grw", Some("_valued"));
    let grb_path = find_file_by_ext(graph_dir, "grb", Some("_valued"));

    let grw_g: grw::graph::Undir<u32, u32> = grw::persist::load(&grw_path).unwrap_or_else(|e| {
        eprintln!("failed to load {}: {e}", grw_path.display());
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --features crosscheck`
Expected: PASS

---

### Task 10: Delete old `.shgr` files and regenerate data

- [ ] **Step 1: Delete old .shgr files**

Run: `find data/ -name "*.shgr" -delete`

- [ ] **Step 2: Regenerate graph data**

Run: `cargo run --features crosscheck --bin gen -- all --out-dir data/crosscheck`
Expected: generates new `.grw` files in `data/crosscheck/`

- [ ] **Step 3: Verify data files exist**

Run: `ls data/crosscheck/*.grw`
Expected: list of `.grw` files matching the previous `.shgr` files

- [ ] **Step 4: Full test suite**

Run: `cargo test --features persist`
Expected: all tests pass, including persist module tests

---

## Post-Completion

After all tasks complete, run the full test suite to confirm no regressions:

```bash
cargo test --features persist
cargo check --features crosscheck
```
