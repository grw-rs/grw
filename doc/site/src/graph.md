# `graph!` — Construction

The `graph!` macro constructs a graph from a declarative description of nodes and edges.

## DSL Primitives

| Symbol | Meaning |
|--------|---------|
| `N(id)` | New node with explicit local id |
| `N_()` | New node with auto-assigned id |
| `n(id)` | Reference to previously defined node |
| `E()` | Edge constructor (for attaching values) |
| `.val(v)` | Attach a value to a node or edge |
| `^` | Undirected edge |
| `>>` | Directed edge (source → target) |
| `<<` | Directed edge (target ← source) |
| `&` | Attach explicit edge value before direction operator |
| `,` | Separate independent fragments |

## Operator Precedence

The edge operators (`^`, `>>`, `<<`) are **left-associative** in Rust. This has an important consequence:

- **Flat chaining** `N(0) ^ N(1) ^ N(2)` creates a **star** — all edges radiate from node 0
- **Grouping** `N(0) ^ (N(1) ^ N(2))` creates a **path** — 0 connects to 1, 1 connects to 2

This is because `N(0) ^ N(1)` returns node 0 (with an edge to 1 attached), so the next `^ N(2)` adds another edge from 0.

## Undirected Graphs

### Path (grouping)

```rust
use grw::graph::{self, Graph, edge};

// path: 0 — 1 — 2
let g: Graph<(), edge::Undir<()>> = graph![
    N(0) ^ (N(1) ^ N(2))
].unwrap();
```
<svg viewBox="0 0 286 120" style="max-width:286px;display:block;margin:0.8em auto" role="img" aria-label="path"><line x1="74" y1="50" x2="127" y2="50" stroke="#46c6d6" stroke-width="2.2"/><line x1="159" y1="50" x2="212" y2="50" stroke="#46c6d6" stroke-width="2.2"/><circle cx="58" cy="50" r="16" fill="#f0a63f22"/><circle cx="58" cy="50" r="11" fill="#f0a63f"/><text x="58" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="143" cy="50" r="16" fill="#f0a63f22"/><circle cx="143" cy="50" r="11" fill="#f0a63f"/><text x="143" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><circle cx="228" cy="50" r="16" fill="#f0a63f22"/><circle cx="228" cy="50" r="11" fill="#f0a63f"/><text x="228" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">2</text></svg>

### Triangle (back-reference)

```rust
// triangle: chain + n() closes the cycle
let g: Graph<(), edge::Undir<()>> = graph![
    N(0) ^ (N(1) ^ (N(2) ^ n(0)))
].unwrap();
```
<svg viewBox="0 0 286 146" style="max-width:286px;display:block;margin:0.8em auto" role="img" aria-label="triangle"><line x1="72" y1="86" x2="129" y2="52" stroke="#46c6d6" stroke-width="2.2"/><line x1="157" y1="52" x2="214" y2="86" stroke="#46c6d6" stroke-width="2.2"/><line x1="212" y1="94" x2="74" y2="94" stroke="#46c6d6" stroke-width="2.2"/><circle cx="58" cy="94" r="16" fill="#f0a63f22"/><circle cx="58" cy="94" r="11" fill="#f0a63f"/><text x="58" y="98" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="143" cy="44" r="16" fill="#f0a63f22"/><circle cx="143" cy="44" r="11" fill="#f0a63f"/><text x="143" y="48" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><circle cx="228" cy="94" r="16" fill="#f0a63f22"/><circle cx="228" cy="94" r="11" fill="#f0a63f"/><text x="228" y="98" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">2</text></svg>

### Star (flat chaining)

```rust
// star: flat chaining fans out from one node
let g: Graph<(), edge::Undir<()>> = graph![
    N(0) ^ N(1)
         ^ N(2)
         ^ N(3)
         ^ N(4)
].unwrap();
```
<svg viewBox="0 0 306 203" style="max-width:306px;display:block;margin:0.8em auto" role="img" aria-label="star"><line x1="143" y1="56" x2="69" y2="139" stroke="#46c6d6" stroke-width="2.2"/><line x1="149" y1="59" x2="126" y2="135" stroke="#46c6d6" stroke-width="2.2"/><line x1="158" y1="59" x2="180" y2="135" stroke="#46c6d6" stroke-width="2.2"/><line x1="164" y1="56" x2="238" y2="139" stroke="#46c6d6" stroke-width="2.2"/><circle cx="153" cy="44" r="16" fill="#f0a63f22"/><circle cx="153" cy="44" r="11" fill="#f0a63f"/><text x="153" y="48" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="58" cy="151" r="16" fill="#f0a63f22"/><circle cx="58" cy="151" r="11" fill="#f0a63f"/><text x="58" y="155" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><circle cx="121" cy="151" r="16" fill="#f0a63f22"/><circle cx="121" cy="151" r="11" fill="#f0a63f"/><text x="121" y="155" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">2</text><circle cx="185" cy="151" r="16" fill="#f0a63f22"/><circle cx="185" cy="151" r="11" fill="#f0a63f"/><text x="185" y="155" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">3</text><circle cx="248" cy="151" r="16" fill="#f0a63f22"/><circle cx="248" cy="151" r="11" fill="#f0a63f"/><text x="248" y="155" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">4</text></svg>

