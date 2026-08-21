# Tiered Search Index Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace hard-coded `CsrAdj` engine dependency with a monomorphized `search::Index` trait, enabling four precomputation tiers (Raw, Rev, RevCsr, RevCsrVal) that the user opts into at compile time.

**Architecture:** Define `Index` and `ReverseLookup` traits. Introduce `Indexed<'g, NV, ER, T>` wrapper parameterized by marker types (Rev, RevCsr, RevCsrVal). Make `Ctx` and `State` generic over `I: Index` and `R: ReverseLookup`. Current `CsrAdj` path becomes RevCsr tier with zero performance regression. Raw tier implements `Index` directly on `graph::Graph`.

**Tech Stack:** Rust stable, std::sync::Arc, rayon (parallel engine), smallvec

---

## Phase 1: Define Traits and Tier Markers

### Task 1: Add `search::Index` trait

**Files:**
- Modify: `src/search/engine/mod.rs:1-12` (add trait after imports)

**Step 1: Add the `Index` trait definition**

Add after the existing imports (line 12), before the `Engine` trait:

```rust
pub trait Index<NV: Sync, ER: graph::Edge>: Sync {
    type Neighbors<'a>: ExactSizeIterator<Item = u32> + 'a where Self: 'a;
    type Reverse: ReverseLookup;

    fn degree(&self, n: u32) -> u32;
    fn is_adjacent(&self, n1: u32, n2: u32) -> bool;
    fn neighbors(&self, n: u32) -> Self::Neighbors<'_>;
    fn node_val(&self, n: u32) -> &NV;

    fn slice(&self, n: u32) -> CsrSlice<'_, ER>;

    fn check_edge(
        &self, n1: u32, n2: u32,
        slot: ER::Slot, any_slot: bool, negated: bool,
        pred: Option<&dyn Fn(&ER::Val) -> bool>,
    ) -> bool;

    fn all_node_ids(&self) -> &[id::N];

    fn candidates(
        &self,
        pred: Option<&dyn Fn(&NV) -> bool>,
        min_degree: usize,
        morphism: Morphism,
    ) -> Vec<id::N>;

    fn search_order(&self, query: &Query<NV, ER>) -> (Vec<usize>, Vec<usize>);

    fn val_filtered(&self, query: &Query<NV, ER>) -> Vec<Option<Vec<id::N>>>;

    fn reverse_len(&self) -> usize;

    fn create_reverse(&self) -> Self::Reverse;
}
```

**Step 2: Add the `ReverseLookup` trait**

Add immediately after `Index`:

```rust
pub trait ReverseLookup {
    fn get(&self, n: u32) -> u32;
    fn set(&mut self, n: u32, val: u32);
    fn clear(&mut self, n: u32);
}

impl ReverseLookup for Vec<u32> {
    #[inline(always)]
    fn get(&self, n: u32) -> u32 { self[n as usize] }
    #[inline(always)]
    fn set(&mut self, n: u32, val: u32) { self[n as usize] = val; }
    #[inline(always)]
    fn clear(&mut self, n: u32) { self[n as usize] = UNMAPPED; }
}
```

**Step 3: Run `cargo check --lib`**

Expected: compiles. Traits are defined but not yet used.

### Task 2: Add tier marker types

**Files:**
- Modify: `src/search/engine/mod.rs` (add after ReverseLookup impls)

**Step 1: Define marker types and `Tier` trait**

```rust
pub struct Rev;
pub struct RevCsr;
pub struct RevCsrVal;

pub struct Indexed<'g, NV: Sync, ER: graph::Edge, T: Tier<NV, ER>> {
    graph: &'g graph::Graph<NV, ER>,
    data: T::Data,
}

pub trait Tier<NV: Sync, ER: graph::Edge> {
    type Data;
    fn build(graph: &graph::Graph<NV, ER>) -> Self::Data;
}
```

**Step 2: Run `cargo check --lib`**

Expected: compiles. Markers and Tier trait are defined but no impls yet.

---

## Phase 2: Implement RevCsr Tier (Current CsrAdj Equivalent)

This is the critical phase — RevCsr must produce identical behavior to current code.

### Task 3: Implement `Tier<NV, ER>` for `RevCsr`

**Files:**
- Modify: `src/search/engine/mod.rs`

**Step 1: Define `RevCsrData` and implement `Tier` for `RevCsr`**

