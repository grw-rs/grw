# grw_derive

Procedural macros for the [GRW](https://grw.rs) graph rewriting library.

This crate provides the macros grw uses for runtime type introspection, composite link values, REPL integration and pattern literals. It is not intended to be used directly — it is re-exported through the `grw` crate.

## `#[derive(Val)]`

Derives the `Val` trait on structs and `#[repr(u8)]` enums, providing:

- **Field introspection** — `fields()` returns field names, types, and byte offsets at runtime
- **Layout hash** — deterministic `layout_hash()` for binary compatibility checks (used by `Graph::save`/`load`)
- **Size/alignment** — `size()` and `align()` matching `std::mem`

```rust
use grw::Val;

#[derive(Val)]
struct Particle {
    x: f64,
    y: f64,
    mass: f32,
    active: bool,
}

// runtime field access
let fields = Particle::fields();
assert_eq!(fields[0].name, "x");
assert_eq!(fields.len(), 4);

// layout hash for persistence validation
let hash = Particle::layout_hash();
assert_ne!(hash, 0);
```

Supported field types: `bool`, `i8`–`i64`, `u8`–`u64`, `f32`, `f64`, `String`, nested `Val` structs, `#[repr(u8)]` enums.

### Enums

```rust
#[derive(Val, Clone, Copy)]
#[repr(u8)]
enum Color { Red, Green, Blue }

// variants with discriminants are introspectable
let ft = Color::field_type();
```

## `#[derive(Part)]`

Declares the parts a composite link value may hold. Each variant of the enum is one part kind and carries exactly one payload type:

```rust
use grw::composite::{Part, TypeSet};

#[derive(Debug, Clone, PartialEq)]
struct Signs { since: u32 }

#[derive(Debug, Clone, PartialEq)]
struct Uses { service: u32 }

#[derive(Debug, Clone, PartialEq, Part)]
enum Rel {
    Signs(Signs),
    Uses(Uses),
}

type Link = TypeSet<Rel>;
```

The derive emits:

- **A kind enum** — `RelKind { Signs, Uses }`, fieldless, `Copy + Ord`, one variant per part variant
- **`Kinded`** — `Rel::kind()` returns the variant's `RelKind`
- **`From<Payload>`** — `Rel::from(Signs { .. })`, so a typed payload is accepted wherever a part is
- **`PartOf<Payload>`** — the static kind of each payload type and its projection out of the enum, which gives every composite over `Rel` the typed `get::<Signs>()`, `include(..)`, `exclude::<Signs>()` and `has::<Signs>()`

It refuses at compile time:

- a struct or union
- a generic enum
- a variant that is not a single-field tuple variant (`Signs(Signs)`)
- two variants holding the same payload type

## `#[grw::repl]`

Attribute macro for `impl` blocks. Generates `extern "C"` wrappers and method metadata for REPL/JIT integration:

```rust
#[derive(Val)]
struct Point { x: f64, y: f64 }

#[grw::repl]
impl Point {
    fn distance(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }
}

// methods are discoverable at runtime
let methods = Point::methods();
assert_eq!(methods[0].name, "distance");
assert!(!methods[0].is_static);

assert_eq!(methods[1].name, "new");
assert!(methods[1].is_static);
```

Supports:
- `&self` methods returning scalars (`bool`, `i8`–`i64`, `u8`–`u64`, `f32`, `f64`)
- Static methods (constructors, factories)
- Multi-parameter methods
- `String` return type (metadata only, fn_ptr is null — not callable via FFI)
- `&mut self` methods are silently skipped

## `pattern!` and `search!`

Two more proc macros, `pattern!` and `search!`, parse graph pattern literals (symbolic node ids, value patterns, `with` pins) at compile time into the matcher structures `grw` runs against. The token grammar they share is not implemented here — it lives in the [`grw_pattern`](../pattern/) crate, which this crate depends on and re-expands into generated code.

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.