### With Values

```rust
// node values
let g: Graph<&str, edge::Undir<()>> = graph![
    N(0).val("alice") ^ (N(1).val("bob") ^ N(2).val("carol"))
].unwrap();

// edge values — & E().val(...) before direction operator
let g: Graph<(), edge::Undir<f64>> = graph![
    N(0) & E().val(1.5) ^ (N(1) & E().val(2.0) ^ N(2))
].unwrap();

// both
let g: Graph<&str, edge::Undir<u32>> = graph![
    N(0).val("a") & E().val(10) ^ N(1).val("b")
].unwrap();
```

### Anonymous Nodes

```rust
// ids assigned automatically
let g: Graph<(), edge::Undir<()>> = graph![N_() ^ N_() ^ N_()].unwrap();
```

## Directed Graphs

```rust
// path: 0 → 1 → 2
let g: Graph<(), edge::Dir<()>> = graph![
    N(0) >> (N(1) >> N(2))
].unwrap();
```
<svg viewBox="0 0 286 120" style="max-width:286px;display:block;margin:0.8em auto" role="img" aria-label="directed path"><defs><marker id="dir-path-a" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#46c6d6"/></marker></defs><line x1="74" y1="50" x2="120" y2="50" stroke="#46c6d6" stroke-width="2.2" marker-end="url(#dir-path-a)"/><line x1="159" y1="50" x2="205" y2="50" stroke="#46c6d6" stroke-width="2.2" marker-end="url(#dir-path-a)"/><circle cx="58" cy="50" r="16" fill="#f0a63f22"/><circle cx="58" cy="50" r="11" fill="#f0a63f"/><text x="58" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="143" cy="50" r="16" fill="#f0a63f22"/><circle cx="143" cy="50" r="11" fill="#f0a63f"/><text x="143" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><circle cx="228" cy="50" r="16" fill="#f0a63f22"/><circle cx="228" cy="50" r="11" fill="#f0a63f"/><text x="228" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">2</text></svg>

```rust
// fan-out: flat chaining from one node
let g: Graph<(), edge::Dir<()>> = graph![
    N(0) >> N(1)
         >> N(2)
         >> N(3)
].unwrap();
```
<svg viewBox="0 0 203 244" style="max-width:203px;display:block;margin:0.8em auto" role="img" aria-label="fan-out"><defs><marker id="dir-fanout-a" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#46c6d6"/></marker></defs><line x1="70" y1="108" x2="127" y2="59" stroke="#46c6d6" stroke-width="2.2" marker-end="url(#dir-fanout-a)"/><line x1="74" y1="118" x2="122" y2="118" stroke="#46c6d6" stroke-width="2.2" marker-end="url(#dir-fanout-a)"/><line x1="70" y1="128" x2="127" y2="177" stroke="#46c6d6" stroke-width="2.2" marker-end="url(#dir-fanout-a)"/><circle cx="58" cy="118" r="16" fill="#f0a63f22"/><circle cx="58" cy="118" r="11" fill="#f0a63f"/><text x="58" y="122" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="145" cy="44" r="16" fill="#f0a63f22"/><circle cx="145" cy="44" r="11" fill="#f0a63f"/><text x="145" y="48" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><circle cx="145" cy="118" r="16" fill="#f0a63f22"/><circle cx="145" cy="118" r="11" fill="#f0a63f"/><text x="145" y="122" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">2</text><circle cx="145" cy="192" r="16" fill="#f0a63f22"/><circle cx="145" cy="192" r="11" fill="#f0a63f"/><text x="145" y="196" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">3</text></svg>

```rust
// bidirectional: n() references existing nodes
let g: Graph<(), edge::Dir<()>> = graph![
    N(0) >> N(1),
    n(1) >> n(0),
].unwrap();
```
<svg viewBox="0 0 203 120" style="max-width:203px;display:block;margin:0.8em auto" role="img" aria-label="bidirectional"><defs><marker id="dir-bidir-a" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#46c6d6"/></marker></defs><path d="M74 50 Q 101 33 122 50" fill="none" stroke="#46c6d6" stroke-width="2.2" marker-end="url(#dir-bidir-a)"/><path d="M129 50 Q 101 33 81 50" fill="none" stroke="#46c6d6" stroke-width="2.2" marker-end="url(#dir-bidir-a)"/><circle cx="58" cy="50" r="16" fill="#f0a63f22"/><circle cx="58" cy="50" r="11" fill="#f0a63f"/><text x="58" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="145" cy="50" r="16" fill="#f0a63f22"/><circle cx="145" cy="50" r="11" fill="#f0a63f"/><text x="145" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text></svg>

