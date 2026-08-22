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
use grw::graph::{self, Graph, edge};

let mut g: Graph<(), edge::Undir<()>> = Graph::default();

// add two connected nodes
modify!(g, [N(1) ^ N(2)]).unwrap();
assert_eq!(g.node_count(), 2);
assert_eq!(g.edge_count(), 1);
```
<svg viewBox="0 0 203 120" style="max-width:203px;display:block;margin:0.8em auto" role="img" aria-label="add edge"><line x1="74" y1="50" x2="129" y2="50" stroke="#7ee0a3" stroke-width="2.2"/><circle cx="58" cy="50" r="16" fill="#7ee0a322"/><circle cx="58" cy="50" r="11" fill="#7ee0a3"/><text x="58" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="145" cy="50" r="16" fill="#7ee0a322"/><circle cx="145" cy="50" r="11" fill="#7ee0a3"/><text x="145" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text></svg>

```rust
// connect new node to existing
modify!(g, [X(0) ^ N(3)]).unwrap();
assert_eq!(g.node_count(), 3);
assert_eq!(g.edge_count(), 2);
```
<svg viewBox="0 0 203 182" style="max-width:203px;display:block;margin:0.8em auto" role="img" aria-label="connect existing"><line x1="72" y1="80" x2="130" y2="51" stroke="#46c6d6" stroke-width="2.2"/><line x1="72" y1="94" x2="130" y2="123" stroke="#7ee0a3" stroke-width="2.2"/><circle cx="58" cy="87" r="16" fill="#f0a63f22"/><circle cx="58" cy="87" r="11" fill="#f0a63f"/><text x="58" y="91" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="145" cy="44" r="16" fill="#f0a63f22"/><circle cx="145" cy="44" r="11" fill="#f0a63f"/><text x="145" y="48" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><circle cx="145" cy="130" r="16" fill="#7ee0a322"/><circle cx="145" cy="130" r="11" fill="#7ee0a3"/><text x="145" y="134" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">2</text></svg>

```rust
let mut g: Graph<(), edge::Dir<()>> = Graph::default();

// directed path
modify!(g, [N(1) >> (N(2) >> N(3))]).unwrap();
assert_eq!(g.node_count(), 3);
assert_eq!(g.edge_count(), 2);
```
<svg viewBox="0 0 286 120" style="max-width:286px;display:block;margin:0.8em auto" role="img" aria-label="directed path"><defs><marker id="modify-dir-path-g" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#7ee0a3"/></marker></defs><line x1="74" y1="50" x2="120" y2="50" stroke="#7ee0a3" stroke-width="2.2" marker-end="url(#modify-dir-path-g)"/><line x1="159" y1="50" x2="205" y2="50" stroke="#7ee0a3" stroke-width="2.2" marker-end="url(#modify-dir-path-g)"/><circle cx="58" cy="50" r="16" fill="#7ee0a322"/><circle cx="58" cy="50" r="11" fill="#7ee0a3"/><text x="58" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="143" cy="50" r="16" fill="#7ee0a322"/><circle cx="143" cy="50" r="11" fill="#7ee0a3"/><text x="143" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><circle cx="228" cy="50" r="16" fill="#7ee0a322"/><circle cx="228" cy="50" r="11" fill="#7ee0a3"/><text x="228" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">2</text></svg>

```rust
let mut g: Graph<(), edge::Undir<()>> = Graph::default();

// triangle via back-reference
modify!(g, [N(1) ^ (N(2) ^ (N(3) ^ n(1)))]).unwrap();
assert_eq!(g.node_count(), 3);
assert_eq!(g.edge_count(), 3);
```
<svg viewBox="0 0 286 146" style="max-width:286px;display:block;margin:0.8em auto" role="img" aria-label="triangle"><line x1="72" y1="86" x2="129" y2="52" stroke="#7ee0a3" stroke-width="2.2"/><line x1="157" y1="52" x2="214" y2="86" stroke="#7ee0a3" stroke-width="2.2"/><line x1="212" y1="94" x2="74" y2="94" stroke="#7ee0a3" stroke-width="2.2"/><circle cx="58" cy="94" r="16" fill="#7ee0a322"/><circle cx="58" cy="94" r="11" fill="#7ee0a3"/><text x="58" y="98" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="143" cy="44" r="16" fill="#7ee0a322"/><circle cx="143" cy="44" r="11" fill="#7ee0a3"/><text x="143" y="48" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><circle cx="228" cy="94" r="16" fill="#7ee0a322"/><circle cx="228" cy="94" r="11" fill="#7ee0a3"/><text x="228" y="98" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">2</text></svg>

### With Values

```rust
let mut g: Graph<&str, edge::Undir<u32>> = Graph::default();

