# `modify!` — Mutation

`modify!` applies transactional changes to an existing graph — adding nodes, removing nodes, adding/removing edges, and swapping values — all atomically. It returns a `Modification` describing everything that changed.

## DSL Primitives

| Symbol | Meaning |
|--------|---------|
| `N(id)` | New node (local id for back-references within this modification) |
| `N_()` | New node (auto-assigned local id) |
| `n(id)` | Reference to new node defined earlier in same `modify!` |
| `X(id)` | Existing graph node (by graph node id) |
| `x(id)` | Reference to existing graph node (no value change) |
| `!X(id)` | Remove existing node (and all its edges) |
| `E()` | New edge constructor |
| `e()` | Existing edge reference (for value swap) |
| `!e()` | Remove existing edge |
| `.val(v)` | Set value on node or edge |

## Adding Nodes and Edges

```rust
use grw::graph::{self, MGraph, edge};

let mut g: MGraph<(), edge::Undir<()>> = MGraph::default();

// add two connected nodes
modify!(g, [N(1) ^ N(2)]).unwrap();
assert_eq!(g.node_count(), 2);
assert_eq!(g.edge_count(), 1);
```
<svg viewBox="0 0 237 132" style="max-width:237px;display:block;margin:0.8em auto" role="img" aria-label="add edge"><line x1="90" y1="58" x2="147" y2="58" stroke="#7ee0a3" stroke-width="2.4"/><circle cx="70" cy="58" r="21" fill="#7ee0a326"/><circle cx="70" cy="58" r="15" fill="#7ee0a3"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="167" cy="58" r="21" fill="#7ee0a326"/><circle cx="167" cy="58" r="15" fill="#7ee0a3"/><text x="167" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text></svg>

```rust
// connect new node to existing
modify!(g, [X(0) ^ N(3)]).unwrap();
assert_eq!(g.node_count(), 3);
assert_eq!(g.edge_count(), 2);
```
<svg viewBox="0 0 237 213" style="max-width:237px;display:block;margin:0.8em auto" role="img" aria-label="connect existing"><line x1="88" y1="93" x2="149" y2="61" stroke="#46c6d6" stroke-width="2.4"/><line x1="88" y1="112" x2="149" y2="144" stroke="#7ee0a3" stroke-width="2.4"/><circle cx="70" cy="102" r="21" fill="#f0a63f26"/><circle cx="70" cy="102" r="15" fill="#f0a63f"/><text x="70" y="107" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="167" cy="52" r="21" fill="#f0a63f26"/><circle cx="167" cy="52" r="15" fill="#f0a63f"/><text x="167" y="57" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><circle cx="167" cy="153" r="21" fill="#7ee0a326"/><circle cx="167" cy="153" r="15" fill="#7ee0a3"/><text x="167" y="158" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text></svg>

```rust
let mut g: MGraph<(), edge::Dir<()>> = MGraph::default();

// directed path
modify!(g, [N(1) >> (N(2) >> N(3))]).unwrap();
assert_eq!(g.node_count(), 3);
assert_eq!(g.edge_count(), 2);
```
<svg viewBox="0 0 334 132" style="max-width:334px;display:block;margin:0.8em auto" role="img" aria-label="directed path"><defs><marker id="modify-dir-path-g" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#7ee0a3"/></marker></defs><line x1="90" y1="58" x2="139" y2="58" stroke="#7ee0a3" stroke-width="2.4" marker-end="url(#modify-dir-path-g)"/><line x1="187" y1="58" x2="236" y2="58" stroke="#7ee0a3" stroke-width="2.4" marker-end="url(#modify-dir-path-g)"/><circle cx="70" cy="58" r="21" fill="#7ee0a326"/><circle cx="70" cy="58" r="15" fill="#7ee0a3"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="167" cy="58" r="21" fill="#7ee0a326"/><circle cx="167" cy="58" r="15" fill="#7ee0a3"/><text x="167" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><circle cx="264" cy="58" r="21" fill="#7ee0a326"/><circle cx="264" cy="58" r="15" fill="#7ee0a3"/><text x="264" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text></svg>

