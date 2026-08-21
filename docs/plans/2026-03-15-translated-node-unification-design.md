# Translated Node Unification Design

## Problem

The search and modify DSLs handle exist/context nodes (`X`) inconsistently:

- **Modify `X(id)`**: takes `id::N` (literal graph node ID), applied directly
- **Search `X(id)`**: takes `LocalId` (pattern-local), requires a separate `resolve_bindings` translation table

This prevents pattern reuse in modify and creates a confusing API asymmetry.

## Solution

Unify both DSLs with two distinct constructors:

- **`X(id)`** — literal graph node ID (`id::N`), no translation table needed
- **`T(id)`** — translated node (`LocalId`), requires a binding table to resolve

Both search and modify get the same constructors with identical semantics.

## Node Constructors

| Constructor | ID type    | Meaning                          | DSLs          |
|-------------|------------|----------------------------------|---------------|
| `N(id)`     | `LocalId`  | New node, pattern-local ID       | search, modify |
| `N_()`      | (anon)     | New node, anonymous              | search, modify |
| `n(id)`     | `LocalId`  | Reference to N defined earlier   | search, modify |
| `X(id)`     | `id::N`    | Existing graph node, literal ID  | search, modify |
| `x(id)`     | `id::N`    | Reference to X defined earlier   | search, modify |
| `T(id)`     | `LocalId`  | Translated node, requires bind   | search, modify |
| `t(id)`     | `LocalId`  | Reference to T defined earlier   | search, modify |

`x(id)` references an X node by its literal graph ID (same `id::N` type), since X nodes are keyed by their graph node ID, not a pattern-local ID.

`t(id)` references a T node by its `LocalId`. References to undefined T nodes are rejected by validation (same mechanism as `n(id)` referencing undefined `N(id)`).

All constructors support `.val()`, `.test()`, `!` (negation) where applicable. Negated translated nodes (`!T(id)`) produce a `NegTranslated` type mirroring the existing `NegContext` pattern.

## Return Types

Both DSLs return a two-variant enum based on whether T nodes are present. The enums are lifetime-free — they own their data.

### Search

```rust
pub enum Search<NV, ER: Edge> {
    Resolved(search::Resolved<NV, ER>),
    Unresolved(search::Unresolved<NV, ER>),
}
```

### Modify

```rust
pub enum Modify<NV, ER: Edge> {
    Resolved(modify::Resolved<NV, ER>),
    Unresolved(modify::Unresolved<NV, ER>),
}
```

## Owned Types

Both `Resolved` and `Unresolved` own their query/modification data. No lifetime parameter on either.

```rust
// search module
pub struct Resolved<NV, ER: Edge> {
    pub(crate) query: Query<NV, ER>,
    pub(crate) bindings: Vec<Option<id::N>>,
}

pub struct Unresolved<NV, ER: Edge> {
    pub(crate) query: Query<NV, ER>,
    pub(crate) translated_indices: Vec<usize>,
    pub(crate) base_bindings: Vec<Option<id::N>>,
}
```

`Resolved` holds pre-filled bindings: `Some(id::N)` for Exist nodes, `None` for New nodes.

`Unresolved` holds the same, plus `translated_indices` tracking which pattern node positions need `.bind()` resolution.

## Bind API

`.bind()` takes `&self` (shared borrow) and returns a `Bound` view that borrows the inner query.

```rust
pub struct Bound<'a, NV, ER: Edge> {
    pub(crate) query: &'a Query<NV, ER>,
    pub(crate) bindings: Vec<Option<id::N>>,
}

impl<NV, ER: Edge> search::Unresolved<NV, ER> {
    pub fn bind(&self, bindings: &[(Id, Id)]) -> Result<search::Bound<'_, NV, ER>, BindError> {
        // clone base_bindings, fill T slots from table
    }
}

impl<NV, ER: Edge> modify::Unresolved<NV, ER> {
    pub fn bind(&self, bindings: &[(Id, Id)]) -> Result<modify::Bound<'_, NV, ER>, BindError> {
        // same
    }
}
```

Multiple `Bound` instances can coexist borrowing the same `Unresolved`:

```rust
let Search::Unresolved(pattern) = search![...] else { ... };
let r1 = pattern.bind(&[(0, 5)])?;
let r2 = pattern.bind(&[(0, 99)])?;
// r1 and r2 both alive, both borrowing pattern
```

`.bind()` does NOT take a graph reference. It is a pure pattern-level operation that turns T→X. Graph existence validation happens at search/apply time.

### Execution

Both `Resolved` (owned) and `Bound` (borrowed) can be executed. The engine accepts either via a trait or by having both types expose the same `(&Query, &[Option<id::N>])` pair.