```rust
pub(crate) struct RevCsrData<NV: Sync, ER: graph::Edge> {
    pub(crate) csr: CsrAdj<NV, ER>,
}

impl<NV: Sync + Clone, ER: graph::Edge> Tier<NV, ER> for RevCsr
where
    ER::Val: Clone,
{
    type Data = RevCsrData<NV, ER>;
    fn build(graph: &graph::Graph<NV, ER>) -> Self::Data {
        RevCsrData { csr: CsrAdj::build(graph) }
    }
}
```

**Step 2: Run `cargo check --lib`**

### Task 4: Implement `Index` for `Indexed<RevCsr>`

**Files:**
- Modify: `src/search/engine/mod.rs`

**Step 1: Implement `Index` trait for RevCsr-flavored `Indexed`**

This delegates everything to the existing `CsrAdj` methods, plus wraps `compute_search_order` and `val_filtered` logic. The key here is that `Indexed<RevCsr>` has both `graph` and `data.csr` — the same two references the current `Ctx` uses.

```rust
impl<'g, NV: Sync + Clone, ER: graph::Edge> Index<NV, ER> for Indexed<'g, NV, ER, RevCsr>
where
    ER::Val: Clone,
{
    type Neighbors<'a> = std::iter::Copied<std::slice::Iter<'a, u32>> where Self: 'a;
    type Reverse = Vec<u32>;

    #[inline(always)]
    fn degree(&self, n: u32) -> u32 { self.data.csr.degree(n) }

    #[inline(always)]
    fn is_adjacent(&self, n1: u32, n2: u32) -> bool { self.data.csr.is_adjacent(n1, n2) }

    #[inline(always)]
    fn neighbors(&self, n: u32) -> Self::Neighbors<'_> { self.data.csr.neighbors(n).iter().copied() }

    #[inline(always)]
    fn node_val(&self, n: u32) -> &NV { self.data.csr.node_val(n) }

    #[inline(always)]
    fn slice(&self, n: u32) -> CsrSlice<'_, ER> { self.data.csr.slice(n) }

    fn check_edge(
        &self, n1: u32, n2: u32,
        slot: ER::Slot, any_slot: bool, negated: bool,
        pred: Option<&dyn Fn(&ER::Val) -> bool>,
    ) -> bool {
        let store = self.data.csr.edge_store_at(n1, n2);
        let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
        edge_check::<ER>(store, any_slot, negated, actual_slot, pred)
    }

    fn all_node_ids(&self) -> &[id::N] { self.data.csr.all_node_ids() }

    fn candidates(
        &self,
        pred: Option<&dyn Fn(&NV) -> bool>,
        min_degree: usize,
        morphism: Morphism,
    ) -> Vec<id::N> {
        self.data.csr.all_node_ids().iter().copied()
            .filter(|&n| {
                let d = self.data.csr.degree(*n) as usize;
                let deg_ok = match morphism {
                    Morphism::Iso => d == min_degree,
                    Morphism::SubIso | Morphism::Mono => d >= min_degree,
                    Morphism::Homo => true,
                };
                if !deg_ok { return false; }
                if let Some(p) = pred {
                    if !p(self.data.csr.node_val(*n)) { return false; }
                }
                true
            })
            .collect()
    }

    fn search_order(&self, query: &Query<NV, ER>) -> (Vec<usize>, Vec<usize>) {
        compute_search_order(query, &self.data.csr)
    }

    fn val_filtered(&self, query: &Query<NV, ER>) -> Vec<Option<Vec<id::N>>> {
        query.node_preds.iter()
            .map(|pred_opt| {
                pred_opt.as_ref().map(|pred| {
                    self.data.csr.all_node_ids().iter().copied()
                        .filter(|&n| pred(self.data.csr.node_val(*n)))
                        .collect()
                })
            })
            .collect()
    }

    fn reverse_len(&self) -> usize {
        if self.data.csr.offsets.len() > 1 { self.data.csr.offsets.len() - 1 } else { 0 }
    }

    fn create_reverse(&self) -> Vec<u32> {
        vec![UNMAPPED; self.reverse_len()]
    }
}
```

**Step 2: Run `cargo check --lib`**

### Task 5: Add `graph.index(RevCsr)` API

**Files:**
- Modify: `src/search/engine/mod.rs:256-264` (existing `graph.index()` impl block)

**Step 1: Add parameterized `index` method**

Keep the old `graph.index()` working for now (returns `Graph`), and add a new method:

