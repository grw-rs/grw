# Edge Policy Restructuring Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Split edge verification into structural (adjacency-class) and predicate (closure-class) paths so patterns without edge predicates never pay `check_edge`/`dyn Fn` overhead.

**Architecture:** Add `has_edge_in_slot` to the `Index` trait for cheap slot-aware adjacency checks. Split `adj` into `adj_check` (structural, no predicate) and `adj_pred` (has predicate). Rename `HAS_FEATURES` → `HAS_PREDICATES` to gate only the predicate loop. The original `adj` list is kept for fallback paths (`is_feasible`, `get_covered_slots`, etc.).

**Tech Stack:** Rust, no new dependencies.

---

### Task 1: Rename policy types and flags

**Files:**
- Modify: `src/search/engine/policy.rs:1-9`
- Modify: `src/search/engine/seq.rs` (lines 217, 233, 255, 395, 410, 458, 685, 689-696)
- Modify: `src/search/engine/mod.rs` (lines 1327, 1499)
- Modify: `src/search/query/mod.rs` (lines 64-65)
- Modify: `src/search/query/compile.rs` (lines 611-631, 662-663)

This task is purely mechanical renaming — no logic changes.

**Step 1: Update policy.rs**

Replace the entire contents of `src/search/engine/policy.rs`:

```rust
pub(crate) trait Edge {
    const HAS_PREDICATES: bool;
}

pub(crate) struct PlainEdges;
pub(crate) struct PredEdges;

impl Edge for PlainEdges { const HAS_PREDICATES: bool = false; }
impl Edge for PredEdges { const HAS_PREDICATES: bool = true; }

pub(crate) trait Negation {
    const ACTIVE: bool;
}

pub(crate) struct NoNegation;
pub(crate) struct WithNegation;

impl Negation for NoNegation { const ACTIVE: bool = false; }
impl Negation for WithNegation { const ACTIVE: bool = true; }

pub(crate) trait Ban {
    const ACTIVE: bool;
}

pub(crate) struct NoBans;
pub(crate) struct WithBans;

impl Ban for NoBans { const ACTIVE: bool = false; }
impl Ban for WithBans { const ACTIVE: bool = true; }

pub(crate) trait Emit {
    const COUNT_ONLY: bool;
}

pub(crate) struct Collect;
pub(crate) struct Count;

impl Emit for Collect { const COUNT_ONLY: bool = false; }
impl Emit for Count { const COUNT_ONLY: bool = true; }
```

**Step 2: Rename fields in Query struct (`src/search/query/mod.rs:64-65`)**

```
has_edge_features  → has_predicates
node_has_edge_features → node_has_predicates
```

**Step 3: Update compile.rs (`src/search/query/compile.rs`)**

At line 611-631, rename variables and narrow the classification. `has_predicates` is true only when actual edge predicates exist:

```rust
let has_predicates = flat.edge_preds.iter().any(|p| p.is_some());

let mut node_has_predicates = vec![false; node_count];
for (ni, _node) in flat.nodes.iter().enumerate() {
    for &(_, _, _negated, edge_idx) in &adj[ni] {
        if flat.edge_preds[edge_idx].is_some() {
            node_has_predicates[ni] = true;
            break;
        }
    }
}
```

At lines 662-663, rename the field assignments:
```
has_edge_features → has_predicates
node_has_edge_features → node_has_predicates
```

**Step 4: Update seq.rs references**

All `EP::HAS_FEATURES` → `EP::HAS_PREDICATES`.
All `ctx.query.has_edge_features` → `ctx.query.has_predicates`.
All `ctx.query.node_has_edge_features` → `ctx.query.node_has_predicates`.
All `policy::SimpleEdges` → `policy::PlainEdges`.
All `policy::RichEdges` → `policy::PredEdges`.

Affected lines: 217, 233, 255, 395, 410, 458, 685, 689-696.

**Step 5: Update mod.rs references**

All `ctx.query.node_has_edge_features` → `ctx.query.node_has_predicates` at lines 1327, 1499.

**Step 6: Verify it compiles**

