# Watcher Trait Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a unified zero-cost `Watcher` trait to grw for search stepping and mutation tracking, compiled away when unused via `Silent`.

**Architecture:** A single `Watcher<NV, E: Edge>` trait in `grw::watch` covers both search events and mutation events. The sequential matcher and graph mutation code become generic over `W: Watcher`. `Silent` (with `const ACTIVE: bool = false`) monomorphizes away to zero overhead. A `WatchedGraph` wrapper passes the same watcher to both search and mutation paths.

**Tech Stack:** Rust, existing grw crate internals

**Spec:** `docs/plans/2026-03-14-visualizer-protocol-design.md`

---

## File Structure

```
src/watch/
  mod.rs          — Watcher trait, Control, BanVerdict, Silent, WatchedGraph
```

Single file — trait + types + wrapper are small enough to stay together.

**Modified files:**
- `src/lib.rs:134` — add `pub mod watch;`
- `src/graph/mod.rs:3` — change `pub(crate) mod dsl;` to `pub mod dsl;`
- `src/search/engine/seq.rs` — add watcher generic to `advance()`, `Iter`, new `WatchedIter`
- `src/search/engine/mod.rs` — add watcher to `is_feasible`, `check_ban_clusters`
- `src/modify/apply.rs` — extend `Modification` with edge IDs and added edges

---

## Chunk 1: Prerequisites and Watcher Trait

### Task 1: Make `dsl` module public and extend `Modification`

**Files:**
- Modify: `src/graph/mod.rs:3`
- Modify: `src/modify/apply.rs:11-29`

- [ ] **Step 1: Change dsl visibility**

In `src/graph/mod.rs` line 3, change:
```rust
pub(crate) mod dsl;
```
to:
```rust
pub mod dsl;
```

- [ ] **Step 2: Extend `Modification` with edge IDs and added edges**

In `src/modify/apply.rs`, change:
```rust
pub struct Modification<NV, ER: graph::Edge> {
    pub new_node_ids: FxHashMap<LocalId, id::N>,
    pub removed_nodes: Vec<(id::N, NV)>,
    pub removed_edges: Vec<(NR<id::N>, ER::Slot, ER::Val)>,
    pub swapped_node_vals: Vec<(id::N, NV)>,
    pub swapped_edge_vals: Vec<(NR<id::N>, ER::Slot, ER::Val)>,
}
```
to:
```rust
pub struct Modification<NV, ER: graph::Edge> {
    pub new_node_ids: FxHashMap<LocalId, id::N>,
    pub added_edges: Vec<(id::E, id::N, id::N, ER::Slot)>,
    pub removed_nodes: Vec<(id::N, NV)>,
    pub removed_edges: Vec<(id::E, NR<id::N>, ER::Slot, ER::Val)>,
    pub swapped_node_vals: Vec<(id::N, NV)>,
    pub swapped_edge_vals: Vec<(id::E, NR<id::N>, ER::Slot, ER::Val)>,
}
```

Update `Default` impl to add `added_edges: Vec::new()`.

- [ ] **Step 3: Update `apply_ops` to populate new fields**

In `apply_ops`, update each mutation site:

Edge removal (~line 321):
```rust
result.removed_edges.push((eid, nr, rec.slot, rec.val));
```

Edge swap (~line 363):
```rust
result.swapped_edge_vals.push((eid, nr, stored_slot, old_val));
```

Edge addition (~line 388), after `self.edges.count += 1;`:
```rust
result.added_edges.push((eid, n1, n2, stored_slot));
```

Cascade edge removal during node removal (~line 416):
```rust
result.removed_edges.push((eid, nr, rec.slot, rec.val));
```

- [ ] **Step 4: Fix all destructuring sites**

Search the crate for code that destructures `removed_edges` or `swapped_edge_vals` tuples and add the `id::E` field. Run `cargo check` iteratively until clean.

- [ ] **Step 5: Verify**

Run: `cargo check && cargo test`

- [ ] **Step 6: Commit**

```
git add src/graph/mod.rs src/modify/apply.rs
git commit -m "make dsl public, extend Modification with edge IDs"
```

---

### Task 2: Create `watch` module with Watcher trait