```rust
let mut g: MGraph<(), edge::Undir<()>> = MGraph::default();

// triangle via back-reference
modify!(g, [N(1) ^ (N(2) ^ (N(3) ^ n(1)))]).unwrap();
assert_eq!(g.node_count(), 3);
assert_eq!(g.edge_count(), 3);
```
<svg viewBox="0 0 334 171" style="max-width:334px;display:block;margin:0.8em auto" role="img" aria-label="triangle"><line x1="87" y1="101" x2="150" y2="62" stroke="#7ee0a3" stroke-width="2.4"/><line x1="184" y1="62" x2="247" y2="101" stroke="#7ee0a3" stroke-width="2.4"/><line x1="244" y1="111" x2="90" y2="111" stroke="#7ee0a3" stroke-width="2.4"/><circle cx="70" cy="111" r="21" fill="#7ee0a326"/><circle cx="70" cy="111" r="15" fill="#7ee0a3"/><text x="70" y="116" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="167" cy="52" r="21" fill="#7ee0a326"/><circle cx="167" cy="52" r="15" fill="#7ee0a3"/><text x="167" y="57" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><circle cx="264" cy="111" r="21" fill="#7ee0a326"/><circle cx="264" cy="111" r="15" fill="#7ee0a3"/><text x="264" y="116" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text></svg>

### With Values

```rust
let mut g: MGraph<&str, edge::Undir<u32>> = MGraph::default();

// node and edge values
modify!(g, [
    N(1).val("a") & E().val(42u32) ^ N(2).val("b")
]).unwrap();
assert_eq!(g.node_count(), 2);
assert_eq!(g.edge_count(), 1);
```
<svg viewBox="0 0 258 132" style="max-width:258px;display:block;margin:0.8em auto" role="img" aria-label="valued"><line x1="90" y1="58" x2="168" y2="58" stroke="#7ee0a3" stroke-width="2.4"/><circle cx="70" cy="58" r="21" fill="#7ee0a326"/><circle cx="70" cy="58" r="15" fill="#7ee0a3"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><text x="70" y="29" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">"a"</text><circle cx="188" cy="58" r="21" fill="#7ee0a326"/><circle cx="188" cy="58" r="15" fill="#7ee0a3"/><text x="188" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><text x="188" y="29" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">"b"</text><text x="129" y="70" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">42</text></svg>

```rust
// isolated node
modify!(g, [N(3).val("c")]).unwrap();
assert_eq!(g.node_count(), 3);
```
<svg viewBox="0 0 258 218" style="max-width:258px;display:block;margin:0.8em auto" role="img" aria-label="isolated"><line x1="90" y1="158" x2="168" y2="158" stroke="#46c6d6" stroke-width="2.4"/><circle cx="70" cy="158" r="21" fill="#f0a63f26"/><circle cx="70" cy="158" r="15" fill="#f0a63f"/><text x="70" y="163" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><text x="70" y="129" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">"a"</text><circle cx="188" cy="158" r="21" fill="#f0a63f26"/><circle cx="188" cy="158" r="15" fill="#f0a63f"/><text x="188" y="163" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><text x="188" y="129" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">"b"</text><circle cx="70" cy="52" r="21" fill="#7ee0a326"/><circle cx="70" cy="52" r="15" fill="#7ee0a3"/><text x="70" y="57" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text><text x="70" y="23" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">"c"</text><text x="129" y="170" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">42</text></svg>

## Removing Nodes and Edges

```rust
let mut g: MGraph<(), edge::Undir<()>> = MGraph::default();
modify!(g, [N(1) ^ N(2) ^ N(3)]).unwrap();

// remove node 1 (and its edges)
modify!(g, [!X(1)]).unwrap();
assert_eq!(g.node_count(), 2);
```
<svg viewBox="0 0 334 132" style="max-width:334px;display:block;margin:0.8em auto" role="img" aria-label="remove node"><line x1="90" y1="58" x2="147" y2="58" stroke="#e2596e" stroke-width="2.4" stroke-dasharray="6 5"/><line x1="112" y1="51" x2="126" y2="65" stroke="#e2596e" stroke-width="2.4"/><line x1="112" y1="65" x2="126" y2="51" stroke="#e2596e" stroke-width="2.4"/><line x1="187" y1="58" x2="244" y2="58" stroke="#e2596e" stroke-width="2.4" stroke-dasharray="6 5"/><line x1="209" y1="51" x2="223" y2="65" stroke="#e2596e" stroke-width="2.4"/><line x1="209" y1="65" x2="223" y2="51" stroke="#e2596e" stroke-width="2.4"/><circle cx="70" cy="58" r="21" fill="#f0a63f26"/><circle cx="70" cy="58" r="15" fill="#f0a63f"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="167" cy="58" r="19" fill="#e2596e1f"/><circle cx="167" cy="58" r="15" fill="none" stroke="#e2596e" stroke-width="2.2" stroke-dasharray="4 4"/><text x="167" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#e2596e">1</text><circle cx="264" cy="58" r="21" fill="#f0a63f26"/><circle cx="264" cy="58" r="15" fill="#f0a63f"/><text x="264" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text></svg>

