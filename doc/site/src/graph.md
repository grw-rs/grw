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
<svg viewBox="0 0 334 132" style="max-width:334px;display:block;margin:0.8em auto" role="img" aria-label="path"><line x1="90" y1="58" x2="147" y2="58" stroke="#46c6d6" stroke-width="2.4"/><line x1="187" y1="58" x2="244" y2="58" stroke="#46c6d6" stroke-width="2.4"/><circle cx="70" cy="58" r="21" fill="#f0a63f26"/><circle cx="70" cy="58" r="15" fill="#f0a63f"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="167" cy="58" r="21" fill="#f0a63f26"/><circle cx="167" cy="58" r="15" fill="#f0a63f"/><text x="167" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><circle cx="264" cy="58" r="21" fill="#f0a63f26"/><circle cx="264" cy="58" r="15" fill="#f0a63f"/><text x="264" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text></svg>

### Triangle (back-reference)

```rust
// triangle: chain + n() closes the cycle
let g: Graph<(), edge::Undir<()>> = graph![
    N(0) ^ (N(1) ^ (N(2) ^ n(0)))
].unwrap();
```
<svg viewBox="0 0 334 171" style="max-width:334px;display:block;margin:0.8em auto" role="img" aria-label="triangle"><line x1="87" y1="101" x2="150" y2="62" stroke="#46c6d6" stroke-width="2.4"/><line x1="184" y1="62" x2="247" y2="101" stroke="#46c6d6" stroke-width="2.4"/><line x1="244" y1="111" x2="90" y2="111" stroke="#46c6d6" stroke-width="2.4"/><circle cx="70" cy="111" r="21" fill="#f0a63f26"/><circle cx="70" cy="111" r="15" fill="#f0a63f"/><text x="70" y="116" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="167" cy="52" r="21" fill="#f0a63f26"/><circle cx="167" cy="52" r="15" fill="#f0a63f"/><text x="167" y="57" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><circle cx="264" cy="111" r="21" fill="#f0a63f26"/><circle cx="264" cy="111" r="15" fill="#f0a63f"/><text x="264" y="116" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text></svg>

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
<svg viewBox="0 0 359 237" style="max-width:359px;display:block;margin:0.8em auto" role="img" aria-label="star"><line x1="166" y1="67" x2="83" y2="162" stroke="#46c6d6" stroke-width="2.4"/><line x1="174" y1="71" x2="148" y2="158" stroke="#46c6d6" stroke-width="2.4"/><line x1="185" y1="71" x2="210" y2="158" stroke="#46c6d6" stroke-width="2.4"/><line x1="193" y1="67" x2="276" y2="162" stroke="#46c6d6" stroke-width="2.4"/><circle cx="179" cy="52" r="21" fill="#f0a63f26"/><circle cx="179" cy="52" r="15" fill="#f0a63f"/><text x="179" y="57" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="70" cy="177" r="21" fill="#f0a63f26"/><circle cx="70" cy="177" r="15" fill="#f0a63f"/><text x="70" y="182" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><circle cx="143" cy="177" r="21" fill="#f0a63f26"/><circle cx="143" cy="177" r="15" fill="#f0a63f"/><text x="143" y="182" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text><circle cx="216" cy="177" r="21" fill="#f0a63f26"/><circle cx="216" cy="177" r="15" fill="#f0a63f"/><text x="216" y="182" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">3</text><circle cx="289" cy="177" r="21" fill="#f0a63f26"/><circle cx="289" cy="177" r="15" fill="#f0a63f"/><text x="289" y="182" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">4</text></svg>

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
<svg viewBox="0 0 334 132" style="max-width:334px;display:block;margin:0.8em auto" role="img" aria-label="directed path"><defs><marker id="dir-path-a" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#46c6d6"/></marker></defs><line x1="90" y1="58" x2="139" y2="58" stroke="#46c6d6" stroke-width="2.4" marker-end="url(#dir-path-a)"/><line x1="187" y1="58" x2="236" y2="58" stroke="#46c6d6" stroke-width="2.4" marker-end="url(#dir-path-a)"/><circle cx="70" cy="58" r="21" fill="#f0a63f26"/><circle cx="70" cy="58" r="15" fill="#f0a63f"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="167" cy="58" r="21" fill="#f0a63f26"/><circle cx="167" cy="58" r="15" fill="#f0a63f"/><text x="167" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><circle cx="264" cy="58" r="21" fill="#f0a63f26"/><circle cx="264" cy="58" r="15" fill="#f0a63f"/><text x="264" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text></svg>