Run: `cargo check --lib --tests`
Expected: PASS (purely mechanical rename, no logic change)

---

### Task 2: Add `has_edge_in_slot` to the Index trait and all implementations

**Files:**
- Modify: `src/search/engine/mod.rs` (trait definition at line 14, plus 4 impl blocks)

**Step 1: Add method to Index trait (after `is_adjacent` at line 19)**

```rust
fn has_edge_in_slot(&self, n1: u32, n2: u32, slot: ER::Slot, any_slot: bool) -> bool;
```

**Step 2: Implement for `Indexed<'_, NV, ER, RevData>` (after `is_adjacent` at line ~148)**

```rust
#[inline(always)]
fn has_edge_in_slot(&self, n1: u32, n2: u32, slot: ER::Slot, any_slot: bool) -> bool {
    if any_slot || ER::SLOT_COUNT == 1 {
        return self.is_adjacent(n1, n2);
    }
    let n1_id = id::N(n1 as Id);
    let n2_id = id::N(n2 as Id);
    let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
    self.graph.edges_between(n1_id, n2_id)
        .any(|(s, _)| s == actual_slot)
}
```

**Step 3: Implement for `Indexed<'_, NV, ER, RawData>` (after `is_adjacent` at line ~254)**

Same implementation body as Step 2.

**Step 4: Implement for `Indexed<'_, NV, ER, CsrAdj<NV, ER>>` (after `is_adjacent` at line ~360)**

```rust
#[inline(always)]
fn has_edge_in_slot(&self, n1: u32, n2: u32, slot: ER::Slot, any_slot: bool) -> bool {
    if any_slot || ER::SLOT_COUNT == 1 {
        return self.data.is_adjacent(n1, n2);
    }
    let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
    match self.data.edge_store_at(n1, n2) {
        Some(store) => ER::csr_store_val(store, actual_slot).is_some(),
        None => false,
    }
}
```

**Step 5: Implement for `Indexed<'_, NV, ER, RevCsrValData<NV, ER>>` (after `is_adjacent` at line ~464)**

```rust
#[inline(always)]
fn has_edge_in_slot(&self, n1: u32, n2: u32, slot: ER::Slot, any_slot: bool) -> bool {
    if any_slot || ER::SLOT_COUNT == 1 {
        return self.data.csr.is_adjacent(n1, n2);
    }
    let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
    match self.data.csr.edge_store_at(n1, n2) {
        Some(store) => ER::csr_store_val(store, actual_slot).is_some(),
        None => false,
    }
}
```

**Step 6: Implement for `CsrAdj<NV, ER>` (after `is_adjacent` at line ~816)**

```rust
#[inline(always)]
fn has_edge_in_slot(&self, n1: u32, n2: u32, slot: ER::Slot, any_slot: bool) -> bool {
    if any_slot || ER::SLOT_COUNT == 1 {
        return self.is_adjacent(n1, n2);
    }
    let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
    match CsrAdj::edge_store_at(self, n1, n2) {
        Some(store) => ER::csr_store_val(store, actual_slot).is_some(),
        None => false,
    }
}
```

**Step 7: Verify it compiles**

Run: `cargo check --lib --tests`
Expected: PASS

---

### Task 3: Add `adj_check` and `adj_pred` to Query and populate in compile.rs

**Files:**
- Modify: `src/search/query/mod.rs:49` (add fields after `adj`)
- Modify: `src/search/query/compile.rs` (build the new lists, add to Query constructor)

**Step 1: Add fields to Query struct (`src/search/query/mod.rs`, after line 49)**

```rust
pub(crate) adj_check: Vec<Vec<(usize, ER::Slot, bool, bool)>>,
pub(crate) adj_pred: Vec<Vec<(usize, ER::Slot, bool, usize)>>,
```

`adj_check` entries: `(neighbor_idx, slot, any_slot, negated)` — structural edges, no predicate.
`adj_pred` entries: `(neighbor_idx, slot, negated, edge_idx)` — edges with predicates.

**Step 2: Build the split lists in compile.rs (after the `adj` construction, before `has_predicates`)**