```rust
impl<NV: Sync + Clone, ER: graph::Edge> graph::Graph<NV, ER>
where
    ER::Val: Clone,
{
    pub fn index_tier(&self, _tier: RevCsr) -> Indexed<'_, NV, ER, RevCsr> {
        Indexed { graph: self, data: RevCsr::build(self) }
    }
}
```

We name it `index_tier` temporarily to avoid conflicting with the existing `index()` method. We'll rename during migration.

**Step 2: Run `cargo check --lib`**

**Step 3: Verify by adding a temporary test at the bottom of mod.rs tests**

```rust
#[test]
fn tier_api_compiles() {
    use crate::graph;
    let g: graph::Graph<&str, crate::graph::edge::Undirected<()>> = graph::Graph::new();
    let _idx = g.index_tier(super::engine::RevCsr);
}
```

Run: `cargo test --lib tier_api_compiles`

---

## Phase 3: Make Engine Generic Over Index

This is the largest phase. We make `Ctx` and `State` generic over the `Index` trait, then update all call sites in seq.rs.

### Task 6: Make `Ctx` generic

**Files:**
- Modify: `src/search/engine/mod.rs:266-270` (Ctx definition)

**Step 1: Replace `Ctx` definition**

Current:
```rust
pub(crate) struct Ctx<'a, NV: Sync, ER: graph::Edge> {
    pub(crate) query: &'a Query<NV, ER>,
    pub(crate) target: &'a graph::Graph<NV, ER>,
    pub(crate) csr: &'a CsrAdj<NV, ER>,
}
```

New:
```rust
pub(crate) struct Ctx<'a, NV: Sync, ER: graph::Edge, I: Index<NV, ER>> {
    pub(crate) query: &'a Query<NV, ER>,
    pub(crate) target: &'a graph::Graph<NV, ER>,
    pub(crate) index: &'a I,
}
```

This will cause many compile errors — that's expected. We fix them in subsequent steps.

**Step 2: Create type alias for backward compat during migration**

```rust
pub(crate) type CsrCtx<'a, NV, ER> = Ctx<'a, NV, ER, Indexed<'a, NV, ER, RevCsr>>;
```

This won't help much since the generic propagates, but it documents intent.

### Task 7: Make `State` generic over `ReverseLookup`

**Files:**
- Modify: `src/search/engine/mod.rs:379-390` (State definition)

**Step 1: Replace `State` definition**

Current `State.reverse` is `Vec<u32>`. For RevCsr tier this is still `Vec<u32>` — no change in behavior. We make it generic so Raw tier can use `FxHashMap` later.

```rust
pub(crate) struct State<R: ReverseLookup> {
    pub(crate) bindings: Vec<Option<id::N>>,
    pub(crate) mapping: Vec<u32>,
    pub(crate) reverse: R,
    pub(crate) search_order: Vec<usize>,
    pub(crate) search_depth_of: Vec<usize>,
    pub(crate) stack: Vec<StackFrame>,
    pub(crate) exhausted: bool,
    pub(crate) candidate_pool: Vec<Vec<id::N>>,
    pub(crate) forward_verified_depths: u64,
    pub(crate) val_filtered: Arc<Vec<Option<Vec<id::N>>>>,
}
```

This also causes many compile errors. Fix them together with Task 6.

### Task 8: Update `Shared::precompute` to use `Index`

**Files:**
- Modify: `src/search/engine/mod.rs:343-377`

**Step 1: Change `Shared::precompute` signature**

Current:
```rust
pub(crate) fn precompute<NV: Sync, ER: graph::Edge>(
    query: &Query<NV, ER>,
    target: &graph::Graph<NV, ER>,
    csr: &CsrAdj<NV, ER>,
) -> Self
```

New:
```rust
pub(crate) fn precompute<NV: Sync, ER: graph::Edge, I: Index<NV, ER>>(
    query: &Query<NV, ER>,
    target: &graph::Graph<NV, ER>,
    index: &I,
) -> Self
```

Replace `csr` references inside with `index` calls:
- `compute_search_order(query, csr)` → `index.search_order(query)`
- `csr.all_node_ids()` → `index.all_node_ids()`
- `csr.node_val(...)` → `index.node_val(...)`

### Task 9: Update `State::new` and `State::new_from_shared` to use `Index`

**Files:**
- Modify: `src/search/engine/mod.rs:473-673`