**Files:**
- Create: `src/watch/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create `src/watch/mod.rs`**

```rust
use crate::id;
use crate::graph::dsl::LocalId;
use crate::graph::Edge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Continue,
    Pause,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BanVerdict {
    SharedEdgeFail,
    NoCandidate,
    Fired,
}

pub trait Watcher<NV, E: Edge> {
    const ACTIVE: bool = true;

    fn on_bind(&mut self, step: usize, pattern_node: LocalId, graph_node: id::N) -> Control;
    fn on_unbind(&mut self, step: usize, pattern_node: LocalId);
    fn on_edge_test(
        &mut self,
        pattern_edge: usize,
        src: id::N,
        tgt: id::N,
        exists: bool,
        pred_pass: bool,
        negated: bool,
    ) -> Control;
    fn on_ban_verdict(&mut self, cluster: usize, verdict: BanVerdict) -> Control;
    fn on_match(&mut self, mapping: &[(LocalId, id::N)]) -> Control;

    fn on_node_added(&mut self, id: id::N, val: &NV);
    fn on_node_removed(&mut self, id: id::N);
    fn on_node_changed(&mut self, id: id::N, val: &NV);
    fn on_edge_added(&mut self, id: id::E, n1: id::N, n2: id::N, slot: &E::Slot, val: &E::Val);
    fn on_edge_removed(&mut self, id: id::E);
    fn on_edge_changed(&mut self, id: id::E, val: &E::Val);
}

pub struct Silent;

impl<NV, E: Edge> Watcher<NV, E> for Silent {
    const ACTIVE: bool = false;

    #[inline(always)]
    fn on_bind(&mut self, _step: usize, _pn: LocalId, _gn: id::N) -> Control { Control::Continue }
    #[inline(always)]
    fn on_unbind(&mut self, _step: usize, _pn: LocalId) {}
    #[inline(always)]
    fn on_edge_test(&mut self, _pe: usize, _src: id::N, _tgt: id::N, _e: bool, _p: bool, _n: bool) -> Control { Control::Continue }
    #[inline(always)]
    fn on_ban_verdict(&mut self, _cluster: usize, _verdict: BanVerdict) -> Control { Control::Continue }
    #[inline(always)]
    fn on_match(&mut self, _mapping: &[(LocalId, id::N)]) -> Control { Control::Continue }

    #[inline(always)]
    fn on_node_added(&mut self, _id: id::N, _val: &NV) {}
    #[inline(always)]
    fn on_node_removed(&mut self, _id: id::N) {}
    #[inline(always)]
    fn on_node_changed(&mut self, _id: id::N, _val: &NV) {}
    #[inline(always)]
    fn on_edge_added(&mut self, _id: id::E, _n1: id::N, _n2: id::N, _slot: &E::Slot, _val: &E::Val) {}
    #[inline(always)]
    fn on_edge_removed(&mut self, _id: id::E) {}
    #[inline(always)]
    fn on_edge_changed(&mut self, _id: id::E, _val: &E::Val) {}
}

pub struct WatchedGraph<'a, NV, E: Edge, W: Watcher<NV, E>> {
    pub(crate) graph: &'a mut crate::graph::Graph<NV, E>,
    pub(crate) watcher: &'a mut W,
}

impl<'a, NV: Sync, E: Edge, W: Watcher<NV, E>> WatchedGraph<'a, NV, E, W> {
    pub fn graph(&self) -> &crate::graph::Graph<NV, E> {
        self.graph
    }

    pub fn apply(
        &mut self,
        fragment: crate::modify::Fragment<NV, E, crate::modify::Checked>,
    ) -> Result<crate::modify::Modification<NV, E>, crate::modify::error::Apply> {
        let result = self.graph.apply(fragment)?;
        self.notify(&result);
        Ok(result)
    }

    pub fn modify(
        &mut self,
        ops: Vec<crate::modify::Node<NV, E>>,
    ) -> Result<crate::modify::Modification<NV, E>, crate::modify::error::Modify> {
        let result = self.graph.modify(ops)?;
        self.notify(&result);
        Ok(result)
    }

