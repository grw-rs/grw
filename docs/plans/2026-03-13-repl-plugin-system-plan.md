# REPL Plugin System Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Auto-compile and cache cdylib plugins per graph type so the REPL can load `.grw` files and evaluate predicates without glue code.

**Architecture:** Extend the `Val` trait with `layout_hash()` and provide built-in impls for primitives. Upgrade persist to v2 header with layout hashes. Build a plugin system that generates, compiles, caches, and loads cdylib plugins via `libloading` using a C ABI.

**Tech Stack:** Rust, libloading, sha2, serde_json, bincode, memmap2

**Spec:** `docs/plans/2026-03-13-repl-plugin-system-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/layout/mod.rs` | Modify | `layout_hash()` on Val, FNV helpers, built-in primitive impls, `impl_val!` macro |
| `grw_derive/src/val.rs` | Modify | Generate `layout_hash()` in derive expansion |
| `grw_derive/tests/basic.rs` | Modify | Tests for layout_hash |
| `src/persist/mod.rs` | Modify | v2 header (44 bytes), v1/v2 branching, Val bound on save |
| `grw_repl/Cargo.toml` | Modify | Add libloading, sha2, serde_json, hex, dirs deps; enable `persist` feature on grw |
| `grw_repl/src/lib.rs` | Modify | Add `pub mod plugin` |
| `grw_repl/src/plugin/mod.rs` | Create | Plugin, LoadedGraph, load_or_compile orchestrator |
| `grw_repl/src/plugin/abi.rs` | Create | PluginResult, C ABI type definitions |
| `grw_repl/src/plugin/cache.rs` | Create | Cache key computation, directory management |
| `grw_repl/src/plugin/codegen.rs` | Create | Plugin crate generation (Cargo.toml + lib.rs templates) |

---

## Chunk 1: Val Layout Hash

### Task 1: FNV helpers and layout_hash() on Val trait

**Files:**
- Modify: `src/layout/mod.rs`

- [ ] **Step 1: Add FNV-1a helper functions and layout_hash to Val trait**

In `src/layout/mod.rs`, add the FNV constants and helpers before the `Val` trait, and add `layout_hash()` to the trait:

```rust
pub const FNV_OFFSET: u64 = 14695981039346656037;
pub const FNV_PRIME: u64 = 1099511628211;

pub fn fnv_hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash = (hash ^ b as u64).wrapping_mul(FNV_PRIME);
    }
    hash
}

pub fn fnv_hash_u64(hash: u64, val: u64) -> u64 {
    fnv_hash_bytes(hash, &val.to_le_bytes())
}

pub fn fnv_hash_byte(hash: u64, byte: u8) -> u64 {
    (hash ^ byte as u64).wrapping_mul(FNV_PRIME)
}
```

Add `layout_hash()` to the `Val` trait:

```rust
pub trait Val: 'static {
    fn fields() -> &'static [FieldInfo];
    fn layout_hash() -> u64;
    fn size() -> usize;
    fn align() -> usize;
}
```

- [ ] **Step 2: Add built-in Val impls for primitives**

In `src/layout/mod.rs`, add after the `Val` trait. Each primitive gets `fields() -> &[]` and a unique constant `layout_hash()`:

```rust
macro_rules! impl_val_primitive {
    ($ty:ty, $hash:expr) => {
        impl Val for $ty {
            fn fields() -> &'static [FieldInfo] { &[] }
            fn layout_hash() -> u64 { $hash }
            fn size() -> usize { std::mem::size_of::<$ty>() }
            fn align() -> usize { std::mem::align_of::<$ty>() }
        }
    };
}

impl_val_primitive!((), 0x00);
impl_val_primitive!(bool, 0x01);
impl_val_primitive!(i8, 0x02);
impl_val_primitive!(i16, 0x03);
impl_val_primitive!(i32, 0x04);
impl_val_primitive!(i64, 0x05);
impl_val_primitive!(u8, 0x06);
impl_val_primitive!(u16, 0x07);
impl_val_primitive!(u32, 0x08);
impl_val_primitive!(u64, 0x09);
impl_val_primitive!(f32, 0x0A);
impl_val_primitive!(f64, 0x0B);
impl_val_primitive!(String, 0x0C);
```

- [ ] **Step 3: Add `impl_val!` public macro**

In `src/layout/mod.rs`, add the public macro for foreign types:

```rust
#[macro_export]
macro_rules! impl_val {
    ($ty:ty) => {
        impl $crate::layout::Val for $ty {
            fn fields() -> &'static [$crate::layout::FieldInfo] { &[] }
            fn layout_hash() -> u64 {
                $crate::layout::fnv_hash_bytes(
                    $crate::layout::FNV_OFFSET,
                    std::any::type_name::<$ty>().as_bytes(),
                )
            }
            fn size() -> usize { std::mem::size_of::<$ty>() }
            fn align() -> usize { std::mem::align_of::<$ty>() }
        }
    };
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check --features persist`
Expected: FAIL — `grw_derive::Val` does not generate `layout_hash()` yet. The derive macro needs updating in Task 2.

---

### Task 2: Generate layout_hash in derive macro

**Files:**
- Modify: `grw_derive/src/val.rs`

The derive macro needs to generate a `layout_hash()` method that hashes: field count, then for each field: name bytes + type discriminant byte. For `Struct` fields, also hash the nested type's `layout_hash()`.

Type discriminant constants (matching FieldType variant order):
- Bool=1, I8=2, I16=3, I32=4, I64=5, U8=6, U16=7, U32=8, U64=9, F32=10, F64=11, String=12, Struct=13

- [ ] **Step 1: Add discriminant helper to val.rs**

Add a function that returns the discriminant byte for a type, or `None` for struct types:

```rust
fn type_discriminant(seg: &PathSegment) -> Option<u8> {
    match seg.ident.to_string().as_str() {
        "bool" => Some(1),
        "i8" => Some(2),
        "i16" => Some(3),
        "i32" => Some(4),
        "i64" => Some(5),
        "u8" => Some(6),
        "u16" => Some(7),
        "u32" => Some(8),
        "u64" => Some(9),
        "f32" => Some(10),
        "f64" => Some(11),
        "String" => Some(12),
        _ => None,
    }
}
```