```rust
let mut adj_check: Vec<Vec<(usize, ER::Slot, bool, bool)>> = vec![Vec::new(); node_count];
let mut adj_pred: Vec<Vec<(usize, ER::Slot, bool, usize)>> = vec![Vec::new(); node_count];

for ni in 0..node_count {
    for &(neighbor_idx, slot, negated, edge_idx) in &adj[ni] {
        if flat.edges[edge_idx].ban_only {
            continue;
        }
        if flat.edge_preds[edge_idx].is_some() {
            adj_pred[ni].push((neighbor_idx, slot, negated, edge_idx));
        } else {
            let any_slot = flat.edges[edge_idx].any_slot;
            adj_check[ni].push((neighbor_idx, slot, any_slot, negated));
        }
    }
}
```

**Step 3: Add fields to Query constructor (at line ~644)**

Add `adj_check,` and `adj_pred,` to the `Query { ... }` construction block.

**Step 4: Verify it compiles**

Run: `cargo check --lib --tests`
Expected: PASS

---

### Task 4: Wire `adj_check` and `adj_pred` into `candidates_for_into` hot path

**Files:**
- Modify: `src/search/engine/seq.rs:210-275` (`candidates_for_into` with anchor)

**Step 1: Widen `can_fast_verify` condition (line 216-218)**

Change:
```rust
let can_fast_verify = ER::SLOT_COUNT == 1
    && !ctx.query.node_has_predicates[pattern_idx]
    && pattern_idx < 64;
```

To (remove `SLOT_COUNT` requirement — structural edges handle multi-slot):
```rust
let can_fast_verify = !ctx.query.node_has_predicates[pattern_idx]
    && pattern_idx < 64;
```

**Step 2: After the bitset adjacency loop (line ~231), add structural check in the filter**

The current filter at lines 236-274 has this structure:
```
.filter(|&raw| {
    // ... degree/morphism/injectivity/node_pred checks ...
    if EP::HAS_FEATURES && !can_fast_verify {
        // check_edge loop over adj[pattern_idx]
    }
    if can_fast_verify {
        // is_adjacent loop over other_mapped
    } else if EP::HAS_FEATURES {
        // is_adjacent loop over other_mapped, then nothing more
    }
    true
})
```

Replace the `EP::HAS_FEATURES && !can_fast_verify` block (lines 255-264) with two separate loops:

```rust
if can_fast_verify {
    for &(neighbor_idx, slot, any_slot, negated) in &ctx.query.adj_check[pattern_idx] {
        let mapped_raw = self.mapping[neighbor_idx];
        if mapped_raw == super::UNMAPPED { continue; }
        if ctx.index.has_edge_in_slot(raw, mapped_raw, slot, any_slot) == negated {
            return false;
        }
    }
}
if EP::HAS_PREDICATES && !can_fast_verify {
    for &(neighbor_idx, slot, negated, edge_idx) in &ctx.query.adj_pred[pattern_idx] {
        let mapped_raw = self.mapping[neighbor_idx];
        if mapped_raw == super::UNMAPPED { continue; }
        let any_slot = ctx.query.edges[edge_idx].any_slot;
        if !ctx.index.check_edge(raw, mapped_raw, slot, any_slot, negated, ctx.query.edge_preds[edge_idx].as_deref()) {
            return false;
        }
    }
}
```

Note: when `can_fast_verify` is true, the bitset loop already handles positive non-negated adjacency via `other_mapped`/`is_adjacent`. The `adj_check` loop handles the remaining structural edges (negated, any_slot, multi-slot). When `can_fast_verify` is false (node has predicates), the full `adj_pred` loop runs via `check_edge`.

**Step 3: Update the `else if EP::HAS_FEATURES` block (line 233-234)**

This block sets `forward_verified_depths` when `HAS_FEATURES` is true. Change to:
```rust
} else if EP::HAS_PREDICATES {
    self.forward_verified_depths |= 1u64 << depth;
}
```

**Step 4: Verify it compiles**

Run: `cargo check --lib --tests`
Expected: PASS