**Step 1: Change `State::new` signature**

```rust
pub(crate) fn new<NV: Sync, ER: graph::Edge, I: Index<NV, ER>>(
    query: &Query<NV, ER>,
    target: &graph::Graph<NV, ER>,
    index: &I,
    bindings: Vec<Option<id::N>>,
) -> State<I::Reverse>
```

Inside, replace:
- `csr.offsets.len()` → `index.reverse_len()`
- `compute_search_order(query, csr)` → `index.search_order(query)`
- `csr.all_node_ids()` → `index.all_node_ids()`
- `csr.node_val(...)` → `index.node_val(...)`
- `vec![UNMAPPED; reverse_len]` → `index.create_reverse()`
- val_filtered computation → `index.val_filtered(query)`

**Step 2: Change `State::new_from_shared` similarly**

```rust
pub(crate) fn new_from_shared<NV: Sync, ER: graph::Edge, I: Index<NV, ER>>(
    query: &Query<NV, ER>,
    target: &graph::Graph<NV, ER>,
    index: &I,
    shared: &Shared,
    bindings: Vec<Option<id::N>>,
) -> State<I::Reverse>
```

### Task 10: Update all `State` method signatures to use generic `Ctx<I>` and `R: ReverseLookup`

**Files:**
- Modify: `src/search/engine/mod.rs` (every `impl State { ... }` block)
- Modify: `src/search/engine/seq.rs` (all `State` method calls)

This is the bulk of the work. Every method on `State` that takes `&Ctx<'_, NV, ER>` must become `&Ctx<'_, NV, ER, I>` where `I: Index<NV, ER>`. Every `self.reverse[x]` becomes `self.reverse.get(x)` / `self.reverse.set(x, v)` / `self.reverse.clear(x)`.

**Key call sites in seq.rs (25 `ctx.csr.*` calls):**

Replace each `ctx.csr.X(...)` with `ctx.index.X(...)`:
- `ctx.csr.degree(n)` → `ctx.index.degree(n)` (7 sites)
- `ctx.csr.node_val(n)` → `ctx.index.node_val(n)` (6 sites)
- `ctx.csr.all_node_ids()` → `ctx.index.all_node_ids()` (3 sites)
- `ctx.csr.neighbors(n)` → collect from `ctx.index.neighbors(n)` (3 sites — note: returns iterator not slice now)
- `ctx.csr.slice(n)` → `ctx.index.slice(n)` (2 sites)
- `ctx.csr.is_adjacent(a, b)` → `ctx.index.is_adjacent(a, b)` (2 sites)
- `ctx.csr.node_vals.len()` → special handling (1 site in `candidates_for_into`)

**Key `self.reverse` access pattern changes:**

All `self.reverse[x as usize]` reads become `self.reverse.get(x)`.
All `self.reverse[x as usize] = val` writes become `self.reverse.set(x, val)`.
All `self.reverse[x as usize] = UNMAPPED` writes become `self.reverse.clear(x)`.

There are approximately 20+ reverse access sites across mod.rs and seq.rs.

**Neighbors iteration change:**

Current code uses `ctx.csr.neighbors(n)` which returns `&[u32]` and iterates with `.iter()`. The `Index` trait returns `Self::Neighbors<'_>: ExactSizeIterator<Item = u32>`. For RevCsr, this is `Copied<Iter<'_, u32>>` — should be equivalent after compiler optimization. But call sites need adjustment:

- `for &raw in ctx.csr.neighbors(n)` → `for raw in ctx.index.neighbors(n)`
- `ctx.csr.neighbors(n).iter().map(...)` → `ctx.index.neighbors(n).map(...)`

**Step 1: Update all `impl State` blocks in mod.rs**

Change every method signature. The `impl` blocks are at lines:
- 110-177 (initial_candidates, candidates_for_into)
- 302-410 (check_ban_clusters, ban_cluster_satisfiable, ban_backtrack, is_canonical_at, is_canonical_leaf)
- 412-539 (count_leaf_fused)
- 541-744 (advance)
- 676-701 (any_slot_pred_matches, is_feasible_reverse_only)
- 703-728 (is_feasible_fast)
- 730-823 (is_feasible)
- 825-907 (lookahead_ok, has_any_candidate)
- 909-946 (best_mapped_neighbor)
- 948-1002 (check_negated_connected, check_negated_free_violation, get_covered_slots)
- 1004+ (has_uncovered_edge, uncovered_pred_matches, ban_shared_edges_satisfied, ban_node_feasible, build_match)