- [ ] **Step 2: Generate layout_hash() in the expand function**

In the `expand` function, after the existing `field_entries` vector, build a vector of hash steps:

```rust
let hash_steps: Vec<TokenStream> = fields.iter().map(|f| {
    let field_name = f.ident.as_ref().unwrap();
    let field_name_str = field_name.to_string();
    let name_bytes = field_name_str.as_bytes();
    let ty = &f.ty;

    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if let Some(disc) = type_discriminant(seg) {
                return quote! {
                    h = grw::layout::fnv_hash_bytes(h, &[#(#name_bytes),*]);
                    h = grw::layout::fnv_hash_byte(h, #disc);
                };
            }
        }
    }
    // Struct type — hash discriminant 13 + nested layout_hash
    quote! {
        h = grw::layout::fnv_hash_bytes(h, &[#(#name_bytes),*]);
        h = grw::layout::fnv_hash_byte(h, 13u8);
        h = grw::layout::fnv_hash_u64(h, <#ty as grw::layout::Val>::layout_hash());
    }
}).collect();
```

Then add the `layout_hash()` method to the generated impl:

```rust
Ok(quote! {
    impl grw::layout::Val for #name {
        fn fields() -> &'static [grw::layout::FieldInfo] {
            static FIELDS: std::sync::LazyLock<[grw::layout::FieldInfo; #field_count]> =
                std::sync::LazyLock::new(|| [
                    #(#field_entries),*
                ]);
            &*FIELDS
        }

        fn layout_hash() -> u64 {
            let mut h = grw::layout::FNV_OFFSET;
            h = grw::layout::fnv_hash_u64(h, #field_count as u64);
            #(#hash_steps)*
            h
        }

        fn size() -> usize {
            std::mem::size_of::<#name>()
        }

        fn align() -> usize {
            std::mem::align_of::<#name>()
        }
    }
})
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check --features persist`
Expected: PASS

- [ ] **Step 4: Add layout_hash tests to grw_derive**

In `grw_derive/tests/basic.rs`, add:

```rust
#[test]
fn layout_hash_deterministic() {
    let h1 = Simple::layout_hash();
    let h2 = Simple::layout_hash();
    assert_eq!(h1, h2);
    assert_ne!(h1, 0);
}

#[test]
fn layout_hash_differs_between_types() {
    assert_ne!(Simple::layout_hash(), Nested::layout_hash());
}

#[test]
fn layout_hash_primitives() {
    use grw::layout::Val;
    assert_ne!(<u32 as Val>::layout_hash(), <i32 as Val>::layout_hash());
    assert_ne!(<u32 as Val>::layout_hash(), <f64 as Val>::layout_hash());
    assert_eq!(<u32 as Val>::layout_hash(), <u32 as Val>::layout_hash());
}

#[test]
fn layout_hash_nested_includes_inner() {
    #[derive(Val)]
    struct Inner { x: f64 }

    #[derive(Val)]
    struct OuterA { inner: Inner }

    #[derive(Val)]
    struct OuterB { inner: Inner, extra: bool }

    assert_ne!(OuterA::layout_hash(), OuterB::layout_hash());
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p grw_derive`
Expected: all tests pass (existing 4 + new 4 = 8)

---

## Chunk 2: Persist v2 Header

### Task 3: Upgrade persist to v2 header with layout hashes

**Files:**
- Modify: `src/persist/mod.rs`

- [ ] **Step 1: Update constants and Header struct**

Replace the constants and Header at the top of `src/persist/mod.rs`:

```rust
use crate::graph;
use crate::layout;

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

const MAGIC: [u8; 4] = *b"GRW\0";
const CURRENT_VERSION: u16 = 2;
const V1_HEADER_SIZE: usize = 28;
const V2_HEADER_SIZE: usize = 44;

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
```

- [ ] **Step 2: Update save to write v2 header with Val bound**

Replace the `save` function:

```rust
pub fn save<NV, E>(graph: &graph::Graph<NV, E>, path: &Path) -> io::Result<()>
where
    NV: serde::Serialize + layout::Val,
    E: graph::Edge,
    E::Slot: serde::Serialize,
    E::Val: serde::Serialize + layout::Val,
{
    let nv_type = std::any::type_name::<NV>();
    let ev_type = std::any::type_name::<E::Val>();
    let nv_type_bytes = nv_type.as_bytes();
    let ev_type_bytes = ev_type.as_bytes();

    let mut file = File::create(path)?;
    file.write_all(&MAGIC)?;
    file.write_all(&CURRENT_VERSION.to_le_bytes())?;
    file.write_all(&[E::EDGE_KIND])?;
    file.write_all(&[0u8])?;
    file.write_all(&(graph.node_count() as u64).to_le_bytes())?;
    file.write_all(&(graph.edge_count() as u64).to_le_bytes())?;
    file.write_all(&NV::layout_hash().to_le_bytes())?;
    file.write_all(&<E::Val as layout::Val>::layout_hash().to_le_bytes())?;
    file.write_all(&(nv_type_bytes.len() as u16).to_le_bytes())?;
    file.write_all(&(ev_type_bytes.len() as u16).to_le_bytes())?;
    file.write_all(nv_type_bytes)?;
    file.write_all(ev_type_bytes)?;
    bincode::serialize_into(&mut file, graph)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}
```

- [ ] **Step 3: Update parse_header with v1/v2 branching**

Replace `parse_header`:

```rust
fn parse_header(data: &[u8]) -> io::Result<Header> {
    if data.len() < V1_HEADER_SIZE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file too short"));
    }
    if &data[..4] != &MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic"));
    }
    let version = u16::from_le_bytes([data[4], data[5]]);
    match version {
        1 => parse_v1(data),
        2 => parse_v2(data),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported version: {version}"),
        )),
    }
}

fn parse_v1(data: &[u8]) -> io::Result<Header> {
    let edge_kind = data[6];
    let node_count = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let edge_count = u64::from_le_bytes(data[16..24].try_into().unwrap());
    let nv_type_len = u16::from_le_bytes([data[24], data[25]]) as usize;
    let ev_type_len = u16::from_le_bytes([data[26], data[27]]) as usize;

    let required = V1_HEADER_SIZE + nv_type_len + ev_type_len;
    if data.len() < required {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file truncated in type strings"));
    }

    let nv_type = std::str::from_utf8(&data[V1_HEADER_SIZE..V1_HEADER_SIZE + nv_type_len])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        .to_owned();
    let ev_type = std::str::from_utf8(&data[V1_HEADER_SIZE + nv_type_len..V1_HEADER_SIZE + nv_type_len + ev_type_len])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        .to_owned();

    Ok(Header {
        version: 1,
        edge_kind,
        node_count,
        edge_count,
        nv_layout_hash: 0,
        ev_layout_hash: 0,
        nv_type,
        ev_type,
    })
}

fn parse_v2(data: &[u8]) -> io::Result<Header> {
    if data.len() < V2_HEADER_SIZE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file too short for v2 header"));
    }
    let edge_kind = data[6];
    let node_count = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let edge_count = u64::from_le_bytes(data[16..24].try_into().unwrap());
    let nv_layout_hash = u64::from_le_bytes(data[24..32].try_into().unwrap());
    let ev_layout_hash = u64::from_le_bytes(data[32..40].try_into().unwrap());
    let nv_type_len = u16::from_le_bytes([data[40], data[41]]) as usize;
    let ev_type_len = u16::from_le_bytes([data[42], data[43]]) as usize;

    let required = V2_HEADER_SIZE + nv_type_len + ev_type_len;
    if data.len() < required {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file truncated in type strings"));
    }

    let nv_type = std::str::from_utf8(&data[V2_HEADER_SIZE..V2_HEADER_SIZE + nv_type_len])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        .to_owned();
    let ev_type = std::str::from_utf8(&data[V2_HEADER_SIZE + nv_type_len..V2_HEADER_SIZE + nv_type_len + ev_type_len])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        .to_owned();

    Ok(Header {
        version: 2,
        edge_kind,
        node_count,
        edge_count,
        nv_layout_hash,
        ev_layout_hash,
        nv_type,
        ev_type,
    })
}
```

- [ ] **Step 4: Update load to use version-aware data offset**