// node and edge values
modify!(g, [
    N(1).val("a") & E().val(42u32) ^ N(2).val("b")
]).unwrap();
assert_eq!(g.node_count(), 2);
assert_eq!(g.edge_count(), 1);
```
<svg viewBox="0 0 221 120" style="max-width:221px;display:block;margin:0.8em auto" role="img" aria-label="valued"><line x1="74" y1="50" x2="147" y2="50" stroke="#7ee0a3" stroke-width="2.2"/><circle cx="58" cy="50" r="16" fill="#7ee0a322"/><circle cx="58" cy="50" r="11" fill="#7ee0a3"/><text x="58" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><text x="58" y="28" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">"a"</text><circle cx="163" cy="50" r="16" fill="#7ee0a322"/><circle cx="163" cy="50" r="11" fill="#7ee0a3"/><text x="163" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><text x="163" y="28" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">"b"</text><text x="110" y="61" text-anchor="middle" font-family="monospace" font-size="10.5" fill="#8b95a9">42</text></svg>

```rust
// isolated node
modify!(g, [N(3).val("c")]).unwrap();
assert_eq!(g.node_count(), 3);
```
<svg viewBox="0 0 221 187" style="max-width:221px;display:block;margin:0.8em auto" role="img" aria-label="isolated"><line x1="74" y1="135" x2="147" y2="135" stroke="#46c6d6" stroke-width="2.2"/><circle cx="58" cy="135" r="16" fill="#f0a63f22"/><circle cx="58" cy="135" r="11" fill="#f0a63f"/><text x="58" y="139" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><text x="58" y="113" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">"a"</text><circle cx="163" cy="135" r="16" fill="#f0a63f22"/><circle cx="163" cy="135" r="11" fill="#f0a63f"/><text x="163" y="139" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><text x="163" y="113" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">"b"</text><circle cx="58" cy="44" r="16" fill="#7ee0a322"/><circle cx="58" cy="44" r="11" fill="#7ee0a3"/><text x="58" y="48" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">2</text><text x="58" y="22" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">"c"</text><text x="110" y="146" text-anchor="middle" font-family="monospace" font-size="10.5" fill="#8b95a9">42</text></svg>

## Removing Nodes and Edges

```rust
let mut g: Graph<(), edge::Undir<()>> = Graph::default();
modify!(g, [N(1) ^ N(2) ^ N(3)]).unwrap();

// remove node 1 (and its edges)
modify!(g, [!X(1)]).unwrap();
assert_eq!(g.node_count(), 2);
```
<svg viewBox="0 0 286 120" style="max-width:286px;display:block;margin:0.8em auto" role="img" aria-label="remove node"><line x1="74" y1="50" x2="127" y2="50" stroke="#e2596e" stroke-width="2.2" stroke-dasharray="6 5"/><line x1="94" y1="44" x2="106" y2="56" stroke="#e2596e" stroke-width="2"/><line x1="94" y1="56" x2="106" y2="44" stroke="#e2596e" stroke-width="2"/><line x1="159" y1="50" x2="212" y2="50" stroke="#e2596e" stroke-width="2.2" stroke-dasharray="6 5"/><line x1="179" y1="44" x2="191" y2="56" stroke="#e2596e" stroke-width="2"/><line x1="179" y1="56" x2="191" y2="44" stroke="#e2596e" stroke-width="2"/><circle cx="58" cy="50" r="16" fill="#f0a63f22"/><circle cx="58" cy="50" r="11" fill="#f0a63f"/><text x="58" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="143" cy="50" r="14" fill="#e2596e1f"/><circle cx="143" cy="50" r="11" fill="none" stroke="#e2596e" stroke-width="2" stroke-dasharray="4 4"/><text x="143" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#e2596e">1</text><circle cx="228" cy="50" r="16" fill="#f0a63f22"/><circle cx="228" cy="50" r="11" fill="#f0a63f"/><text x="228" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">2</text></svg>

```rust
let mut g: Graph<(), edge::Dir<()>> = Graph::default();
modify!(g, [N(1) >> N(2)]).unwrap();

