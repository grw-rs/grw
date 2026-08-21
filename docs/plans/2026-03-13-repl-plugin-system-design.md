# REPL Plugin System Design

Auto-compile and cache cdylib plugins per (NV, EV) type combination so the REPL can load `.grw` files and evaluate predicates without the user writing any glue code.

Scope: plugin trait (C ABI), cache management, crate generation, compilation, `libloading`-based loader, `.grw` header v2 with layout hashes, `Val` impls for primitives, `impl_val!` macro.
Out of scope: Cranelift JIT, method calls in predicates, search pattern execution, edge value inspection, REPL CLI.

## Plugin ABI

The plugin is a cdylib exposing `extern "C"` functions. No Rust trait objects across the dylib boundary (different compiler invocations = different vtables).

```rust
#[repr(C)]
pub struct PluginResult {
    pub data_ptr: *mut u8,
    pub data_len: usize,
    pub error_ptr: *mut u8,
    pub error_len: usize,
}
```

`PluginResult` invariant: on success, `error_ptr` is null and `error_len` is 0. On error, `data_ptr` is null and `data_len` is 0. Exactly one side is populated, never both.

```rust
extern "C" fn grw_plugin_load(path_ptr: *const u8, path_len: usize) -> PluginResult;
extern "C" fn grw_plugin_free(graph: *mut c_void);
extern "C" fn grw_plugin_free_result(result: PluginResult);
extern "C" fn grw_plugin_node_count(graph: *const c_void) -> u64;
extern "C" fn grw_plugin_edge_count(graph: *const c_void) -> u64;
extern "C" fn grw_plugin_eval_pred(graph: *const c_void, src_ptr: *const u8, src_len: usize) -> PluginResult;
extern "C" fn grw_plugin_inspect_node(graph: *const c_void, node_idx: u64) -> PluginResult;
extern "C" fn grw_plugin_node_fields() -> PluginResult;
```

`grw_plugin_load`: on success, `data_ptr` is the opaque graph handle (cast from `Box<Graph<NV, E>>` via `Box::into_raw` then to `*mut u8`), `data_len` is 0 (unused). On error, `error_ptr`/`error_len` contain the error message. The caller passes `data_ptr` as `*mut c_void` to subsequent functions.

`grw_plugin_free`: frees the graph handle (reconstructs `Box<Graph<NV, E>>` and drops it).

`grw_plugin_free_result`: frees both `data_ptr` and `error_ptr` if non-null (reconstructs `Vec<u8>` from ptr/len and drops). Must be called for every `PluginResult` returned by `eval_pred`, `inspect_node`, and `node_fields`.

The plugin knows the concrete NV and E types — all type-specific logic (deserialize, predicate eval, field inspection) is baked in at compile time.

## Cache Structure

Location: `~/.cache/grw/plugins/<hash>/`