Replace the `load` function:

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
    let fixed_size = match header.version {
        1 => V1_HEADER_SIZE,
        2 => V2_HEADER_SIZE,
        _ => unreachable!(),
    };
    let data_offset = fixed_size + header.nv_type.len() + header.ev_type.len();
    let data = &mmap[data_offset..];
    bincode::deserialize(data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
```

- [ ] **Step 5: Update read_header (no changes needed, already delegates to parse_header)**

No code change. `read_header` already calls `parse_header` which now handles both versions.

- [ ] **Step 6: Verify compilation**

Run: `cargo check --features persist`
Expected: PASS

- [ ] **Step 7: Update persist tests**

Update the tests. Key changes: `header.version` is now 2 for newly saved files, add layout hash assertions, add a test for loading v1 files.

In the tests, change `assert_eq!(header.version, 1)` to `assert_eq!(header.version, 2)` in `read_header_round_trip`.

Add new tests:

```rust
#[test]
fn read_header_has_layout_hashes() {
    use edge::undir::E::U;
    let g: graph::Undir<u32, u32> = (
        vec![(0, 10u32), (1, 20)],
        vec![(U(0, 1), 100u32)],
    )
        .try_into()
        .unwrap();
    let path = tmp_path("rt_layout_hashes.grw");
    save(&g, &path).unwrap();
    let header = read_header(&path).unwrap();
    assert_eq!(header.version, 2);
    assert_ne!(header.nv_layout_hash, 0);
    assert_ne!(header.ev_layout_hash, 0);
    assert_eq!(header.nv_layout_hash, <u32 as crate::layout::Val>::layout_hash());
    assert_eq!(header.ev_layout_hash, <u32 as crate::layout::Val>::layout_hash());
}

#[test]
fn read_header_unit_type_hashes() {
    use edge::dir::E::D;
    let g: graph::Dir0 = vec![D(0, 1)].try_into().unwrap();
    let path = tmp_path("rt_unit_hashes.grw");
    save(&g, &path).unwrap();
    let header = read_header(&path).unwrap();
    assert_eq!(header.nv_layout_hash, <() as crate::layout::Val>::layout_hash());
    assert_eq!(header.ev_layout_hash, <() as crate::layout::Val>::layout_hash());
}

#[test]
fn load_v1_file_still_works() {
    use edge::undir::E::U;
    let path = tmp_path("rt_v1_compat.grw");
    // Manually write a v1 header
    let nv_type = std::any::type_name::<()>().as_bytes();
    let ev_type = std::any::type_name::<()>().as_bytes();
    let g: graph::Undir0 = vec![U(0, 1)].try_into().unwrap();
    let bincode_data = bincode::serialize(&g).unwrap();

    let mut data = Vec::new();
    data.extend_from_slice(b"GRW\0");
    data.extend_from_slice(&1u16.to_le_bytes()); // version 1
    data.push(0); // edge_kind Undir
    data.push(0); // reserved
    data.extend_from_slice(&(g.node_count() as u64).to_le_bytes());
    data.extend_from_slice(&(g.edge_count() as u64).to_le_bytes());
    data.extend_from_slice(&(nv_type.len() as u16).to_le_bytes());
    data.extend_from_slice(&(ev_type.len() as u16).to_le_bytes());
    data.extend_from_slice(nv_type);
    data.extend_from_slice(ev_type);
    data.extend_from_slice(&bincode_data);
    std::fs::write(&path, &data).unwrap();

    let g2: graph::Undir0 = load(&path).unwrap();
    assert_eq!(g.node_count(), g2.node_count());
    assert_eq!(g.edge_count(), g2.edge_count());

    let header = read_header(&path).unwrap();
    assert_eq!(header.version, 1);
    assert_eq!(header.nv_layout_hash, 0);
    assert_eq!(header.ev_layout_hash, 0);
}
```

Also update `header_validation_bad_version` test — the fabricated data with version 99 should still fail:

```rust
#[test]
fn header_validation_bad_version() {
    let path = tmp_path("rt_bad_version.grw");
    let mut data = vec![0u8; 48];
    data[..4].copy_from_slice(b"GRW\0");
    data[4..6].copy_from_slice(&99u16.to_le_bytes());
    std::fs::write(&path, &data).unwrap();
    let Err(err) = load::<(), edge::Undir<()>>(&path) else {
        panic!("expected error");
    };
    let msg = err.to_string();
    assert!(msg.contains("version"), "expected version error, got: {msg}");
}
```

- [ ] **Step 8: Run persist tests**

Run: `cargo test --features persist persist`
Expected: all tests PASS (11 existing + 3 new = 14)

- [ ] **Step 9: Run full test suite**

Run: `cargo test --features persist`
Expected: all tests PASS, no regressions

---

## Chunk 3: Plugin System

### Task 4: Add dependencies to grw_repl

**Files:**
- Modify: `grw_repl/Cargo.toml`

- [ ] **Step 1: Add plugin system dependencies**

Add to `[dependencies]` in `grw_repl/Cargo.toml`:

```toml
libloading = "0.8"
sha2 = "0.10"
serde_json = "1"
hex = "0.4"
dirs = "6"
```

Also update the existing `grw` dependency to enable the `persist` feature:

```toml
grw = { path = "..", features = ["persist"] }
```

This is required because `load_or_compile` calls `grw::persist::read_header`.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p grw_repl`
Expected: PASS

---

### Task 5: ABI types module

**Files:**
- Create: `grw_repl/src/plugin/abi.rs`

- [ ] **Step 1: Create the ABI types**

```rust
use std::ffi::c_void;

#[repr(C)]
pub struct PluginResult {
    pub data_ptr: *mut u8,
    pub data_len: usize,
    pub error_ptr: *mut u8,
    pub error_len: usize,
}

impl PluginResult {
    pub fn into_string_result(self, free: unsafe extern "C" fn(PluginResult)) -> Result<String, String> {
        if !self.error_ptr.is_null() {
            let err = unsafe {
                String::from_utf8_lossy(std::slice::from_raw_parts(self.error_ptr, self.error_len))
                    .into_owned()
            };
            unsafe { free(self) };
            Err(err)
        } else if !self.data_ptr.is_null() {
            let data = unsafe {
                String::from_utf8_lossy(std::slice::from_raw_parts(self.data_ptr, self.data_len))
                    .into_owned()
            };
            unsafe { free(self) };
            Ok(data)
        } else {
            Ok(String::new())
        }
    }
}

pub type LoadFn = unsafe extern "C" fn(*const u8, usize) -> PluginResult;
pub type FreeFn = unsafe extern "C" fn(*mut c_void);
pub type FreeResultFn = unsafe extern "C" fn(PluginResult);
pub type CountFn = unsafe extern "C" fn(*const c_void) -> u64;
pub type EvalPredFn = unsafe extern "C" fn(*const c_void, *const u8, usize) -> PluginResult;
pub type InspectNodeFn = unsafe extern "C" fn(*const c_void, u64) -> PluginResult;
pub type NodeFieldsFn = unsafe extern "C" fn() -> PluginResult;
```

- [ ] **Step 2: Verify compilation**

Create `grw_repl/src/plugin/mod.rs` with just `pub mod abi;` and add `pub mod plugin;` to `grw_repl/src/lib.rs`.

Run: `cargo check -p grw_repl`
Expected: PASS

---

### Task 6: Cache module

**Files:**
- Create: `grw_repl/src/plugin/cache.rs`

- [ ] **Step 1: Create cache key computation and directory management**

```rust
use sha2::{Sha256, Digest};
use std::path::{Path, PathBuf};

pub struct CacheKey {
    pub nv_type: String,
    pub ev_type: String,
    pub edge_kind: u8,
    pub nv_layout_hash: u64,
    pub ev_layout_hash: u64,
    pub rustc_version: String,
    pub grw_version: String,
}

impl CacheKey {
    pub fn hash_hex(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.nv_type.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.ev_type.as_bytes());
        hasher.update(b"\0");
        hasher.update(&[self.edge_kind]);
        hasher.update(&self.nv_layout_hash.to_le_bytes());
        hasher.update(&self.ev_layout_hash.to_le_bytes());
        hasher.update(self.rustc_version.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.grw_version.as_bytes());
        let result = hasher.finalize();
        hex::encode(&result[..8])
    }

    pub fn to_json(&self) -> String {
        serde_json::json!({
            "nv_type": self.nv_type,
            "ev_type": self.ev_type,
            "edge_kind": self.edge_kind,
            "nv_layout_hash": self.nv_layout_hash,
            "ev_layout_hash": self.ev_layout_hash,
            "rustc_version": self.rustc_version,
            "grw_version": self.grw_version,
        }).to_string()
    }
}

pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("grw")
        .join("plugins")
}

pub fn plugin_path(key: &CacheKey) -> PathBuf {
    let hash = key.hash_hex();
    let dir = cache_dir().join(&hash);
    let ext = if cfg!(target_os = "macos") { "dylib" } else if cfg!(target_os = "windows") { "dll" } else { "so" };
    dir.join(format!("plugin.{ext}"))
}

pub fn meta_path(key: &CacheKey) -> PathBuf {
    let hash = key.hash_hex();
    cache_dir().join(&hash).join("meta.json")
}

pub fn rustc_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|_| "unknown".into())
}

pub fn find_types_crate(grw_path: &Path) -> Option<PathBuf> {
    let mut dir = grw_path.parent()?;
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            return Some(dir.to_owned());
        }
        dir = dir.parent()?;
    }
}
```

Note: `hex`, `dirs`, and `serde_json` were already added to `grw_repl/Cargo.toml` in Task 4.

- [ ] **Step 2: Update plugin/mod.rs to include cache module**

Add `pub mod cache;` to `grw_repl/src/plugin/mod.rs`.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p grw_repl`
Expected: PASS

- [ ] **Step 4: Add cache tests**

Add to `grw_repl/src/plugin/cache.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_deterministic() {
        let k1 = CacheKey {
            nv_type: "my_crate::Cargo".into(),
            ev_type: "u32".into(),
            edge_kind: 0,
            nv_layout_hash: 12345,
            ev_layout_hash: 67890,
            rustc_version: "rustc 1.85.0".into(),
            grw_version: "0.1.0".into(),
        };
        let k2 = CacheKey {
            nv_type: "my_crate::Cargo".into(),
            ev_type: "u32".into(),
            edge_kind: 0,
            nv_layout_hash: 12345,
            ev_layout_hash: 67890,
            rustc_version: "rustc 1.85.0".into(),
            grw_version: "0.1.0".into(),
        };
        assert_eq!(k1.hash_hex(), k2.hash_hex());
    }

    #[test]
    fn cache_key_changes_with_layout_hash() {
        let k1 = CacheKey {
            nv_type: "my_crate::Cargo".into(),
            ev_type: "u32".into(),
            edge_kind: 0,
            nv_layout_hash: 12345,
            ev_layout_hash: 67890,
            rustc_version: "rustc 1.85.0".into(),
            grw_version: "0.1.0".into(),
        };
        let k2 = CacheKey {
            nv_type: "my_crate::Cargo".into(),
            ev_type: "u32".into(),
            edge_kind: 0,
            nv_layout_hash: 99999,
            ev_layout_hash: 67890,
            rustc_version: "rustc 1.85.0".into(),
            grw_version: "0.1.0".into(),
        };
        assert_ne!(k1.hash_hex(), k2.hash_hex());
    }

    #[test]
    fn find_types_crate_walks_up() {
        let tmp = std::env::temp_dir().join("grw_cache_test");
        let nested = tmp.join("sub").join("deep");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(tmp.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        let fake_grw = nested.join("data.grw");
        std::fs::write(&fake_grw, b"").unwrap();
        let result = find_types_crate(&fake_grw);
        assert_eq!(result, Some(tmp.clone()));
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p grw_repl`
Expected: PASS

---

### Task 7: Codegen module

**Files:**
- Create: `grw_repl/src/plugin/codegen.rs`

- [ ] **Step 1: Create codegen module**

This module generates the temporary crate source. It needs the type names, edge kind, and crate path.

```rust
use std::path::Path;

pub struct PluginSpec {
    pub nv_type: String,
    pub ev_type: String,
    pub edge_kind: u8,
    pub types_crate_path: String,
    pub grw_repl_crate_path: String,
}

impl PluginSpec {
    pub fn cargo_toml(&self) -> String {
        let crate_name = self.types_crate_name();
        let repl_path = &self.grw_repl_crate_path;
        let grw_path = std::path::Path::new(&self.grw_repl_crate_path)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.types_crate_path.clone());
        let user_crate_dep = match crate_name {
            Some(name) => format!(
                r#"{name} = {{ path = "{path}" }}"#,
                name = name,
                path = self.types_crate_path,
            ),
            None => String::new(),
        };
        let mut deps = format!(
            r#"grw = {{ path = "{grw_path}", features = ["persist"] }}
grw_repl = {{ path = "{repl_path}" }}
serde_json = "1""#,
            grw_path = grw_path,
            repl_path = repl_path,
        );
        if !user_crate_dep.is_empty() {
            deps = format!("{user_crate_dep}\n{deps}");
        }
        format!(
            r#"[package]
name = "grw_plugin"
version = "0.0.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
{deps}
"#,
            deps = deps,
        )
    }

    pub fn lib_rs(&self) -> String {
        let edge_type = match self.edge_kind {
            0 => format!("grw::edge::Undir<{}>", self.ev_use_path()),
            1 => format!("grw::edge::Dir<{}>", self.ev_use_path()),
            2 => format!("grw::edge::Anydir<{}>", self.ev_use_path()),
            _ => panic!("invalid edge_kind: {}", self.edge_kind),
        };
        let nv_path = self.nv_use_path();

        format!(
            r#"use std::ffi::c_void;
use grw::layout::Val;

type NV = {nv_path};
type E = {edge_type};

#[repr(C)]
pub struct PluginResult {{
    pub data_ptr: *mut u8,
    pub data_len: usize,
    pub error_ptr: *mut u8,
    pub error_len: usize,
}}

fn ok_result(data: String) -> PluginResult {{
    let mut bytes = data.into_bytes();
    let ptr = bytes.as_mut_ptr();
    let len = bytes.len();
    std::mem::forget(bytes);
    PluginResult {{ data_ptr: ptr, data_len: len, error_ptr: std::ptr::null_mut(), error_len: 0 }}
}}

fn err_result(msg: String) -> PluginResult {{
    let mut bytes = msg.into_bytes();
    let ptr = bytes.as_mut_ptr();
    let len = bytes.len();
    std::mem::forget(bytes);
    PluginResult {{ data_ptr: std::ptr::null_mut(), data_len: 0, error_ptr: ptr, error_len: len }}
}}

#[unsafe(no_mangle)]
pub extern "C" fn grw_plugin_load(path_ptr: *const u8, path_len: usize) -> PluginResult {{
    let path_bytes = unsafe {{ std::slice::from_raw_parts(path_ptr, path_len) }};
    let path_str = match std::str::from_utf8(path_bytes) {{
        Ok(s) => s,
        Err(e) => return err_result(format!("invalid path: {{e}}")),
    }};
    let path = std::path::Path::new(path_str);
    match grw::persist::load::<NV, E>(path) {{
        Ok(graph) => {{
            let boxed = Box::new(graph);
            let ptr = Box::into_raw(boxed) as *mut u8;
            PluginResult {{ data_ptr: ptr, data_len: 0, error_ptr: std::ptr::null_mut(), error_len: 0 }}
        }}
        Err(e) => err_result(format!("{{e}}")),
    }}
}}

#[unsafe(no_mangle)]
pub extern "C" fn grw_plugin_free(graph: *mut c_void) {{
    if !graph.is_null() {{
        unsafe {{ drop(Box::from_raw(graph as *mut grw::graph::Graph<NV, E>)) }};
    }}
}}

#[unsafe(no_mangle)]
pub extern "C" fn grw_plugin_free_result(result: PluginResult) {{
    if !result.data_ptr.is_null() {{
        unsafe {{ drop(Vec::from_raw_parts(result.data_ptr, result.data_len, result.data_len)) }};
    }}
    if !result.error_ptr.is_null() {{
        unsafe {{ drop(Vec::from_raw_parts(result.error_ptr, result.error_len, result.error_len)) }};
    }}
}}

#[unsafe(no_mangle)]
pub extern "C" fn grw_plugin_node_count(graph: *const c_void) -> u64 {{
    let g = unsafe {{ &*(graph as *const grw::graph::Graph<NV, E>) }};
    g.node_count() as u64
}}

#[unsafe(no_mangle)]
pub extern "C" fn grw_plugin_edge_count(graph: *const c_void) -> u64 {{
    let g = unsafe {{ &*(graph as *const grw::graph::Graph<NV, E>) }};
    g.edge_count() as u64
}}

#[unsafe(no_mangle)]
pub extern "C" fn grw_plugin_eval_pred(graph: *const c_void, src_ptr: *const u8, src_len: usize) -> PluginResult {{
    let src = match std::str::from_utf8(unsafe {{ std::slice::from_raw_parts(src_ptr, src_len) }}) {{
        Ok(s) => s,
        Err(e) => return err_result(format!("invalid predicate: {{e}}")),
    }};
    let pred = match grw_repl::compile_predicate::<NV>(src) {{
        Ok(p) => p,
        Err(e) => return err_result(format!("{{e}}")),
    }};
    let g = unsafe {{ &*(graph as *const grw::graph::Graph<NV, E>) }};
    let matching: Vec<u64> = (0..g.node_count() as u64)
        .filter(|&i| {{
            if let Some(nv) = g.get(grw::id::N(i as u32)) {{
                pred(nv)
            }} else {{
                false
            }}
        }})
        .collect();
    ok_result(serde_json::to_string(&matching).unwrap())
}}

#[unsafe(no_mangle)]
pub extern "C" fn grw_plugin_inspect_node(graph: *const c_void, node_idx: u64) -> PluginResult {{
    let g = unsafe {{ &*(graph as *const grw::graph::Graph<NV, E>) }};
    match g.get(grw::id::N(node_idx as u32)) {{
        Some(nv) => {{
            let fields = NV::fields();
            let ptr = nv as *const NV as *const u8;
            let mut map = serde_json::Map::new();
            for field in fields {{
                let val = grw_repl::interp::read_field_value(fields, ptr, field.name);
                map.insert(field.name.to_string(), serde_json::Value::String(format!("{{val:?}}")));
            }}
            ok_result(serde_json::to_string(&map).unwrap())
        }}
        None => err_result(format!("node {{node_idx}} not found")),
    }}
}}

#[unsafe(no_mangle)]
pub extern "C" fn grw_plugin_node_fields() -> PluginResult {{
    let fields = NV::fields();
    let info: Vec<serde_json::Value> = fields.iter().map(|f| {{
        serde_json::json!({{ "name": f.name, "type": format!("{{:?}}", f.ty) }})
    }}).collect();
    ok_result(serde_json::to_string(&info).unwrap())
}}
"#,
            nv_path = nv_path,
            edge_type = edge_type,
        )
    }

    fn types_crate_name(&self) -> Option<String> {
        let first_seg = |ty: &str| -> Option<String> {
            if !ty.contains("::") { return None; }
            let seg = ty.split("::").next()?;
            match seg {
                "std" | "core" | "alloc" => None,
                _ => Some(seg.to_string()),
            }
        };
        first_seg(&self.nv_type).or_else(|| first_seg(&self.ev_type))
    }

    fn nv_use_path(&self) -> String {
        self.resolve_type_path(&self.nv_type)
    }

    fn ev_use_path(&self) -> String {
        self.resolve_type_path(&self.ev_type)
    }

    fn resolve_type_path(&self, ty: &str) -> String {
        if !ty.contains("::") {
            return ty.to_string();
        }
        let first = ty.split("::").next().unwrap();
        match first {
            "alloc" => ty.replace("alloc::string::String", "String"),
            "std" | "core" => ty.to_string(),
            _ => ty.to_string(),
        }
    }
}
```

- [ ] **Step 2: Add codegen module to plugin/mod.rs**

Add `pub mod codegen;` to `grw_repl/src/plugin/mod.rs`.

- [ ] **Step 3: Add codegen tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_toml_with_user_crate() {
        let spec = PluginSpec {
            nv_type: "my_crate::Cargo".into(),
            ev_type: "u32".into(),
            edge_kind: 0,
            types_crate_path: "/home/user/my_project".into(),
            grw_repl_crate_path: "/home/user/grw/grw_repl".into(),
        };
        let toml = spec.cargo_toml();
        assert!(toml.contains(r#"my_crate = { path = "/home/user/my_project" }"#));
        assert!(toml.contains(r#"grw = { path = "/home/user/grw", features = ["persist"] }"#));
        assert!(toml.contains("grw_repl = { path ="));
        assert!(toml.contains("serde_json = \"1\""));
        assert!(toml.contains("cdylib"));
    }

    #[test]
    fn cargo_toml_primitives_only() {
        let spec = PluginSpec {
            nv_type: "u32".into(),
            ev_type: "()".into(),
            edge_kind: 1,
            types_crate_path: "/home/user/project".into(),
            grw_repl_crate_path: "/home/user/grw/grw_repl".into(),
        };
        let toml = spec.cargo_toml();
        assert!(toml.contains(r#"grw = { path = "/home/user/grw", features = ["persist"] }"#));
        assert!(!toml.contains("u32 = {"));
        assert!(toml.contains("grw_repl = { path ="));
        assert!(toml.contains("serde_json = \"1\""));
    }

    #[test]
    fn lib_rs_edge_kind_mapping() {
        let spec = PluginSpec {
            nv_type: "u32".into(),
            ev_type: "()".into(),
            edge_kind: 0,
            types_crate_path: "/tmp".into(),
            grw_repl_crate_path: "/tmp/grw_repl".into(),
        };
        let lib = spec.lib_rs();
        assert!(lib.contains("grw::edge::Undir<()>"));

        let spec_dir = PluginSpec {
            nv_type: "u32".into(),
            ev_type: "()".into(),
            edge_kind: 1,
            types_crate_path: "/tmp".into(),
            grw_repl_crate_path: "/tmp/grw_repl".into(),
        };
        assert!(spec_dir.lib_rs().contains("grw::edge::Dir<()>"));
    }

    #[test]
    fn resolve_alloc_string() {
        let spec = PluginSpec {
            nv_type: "alloc::string::String".into(),
            ev_type: "u32".into(),
            edge_kind: 0,
            types_crate_path: "/tmp".into(),
            grw_repl_crate_path: "/tmp/grw_repl".into(),
        };
        assert_eq!(spec.nv_use_path(), "String");
    }
}
```

Tests use explicit struct construction for each case — no Clone needed.

- [ ] **Step 4: Verify tests**

Run: `cargo test -p grw_repl`
Expected: PASS

---

### Task 8: Plugin loader and orchestrator

**Files:**
- Modify: `grw_repl/src/plugin/mod.rs`

- [ ] **Step 1: Implement Plugin struct and LoadedGraph**

In `grw_repl/src/plugin/mod.rs`:

```rust
pub mod abi;
pub mod cache;
pub mod codegen;

use std::ffi::c_void;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum PluginError {
    NoCrateFound,
    CompilationFailed(String),
    LoadFailed(String),
    PluginError(String),
    HeaderError(String),
    V1NotSupported,
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCrateFound => write!(f, "could not find types crate. Use --types-crate path/to/crate or run from within the project directory"),
            Self::CompilationFailed(msg) => write!(f, "plugin compilation failed:\n{msg}"),
            Self::LoadFailed(msg) => write!(f, "failed to load plugin: {msg}"),
            Self::PluginError(msg) => write!(f, "plugin error: {msg}"),
            Self::HeaderError(msg) => write!(f, "header error: {msg}"),
            Self::V1NotSupported => write!(f, "this graph was saved without layout metadata. Re-save with the current version to use the REPL"),
        }
    }
}

impl std::error::Error for PluginError {}

pub struct Plugin {
    _lib: libloading::Library,
    load_fn: abi::LoadFn,
    free_fn: abi::FreeFn,
    free_result_fn: abi::FreeResultFn,
    node_count_fn: abi::CountFn,
    edge_count_fn: abi::CountFn,
    eval_pred_fn: abi::EvalPredFn,
    inspect_node_fn: abi::InspectNodeFn,
    node_fields_fn: abi::NodeFieldsFn,
}

impl Plugin {
    pub fn open(path: &Path) -> Result<Self, PluginError> {
        let lib = unsafe { libloading::Library::new(path) }
            .map_err(|e| PluginError::LoadFailed(e.to_string()))?;

        unsafe {
            let load_fn = *lib.get::<abi::LoadFn>(b"grw_plugin_load")
                .map_err(|e| PluginError::LoadFailed(e.to_string()))?;
            let free_fn = *lib.get::<abi::FreeFn>(b"grw_plugin_free")
                .map_err(|e| PluginError::LoadFailed(e.to_string()))?;
            let free_result_fn = *lib.get::<abi::FreeResultFn>(b"grw_plugin_free_result")
                .map_err(|e| PluginError::LoadFailed(e.to_string()))?;
            let node_count_fn = *lib.get::<abi::CountFn>(b"grw_plugin_node_count")
                .map_err(|e| PluginError::LoadFailed(e.to_string()))?;
            let edge_count_fn = *lib.get::<abi::CountFn>(b"grw_plugin_edge_count")
                .map_err(|e| PluginError::LoadFailed(e.to_string()))?;
            let eval_pred_fn = *lib.get::<abi::EvalPredFn>(b"grw_plugin_eval_pred")
                .map_err(|e| PluginError::LoadFailed(e.to_string()))?;
            let inspect_node_fn = *lib.get::<abi::InspectNodeFn>(b"grw_plugin_inspect_node")
                .map_err(|e| PluginError::LoadFailed(e.to_string()))?;
            let node_fields_fn = *lib.get::<abi::NodeFieldsFn>(b"grw_plugin_node_fields")
                .map_err(|e| PluginError::LoadFailed(e.to_string()))?;

            Ok(Plugin {
                _lib: lib,
                load_fn,
                free_fn,
                free_result_fn,
                node_count_fn,
                edge_count_fn,
                eval_pred_fn,
                inspect_node_fn,
                node_fields_fn,
            })
        }
    }

    pub fn load_graph(&self, grw_path: &Path) -> Result<LoadedGraph, PluginError> {
        let path_str = grw_path.to_str()
            .ok_or_else(|| PluginError::LoadFailed("non-UTF8 path".into()))?;
        let result = unsafe { (self.load_fn)(path_str.as_ptr(), path_str.len()) };
        if !result.error_ptr.is_null() {
            let err = unsafe {
                String::from_utf8_lossy(std::slice::from_raw_parts(result.error_ptr, result.error_len))
                    .into_owned()
            };
            unsafe { (self.free_result_fn)(result) };
            Err(PluginError::PluginError(err))
        } else {
            Ok(LoadedGraph {
                handle: result.data_ptr as *mut c_void,
                plugin: self,
            })
        }
    }

    pub fn node_fields(&self) -> Result<String, PluginError> {
        let result = unsafe { (self.node_fields_fn)() };
        result.into_string_result(self.free_result_fn)
            .map_err(PluginError::PluginError)
    }
}

pub struct LoadedGraph<'a> {
    handle: *mut c_void,
    plugin: &'a Plugin,
}

impl<'a> LoadedGraph<'a> {
    pub fn node_count(&self) -> u64 {
        unsafe { (self.plugin.node_count_fn)(self.handle) }
    }

    pub fn edge_count(&self) -> u64 {
        unsafe { (self.plugin.edge_count_fn)(self.handle) }
    }

    pub fn eval_pred(&self, src: &str) -> Result<String, PluginError> {
        let result = unsafe { (self.plugin.eval_pred_fn)(self.handle, src.as_ptr(), src.len()) };
        result.into_string_result(self.plugin.free_result_fn)
            .map_err(PluginError::PluginError)
    }

    pub fn inspect_node(&self, idx: u64) -> Result<String, PluginError> {
        let result = unsafe { (self.plugin.inspect_node_fn)(self.handle, idx) };
        result.into_string_result(self.plugin.free_result_fn)
            .map_err(PluginError::PluginError)
    }
}

impl Drop for LoadedGraph<'_> {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { (self.plugin.free_fn)(self.handle) };
        }
    }
}
```

- [ ] **Step 2: Add load_or_compile orchestrator**

Add to `grw_repl/src/plugin/mod.rs`:

```rust
pub fn load_or_compile(grw_path: &Path, types_crate: Option<&Path>) -> Result<Plugin, PluginError> {
    let header = grw::persist::read_header(grw_path)
        .map_err(|e| PluginError::HeaderError(e.to_string()))?;

    if header.version < 2 {
        return Err(PluginError::V1NotSupported);
    }

    let types_crate_path = match types_crate {
        Some(p) => p.to_owned(),
        None => cache::find_types_crate(grw_path)
            .ok_or(PluginError::NoCrateFound)?,
    };

    let key = cache::CacheKey {
        nv_type: header.nv_type.clone(),
        ev_type: header.ev_type.clone(),
        edge_kind: header.edge_kind,
        nv_layout_hash: header.nv_layout_hash,
        ev_layout_hash: header.ev_layout_hash,
        rustc_version: cache::rustc_version(),
        grw_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let plugin_so = cache::plugin_path(&key);

    if plugin_so.exists() {
        return Plugin::open(&plugin_so);
    }

    eprintln!("Compiling plugin for {}...", header.nv_type);
    let start = std::time::Instant::now();

    let grw_repl_crate_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let spec = codegen::PluginSpec {
        nv_type: header.nv_type,
        ev_type: header.ev_type,
        edge_kind: header.edge_kind,
        types_crate_path: types_crate_path.to_string_lossy().into_owned(),
        grw_repl_crate_path: grw_repl_crate_path.to_string_lossy().into_owned(),
    };

    let tmp_dir = std::env::temp_dir().join(format!("grw_plugin_{}", key.hash_hex()));
    std::fs::create_dir_all(tmp_dir.join("src")).map_err(|e| PluginError::CompilationFailed(e.to_string()))?;
    std::fs::write(tmp_dir.join("Cargo.toml"), spec.cargo_toml())
        .map_err(|e| PluginError::CompilationFailed(e.to_string()))?;
    std::fs::write(tmp_dir.join("src/lib.rs"), spec.lib_rs())
        .map_err(|e| PluginError::CompilationFailed(e.to_string()))?;

    let output = std::process::Command::new("cargo")
        .arg("build")
        .arg("--release")
        .current_dir(&tmp_dir)
        .output()
        .map_err(|e| PluginError::CompilationFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(PluginError::CompilationFailed(stderr.into_owned()));
    }

    let ext = if cfg!(target_os = "macos") { "dylib" } else if cfg!(target_os = "windows") { "dll" } else { "so" };
    let built_lib = tmp_dir.join("target/release").join(format!("libgrw_plugin.{ext}"));

    let cache_entry = plugin_so.parent().unwrap();
    std::fs::create_dir_all(cache_entry).map_err(|e| PluginError::CompilationFailed(e.to_string()))?;
    std::fs::copy(&built_lib, &plugin_so).map_err(|e| PluginError::CompilationFailed(e.to_string()))?;
    std::fs::write(cache::meta_path(&key), key.to_json())
        .map_err(|e| PluginError::CompilationFailed(e.to_string()))?;

    let _ = std::fs::remove_dir_all(&tmp_dir);

    let elapsed = start.elapsed();
    eprintln!("done ({:.1}s)", elapsed.as_secs_f64());

    Plugin::open(&plugin_so)
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p grw_repl`
Expected: PASS

---

### Task 9: Add interp helper for node inspection

**Files:**
- Modify: `grw_repl/src/interp.rs`

The generated plugin code calls `grw_repl::interp::read_field_value` for node inspection. We need to expose a public helper. The existing `read_value` function is private, so make it `pub` and add a convenience wrapper.

- [ ] **Step 1: Make `read_value` public and add `read_field_value` helper**

In `grw_repl/src/interp.rs`, change `fn read_value` to `pub fn read_value`.

Then add the convenience function:

```rust
pub fn read_field_value(layout: &[FieldInfo], ptr: *const u8, field_name: &str) -> Value {
    let field = layout.iter()
        .find(|f| f.name == field_name)
        .unwrap_or_else(|| panic!("field `{field_name}` not found in layout"));
    read_value(field, ptr)
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p grw_repl`
Expected: PASS

---

### Task 10: Final verification

- [ ] **Step 1: Run all grw tests**

Run: `cargo test --features persist`
Expected: all tests PASS

- [ ] **Step 2: Run grw_derive tests**

Run: `cargo test -p grw_derive`
Expected: all tests PASS

- [ ] **Step 3: Run grw_repl tests**

Run: `cargo test -p grw_repl`
Expected: all tests PASS

- [ ] **Step 4: Check crosscheck compilation**

Run: `cargo check --features crosscheck`
Expected: PASS

---

## Post-Completion

The plugin system is now structurally complete. The full end-to-end flow (save .grw file → load_or_compile → eval predicates) can be manually tested by:

1. Saving a graph with valued types from a project that derives `Val`
2. Running `load_or_compile` pointing at the .grw file
3. Calling `eval_pred` and `inspect_node` on the loaded graph

Automated integration testing requires a fixture crate with cargo and rustc available — this is best done as a follow-up after the REPL CLI phase, which will exercise the full flow naturally.