// remove edge, keep both nodes
modify!(g, [X(0) & !e() >> x(1)]).unwrap();
assert_eq!(g.node_count(), 2);
assert_eq!(g.edge_count(), 0);
```
<svg viewBox="0 0 203 120" style="max-width:203px;display:block;margin:0.8em auto" role="img" aria-label="remove edge"><defs><marker id="modify-remove-edge-r" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#e2596e"/></marker></defs><line x1="74" y1="50" x2="122" y2="50" stroke="#e2596e" stroke-width="2.2" stroke-dasharray="6 5" marker-end="url(#modify-remove-edge-r)"/><line x1="92" y1="44" x2="104" y2="56" stroke="#e2596e" stroke-width="2"/><line x1="92" y1="56" x2="104" y2="44" stroke="#e2596e" stroke-width="2"/><circle cx="58" cy="50" r="16" fill="#f0a63f22"/><circle cx="58" cy="50" r="11" fill="#f0a63f"/><text x="58" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="145" cy="50" r="16" fill="#f0a63f22"/><circle cx="145" cy="50" r="11" fill="#f0a63f"/><text x="145" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text></svg>

## Updating Values

```rust
let mut g: Graph<&str, edge::Undir<()>> = Graph::default();
modify!(g, [N(1).val("old")]).unwrap();
assert_eq!(g.get(0), Some(&"old"));

// swap node value
modify!(g, [X(0).val("new")]).unwrap();
assert_eq!(g.get(0), Some(&"new"));
```
<svg viewBox="0 0 121 120" style="max-width:121px;display:block;margin:0.8em auto" role="img" aria-label="swap value"><circle cx="61" cy="50" r="16" fill="#7ee0a322"/><circle cx="61" cy="50" r="11" fill="#7ee0a3"/><text x="61" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><text x="61" y="28" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">"new"</text></svg>

```rust
let mut g: Graph<(), edge::Undir<u32>> = Graph::default();
modify!(g, [N(1) & E().val(100u32) ^ N(2)]).unwrap();

// swap edge value
modify!(g, [X(0) & e().val(200u32) ^ X(1)]).unwrap();
```
<svg viewBox="0 0 224 120" style="max-width:224px;display:block;margin:0.8em auto" role="img" aria-label="swap edge"><line x1="74" y1="50" x2="150" y2="50" stroke="#7ee0a3" stroke-width="2.2"/><circle cx="58" cy="50" r="16" fill="#f0a63f22"/><circle cx="58" cy="50" r="11" fill="#f0a63f"/><text x="58" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="166" cy="50" r="16" fill="#f0a63f22"/><circle cx="166" cy="50" r="11" fill="#f0a63f"/><text x="166" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><text x="112" y="61" text-anchor="middle" font-family="monospace" font-size="10.5" fill="#8b95a9">200</text></svg>

## Modification Result

The returned `Modification` struct contains:

- **`new_node_ids`** — mapping from local ids to real graph ids
- **`added_edges`** — edges that were created
- **`removed_nodes`** / **`removed_edges`** — what was deleted
- **`swapped_node_vals`** / **`swapped_edge_vals`** — old values that were replaced

```rust
let mut g: Graph<(), edge::Undir<()>> = Graph::default();
let result = modify!(g, [N(1) ^ N(2)]).unwrap();

// the local id 1 was assigned a real graph node id
let real_id = result.new_node_ids[&LocalId(1)];
```

## Color Legend for Diagrams

Throughout this documentation:
- <span style="color:#2171b5">**Blue**</span> — added or changed elements
- <span style="color:#d32f2f">**Red dashed**</span> — removed elements
- **Black** — existing, unchanged elements