Cache key inputs:
- `nv_type` string (from `.grw` header)
- `ev_type` string (from `.grw` header)
- `edge_kind` (0/1/2)
- `nv_layout_hash` (from `.grw` header v2)
- `ev_layout_hash` (from `.grw` header v2)
- `rustc --version` output
- `grw` crate version (from the types crate's resolved `grw` dependency)

Key is SHA-256 of the above, truncated to 16 hex chars. Collisions are accepted as astronomically unlikely for a cache key (not security-critical).

```
~/.cache/grw/plugins/
  a3f1c9e02b7d4e8a/
    plugin.so
    meta.json
```

`meta.json` stores the full cache key inputs for human inspection.

Cache hit: load `plugin.so` directly.
Cache miss: generate crate, compile, copy artifact, write `meta.json`.
No automatic cache eviction. `rm -rf ~/.cache/grw/plugins/` to clean.

## Types Crate Discovery

1. If `--types-crate path` is provided, use that.
2. Otherwise, walk up from the `.grw` file's directory looking for `Cargo.toml`.
3. If not found, error with actionable message suggesting `--types-crate`.

The type name from the header (e.g. `my_crate::Cargo`) gives the crate name as its first path segment. The discovered `Cargo.toml` directory becomes the `path = "..."` dependency. Cargo resolves the crate name automatically.

Type name prefixes that require no external dependency (no crate added to generated Cargo.toml):
- Primitives with no path: `u32`, `i64`, `bool`, `f64`, `()`, etc.
- `std::*`, `core::*`, `alloc::*` prefixes (e.g. `alloc::string::String`)

All other first path segments are treated as crate names requiring a path dependency.

For types from transitive dependencies (e.g. `ev_type = "other_crate::Weight"`), the generated plugin inherits the dependency graph from the types crate — no extra configuration needed.

## Plugin Crate Generation

On cache miss, generate a temporary crate:

```
/tmp/grw_plugin_XXXX/
  Cargo.toml
  src/lib.rs
```

Generated `Cargo.toml`:
```toml
[package]
name = "grw_plugin"
version = "0.0.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
my_crate = { path = "/abs/path/to/users/project", features = ["persist"] }
```

The user's crate must depend on `grw` with the `persist` feature enabled. The `grw` crate is NOT listed as a direct dependency — it comes transitively through the user's crate. If the user's crate does not enable the `persist` feature, the generated plugin will fail to compile with a clear cargo error pointing at the missing `persist` module.

Generated `src/lib.rs`: imports the concrete types, implements all `extern "C"` functions. Edge type selected from `edge_kind` via:

```rust
// edge_kind 0 → grw::edge::Undir<EV>
// edge_kind 1 → grw::edge::Dir<EV>
// edge_kind 2 → grw::edge::Anydir<EV>
```

The codegen emits one of three type aliases based on the `edge_kind` value read from the header, then uses that alias in `persist::load::<NV, EdgeType>()`, `compile_predicate::<NV>()`, etc.

Compilation: `cargo build --release` in the temp dir. Copy `.so`/`.dylib` to cache directory. Clean up temp dir.

## REPL-Side Plugin Loader

New `plugin` module in `grw_repl`:

```rust
pub struct Plugin {
    _lib: libloading::Library,
    load_graph: unsafe extern "C" fn(*const u8, usize) -> PluginResult,
    free_graph: unsafe extern "C" fn(*mut c_void),
    free_result: unsafe extern "C" fn(PluginResult),
    node_count: unsafe extern "C" fn(*const c_void) -> u64,
    edge_count: unsafe extern "C" fn(*const c_void) -> u64,
    eval_pred: unsafe extern "C" fn(*const c_void, *const u8, usize) -> PluginResult,
    inspect_node: unsafe extern "C" fn(*const c_void, u64) -> PluginResult,
    node_fields: unsafe extern "C" fn() -> PluginResult,
}

pub struct LoadedGraph { /* holds *mut c_void, ref to Plugin for drop */ }

impl Plugin {
    pub fn open(path: &Path) -> Result<Self, PluginError>;
    pub fn load_graph(&self, grw_path: &Path) -> Result<LoadedGraph, PluginError>;
    pub fn node_fields(&self) -> Result<String, PluginError>;
}

impl LoadedGraph {
    pub fn node_count(&self) -> u64;
    pub fn edge_count(&self) -> u64;
    pub fn eval_pred(&self, src: &str) -> Result<String, PluginError>;
    pub fn inspect_node(&self, idx: u64) -> Result<String, PluginError>;
}
```

`node_fields` is on `Plugin` (not `LoadedGraph`) since it's a type-level property, not instance-level.

Orchestrator:
```rust
pub fn load_or_compile(grw_path: &Path, types_crate: Option<&Path>) -> Result<Plugin, PluginError>
```

Reads header, computes cache key, checks cache, generates + compiles on miss, loads plugin.

## `.grw` Header v2

Extend the fixed header with layout hashes. `persist` supports both v1 and v2.

```
offset  size  field
─────────────────────────────────
0       4     magic: "GRW\0"
4       2     version: u16 (le) = 2
6       1     edge_kind: u8
7       1     reserved: u8 (0)
8       8     node_count: u64 (le)
16      8     edge_count: u64 (le)
24      8     nv_layout_hash: u64 (le)
32      8     ev_layout_hash: u64 (le)
40      2     nv_type_len: u16 (le)
42      2     ev_type_len: u16 (le)
44      var   nv_type: utf8 bytes
44+nv   var   ev_type: utf8 bytes
44+nv+ev var  data: bincode payload
```

44-byte fixed header (was 28 in v1).

### Version branching in `parse_header`

`parse_header` reads the version field and dispatches:
- **Version 1** (28-byte header): layout hashes default to 0, type string lengths at offsets 24-27, type strings at offset 28. Bincode data follows type strings.
- **Version 2** (44-byte header): layout hashes at offsets 24-39, type string lengths at offsets 40-43, type strings at offset 44. Bincode data follows type strings.
- **Other versions**: reject with `InvalidData`.

`Header` struct includes `nv_layout_hash: u64` and `ev_layout_hash: u64` (0 for v1 files).

`persist::save` always writes version 2 (requires `NV: Val` and `E::Val: Val` bounds in addition to `Serialize`). `persist::load` accepts both v1 and v2 — it does NOT require `Val` (skips hash fields regardless of version).

The REPL requires version 2 (non-zero layout hashes). If it encounters version 1, it errors: "This graph was saved without layout metadata. Re-save with the current version to use the REPL."

## Val Trait Extension

Add `layout_hash()` to the `Val` trait:

```rust
pub trait Val: 'static {
    fn fields() -> &'static [FieldInfo];
    fn layout_hash() -> u64;
    fn size() -> usize;
    fn align() -> usize;
}
```

Hash inputs (FNV-1a or similar): field count, then for each field: name bytes, type discriminant. For `FieldType::Struct` variants, recursively include the nested type's `layout_hash()`. Does NOT include offsets (those depend on compiler layout and are covered by the rustc version in the cache key).

### Built-in Val impls

Provided in `grw::layout` for: `()`, `bool`, `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `f32`, `f64`, `String`.

Each returns `fields() -> &[]` and a unique constant `layout_hash()`.

### `impl_val!` macro

For foreign types (type path argument) the user wants as graph values but can't derive Val on:

```rust
grw::impl_val!(std::path::PathBuf);
```

Generates a Val impl with empty fields and a layout hash derived from the type name string. These types are opaque in the REPL (no field access) but work for persist and cache invalidation.

## Error Handling

- No `Cargo.toml` found: "Could not find types crate. Use `--types-crate path/to/crate` or run from within the project directory."
- Compilation failure: show cargo stderr, suggest `--types-crate` if wrong crate auto-discovered.
- `persist` feature not enabled: cargo error will mention missing `persist` module — the REPL reports: "The types crate must depend on `grw` with the `persist` feature enabled."
- Cache dir not writable: fall back to temp dir, warn user.
- Version 1 `.grw` file with REPL: "This graph was saved without layout metadata. Re-save with the current version to use the REPL."
- Graph load failure in plugin: `PluginResult` with error message (deserialization failure, IO error, edge kind mismatch).

Compilation progress printed to stderr: `Compiling plugin for my_crate::Cargo... done (3.2s)`.

## Files Modified

- `src/layout/mod.rs` — add `layout_hash()` to Val trait, built-in impls for primitives, `impl_val!` macro
- `grw_derive/src/val.rs` — generate `layout_hash()` in derive expansion
- `src/persist/mod.rs` — v2 header (44 bytes), v1/v2 branching in parse_header, layout hashes in save/read_header, `Val` bound on save
- `grw_repl/Cargo.toml` — add `libloading`, `sha2`, `serde_json` dependencies
- `grw_repl/src/plugin/mod.rs` — create: Plugin, LoadedGraph, load_or_compile orchestrator
- `grw_repl/src/plugin/cache.rs` — create: cache key computation, cache directory management
- `grw_repl/src/plugin/codegen.rs` — create: plugin crate generation (Cargo.toml + lib.rs templates)
- `grw_repl/src/plugin/abi.rs` — create: PluginResult, C ABI types shared between plugin and loader

## Known Limitations

- Method calls not supported in predicates (field access and operators only).
- `impl_val!` types are opaque in REPL — no field-level access.
- Edge value inspection not supported in v1 — only node predicates and node inspection.
- No automatic cache eviction.
- Plugin compilation requires `cargo` and `rustc` on PATH.
- `std::any::type_name` output is "best effort" — rustc version in cache key mitigates format changes.
- Minimum rustc version: 1.85+ (edition 2024 support required for generated plugin crate).