---

### Task 5: Wire `adj_check` and `adj_pred` into `count_leaf_fused` hot path

**Files:**
- Modify: `src/search/engine/seq.rs:385-500` (`count_leaf_fused`)

**Step 1: Widen `can_fast` condition (line 395)**

Change:
```rust
let can_fast = ER::SLOT_COUNT == 1 && !ctx.query.node_has_predicates[leaf_pi] && leaf_pi < 64;
```

To:
```rust
let can_fast = !ctx.query.node_has_predicates[leaf_pi] && leaf_pi < 64;
```

**Step 2: Update the `else if EP::HAS_FEATURES` block for `other_mapped` collection (lines 410-418)**

This block collects `other_mapped` neighbors for the HAS_FEATURES path (skipping negated/ban_only edges). With the new design, when `can_fast` is true, `other_mapped` is collected via bitset (positive edges). Negated/structural edges are checked separately. When `can_fast` is false (has predicates), we still need `other_mapped` for the adjacency pre-filter.

Change `EP::HAS_FEATURES` → `EP::HAS_PREDICATES`:
```rust
} else if EP::HAS_PREDICATES {
    for &(neighbor_idx, _slot, negated, edge_idx) in &ctx.query.adj[leaf_pi] {
        if negated || ctx.query.edges[edge_idx].ban_only { continue; }
        let mapped = self.mapping[neighbor_idx];
        if mapped != super::UNMAPPED && mapped != anchor_raw {
            other_mapped[other_count] = mapped;
            other_count += 1;
        }
    }
}
```

**Step 3: In the `can_fast` candidate loop (lines 443-457), add `adj_check` structural verification**

After the existing `is_adjacent` loop over `other_mapped` (lines 444-451), add:

```rust
if ok {
    for &(neighbor_idx, slot, any_slot, negated) in &ctx.query.adj_check[leaf_pi] {
        let mapped_raw = self.mapping[neighbor_idx];
        if mapped_raw == super::UNMAPPED { continue; }
        if ctx.index.has_edge_in_slot(raw, mapped_raw, slot, any_slot) == negated {
            ok = false;
            break;
        }
    }
}
```

**Step 4: In the `EP::HAS_FEATURES` candidate loop (lines 458-488), replace with `EP::HAS_PREDICATES` and use `adj_pred`**

Change `} else if EP::HAS_FEATURES {` to `} else if EP::HAS_PREDICATES {` at line 458.

Replace the `check_edge` loop (lines 468-476) to iterate `adj_pred` instead of `adj`:

```rust
for &(neighbor_idx, slot, negated, edge_idx) in &ctx.query.adj_pred[leaf_pi] {
    if ctx.query.edges[edge_idx].ban_only { continue; }
    let mapped_raw = self.mapping[neighbor_idx];
    if mapped_raw == super::UNMAPPED { continue; }
    let any_slot = ctx.query.edges[edge_idx].any_slot;
    if !ctx.index.check_edge(raw, mapped_raw, slot, any_slot, negated, ctx.query.edge_preds[edge_idx].as_deref()) {
        ok = false; break;
    }
}
```

Note: `ban_only` check is redundant here since `adj_pred` already excludes `ban_only` edges (Task 3 Step 2 skips them). But keeping it is harmless and defensive.

**Step 5: Verify it compiles**

Run: `cargo check --lib --tests`
Expected: PASS

---

### Task 6: Run full test suite and verify correctness

**Files:** None (testing only)

**Step 1: Run lib tests**

Run: `cargo test --release --lib`
Expected: All tests pass (281+)

**Step 2: Run par_correctness integration test**

Run: `cargo test --release --test par_correctness -- --nocapture`
Expected: All 12 tests pass — seq and par counts match across all direction/valuedness combos.

**Step 3: Run benchmarks to verify no regression**

Run: `cargo run --release --bin bench_search -- --scenario subgraph-in-random --size 1000 --pattern 7 --iters 5 --seed 42`
Expected: No regression vs baseline. Potential improvement for patterns with negated/any_slot edges.

---
