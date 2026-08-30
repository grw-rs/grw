# GRW — Graph Rewriting in Rust

GRW is an embedded graph rewriting system that runs inside a Rust process. It provides type-safe domain-specific languages for graph construction, transactional mutation, and morphism-based pattern matching.

## The Three DSLs

- **`mgraph!`** — graph literal (like `vec!`) for constructing graphs declaratively
- **`modify!`** — transactional graph mutation: add/remove/change nodes and edges atomically
- **`search!`** — graph pattern matching iterator with morphism control

All DSL fragments are plain Rust structs — they can be constructed, composed, and manipulated programmatically before being passed to the macros or the underlying `from_fragment()` / `modify()` / `compile()` functions directly.

These DSLs are built with `macro_rules!` — no procedural macros — by overloading Rust operators (`^`, `>>`, `<<`, `&`, `!`) to express graph edge semantics.

## Quick Example

```rust
use grw::*;
use grw::graph::edge;

// construct a triangle
let g: graph::MUndir0 = mgraph![
    N(0) ^ (N(1) ^ (N(2) ^ n(0)))
].unwrap();

// search for edges (Mono morphism)
let session = search![&g,
    get(Mono) { N(0) ^ N(1) }
].unwrap();

for m in &session {
    let a = m.get(0).unwrap();
    let b = m.get(1).unwrap();
    println!("{:?} — {:?}", a, b);
}
```

## Links

- [GitHub](https://github.com/grw-rs/grw)
- [Codeberg](https://codeberg.org/grw-rs/grw)