All become:
```rust
impl<R: ReverseLookup> State<R> {
    pub(crate) fn method_name<NV: Sync, ER: graph::Edge, I: Index<NV, ER>>(
        &self, ctx: &Ctx<'_, NV, ER, I>, ...
    ) -> ... { ... }
}
```

**Step 2: Update seq.rs**

- Change `Iter` struct to hold `Ctx<'g, NV, ER, Indexed<'g, NV, ER, RevCsr>>` and `State<Vec<u32>>`
- Update `Iter::new`, `Iter::new_bound` to build `Ctx { query, target: target.inner, index: ... }`
- Update `IntoIter` similarly
- Update `OwnedIter` to hold `Indexed` instead of separate `CsrAdj`
- `dispatch_advance!` macro — no changes needed (types propagate)

**Step 3: Run `cargo check --lib`**

Fix all compilation errors. This will be iterative — expect 50-100 errors to work through.

### Task 11: Update par.rs to use generic `Index`

**Files:**
- Modify: `src/search/engine/par.rs`

**Step 1: Replace imports**

```rust
use super::{Match, State, Shared, Ctx, CsrAdj, Index, Indexed, RevCsr, ReverseLookup};
```

**Step 2: Update `Iter` struct**

```rust
pub struct Iter<'g, NV: Sync, ER: graph::Edge> {
    query: &'g Query<NV, ER>,
    indexed: &'g Indexed<'g, NV, ER, RevCsr>,
    receiver: Option<mpsc::Receiver<Vec<Match>>>,
    worker: Option<std::thread::JoinHandle<()>>,
    hint: ParHint,
    pending: std::vec::IntoIter<Match>,
}
```

**Step 3: Update `start_streaming`**

Replace:
- `self.indexed.inner` → `self.indexed.graph`
- `&self.indexed.csr` → removed (use `self.indexed` directly as the `Index` impl)
- `Ctx { query, csr, target: inner }` → `Ctx { query, target: inner, index: indexed }`
- `State::new_from_shared(query, inner, csr, ...)` → `State::new_from_shared(query, inner, indexed, ...)`
- `Shared::precompute(query, inner, csr)` → `Shared::precompute(query, inner, indexed)`

Only 2 unsafe pointer casts needed now: `query_addr` and `indexed_addr` (was 3: query, inner, csr).

**Step 4: Update `par_count` similarly**

**Step 5: Run `cargo check --lib`**

### Task 12: Update `Session` and public API

**Files:**
- Modify: `src/search/engine/mod.rs:245-334` (Graph, Session, impls)

**Step 1: Update `Graph` to use `Indexed<RevCsr>`**

Option A: Replace `Graph<'g, NV, ER>` with a type alias:
```rust
pub type Graph<'g, NV, ER> = Indexed<'g, NV, ER, RevCsr>;
```

This preserves backward compatibility — all existing code that uses `search::Graph` continues to work.

But: the internal field names change. `Graph.inner` → `Indexed.graph`, `Graph.csr` → `Indexed.data.csr`.

Option B: Keep `Graph` as-is for now, add conversion methods. Migrate later.

**Recommended: Option A.** Replace `Graph` with type alias. Update all internal references.

- `graph.index()` returns `Graph<'_, NV, ER>` which is now `Indexed<'_, NV, ER, RevCsr>`
- `Session` stores `Indexed<'g, NV, ER, RevCsr>` (via the type alias)

**Step 2: Update `graph.index()` to return `Indexed<RevCsr>`**

```rust
impl<NV: Sync + Clone, ER: graph::Edge> graph::Graph<NV, ER>
where
    ER::Val: Clone,
{
    pub fn index(&self) -> Graph<'_, NV, ER> {
        Indexed { graph: self, data: RevCsr::build(self) }
    }
}
```

Remove the temporary `index_tier` method from Task 5.

**Step 3: Update `Session` field access**

In `Session::from_search`, `Session::iter`, etc., update field access:
- `self.indexed.inner` → `self.indexed.graph`
- `&self.indexed.csr` → `&self.indexed.data.csr` (or use `self.indexed` as `Index`)

**Step 4: Update `Graph::graph()` method**