```rust
// fan-out: flat chaining from one node
let g: Graph<(), edge::Dir<()>> = graph![
    N(0) >> N(1)
         >> N(2)
         >> N(3)
].unwrap();
```
<svg viewBox="0 0 237 286" style="max-width:237px;display:block;margin:0.8em auto" role="img" aria-label="fan-out"><defs><marker id="dir-fanout-a" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#46c6d6"/></marker></defs><line x1="85" y1="126" x2="146" y2="71" stroke="#46c6d6" stroke-width="2.4" marker-end="url(#dir-fanout-a)"/><line x1="90" y1="139" x2="139" y2="139" stroke="#46c6d6" stroke-width="2.4" marker-end="url(#dir-fanout-a)"/><line x1="85" y1="152" x2="146" y2="207" stroke="#46c6d6" stroke-width="2.4" marker-end="url(#dir-fanout-a)"/><circle cx="70" cy="139" r="21" fill="#f0a63f26"/><circle cx="70" cy="139" r="15" fill="#f0a63f"/><text x="70" y="144" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="167" cy="52" r="21" fill="#f0a63f26"/><circle cx="167" cy="52" r="15" fill="#f0a63f"/><text x="167" y="57" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><circle cx="167" cy="139" r="21" fill="#f0a63f26"/><circle cx="167" cy="139" r="15" fill="#f0a63f"/><text x="167" y="144" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text><circle cx="167" cy="226" r="21" fill="#f0a63f26"/><circle cx="167" cy="226" r="15" fill="#f0a63f"/><text x="167" y="231" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">3</text></svg>

```rust
// bidirectional: n() references existing nodes
let g: Graph<(), edge::Dir<()>> = graph![
    N(0) >> N(1),
    n(1) >> n(0),
].unwrap();
```
<svg viewBox="0 0 237 132" style="max-width:237px;display:block;margin:0.8em auto" role="img" aria-label="bidirectional"><defs><marker id="dir-bidir-a" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#46c6d6"/></marker></defs><path d="M90 58 Q 119 38 139 58" fill="none" stroke="#46c6d6" stroke-width="2.4" marker-end="url(#dir-bidir-a)"/><path d="M147 58 Q 119 38 98 58" fill="none" stroke="#46c6d6" stroke-width="2.4" marker-end="url(#dir-bidir-a)"/><circle cx="70" cy="58" r="21" fill="#f0a63f26"/><circle cx="70" cy="58" r="15" fill="#f0a63f"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="167" cy="58" r="21" fill="#f0a63f26"/><circle cx="167" cy="58" r="15" fill="#f0a63f"/><text x="167" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text></svg>

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
<svg viewBox="0 0 472 132" style="max-width:472px;display:block;margin:0.8em auto" role="img" aria-label="anydir mixed"><defs><marker id="anydir-mixed-a" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#46c6d6"/></marker></defs><line x1="90" y1="58" x2="147" y2="58" stroke="#46c6d6" stroke-width="2.4" marker-end="url(#anydir-mixed-a)"/><line x1="195" y1="58" x2="261" y2="58" stroke="#46c6d6" stroke-width="2.4" marker-end="url(#anydir-mixed-a)"/><line x1="309" y1="58" x2="374" y2="58" stroke="#46c6d6" stroke-width="2.4" marker-end="url(#anydir-mixed-a)"/><circle cx="70" cy="58" r="21" fill="#f0a63f26"/><circle cx="70" cy="58" r="15" fill="#f0a63f"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="175" cy="58" r="21" fill="#f0a63f26"/><circle cx="175" cy="58" r="15" fill="#f0a63f"/><text x="175" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><circle cx="289" cy="58" r="21" fill="#f0a63f26"/><circle cx="289" cy="58" r="15" fill="#f0a63f"/><text x="289" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text><circle cx="402" cy="58" r="21" fill="#f0a63f26"/><circle cx="402" cy="58" r="15" fill="#f0a63f"/><text x="402" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">3</text><text x="119" y="70" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">^</text><text x="228" y="70" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">&gt;&gt;</text><text x="341" y="70" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">&lt;&lt;</text></svg>

```rust
// all three edge types between one pair
let g: Graph<(), edge::Anydir<()>> = graph![
    N(0) ^ N(1),         // undirected
    n(0) >> n(1),        // directed 0 → 1
    n(1) >> n(0),        // directed 1 → 0
].unwrap();
```
<svg viewBox="0 0 253 132" style="max-width:253px;display:block;margin:0.8em auto" role="img" aria-label="anydir triple"><defs><marker id="anydir-triple-a" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#46c6d6"/></marker></defs><path d="M90 58 Q 127 18 155 58" fill="none" stroke="#46c6d6" stroke-width="2.4" marker-end="url(#anydir-triple-a)"/><line x1="90" y1="58" x2="155" y2="58" stroke="#46c6d6" stroke-width="2.4" marker-end="url(#anydir-triple-a)"/><path d="M163 58 Q 127 18 98 58" fill="none" stroke="#46c6d6" stroke-width="2.4" marker-end="url(#anydir-triple-a)"/><circle cx="70" cy="58" r="21" fill="#f0a63f26"/><circle cx="70" cy="58" r="15" fill="#f0a63f"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="183" cy="58" r="21" fill="#f0a63f26"/><circle cx="183" cy="58" r="15" fill="#f0a63f"/><text x="183" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><text x="127" y="28" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">&gt;&gt;</text><text x="123" y="70" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">^</text><text x="127" y="28" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">&lt;&lt;</text></svg>

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