```rust
// incoming edges with <<
let g: Graph<(), edge::Dir<()>> = graph![
    N(0) << N(1),   // edge from 1 to 0
].unwrap();
```

## Anydirected Graphs

Mix undirected and directed edges in one graph:

```rust
let g: Graph<(), edge::Anydir<()>> = graph![
    N(0) ^ (N(1) >> N(2)),  // 0 — 1 → 2
    N(3) << n(2),            // 2 → 3
].unwrap();
```
<svg viewBox="0 0 403 120" style="max-width:403px;display:block;margin:0.8em auto" role="img" aria-label="anydir mixed"><defs><marker id="anydir-mixed-a" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#46c6d6"/></marker></defs><line x1="74" y1="50" x2="126" y2="50" stroke="#46c6d6" stroke-width="2.2" marker-end="url(#anydir-mixed-a)"/><line x1="165" y1="50" x2="224" y2="50" stroke="#46c6d6" stroke-width="2.2" marker-end="url(#anydir-mixed-a)"/><line x1="263" y1="50" x2="322" y2="50" stroke="#46c6d6" stroke-width="2.2" marker-end="url(#anydir-mixed-a)"/><circle cx="58" cy="50" r="16" fill="#f0a63f22"/><circle cx="58" cy="50" r="11" fill="#f0a63f"/><text x="58" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="149" cy="50" r="16" fill="#f0a63f22"/><circle cx="149" cy="50" r="11" fill="#f0a63f"/><text x="149" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><circle cx="247" cy="50" r="16" fill="#f0a63f22"/><circle cx="247" cy="50" r="11" fill="#f0a63f"/><text x="247" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">2</text><circle cx="345" cy="50" r="16" fill="#f0a63f22"/><circle cx="345" cy="50" r="11" fill="#f0a63f"/><text x="345" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">3</text><text x="100" y="61" text-anchor="middle" font-family="monospace" font-size="10.5" fill="#8b95a9">^</text><text x="194" y="61" text-anchor="middle" font-family="monospace" font-size="10.5" fill="#8b95a9">&gt;&gt;</text><text x="292" y="61" text-anchor="middle" font-family="monospace" font-size="10.5" fill="#8b95a9">&lt;&lt;</text></svg>

```rust
// all three edge types between one pair
let g: Graph<(), edge::Anydir<()>> = graph![
    N(0) ^ N(1),         // undirected
    n(0) >> n(1),        // directed 0 → 1
    n(1) >> n(0),        // directed 1 → 0
].unwrap();
```
<svg viewBox="0 0 217 120" style="max-width:217px;display:block;margin:0.8em auto" role="img" aria-label="anydir triple"><defs><marker id="anydir-triple-a" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#46c6d6"/></marker></defs><path d="M74 50 Q 108 16 136 50" fill="none" stroke="#46c6d6" stroke-width="2.2" marker-end="url(#anydir-triple-a)"/><line x1="74" y1="50" x2="136" y2="50" stroke="#46c6d6" stroke-width="2.2" marker-end="url(#anydir-triple-a)"/><path d="M143 50 Q 108 16 81 50" fill="none" stroke="#46c6d6" stroke-width="2.2" marker-end="url(#anydir-triple-a)"/><circle cx="58" cy="50" r="16" fill="#f0a63f22"/><circle cx="58" cy="50" r="11" fill="#f0a63f"/><text x="58" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><circle cx="159" cy="50" r="16" fill="#f0a63f22"/><circle cx="159" cy="50" r="11" fill="#f0a63f"/><text x="159" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><text x="108" y="26" text-anchor="middle" font-family="monospace" font-size="10.5" fill="#8b95a9">&gt;&gt;</text><text x="105" y="61" text-anchor="middle" font-family="monospace" font-size="10.5" fill="#8b95a9">^</text><text x="108" y="26" text-anchor="middle" font-family="monospace" font-size="10.5" fill="#8b95a9">&lt;&lt;</text></svg>

## Turbofish Syntax

When the type can't be inferred, use the turbofish form:

```rust
let g = graph![<(), grw::graph::edge::Undir<()>>; N(0) ^ N(1)].unwrap();
```

## Multiple Fragments

Use commas to separate disconnected components or back-references:

```rust
// two separate edges, then connect them
let g: Graph<(), edge::Undir<()>> = graph![
    N(0) ^ N(1),
    N(2) ^ N(3),
    n(0) ^ n(2),
].unwrap();
```