```rust
impl<'g, NV: Sync + Clone, ER: graph::Edge> Indexed<'g, NV, ER, RevCsr>
where
    ER::Val: Clone,
{
    pub fn graph(&self) -> &graph::Graph<NV, ER> {
        self.graph
    }
}
```

**Step 5: Run `cargo check --lib`**

### Task 13: Update search macros and tests

**Files:**
- Modify: `src/search/mod.rs:56-82` (search! macro)

The `search!` macro uses `Session::from_search` and `OwnedIter::from_graph_and_search` — these should still work since they use `Graph` which is now `Indexed<RevCsr>`.

**Step 1: Run `cargo test --lib`**

All existing tests should pass. The RevCsr tier produces identical behavior.

**Step 2: Run `cargo check --features bench --bin bench_search`**

Verify all binaries still compile.

**Step 3: Run `cargo check --features bench --bin crosscheck`**

**Step 4: Run `cargo check --features bench --bin prof_search`**

---

## Phase 4: Implement Rev Tier

### Task 14: Implement `Tier` for `Rev`

**Files:**
- Modify: `src/search/engine/mod.rs`

**Step 1: Define `RevData` and implement `Tier`**

```rust
pub(crate) struct RevData {
    pub(crate) reverse_len: usize,
}

impl<NV: Sync, ER: graph::Edge> Tier<NV, ER> for Rev {
    type Data = RevData;
    fn build(graph: &graph::Graph<NV, ER>) -> Self::Data {
        RevData { reverse_len: graph.nodes.store.len() }
    }
}
```

**Step 2: Implement `Index` for `Indexed<Rev>`**

Rev tier uses the live graph directly for all data access. The only precomputation is knowing the reverse array size (so `create_reverse` returns a dense `Vec` instead of `FxHashMap`).

All methods delegate to `self.graph` (the live graph):
- `degree(n)` → `self.graph.nodes.get(id::N(n as Id)).map_or(0, |node| node.adj.len() as u32)`
- `is_adjacent(n1, n2)` → `self.graph.nodes.get(id::N(n1 as Id)).map_or(false, |node| node.adj.contains(n2))`
- `neighbors(n)` → iterator over `self.graph.nodes.get(n).adj`
- `node_val(n)` → `self.graph.nodes.get(n).val`
- `all_node_ids()` → must collect on the fly (no precomputed array) — or store it in `RevData`
- `check_edge(...)` → uses `self.graph.edges_between` / `self.graph.get_edge_val`
- `search_order(query)` → returns `(query.search_order.clone(), query.search_depth_of.clone())` (no CsrAdj to scan)
- `val_filtered(query)` → iterates live graph nodes with predicates
- `create_reverse()` → `vec![UNMAPPED; self.data.reverse_len]`

Note: `slice()` won't be available for Rev — this method is CsrAdj-specific. We need to either:
- Make `slice` return an `Option` (breaking)
- Move edge-checking logic entirely into `check_edge` (cleaner)
- Provide a default `slice` that panics (unsafe contract)

**Decision:** The `slice` method should be removed from the `Index` trait. All edge checking should go through `check_edge`. This means `is_feasible` in mod.rs needs to be refactored to use `index.check_edge(n1, n2, slot, any_slot, negated, pred)` instead of `cand.edge_store_at(mapped_id)` + `edge_check::<ER>(...)`.

This is a significant refactor to `is_feasible`, `candidates_for_into`, and `count_leaf_fused` — but it's necessary for the Raw tier to work without CSR data.

**Step 3: Run `cargo check --lib`**

### Task 15: Add `graph.index(Rev)` API

**Files:**
- Modify: `src/search/engine/mod.rs`

```rust
impl<NV: Sync, ER: graph::Edge> graph::Graph<NV, ER> {
    pub fn index_rev(&self) -> Indexed<'_, NV, ER, Rev> {
        Indexed { graph: self, data: Rev::build(self) }
    }
}
```

Note: Rev tier requires only `NV: Sync` — no Clone or Hash.

**Step 1: Run `cargo check --lib`**

---

## Phase 5: Implement Raw Tier (Graph Implements Index Directly)

### Task 16: Implement `Index` directly on `graph::Graph`

**Files:**
- Modify: `src/search/engine/mod.rs`

The Raw tier has zero precomputation. The `graph::Graph` itself implements `Index`.