    fn notify(&mut self, result: &crate::modify::Modification<NV, E>) {
        for (&_local, &real_id) in &result.new_node_ids {
            if let Some(val) = self.graph.get(real_id) {
                self.watcher.on_node_added(real_id, val);
            }
        }

        for &(nid, ref _old_val) in &result.swapped_node_vals {
            if let Some(val) = self.graph.get(nid) {
                self.watcher.on_node_changed(nid, val);
            }
        }

        for &(eid, n1, n2, ref slot) in &result.added_edges {
            if let Some(val) = self.graph.edge_val_by_id(eid) {
                self.watcher.on_edge_added(eid, n1, n2, slot, val);
            }
        }

        for &(eid, ref _nr, ref _slot, ref _val) in &result.removed_edges {
            self.watcher.on_edge_removed(eid);
        }

        for &(nid, ref _old_val) in &result.removed_nodes {
            self.watcher.on_node_removed(nid);
        }
    }

    pub fn watcher(&mut self) -> &mut W {
        self.watcher
    }
}

#[cfg(test)]
mod tests;
```

Note: `edge_val_by_id(eid)` may not exist on `Graph` yet. If not, add it (simple: `self.edges.store[*eid as usize].as_ref().map(|r| &r.val)`). Alternatively, skip the edge value lookup for `on_edge_added` and rely on the caller knowing the value — but the watcher trait expects `&E::Val`.

- [ ] **Step 2: Add `watched` method to Graph**

In `src/graph/mod.rs`, add to the main `impl<NV, ER: Edge> Graph<NV, ER>` block:
```rust
pub fn watched<'a, W: crate::watch::Watcher<NV, ER>>(
    &'a mut self,
    watcher: &'a mut W,
) -> crate::watch::WatchedGraph<'a, NV, ER, W> {
    crate::watch::WatchedGraph { graph: self, watcher }
}
```

- [ ] **Step 3: Add `edge_val_by_id` to Graph if needed**

If `Graph` doesn't have edge value access by `id::E`, add:
```rust
pub fn edge_val_by_id(&self, eid: id::E) -> Option<&ER::Val> {
    self.edges.store.get(*eid as usize)
        .and_then(|opt| opt.as_ref())
        .map(|rec| &rec.val)
}
```

- [ ] **Step 4: Add module to `src/lib.rs`**

After `pub mod search;` (line 134):
```rust
pub mod watch;
```

And add re-exports:
```rust
pub use watch::{Watcher, Control, BanVerdict, Silent};
```

- [ ] **Step 5: Create empty `src/watch/tests.rs`**

```rust
```

- [ ] **Step 6: Verify**

Run: `cargo check`

- [ ] **Step 7: Commit**

```
git add src/watch/ src/lib.rs src/graph/mod.rs
git commit -m "add watch module with unified Watcher trait, WatchedGraph"
```

---

## Chunk 2: Watcher Integration into Sequential Matcher

### Task 3: Thread watcher through `advance()` and helpers

**Files:**
- Modify: `src/search/engine/seq.rs`
- Modify: `src/search/engine/mod.rs`

- [ ] **Step 1: Add watcher parameter to `advance()` signature**

In `src/search/engine/seq.rs`, change `advance` (~line 515):

From:
```rust
pub(crate) fn advance<
    NV, ER: graph::Edge, EP: feature::Edge, NP: feature::Negation,
    BP: feature::Ban, EM: feature::Emit, I: Index<NV, ER>,
