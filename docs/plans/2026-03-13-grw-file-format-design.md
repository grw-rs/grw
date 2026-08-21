# `.grw` File Format Design

Replaces the current `.shgr` format in `persist` module. Self-contained binary format with type metadata for the REPL plugin system.

Scope: new file format, persist module rewrite, Edge::EDGE_KIND constant, migration of callers.
Out of scope: plugin compilation, dynamic loading, REPL CLI, TypeName trait (eliminated — using std::any::type_name).

## Binary Layout

```
offset  size  field
─────────────────────────────────
0       4     magic: "GRW\0"
4       2     version: u16 (le)
6       1     edge_kind: u8
7       1     reserved: u8 (0)
8       8     node_count: u64 (le)
16      8     edge_count: u64 (le)
24      2     nv_type_len: u16 (le)
26      2     ev_type_len: u16 (le)
28      var   nv_type: utf8 bytes
28+nv   var   ev_type: utf8 bytes
28+nv+ev var  data: bincode payload
```

28-byte fixed header + variable-length type strings + data.

Version starts at 1 (new lineage, unrelated to `.shgr` version numbering which was at 2). The version field is u16 (was u32 in `.shgr`). No backward compatibility with `.shgr` (different magic, regenerate test data).

### Edge Kind

| value | type |
|-------|------|
| 0 | `edge::Undir<EV>` |
| 1 | `edge::Dir<EV>` |
| 2 | `edge::Anydir<EV>` |

### Type Strings

Produced by `std::any::type_name::<T>()` — fully qualified Rust paths, works on any type with zero trait bounds. Examples:

- `std::any::type_name::<()>()` → `"()"`
- `std::any::type_name::<i32>()` → `"i32"`
- `std::any::type_name::<String>()` → `"alloc::string::String"`
- `std::any::type_name::<my_crate::Cargo>()` → `"my_crate::Cargo"`

No custom trait needed. The persist module calls `type_name::<NV>()` and `type_name::<E::Val>()` automatically during save. No change to caller code.

## `Edge::EDGE_KIND` Constant

Add to the `Edge` trait:
```rust
pub trait Edge {
    const EDGE_KIND: u8;
    // ... existing associated types and methods
}
```

With `Undir::EDGE_KIND = 0`, `Dir::EDGE_KIND = 1`, `Anydir::EDGE_KIND = 2`.

## Updated `persist` API

### Header type

```rust
pub struct Header {
    pub version: u16,
    pub edge_kind: u8,
    pub node_count: u64,
    pub edge_count: u64,
    pub nv_type: String,
    pub ev_type: String,
}
```

### Functions

```rust
pub fn save<NV, E>(graph: &graph::Graph<NV, E>, path: &Path) -> io::Result<()>
where
    NV: serde::Serialize,
    E: graph::Edge,
    E::Val: serde::Serialize,
    E::Slot: serde::Serialize,
```

Signature is identical to current — no new trait bounds. Internally uses `std::any::type_name::<NV>()` and `std::any::type_name::<E::Val>()` to write type strings into the header.

Uses buffered file writes (not mmap). Writes: magic, version=1, edge_kind from `E::EDGE_KIND`, reserved=0, node_count, edge_count, nv_type, ev_type, then bincode serialized graph data.

```rust
pub fn load<NV, E>(path: &Path) -> io::Result<graph::Graph<NV, E>>
where
    NV: serde::de::DeserializeOwned,
    E: graph::Edge,
    E::Val: serde::de::DeserializeOwned,
    E::Slot: serde::de::DeserializeOwned,
```

Validates magic + version. Validates `edge_kind` matches `E::EDGE_KIND` (hard error on mismatch). Skips past type strings. Deserializes bincode payload. Type string mismatch is not checked by `load` (caller knows their types).

```rust
pub fn read_header(path: &Path) -> io::Result<Header>
```

Reads fixed header + type strings without deserializing bincode. Validates magic + version. Used by the REPL plugin system to determine which types to compile for. Implementation uses mmap (same as `load`).

## Error Handling

`save`: `io::Result<()>` (same as current).

`load` errors:
- Bad magic → `InvalidData`
- Unsupported version → `InvalidData`
- Edge kind mismatch → `InvalidData` with message identifying expected vs actual
- Truncated file → `InvalidData`
- Bincode deserialization failure → `InvalidData`

`read_header` errors:
- Bad magic → `InvalidData`
- Unsupported version → `InvalidData`
- Truncated file → `InvalidData`

## Known Limitations

- Type string length capped at 65535 bytes (u16). Sufficient for any practical Rust path.
- `std::any::type_name` output format is documented as "best effort, not stabilized." In practice it has been consistent across rustc versions. The REPL plugin cache key includes rustc version, so a format change triggers a recompile rather than a mismatch.
- `node_count` and `edge_count` are u64 in the file format. `Graph::node_count()` returns `usize`. On 64-bit platforms this is a no-op cast. 32-bit platforms are not a target.
- No alignment padding between type strings and bincode payload. Bincode does byte-level deserialization, so no alignment is required.

## Migration

### Files modified:
- `src/persist/mod.rs` — rewrite save/load, add read_header, Header, update tests
- `src/graph/edge.rs` — add `const EDGE_KIND: u8` to Edge trait and impls

### Callers updated:
- `src/bin/crosscheck.rs` — rename `.shgr` → `.grw` in `cell_path()` hardcoded extension
- `src/bin/verify_patterns.rs` — rename `"shgr"` → `"grw"` in `find_file_by_ext` call, rename `shgr_path` variable
- `src/bin/gen/valued.rs` — rename `.shgr` → `.grw` in output path, rename `valued_shgr_path` variable

### Data regenerated:
- Delete existing `.shgr` files in `data/` (they will not load under the new format)
- Regenerate with `cargo run --features crosscheck --bin gen -- all --out-dir data/crosscheck` (now produces `.grw` files)
- Rename `.shgr` references to `.grw` in all test and binary source files

### No backward compatibility:
Old `.shgr` files will not load (different magic). All test data is generated, easily regenerated.