```rust
impl<NV: Sync, ER: graph::Edge> Index<NV, ER> for graph::Graph<NV, ER> {
    type Neighbors<'a> = /* iterator adapter over SmallVec */ where Self: 'a;
    type Reverse = rustc_hash::FxHashMap<u32, u32>;

    fn degree(&self, n: u32) -> u32 { ... }
    fn is_adjacent(&self, n1: u32, n2: u32) -> bool { ... }
    fn neighbors(&self, n: u32) -> Self::Neighbors<'_> { ... }
    fn node_val(&self, n: u32) -> &NV { ... }
    fn check_edge(...) -> bool { ... }
    fn all_node_ids(&self) -> &[id::N] { /* must store or collect */ }
    fn candidates(...) -> Vec<id::N> { /* iterate sparse nodes */ }
    fn search_order(query) -> (Vec<usize>, Vec<usize>) { /* return query's pre-compiled order */ }
    fn val_filtered(query) -> Vec<Option<Vec<id::N>>> { /* iterate live graph */ }
    fn reverse_len(&self) -> usize { 0 }
    fn create_reverse(&self) -> FxHashMap<u32, u32> { FxHashMap::default() }
}
```

And implement `ReverseLookup` for `FxHashMap<u32, u32>`:

```rust
impl ReverseLookup for rustc_hash::FxHashMap<u32, u32> {
    fn get(&self, n: u32) -> u32 { self.get(&n).copied().unwrap_or(UNMAPPED) }
    fn set(&mut self, n: u32, val: u32) { self.insert(n, val); }
    fn clear(&mut self, n: u32) { self.remove(&n); }
}
```

**Challenge:** `all_node_ids()` returns `&[id::N]` but Raw has no precomputed array. Options:
1. Change return to `Cow<'_, [id::N]>` (affects trait)
2. Store `all_node_ids` eagerly even in Raw (defeats purpose)
3. Change `all_node_ids` to return an iterator instead of slice

**Decision:** Change `all_node_ids` and related methods to avoid requiring a stored slice. Or accept that Raw tier collects `all_node_ids` on demand — it's O(|V|) but only happens when the backtracking engine needs the full candidate list (disconnected patterns). For connected patterns, candidates come from neighbor expansion which doesn't need `all_node_ids`.

This needs careful design in Task 14/16. We may need to split `all_node_ids` into a method that returns owned `Vec<id::N>` rather than a reference.

### Task 17: Update `Session::new` API for Raw tier

**Files:**
- Modify: `src/search/engine/mod.rs`

```rust
impl<'g, NV: Sync + 'g, ER: graph::Edge + 'g> Session<'g, NV, ER> {
    pub fn new(query: Query<NV, ER>, graph: &'g graph::Graph<NV, ER>) -> Self {
        // Uses Raw tier — zero precompute
        ...
    }
}
```

---

## Phase 6: RevCsrVal Tier

### Task 18: Implement RevCsrVal tier

**Files:**
- Modify: `src/search/engine/mod.rs`

```rust
pub(crate) struct RevCsrValData<NV: Sync + Eq + std::hash::Hash, ER: graph::Edge> {
    pub(crate) csr: CsrAdj<NV, ER>,
    pub(crate) value_groups: rustc_hash::FxHashMap<NV, Vec<id::N>>,
}

impl<NV: Sync + Clone + Eq + std::hash::Hash, ER: graph::Edge> Tier<NV, ER> for RevCsrVal
where
    ER::Val: Clone,
{
    type Data = RevCsrValData<NV, ER>;
    fn build(graph: &graph::Graph<NV, ER>) -> Self::Data {
        let csr = CsrAdj::build(graph);
        let mut value_groups: rustc_hash::FxHashMap<NV, Vec<id::N>> =
            rustc_hash::FxHashMap::default();
        for &n in csr.all_node_ids() {
            let val = csr.node_val(*n).clone();
            value_groups.entry(val).or_default().push(n);
        }
        RevCsrValData { csr, value_groups }
    }
}
```

`Index` impl is identical to RevCsr except:
- `candidates(pred, ...)` → looks up value groups instead of scanning all nodes
- `search_order(query)` → scans value group sizes for selectivity (O(|P|×|distinct|) not O(|P|×|V|))
- `val_filtered(query)` → uses value groups for efficient filtering

### Task 19: Add `graph.index(RevCsrVal)` API

**Files:**
- Modify: `src/search/engine/mod.rs`

