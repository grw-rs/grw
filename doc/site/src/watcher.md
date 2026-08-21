# Watcher

The `Watcher` trait provides an observer pattern for graph mutations and search events. Implement it to receive callbacks when the graph changes or when the search engine makes decisions.

## The Watcher Trait

```rust
pub trait Watcher<NV, E: Edge> {
    const ACTIVE: bool = true;

    // Search events
    fn on_bind(&mut self, step: usize, pattern_node: LocalId, graph_node: id::N) -> Control;
    fn on_unbind(&mut self, step: usize, pattern_node: LocalId);
    fn on_edge_test(&mut self, ...) -> Control;
    fn on_ban_verdict(&mut self, cluster: usize, verdict: BanVerdict) -> Control;
    fn on_match(&mut self, mapping: &[(LocalId, id::N)]) -> Control;

    // Mutation events
    fn on_node_added(&mut self, id: id::N, val: &NV);
    fn on_node_removed(&mut self, id: id::N);
    fn on_node_changed(&mut self, id: id::N, val: &NV);
    fn on_edge_added(&mut self, id: id::E, n1: id::N, n2: id::N, slot: &E::Slot, val: &E::Val);
    fn on_edge_removed(&mut self, id: id::E);
    fn on_edge_changed(&mut self, id: id::E, val: &E::Val);
}
```

## Control Flow

Watcher callbacks return `Control`:
- **`Control::Continue`** — keep going
- **`Control::Pause`** — pause (for interactive debugging)
- **`Control::Stop`** — abort the search

## Silent Watcher

`Silent` is a no-op implementation with `ACTIVE: bool = false`. The compiler optimizes all callbacks away when `Silent` is used:

```rust
use grw::watch::Silent;
// Silent is used by default — zero overhead
```

## WatchedGraph

To observe mutations, wrap the graph with a watcher:

```rust
let mut watcher = MyWatcher::new();
let mut watched = g.watched(&mut watcher);

// mutations through watched notify the watcher
watched.modify(ops).unwrap();

// watcher received on_node_added, on_edge_added, etc.
```

## Ban Verdicts

During search, ban clusters report their outcomes:

- **`BanVerdict::Fired`** — the ban pattern matched, rejecting the candidate
- **`BanVerdict::SharedEdgeFail`** — shared edge check eliminated the candidate
- **`BanVerdict::NoCandidate`** — no valid mapping for the ban pattern