```rust
let mut g: MGraph<(), edge::Dir<()>> = MGraph::default();
modify!(g, [N(1) >> N(2)]).unwrap();

// remove edge, keep both nodes
modify!(g, [X(0) & !e() >> x(1)]).unwrap();
assert_eq!(g.node_count(), 2);
assert_eq!(g.edge_count(), 0);
```
<svg viewBox="0 0 237 132" style="max-width:237px;display:block;margin:0.8em auto" role="img" aria-label="remove edge"><defs><marker id="modify-remove-edge-r" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#e2596e"/></marker></defs><line x1="90" y1="58" x2="139" y2="58" stroke="#e2596e" stroke-width="2.4" stroke-dasharray="6 5" marker-end="url(#modify-remove-edge-r)"/><line x1="108" y1="51" x2="122" y2="65" stroke="#e2596e" stroke-width="2.4"/><line x1="108" y1="65" x2="122" y2="51" stroke="#e2596e" stroke-width="2.4"/><circle cx="70" cy="58" r="21" fill="#f0a63f26"/><circle cx="70" cy="58" r="15" fill="#f0a63f"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="167" cy="58" r="21" fill="#f0a63f26"/><circle cx="167" cy="58" r="15" fill="#f0a63f"/><text x="167" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text></svg>

## Updating Values

```rust
let mut g: MGraph<&str, edge::Undir<()>> = MGraph::default();
modify!(g, [N(1).val("old")]).unwrap();
assert_eq!(g.get(0), Some(&"old"));

// swap node value
modify!(g, [X(0).val("new")]).unwrap();
assert_eq!(g.get(0), Some(&"new"));
```
<svg viewBox="0 0 141 132" style="max-width:141px;display:block;margin:0.8em auto" role="img" aria-label="swap value"><circle cx="71" cy="58" r="21" fill="#7ee0a326"/><circle cx="71" cy="58" r="15" fill="#7ee0a3"/><text x="71" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><text x="71" y="29" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">"new"</text></svg>

```rust
let mut g: MGraph<(), edge::Undir<u32>> = MGraph::default();
modify!(g, [N(1) & E().val(100u32) ^ N(2)]).unwrap();

// swap edge value
modify!(g, [X(0) & e().val(200u32) ^ X(1)]).unwrap();
```
<svg viewBox="0 0 262 132" style="max-width:262px;display:block;margin:0.8em auto" role="img" aria-label="swap edge"><line x1="90" y1="58" x2="172" y2="58" stroke="#7ee0a3" stroke-width="2.4"/><circle cx="70" cy="58" r="21" fill="#f0a63f26"/><circle cx="70" cy="58" r="15" fill="#f0a63f"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="192" cy="58" r="21" fill="#f0a63f26"/><circle cx="192" cy="58" r="15" fill="#f0a63f"/><text x="192" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><text x="131" y="70" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">200</text></svg>

## Modification Result

The returned `Modification` struct contains:

- **`new_node_ids`** — mapping from local ids to real graph ids
- **`added_edges`** — edges that were created
- **`removed_nodes`** / **`removed_edges`** — what was deleted
- **`swapped_node_vals`** / **`swapped_edge_vals`** — old values that were replaced

```rust
let mut g: MGraph<(), edge::Undir<()>> = MGraph::default();
let result = modify!(g, [N(1) ^ N(2)]).unwrap();

// the local id 1 was assigned a real graph node id
let real_id = result.new_node_ids[&LocalId(1)];
```

## Color Legend for Diagrams

Throughout this documentation:
- <span style="color:#2171b5">**Blue**</span> — added or changed elements
- <span style="color:#d32f2f">**Red dashed**</span> — removed elements
- **Black** — existing, unchanged elements