```rust
impl<NV: Sync + Clone + Eq + std::hash::Hash, ER: graph::Edge> graph::Graph<NV, ER>
where
    ER::Val: Clone,
{
    pub fn index_val(&self) -> Indexed<'_, NV, ER, RevCsrVal> {
        Indexed { graph: self, data: RevCsrVal::build(self) }
    }
}
```

---

## Phase 7: Migration and Cleanup

### Task 20: Unify `graph.index(tier)` API

**Files:**
- Modify: `src/search/engine/mod.rs`

Replace separate `index()`, `index_rev()`, `index_val()` methods with a single generic:

```rust
impl<NV: Sync, ER: graph::Edge> graph::Graph<NV, ER> {
    pub fn index<T: Tier<NV, ER>>(&self, _tier: T) -> Indexed<'_, NV, ER, T> {
        Indexed { graph: self, data: T::build(self) }
    }
}
```

The bounds on each `Tier` impl ensure:
- `graph.index(Rev)` compiles when `NV: Sync`
- `graph.index(RevCsr)` compiles when `NV: Sync + Clone, ER::Val: Clone`
- `graph.index(RevCsrVal)` compiles when `NV: Sync + Clone + Eq + Hash, ER::Val: Clone`

**Step 1: Remove the old `graph.index()` method that returned `Graph`**

**Step 2: Update all call sites:**
- `graph.index()` → `graph.index(RevCsr)` in:
  - `src/search/engine/mod.rs:288` (Session::from_search)
  - `src/search/mod.rs:385,582,614,731,752,774,1327,1352` (tests)
  - `src/bin/prof_search.rs:73`
  - `src/bin/verify_patterns.rs:446`
  - `src/bin/crosscheck.rs:531,625,714,808`

**Step 3: Remove `Engine` trait (design doc says to remove it, Par doesn't implement it)**

**Step 4: Remove `Shared` struct (absorbed into `Index` trait methods)**

Actually — `Shared` is still useful for the parallel engine (precompute once, share across workers). Keep it but update to use `Index`.

**Step 5: Run `cargo test --lib && cargo check --features bench`**

### Task 21: Remove `Graph` type alias

**Files:**
- Modify: `src/search/mod.rs:11` (re-export)
- Modify: `src/search/engine/mod.rs`

If we want to clean up the `Graph` type alias:

```rust
// Before: pub type Graph<'g, NV, ER> = Indexed<'g, NV, ER, RevCsr>;
// After: export Indexed directly
pub use engine::{Indexed, Rev, RevCsr, RevCsrVal};
```

But this is a breaking API change. Defer if there are external consumers.

### Task 22: Update benchmarks

**Files:**
- Modify: `src/bin/bench_search.rs`
- Modify: `src/bin/prof_search.rs`

Update to use new tier API:
```rust
let idx = graph.index(RevCsr);  // was: graph.index()
```

Add tier comparison benchmarks:
```rust
// Raw
let start = Instant::now();
let count_raw = search_raw(&graph, &query);
let raw_time = start.elapsed();

// RevCsr
let idx = graph.index(RevCsr);
let start = Instant::now();
let count_csr = search_csr(&idx, &query);
let csr_time = start.elapsed();
```

### Task 23: Final verification

**Step 1:** `cargo test --lib` — all tests pass

**Step 2:** `cargo check --features bench` — all binaries compile

**Step 3:** Run bench_search to verify no regression:
```bash
cargo run --release --features crosscheck --bin bench_search -- \
  --scenario subgraph-in-random --size 1000 --pattern 7 --iters 5 --seed 42
```

**Step 4:** Run crosscheck for correctness:
```bash
cargo run --release --features crosscheck --bin crosscheck -- --iters 100 --seed 42
```

---

## Execution Order Summary

| Phase | Tasks | Risk | Notes |
|---|---|---|---|
| 1: Traits | 1-2 | Low | Additive, no behavior change |
| 2: RevCsr | 3-5 | Low | Wraps existing CsrAdj, no behavior change |
| 3: Generic engine | 6-13 | **High** | 100+ call site changes, must pass all tests |
| 4: Rev tier | 14-15 | Medium | New capability, needs `slice` removal from trait |
| 5: Raw tier | 16-17 | Medium | `FxHashMap` reverse, `all_node_ids` challenge |
| 6: RevCsrVal | 18-19 | Low | Additive, value group optimization |
| 7: Migration | 20-23 | Medium | API changes, all call sites updated |

Phase 3 is the critical path. If it compiles and tests pass, the rest is incremental.
