# Translated Node Unification Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify search and modify DSLs so both use `X(id::N)` for literal graph node references and `T(LocalId)` for translated nodes requiring a binding table.

**Architecture:** Add `T`/`t` constructors and `Translated`/`NegTranslated` node types to both DSLs. Change search `X`/`x` from `LocalId` to `id::N`. Replace `Search::Free`/`Bound` with `Resolved`/`Unresolved`. Add `Modify::Resolved`/`Unresolved`. `.bind()` on `Unresolved` borrows the inner query and returns a `Bound` view with filled bindings.

**Tech Stack:** Rust, typestate pattern, grw search/modify DSL macros

**Spec:** `docs/plans/2026-03-15-translated-node-unification-design.md`

---

### Task 1: Add NodeKind::Translated and rename NodeKind::Context → Exist

**Files:**
- Modify: `src/search/query/mod.rs:10-14` (NodeKind enum)
- Modify: `src/search/query/compile.rs:10-14` (NodeKind enum, duplicated)
- Modify: `src/search/query/compile.rs:506-512` (context_indices collection)

- [ ] **Step 1: Update NodeKind enum in `src/search/query/mod.rs`**

Change:
```rust
pub(crate) enum NodeKind {
    Free,
    Context,
}
```
To:
```rust
pub(crate) enum NodeKind {
    New,
    Exist,
    Translated,
}
```

- [ ] **Step 2: Update NodeKind in `src/search/query/compile.rs`**

Same rename. Also update all references from `NodeKind::Free` → `NodeKind::New` and `NodeKind::Context` → `NodeKind::Exist` throughout compile.rs.

- [ ] **Step 3: Rename context_indices → exist_indices, add translated_indices**

In compile.rs, update the collection (around line 506):
```rust
let exist_indices: Vec<usize> = flat.nodes.iter().enumerate()
    .filter(|(_, n)| n.kind == NodeKind::Exist)
    .map(|(i, _)| i).collect();
let translated_indices: Vec<usize> = flat.nodes.iter().enumerate()
    .filter(|(_, n)| n.kind == NodeKind::Translated)
    .map(|(i, _)| i).collect();
```

Update the `Query` struct field: `context_indices` → split into `exist_indices` and `translated_indices`.

- [ ] **Step 4: Update all references to context_indices throughout query/engine**

Grep for `context_indices` and update:
- `src/search/query/mod.rs` — Query struct field, resolve_bindings, has_context_nodes
- `src/search/engine/mod.rs` — State::new
- `src/search/engine/seq.rs` — any references
- `src/search/engine/par.rs` — any references

- [ ] **Step 5: Run `cargo check` to verify compilation**

Run: `cargo check`
Expected: compiles with possible warnings about unused translated_indices

- [ ] **Step 6: Commit**

```
rename NodeKind::Context→Exist, add Translated, split context_indices
```

---

### Task 2: Add T/t constructors and Translated node types to search DSL

**Files:**
- Modify: `src/search/dsl/mod.rs:28-50` (Op enum), `src/search/dsl/mod.rs:117-132` (X/x constructors)
- Modify: `src/search/dsl/node.rs:20-45` (Context/NegContext types)

- [ ] **Step 1: Add Translated/TranslatedRef/NegTranslated to Op enum**

In `src/search/dsl/mod.rs`, add variants to Op:
```rust
Translated {
    id: LocalId,
    node_pred: Option<Box<dyn Fn(&NV) -> bool + Send + Sync>>,
    negated: bool,
    edges: Vec<EdgeOp<NV, ER>>,
},
TranslatedRef {
    id: LocalId,
    edges: Vec<EdgeOp<NV, ER>>,
},
```

- [ ] **Step 2: Add Translated, TranslatedRef, NegTranslated structs to node.rs**

In `src/search/dsl/node.rs`, add after the Context/NegContext structs:
```rust
pub struct Translated<NV, V, ER: graph::Edge> {
    pub(crate) id: LocalId,
    pub(crate) v: V,
    pub(crate) pred: Option<Box<dyn Fn(&NV) -> bool + Send + Sync>>,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

pub struct TranslatedRef<NV, ER: graph::Edge> {
    pub(crate) id: LocalId,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

pub struct NegTranslated<NV, V, ER: graph::Edge> {
    pub(crate) id: LocalId,
    pub(crate) v: V,
    pub(crate) pred: Option<Box<dyn Fn(&NV) -> bool + Send + Sync>>,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}
```