>(&mut self, ctx: &Ctx<'_, NV, ER, I>) -> (Option<Match>, usize)
```
To:
```rust
pub(crate) fn advance<
    NV, ER: graph::Edge, EP: feature::Edge, NP: feature::Negation,
    BP: feature::Ban, EM: feature::Emit, I: Index<NV, ER>,
    W: crate::watch::Watcher<NV, ER>,
>(&mut self, ctx: &Ctx<'_, NV, ER, I>, watcher: &mut W) -> (Option<Match>, usize)
```

- [ ] **Step 2: Insert `on_bind` after mapping commitment**

After line 609 (`self.reverse.set(*candidate, pattern_idx as u32);`), compute local_id and fire:
```rust
let local_id = ctx.query.nodes[pattern_idx].local_id;
match watcher.on_bind(depth, local_id, id::N(*candidate)) {
    crate::watch::Control::Stop => {
        self.exhausted = true;
        return (None, total);
    }
    _ => {}
}
```

Move the `let local_id = ...` computation right after `let pattern_idx = self.search_order[depth];` (line 578) so it's available for all callback sites below.

- [ ] **Step 3: Insert `on_unbind` at all unbinding sites**

**Backtracking on exhaustion** (after line 567, clearing prev mapping):
```rust
let prev_local_id = ctx.query.nodes[prev_pattern_idx].local_id;
watcher.on_unbind(depth - 1, prev_local_id);
```

**Lookahead failure** (after line 614):
```rust
watcher.on_unbind(depth, local_id);
```

**After match emission** (after line 628):
```rust
watcher.on_unbind(depth, local_id);
```

**After final-depth rejection** (after line 633):
```rust
watcher.on_unbind(depth, local_id);
```

**After count-only leaf cleanup** (after line 679):
```rust
watcher.on_unbind(depth, local_id);
```

- [ ] **Step 4: Insert `on_match` callback**

Replace the match emission (lines 626-629):
```rust
let result = self.build_match(ctx);
self.mapping[pattern_idx] = super::UNMAPPED;
self.reverse.clear(*candidate);
return (Some(result), 0);
```
with:
```rust
let result = self.build_match(ctx);
let control = watcher.on_match(&result.0);
self.mapping[pattern_idx] = super::UNMAPPED;
self.reverse.clear(*candidate);
watcher.on_unbind(depth, local_id);
match control {
    crate::watch::Control::Stop => {
        self.exhausted = true;
        return (None, total);
    }
    _ => return (Some(result), 0),
}
```

- [ ] **Step 5: Thread watcher through `check_ban_clusters`**

Change signature to accept watcher, fire `on_ban_verdict` when a ban fires:
```rust
fn check_ban_clusters<NV, ER: graph::Edge, I: Index<NV, ER>, W: crate::watch::Watcher<NV, ER>>(
    &self, ctx: &Ctx<'_, NV, ER, I>, watcher: &mut W,
) -> bool {
    for (idx, ban) in ctx.query.ban_clusters.iter().enumerate() {
        if self.ban_cluster_satisfiable(ctx, ban) {
            watcher.on_ban_verdict(idx, crate::watch::BanVerdict::Fired);
            return false;
        }
    }
    true
}
```

Update call site in `advance()` (~line 621):
```rust
&& (!BP::ACTIVE || self.check_ban_clusters(ctx, watcher))
```

- [ ] **Step 6: Add watcher to `is_feasible` for `on_edge_test`**

In `src/search/engine/mod.rs`, change `is_feasible` to accept watcher:
```rust
pub(crate) fn is_feasible<NV, ER: graph::Edge, I: Index<NV, ER>, W: crate::watch::Watcher<NV, ER>>(
    &self, ctx: &Ctx<'_, NV, ER, I>, pattern_idx: usize, candidate: id::N, watcher: &mut W,
) -> bool
```

Gate the fast-path optimization on `!W::ACTIVE`:
```rust
if !W::ACTIVE && ER::SLOT_COUNT == 1 && !ctx.query.node_has_predicates[pattern_idx] && !ctx.query.node_has_neg_adj[pattern_idx] && pattern_idx < 64 {
    return self.is_feasible_fast(ctx, pattern_idx, candidate_id);
}
```

In the adjacency check loop (~line 1393-1411), after `check_edge`, fire observer:
```rust
let mapped_id = self.mapping[neighbor_idx];
if mapped_id != UNMAPPED {
    let result = ctx.index.check_edge(candidate_id, mapped_id, slot, any_slot, effective_negated, ctx.query.edge_preds[edge_idx].as_deref());
    if W::ACTIVE {
        let src = id::N(candidate_id as crate::Id);
        let tgt = id::N(mapped_id as crate::Id);
        watcher.on_edge_test(edge_idx, src, tgt, result || effective_negated, result, effective_negated);
    }
    if !result { return false; }
}
```

The `if W::ACTIVE` guard is evaluated at monomorphization time — when `W = Silent`, the entire block is eliminated.

Update all call sites of `is_feasible` in `advance()` to pass `watcher`. The `is_feasible_fast` and `is_feasible_reverse_only` paths don't need watcher — they're only taken when `!W::ACTIVE`.

- [ ] **Step 7: Update `dispatch_advance!` macro**

```rust
macro_rules! dispatch_advance {
    ($state:expr, $ctx:expr, $emit:ty, $watcher:expr) => {
        match (
            $ctx.query.has_predicates,
            $ctx.query.has_negated_connected,
            $ctx.query.has_ban_clusters,
        ) {
            (false, false, false) => $state.advance::<_, _, feature::PlainEdges, feature::NoNegation, feature::NoBans, $emit, _, _>($ctx, $watcher),
            (false, false, true)  => $state.advance::<_, _, feature::PlainEdges, feature::NoNegation, feature::WithBans, $emit, _, _>($ctx, $watcher),
            (false, true, false)  => $state.advance::<_, _, feature::PlainEdges, feature::WithNegation, feature::NoBans, $emit, _, _>($ctx, $watcher),
            (false, true, true)   => $state.advance::<_, _, feature::PlainEdges, feature::WithNegation, feature::WithBans, $emit, _, _>($ctx, $watcher),
            (true, false, false)  => $state.advance::<_, _, feature::PredEdges, feature::NoNegation, feature::NoBans, $emit, _, _>($ctx, $watcher),
            (true, false, true)   => $state.advance::<_, _, feature::PredEdges, feature::NoNegation, feature::WithBans, $emit, _, _>($ctx, $watcher),
            (true, true, false)   => $state.advance::<_, _, feature::PredEdges, feature::WithNegation, feature::NoBans, $emit, _, _>($ctx, $watcher),
            (true, true, true)    => $state.advance::<_, _, feature::PredEdges, feature::WithNegation, feature::WithBans, $emit, _, _>($ctx, $watcher),
        }
    };
}
```

- [ ] **Step 8: Update `Iter`, `IntoIter`, `OwnedIter` to use `Silent`**

Add `watcher: crate::watch::Silent` field to `Iter`:
```rust
pub struct Iter<'g, NV, ER: graph::Edge> {
    ctx: Ctx<'g, NV, ER, super::Graph<'g, NV, ER>>,
    state: State<Vec<u32>>,
    watcher: crate::watch::Silent,
}
```

Update `Iterator::next()` and `count()`:
```rust
fn next(&mut self) -> Option<Match> {
    dispatch_advance!(self.state, &self.ctx, feature::Collect, &mut self.watcher).0
}
fn count(mut self) -> usize {
    dispatch_advance!(self.state, &self.ctx, feature::Count, &mut self.watcher).1
}
```

Initialize `watcher: crate::watch::Silent` in `new()` and `new_bound()`. Same for `IntoIter` and `OwnedIter`.

- [ ] **Step 9: Add `WatchedIter` and `Seq::search_watched`**

```rust
pub struct WatchedIter<'g, NV, ER: graph::Edge, W: crate::watch::Watcher<NV, ER>> {
    ctx: Ctx<'g, NV, ER, super::Graph<'g, NV, ER>>,
    state: State<Vec<u32>>,
    watcher: W,
}

