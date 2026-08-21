# Persistence

GRW graphs can be saved to and loaded from binary files with layout validation.

## Save and Load

```rust
use std::path::Path;

// save
g.save(Path::new("my_graph.grw")).unwrap();

// load
let g2: graph::Undir0 = Graph::load(Path::new("my_graph.grw")).unwrap();
```

## Binary Format

The `.grw` file format (version 2) consists of:

1. **Magic bytes** — `GRW\0`
2. **Version** — `u16` (currently 2)
3. **Edge kind** — `u8` (0=undir, 1=dir, 2=anydir)
4. **Node/edge counts** — `u64` each
5. **Layout hashes** — `u64` for NV and EV types
6. **Type name strings** — for error messages
7. **Bincode-serialized graph data**

## Layout Validation

When loading, GRW validates that the types match:

- **Edge kind** must match (can't load a directed graph as undirected)
- **Layout hash** for node values must match
- **Layout hash** for edge values must match

```rust
// this will fail: type mismatch
let result = Graph::<i64, edge::Undir<i64>>::load(Path::new("u32_graph.grw"));
// Error: "node value layout mismatch: file has type `u32`, expected `i64`"
```

The layout hash is computed from the type's field names, types, and structure using FNV hashing. This catches:
- Wrong types (`u32` vs `i64`)
- Reordered fields
- Added/removed fields

## Memory-Mapped Loading

`load()` uses `memmap2` for memory-mapped file access. The file header is parsed first, then bincode deserializes directly from the mmap.

## The Val Trait

`save()` and `load()` require both `NV` and `EV` to implement the `Val` trait (for layout hash computation). Primitive types (`()`, `bool`, `u8`–`u64`, `i8`–`i64`, `f32`, `f64`, `String`) implement `Val` automatically. For custom structs, use `#[derive(Val)]`:

```rust
use grw::Val;

#[derive(Val, serde::Serialize, serde::Deserialize)]
struct MyNode {
    x: f64,
    y: f64,
    label: String,
}
```