`search_bound()` takes `&Resolved` or `&Bound` by reference — it does not consume the bindings.

### Bind validation

- All T nodes must have an entry in the table
- Extra entries (IDs not matching any T node) are rejected with `BindError::NotFound`
- No duplicate mappings for the same T node
- Injectivity constraints between T nodes AND between T and X nodes (if morphism requires it)

### Bind error type

```rust
pub enum BindError {
    NotFound(Id),
    Duplicate(Id),
    Missing(Vec<Id>),
    Collision { n1: Id, n2: Id, target: Id },
}
```

`Collision` covers both T-vs-T and T-vs-X conflicts. `n1`/`n2` are the local IDs of the conflicting nodes (either T or X).

### Graph existence validation

Graph node existence is NOT checked by `.bind()`. It is checked at search/apply time by the engine, which validates that all `Some(id::N)` entries in the bindings vector refer to nodes present in the target graph. This preserves the decoupling between binding and execution.

## Internal Representation

### NodeKind

```rust
pub(crate) enum NodeKind {
    New,          // N, N_, n
    Exist,        // X, x — literal graph ID, pre-filled in bindings
    Translated,   // T, t — needs .bind() resolution
}
```

### Compile-time behavior

- `N(0)` → `NodeKind::New`, bindings entry = `None`
- `X(5)` → `NodeKind::Exist`, bindings entry = `Some(id::N(5))` immediately
- `T(0)` → `NodeKind::Translated`, bindings entry = `None`, index tracked in `translated_indices`

### Variant selection

```rust
if translated_indices.is_empty() {
    Search::Resolved(Resolved {
        query,
        bindings,  // X nodes pre-filled, New nodes None
    })
} else {
    Search::Unresolved(Unresolved {
        query,
        translated_indices,
        base_bindings: bindings,  // X nodes pre-filled
    })
}
```

### Mixed patterns

A pattern can mix all three: `X(5) >> T(0) >> N(1)`:
- X(5) is pre-resolved in bindings at compile time
- T(0) needs `.bind()` to fill its slot
- N(1) is free for search to explore

## REPL Integration

### Syntax

```
search!(g, get(Mono) { X(5) >> N(1) })
search!(g, get(Mono) { T(0) >> N(1) }, [(0, 5)])
modify!(g, [X(5) >> N(1)])
modify!(g, [T(0) >> N(1)], [(0, 5)])
```

Translation table `[(id, id), ...]` parsed after the pattern body.

### Structural parser

`search_parse.rs` adds `T`/`t` as node types. JSON includes `"translated": true` on T nodes. Plugin receives bindings array alongside pattern.

### Compiled Rust path

Generated code matches on `Resolved`/`Unresolved` and calls `.bind()` when translation table is present.

## Breaking Changes

### Search DSL

- `X(id)` changes from `Into<LocalId>` → `Into<id::N>`
- `x(id)` changes from `Into<LocalId>` → `Into<id::N>`
- `Search::Free` → `Search::Resolved`
- `Search::Bound` → `Search::Unresolved`
- `BoundQuery` removed, replaced by `Unresolved`
- `resolve_bindings` replaced by `.bind()` on `Unresolved`
- `search_bound()` takes `&Resolved` or `&Bound` instead of consuming `(plan, bindings)` separately

### Modify DSL

- Non-breaking addition: `T`/`t` constructors
- `Modify` enum is new (currently `modify!` returns `Result` directly)

### Migration

All existing search code using `X` with `LocalId` semantics must convert to `T` (if translation table needed) or literal `X` (if graph node ID is known).

## Files Affected

- `src/search/dsl/mod.rs` — X/x/T/t constructors
- `src/search/dsl/node.rs` — node types, Op enum, NegTranslated
- `src/search/query/compile.rs` — compilation, variant selection
- `src/search/query/mod.rs` — Search enum, Resolved/Unresolved/Bound, BindError
- `src/search/engine/seq.rs` — takes &Resolved or &Bound
- `src/search/engine/par.rs` — takes &Resolved or &Bound
- `src/search/engine/mod.rs` — State with bindings
- `src/search/mod.rs` — macro, tests
- `src/modify/dsl/mod.rs` — T/t constructors
- `src/modify/dsl/node.rs` — translated node types, NegTranslated
- `src/modify/apply.rs` — Resolved/Unresolved/Bound, .bind()
- `src/modify/dsl/validate.rs` — validation for T nodes
- All scratch/test files with context nodes
- `grw_repl/src/search_parse.rs` — T node support
- `grw_repl/src/plugin/codegen.rs` — generated code for bind