impl<'g, NV: Clone, ER: graph::Edge, W: crate::watch::Watcher<NV, ER>> Iterator for WatchedIter<'g, NV, ER, W>
where ER::Val: Clone,
{
    type Item = Match;
    fn next(&mut self) -> Option<Match> {
        dispatch_advance!(self.state, &self.ctx, feature::Collect, &mut self.watcher).0
    }
}

impl Seq {
    pub fn search_watched<'g, NV: Clone + 'g, ER: graph::Edge + 'g, W: crate::watch::Watcher<NV, ER>>(
        plan: &'g SeqPlan<NV, ER>,
        target: &'g super::Graph<'g, NV, ER>,
        watcher: W,
    ) -> WatchedIter<'g, NV, ER, W>
    where ER::Val: Clone,
    {
        let ctx = Ctx { query: &plan.query, index: target, target: target.graph };
        let state = State::new(&plan.query, target.graph, target, Vec::new());
        WatchedIter { ctx, state, watcher }
    }
}
```

- [ ] **Step 10: Verify**

Run: `cargo check`

- [ ] **Step 11: Run full test suite**

Run: `cargo test`
Expected: All existing tests pass.

- [ ] **Step 12: Commit**

```
git add src/search/engine/seq.rs src/search/engine/mod.rs
git commit -m "thread Watcher through sequential matcher"
```

---

## Chunk 3: Tests

### Task 4: Search watcher tests

**Files:**
- Create: `src/watch/tests.rs`

- [ ] **Step 1: Write recording watcher helper**

```rust
use super::*;
use crate::id;
use crate::graph::dsl::LocalId;
use crate::graph::edge;