- [ ] **Step 3: Implement .val(), .test(), !, and operator impls for Translated types**

Mirror the existing Context implementations: `.val()` typestate, `.test()` typestate, `Not` for negation, all edge operators (`>>`, `<<`, `^`, `%`).

- [ ] **Step 4: Add From impls converting Translated types to Op**

```rust
impl<NV, V: IntoOptional<NV>, ER: graph::Edge> From<Translated<NV, V, ER>> for Op<NV, ER> {
    fn from(n: Translated<NV, V, ER>) -> Self {
        Op::Translated { id: n.id, node_pred: n.pred, negated: false, edges: n.edges }
    }
}
// ... TranslatedRef, NegTranslated similarly
```

- [ ] **Step 5: Add T() and t() constructor functions**

In `src/search/dsl/mod.rs`:
```rust
#[allow(non_snake_case)]
pub fn T<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::Translated<NV, (), ER> {
    node::Translated { id: local.into(), v: (), pred: None, edges: Vec::new() }
}

#[allow(non_snake_case)]
pub fn t<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::TranslatedRef<NV, ER> {
    node::TranslatedRef { id: local.into(), edges: Vec::new() }
}
```

- [ ] **Step 6: Run `cargo check`**

Run: `cargo check`
Expected: compiles (new types exist but aren't used in compilation yet)

- [ ] **Step 7: Commit**

```
add T/t constructors and Translated node types to search DSL
```

---

### Task 3: Change search X/x from LocalId to id::N

**Files:**
- Modify: `src/search/dsl/mod.rs:117-132` (X/x constructors)
- Modify: `src/search/dsl/node.rs:20-30` (Context/ContextRef structs)
- Modify: `src/search/dsl/node.rs` (From impls for Context)
- Modify: `src/search/dsl/mod.rs:28-50` (Op enum Context variants)

- [ ] **Step 1: Change Context/ContextRef id field from LocalId to id::N**

In `src/search/dsl/node.rs`:
```rust
pub struct Context<NV, V, ER: graph::Edge> {
    pub(crate) id: id::N,  // was LocalId
    // ...
}
pub struct ContextRef<NV, ER: graph::Edge> {
    pub(crate) id: id::N,  // was LocalId
    // ...
}
pub struct NegContext<NV, V, ER: graph::Edge> {
    pub(crate) id: id::N,  // was LocalId
    // ...
}
```

- [ ] **Step 2: Change Op::Context/ContextRef id field from LocalId to id::N**

In `src/search/dsl/mod.rs` Op enum:
```rust
Context {
    id: id::N,  // was LocalId
    // ...
},
ContextRef {
    id: id::N,  // was LocalId
    // ...
},
```

- [ ] **Step 3: Change X/x constructors from Into<LocalId> to Into<id::N>**

In `src/search/dsl/mod.rs`:
```rust
pub fn X<NV, ER: graph::Edge>(id: impl Into<id::N>) -> node::Context<NV, (), ER> {
    node::Context { id: id.into(), v: (), pred: None, edges: Vec::new() }
}

pub fn x<NV, ER: graph::Edge>(id: impl Into<id::N>) -> node::ContextRef<NV, ER> {
    node::ContextRef { id: id.into(), edges: Vec::new() }
}
```

- [ ] **Step 4: Update compile.rs to handle Exist nodes with id::N**

In the compilation pass, when processing `Op::Context { id, .. }`:
- Store the `id::N` value for pre-filling bindings
- Set `NodeKind::Exist`
- Pre-fill `bindings[node_index] = Some(id)`

- [ ] **Step 5: Run `cargo check` — expect failures in tests/scratch**

Run: `cargo check`
Expected: compilation errors in test files that pass `LocalId` to X()

- [ ] **Step 6: Commit (compilation may be broken — that's OK, tests fix in Task 6)**

```
change search X/x from LocalId to id::N
```

---

### Task 4: Wire Translated nodes through search query compilation

**Files:**
- Modify: `src/search/query/compile.rs` — flatten_ops, the main compilation pass

- [ ] **Step 1: Handle Op::Translated in the flatten pass**

In compile.rs, in the match on Op variants, add handling for `Op::Translated` and `Op::TranslatedRef`. These should:
- Create a `PatternNode` with `kind: NodeKind::Translated`
- Track the node index in `translated_indices`
- Leave `bindings[index] = None` (to be filled by `.bind()`)

- [ ] **Step 2: Handle Exist nodes pre-filling bindings**

When processing `Op::Context { id: graph_id, .. }`:
- Create a `PatternNode` with `kind: NodeKind::Exist`
- Pre-fill `bindings[index] = Some(graph_id)`

- [ ] **Step 3: Run `cargo check`**

Run: `cargo check`

- [ ] **Step 4: Commit**

```
wire Translated and Exist nodes through query compilation
```

---

### Task 5: Replace Search::Free/Bound with Resolved/Unresolved/Bound

**Files:**
- Modify: `src/search/query/mod.rs` — Search enum, BoundQuery, resolve_bindings
- Modify: `src/search/mod.rs` — re-exports
- Modify: `src/lib.rs:151` — re-exports

- [ ] **Step 1: Define new types in `src/search/query/mod.rs`**

Replace the existing Search/BoundQuery with:
```rust
pub enum Search<NV, ER: graph::Edge> {
    Resolved(Resolved<NV, ER>),
    Unresolved(Unresolved<NV, ER>),
}

pub struct Resolved<NV, ER: graph::Edge> {
    pub(crate) query: Query<NV, ER>,
    pub(crate) bindings: Vec<Option<id::N>>,
}

pub struct Unresolved<NV, ER: graph::Edge> {
    pub(crate) query: Query<NV, ER>,
    pub(crate) translated_indices: Vec<usize>,
    pub(crate) base_bindings: Vec<Option<id::N>>,
}

pub struct Bound<'a, NV, ER: graph::Edge> {
    pub(crate) query: &'a Query<NV, ER>,
    pub(crate) bindings: Vec<Option<id::N>>,
}
```

- [ ] **Step 2: Implement .bind() on Unresolved**

```rust
impl<NV, ER: graph::Edge> Unresolved<NV, ER> {
    pub fn bind(&self, table: &[(Id, Id)]) -> Result<Bound<'_, NV, ER>, BindError> {
        let mut bindings = self.base_bindings.clone();
        // ... resolve T nodes from table, validate
        Ok(Bound { query: &self.query, bindings })
    }
}
```

- [ ] **Step 3: Add BindError enum**

```rust
pub enum BindError {
    NotFound(Id),
    Duplicate(Id),
    Missing(Vec<Id>),
    Collision { n1: Id, n2: Id, target: Id },
}
```

- [ ] **Step 4: Update compile.rs variant selection**

Replace the `Search::Free`/`Search::Bound` selection with:
```rust
if translated_indices.is_empty() {
    Search::Resolved(Resolved { query, bindings })
} else {
    Search::Unresolved(Unresolved { query, translated_indices, base_bindings: bindings })
}
```

- [ ] **Step 5: Remove BoundQuery and ContextMap**

Delete the old `BoundQuery`, `resolve_bindings`, `ContextMap` trait and impls.

- [ ] **Step 6: Update `src/lib.rs` re-exports**

Change:
```rust
pub use search::{compile, Search, BoundQuery, ContextMap, Seq, RevCsr};
```
To:
```rust
pub use search::{compile, Search, Seq, RevCsr, BindError};
```

- [ ] **Step 7: Run `cargo check`**

Run: `cargo check`
Expected: errors in engine and test code referencing old types

- [ ] **Step 8: Commit**

```
replace Search::Free/Bound with Resolved/Unresolved/Bound
```

---

### Task 6: Update search engine to accept Resolved and Bound

**Files:**
- Modify: `src/search/engine/seq.rs` — search, search_bound, Iter
- Modify: `src/search/engine/par.rs` — parallel search
- Modify: `src/search/engine/mod.rs` — State, SeqPlan

- [ ] **Step 1: Update SeqPlan and search entry points**

Change `Seq::search` to accept `&Resolved`:
```rust
pub fn search<'g>(plan: &'g SeqPlan<NV, ER>, target: &'g Graph, resolved: &'g Resolved<NV, ER>) -> Iter<'g, NV, ER>
```

Add `Seq::search_bound` accepting `&Bound`:
```rust
pub fn search_bound<'g>(plan: &'g SeqPlan<NV, ER>, target: &'g Graph, bound: &'g Bound<'_, NV, ER>) -> Iter<'g, NV, ER>
```

Both extract `(&Query, &[Option<id::N>])` and pass to the internal `Iter::new`.

- [ ] **Step 2: Update State::new to accept bindings by reference**

Change State::new to clone/accept `&[Option<id::N>]` instead of consuming `Vec<Option<id::N>>`.

- [ ] **Step 3: Update parallel engine similarly**

Mirror the seq.rs changes in par.rs.

- [ ] **Step 4: Run `cargo check`**

Run: `cargo check`

- [ ] **Step 5: Commit**

```
update search engine to accept Resolved and Bound types
```

---

### Task 7: Fix all search tests and scratch files

**Files:**
- Modify: `src/search/mod.rs` — tests
- Modify: All `scratch/cluster/ban_context_node/*.rs`
- Modify: All `scratch/any/context_anyslot/*.rs`
- Modify: `tests/` files referencing context nodes

- [ ] **Step 1: Update search macro tests**

Convert tests using `X(0)` with `LocalId` semantics to use `T(0)` with `.bind()`:
```rust
// Before:
let Search::Bound(bq) = pattern else { panic!() };
let bindings = bq.resolve_bindings(graph, &[(0, 5)]);
Seq::search_bound(&plan, &target, bindings)

// After:
let Search::Unresolved(u) = pattern else { panic!() };
let bound = u.bind(&[(0, 5)]).unwrap();
Seq::search_bound(&plan, &target, &bound)
```

- [ ] **Step 2: Update scratch context test files**

All files in `scratch/cluster/ban_context_node/` and `scratch/any/context_anyslot/` — convert `X(local)` to `T(local)` and update the search invocation pattern.

- [ ] **Step 3: Update any integration tests**

Check `tests/` directory for context node usage and update.

- [ ] **Step 4: Run `cargo test`**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 5: Commit**

```
migrate all search tests from X(LocalId) to T(LocalId) with .bind()
```

---

### Task 8: Add T/t to modify DSL

**Files:**
- Modify: `src/modify/dsl/mod.rs` — add T/t constructors, Modify enum
- Modify: `src/modify/dsl/node.rs` — add translated node types
- Modify: `src/modify/dsl/validate.rs` — validation for T nodes
- Modify: `src/modify/apply.rs` — Resolved/Unresolved/Bound, .bind()

- [ ] **Step 1: Add translated module to modify DSL node.rs**

In `src/modify/dsl/node.rs`, add a `translated` module mirroring `exist`:
```rust
pub mod translated {
    pub struct Node<NV, V, ER: graph::Edge> {
        pub(crate) id: LocalId,
        pub(crate) v: V,
        pub(crate) edges: Vec<Edge<NV, ER>>,
    }
    pub struct Ref<NV, ER: graph::Edge> {
        pub(crate) id: LocalId,
        pub(crate) edges: Vec<Edge<NV, ER>>,
    }
    pub struct Rem<NV, ER: graph::Edge>(pub(crate) LocalId, pub PhantomData<(NV, ER)>);
    // .val(), Not impl
}
```

- [ ] **Step 2: Add Translated variant to the Node enum**

```rust
pub enum Node<NV, ER: graph::Edge> {
    New(node::New<NV>, Vec<edge::Edge<NV, ER>>),
    Exist(node::Exist<NV, ER>),
    Translated(node::TranslatedNode<NV, ER>),  // new
}
```

With `TranslatedNode` enum mirroring `Exist` but using `LocalId`:
```rust
pub enum TranslatedNode<NV, ER: graph::Edge> {
    Bind { id: LocalId, op: Bind<NV>, edges: Vec<Edge<NV, ER>> },
    Rem { id: LocalId },
}
```

- [ ] **Step 3: Add T/t constructors**

In `src/modify/dsl/mod.rs`:
```rust
#[allow(non_snake_case)]
pub fn T<NV, ER: graph::Edge>(id: impl Into<LocalId>) -> node::translated::Node<NV, (), ER> {
    node::translated::Node { id: id.into(), v: (), edges: Vec::new() }
}

#[allow(non_snake_case)]
pub fn t<NV, ER: graph::Edge>(id: impl Into<LocalId>) -> node::translated::Ref<NV, ER> {
    node::translated::Ref { id: id.into(), edges: Vec::new() }
}
```

- [ ] **Step 4: Implement operator impls for translated nodes**

Mirror exist node operator impls (`>>`, `<<`, `^`, `%`, `&`, etc.) for translated nodes.

- [ ] **Step 5: Add Modify enum and Resolved/Unresolved/Bound types**

In `src/modify/apply.rs` or a new `src/modify/resolve.rs`:
```rust
pub enum Modify<NV, ER: graph::Edge> {
    Resolved(Resolved<NV, ER>),
    Unresolved(Unresolved<NV, ER>),
}

pub struct Resolved<NV, ER: graph::Edge> {
    pub(crate) ops: Vec<Node<NV, ER>>,
    pub(crate) bindings: Vec<(LocalId, id::N)>,
}

pub struct Unresolved<NV, ER: graph::Edge> {
    pub(crate) ops: Vec<Node<NV, ER>>,
    pub(crate) translated_ids: Vec<LocalId>,
}

pub struct Bound<'a, NV, ER: graph::Edge> {
    pub(crate) ops: &'a [Node<NV, ER>],
    pub(crate) bindings: Vec<(LocalId, id::N)>,
}
```

- [ ] **Step 6: Implement .bind() on modify::Unresolved**

```rust
impl<NV, ER: graph::Edge> Unresolved<NV, ER> {
    pub fn bind(&self, table: &[(Id, Id)]) -> Result<Bound<'_, NV, ER>, BindError> {
        // validate all T nodes covered, no duplicates
        // return Bound with resolved bindings
    }
}
```

- [ ] **Step 7: Update modify apply to handle Translated nodes**

In `src/modify/apply.rs`, `flatten_node` must handle `Node::Translated`:
- Look up the `LocalId` in the bindings to get the actual `id::N`
- Then proceed exactly as `Node::Exist` does

- [ ] **Step 8: Update validate.rs for T nodes**

Add T node validation alongside exist node validation.

- [ ] **Step 9: Update modify! macro**

The `modify!` macro needs to detect T nodes and return `Modify::Resolved` or `Modify::Unresolved`. This may require the macro to call a classification function after collecting ops.

- [ ] **Step 10: Run `cargo check`**

Run: `cargo check`

- [ ] **Step 11: Write tests for modify with T nodes**

Test cases:
- `T(0) >> N(1)` with `.bind(&[(0, 5)])` adds edge from node 5
- `T(0).val(new_val)` with bind swaps node value
- `!T(0)` with bind removes node
- Mixed `X(5) >> T(0)` with bind
- BindError cases: missing, duplicate, not found

- [ ] **Step 12: Run `cargo test`**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 13: Commit**

```
add T/t translated nodes to modify DSL with Resolved/Unresolved/Bound
```

---

### Task 9: REPL integration

**Files:**
- Modify: `grw_repl/src/search_parse.rs` — add T/t node parsing
- Modify: `grw_repl/src/plugin/codegen.rs` — generated code for bind
- Modify: `grw_repl/src/plugin/abi.rs` — search ABI if needed
- Modify: `grw_repl/src/repl.rs` — syntax support

- [ ] **Step 1: Add T/t to structural parser**

In `search_parse.rs`, add `T(id)` and `t(id)` as node types. The JSON output includes `"translated": true` on T nodes.

- [ ] **Step 2: Parse translation table syntax**

Support `[(id, id), ...]` after the pattern body in both search and modify REPL syntax.

- [ ] **Step 3: Update codegen for search with bindings**

When translation table is present, generate code that:
```rust
let Search::Unresolved(u) = pattern else { panic!("expected Unresolved") };
let bound = u.bind(&[(0, 5)]).unwrap();
// search with bound
```

- [ ] **Step 4: Update codegen for modify with bindings**

Same pattern for modify.

- [ ] **Step 5: Update plugin ABI if needed**

The JSON search path may need to pass bindings to the plugin's search function.

- [ ] **Step 6: Run grw_repl tests**

Run: `cd grw_repl && cargo check`

- [ ] **Step 7: Commit**

```
add T node and translation table support to REPL
```

---

### Task 10: Final cleanup and documentation

**Files:**
- Modify: `src/lib.rs` — final re-exports
- Modify: Any remaining compile warnings

- [ ] **Step 1: Clean up re-exports**

Ensure `Search`, `Resolved`, `Unresolved`, `Bound`, `BindError` are properly exported from `src/lib.rs`.

- [ ] **Step 2: Remove dead code**

Remove any remaining references to `BoundQuery`, `ContextMap`, `resolve_bindings`, `Search::Free`, `Search::Bound`.

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: all tests pass, no warnings

- [ ] **Step 4: Commit**

```
clean up re-exports and remove dead code from context node migration
```