#[derive(Default)]
struct Recorder {
    binds: Vec<(usize, LocalId, id::N)>,
    unbinds: Vec<(usize, LocalId)>,
    matches: Vec<Vec<(LocalId, id::N)>>,
    edge_tests: Vec<(usize, id::N, id::N, bool)>,
    stop_after: Option<usize>,
}

impl Watcher<(), edge::Undir<()>> for Recorder {
    const ACTIVE: bool = true;

    fn on_bind(&mut self, step: usize, pn: LocalId, gn: id::N) -> Control {
        self.binds.push((step, pn, gn));
        Control::Continue
    }
    fn on_unbind(&mut self, step: usize, pn: LocalId) {
        self.unbinds.push((step, pn));
    }
    fn on_edge_test(&mut self, pe: usize, src: id::N, tgt: id::N, exists: bool, _pp: bool, _neg: bool) -> Control {
        self.edge_tests.push((pe, src, tgt, exists));
        Control::Continue
    }
    fn on_ban_verdict(&mut self, _c: usize, _v: BanVerdict) -> Control { Control::Continue }
    fn on_match(&mut self, mapping: &[(LocalId, id::N)]) -> Control {
        self.matches.push(mapping.to_vec());
        if let Some(limit) = self.stop_after {
            if self.matches.len() >= limit {
                return Control::Stop;
            }
        }
        Control::Continue
    }

    fn on_node_added(&mut self, _id: id::N, _val: &()) {}
    fn on_node_removed(&mut self, _id: id::N) {}
    fn on_node_changed(&mut self, _id: id::N, _val: &()) {}
    fn on_edge_added(&mut self, _id: id::E, _n1: id::N, _n2: id::N, _s: &_, _v: &()) {}
    fn on_edge_removed(&mut self, _id: id::E) {}
    fn on_edge_changed(&mut self, _id: id::E, _val: &()) {}
}
```

- [ ] **Step 2: Test silent matches normal search**

Build a small graph and pattern using the existing test infrastructure. Check `src/search/engine/tests/` for patterns to follow. Core shape:

```rust
#[test]
fn silent_matches_normal() {
    // Build path graph: 0—1—2
    // Pattern: edge (mono)
    // Compare Seq::search vs Seq::search_watched with Silent
    // Assert same match count
}
```

Use the actual grw graph construction API (check `src/build/` and existing tests for exact usage).

- [ ] **Step 3: Test watcher receives bind/unbind events**

```rust
#[test]
fn watcher_receives_events() {
    // Same graph + pattern
    let recorder = Recorder::default();
    let matches: Vec<_> = Seq::search_watched(&plan, &idx, recorder).collect();

    // WatchedIter owns recorder — need to get it back.
    // Either: change WatchedIter to take &mut W, or add into_watcher() method.
    // For testing, take &mut:
    //   let mut recorder = Recorder::default();
    //   ... search_watched(&plan, &idx, &mut recorder) ...
    //
    // Adjust Seq::search_watched signature if needed to take &mut W.
    // Alternatively: WatchedIter can expose pub fn watcher(&self) -> &W

    assert!(!recorder.binds.is_empty());
    assert!(!recorder.unbinds.is_empty());
}
```

Note on ownership: if `search_watched` takes `W` by value, the test can't inspect the recorder after iteration. Options:
- Take `&mut W` instead (preferred for testing)
- Add `WatchedIter::into_watcher(self) -> W`
- Use `RefCell` in tests

Decide during implementation. The simplest is `&mut W`.

- [ ] **Step 4: Test Control::Stop terminates early**

```rust
#[test]
fn stop_terminates_early() {
    // Build complete graph K4 (6 undirected edges)
    // Pattern: single edge, mono — many matches
    let mut recorder = Recorder { stop_after: Some(2), ..Default::default() };
    let matches: Vec<_> = Seq::search_watched(&plan, &idx, &mut recorder).collect();
    assert!(matches.len() <= 2);
}
```

- [ ] **Step 5: Verify**

Run: `cargo test watch`

- [ ] **Step 6: Commit**

```
git add src/watch/tests.rs
git commit -m "add Watcher search integration tests"
```

---

### Task 5: Mutation watcher tests

**Files:**
- Modify: `src/watch/tests.rs`

- [ ] **Step 1: Write mutation recording watcher**

```rust
#[derive(Default)]
struct MutRecorder {
    nodes_added: Vec<id::N>,
    nodes_removed: Vec<id::N>,
    edges_added: Vec<id::E>,
    edges_removed: Vec<id::E>,
}

impl Watcher<(), edge::Undir<()>> for MutRecorder {
    const ACTIVE: bool = true;

    // search methods — no-op
    fn on_bind(&mut self, _: usize, _: LocalId, _: id::N) -> Control { Control::Continue }
    fn on_unbind(&mut self, _: usize, _: LocalId) {}
    fn on_edge_test(&mut self, _: usize, _: id::N, _: id::N, _: bool, _: bool, _: bool) -> Control { Control::Continue }
    fn on_ban_verdict(&mut self, _: usize, _: BanVerdict) -> Control { Control::Continue }
    fn on_match(&mut self, _: &[(LocalId, id::N)]) -> Control { Control::Continue }

    // mutation methods
    fn on_node_added(&mut self, id: id::N, _val: &()) { self.nodes_added.push(id); }
    fn on_node_removed(&mut self, id: id::N) { self.nodes_removed.push(id); }
    fn on_node_changed(&mut self, _id: id::N, _val: &()) {}
    fn on_edge_added(&mut self, id: id::E, _: id::N, _: id::N, _: &_, _: &()) { self.edges_added.push(id); }
    fn on_edge_removed(&mut self, id: id::E) { self.edges_removed.push(id); }
    fn on_edge_changed(&mut self, _: id::E, _: &()) {}
}
```

- [ ] **Step 2: Test node add fires watcher**

```rust
#[test]
fn watcher_fires_on_node_add() {
    let mut g: crate::Graph<(), edge::Undir<()>> = crate::Graph::new();
    let mut rec = MutRecorder::default();
    {
        let mut wg = g.watched(&mut rec);
        wg.modify(vec![crate::modify::N_(()).into()]).unwrap();
    }
    assert_eq!(rec.nodes_added.len(), 1);
}
```

Follow existing `modify` test patterns in `src/modify/tests.rs` for the correct DSL usage.

- [ ] **Step 3: Test edge add and node remove fires watcher**

```rust
#[test]
fn watcher_fires_on_edge_add_and_node_remove() {
    // Build graph with 2 nodes + 1 edge via WatchedGraph
    // Assert edges_added fires
    // Remove a node
    // Assert nodes_removed and edges_removed fire
}
```

- [ ] **Step 4: Verify**

Run: `cargo test watch`

- [ ] **Step 5: Commit**

```
git add src/watch/tests.rs
git commit -m "add Watcher mutation integration tests"
```

---

### Task 6: Public API re-exports

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: Ensure re-exports**

Verify `src/lib.rs` has:
```rust
pub mod watch;
pub use watch::{Watcher, Control, BanVerdict, Silent};
```

Verify `Seq::search_watched` and `WatchedIter` are accessible through `crate::search::*` or `crate::Seq`.

- [ ] **Step 2: Run full test suite**

Run: `cargo test`

- [ ] **Step 3: Commit**

```
git add src/lib.rs
git commit -m "expose Watcher API publicly"
```
