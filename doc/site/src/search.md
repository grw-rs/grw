# `search!` — Pattern Matching

`search!` compiles a pattern into a query and iterates over all morphism-valid mappings from pattern nodes to target graph nodes. Patterns are organized into **clusters** — `get` (required) and `ban` (forbidden substructures).

<svg viewBox="0 0 334 132" style="max-width:334px;display:block;margin:0.8em auto" role="img" aria-label="pattern and target"><line x1="90" y1="58" x2="147" y2="58" stroke="#7ee0a3" stroke-width="2.4"/><line x1="187" y1="58" x2="244" y2="58" stroke="#7ee0a3" stroke-width="2.4"/><circle cx="70" cy="58" r="21" fill="#7ee0a326"/><circle cx="70" cy="58" r="15" fill="#7ee0a3"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">A</text><circle cx="167" cy="58" r="21" fill="#7ee0a326"/><circle cx="167" cy="58" r="15" fill="#7ee0a3"/><text x="167" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">B</text><circle cx="264" cy="58" r="21" fill="#7ee0a326"/><circle cx="264" cy="58" r="15" fill="#7ee0a3"/><text x="264" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">C</text></svg> <svg viewBox="0 0 334 171" style="max-width:334px;display:block;margin:0.8em auto" role="img" aria-label="target"><line x1="87" y1="101" x2="150" y2="62" stroke="#46c6d6" stroke-width="2.4"/><line x1="184" y1="62" x2="247" y2="101" stroke="#46c6d6" stroke-width="2.4"/><line x1="244" y1="111" x2="90" y2="111" stroke="#46c6d6" stroke-width="2.4"/><circle cx="70" cy="111" r="21" fill="#f0a63f26"/><circle cx="70" cy="111" r="15" fill="#f0a63f"/><text x="70" y="116" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="167" cy="52" r="21" fill="#f0a63f26"/><circle cx="167" cy="52" r="15" fill="#f0a63f"/><text x="167" y="57" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><circle cx="264" cy="111" r="21" fill="#f0a63f26"/><circle cx="264" cy="111" r="15" fill="#f0a63f"/><text x="264" y="116" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text></svg>

## DSL Primitives

| Symbol | Meaning |
|--------|---------|
| `get(morphism) { ... }` | Required pattern cluster — must be found |
| `ban(morphism) { ... }` | Forbidden pattern cluster — matches are rejected |
| `N(id)` | Pattern node |
| `n(id)` | Reference to pattern node |
| `^` `>>` `<<` | Edge operators (same as `mgraph!`) |
| `!N(id)` | Negated node — the edge must NOT exist |
| `N(id).val(v)` | Node value — exact match |
| `N(id).test(\|v\| ...)` | Node value predicate |
| `E().val(v)` | Edge value — exact match |
| `X(id)` | Context node — pinned to a specific graph node |

## Basic Usage

When `search!` receives a graph reference, it creates a `Session` that you iterate with a `for` loop:

```rust
use grw::*;
use grw::graph::edge;

// target graph: triangle
let g: graph::MUndir0 = mgraph![
    N(0) ^ (N(1) ^ (N(2) ^ n(0)))
].unwrap();

// search for edges
let session = search![&g,
    get(Mono) {
        N(0) ^ N(1)
    }
].unwrap();

for m in &session {
    let a = m.get(0).unwrap();
    let b = m.get(1).unwrap();
    println!("{:?} — {:?}", a, b);
}
```

## The Match Struct

Each iteration yields a `Match` — a mapping from pattern node local ids to graph node ids.

```rust
for m in &session {
    // get a specific pattern node's graph mapping
    let node_id: id::N = m.get(0).unwrap();

    // iterate all (pattern_local_id, graph_node_id) pairs
    for &(lid, nid) in m.iter() {
        println!("pattern {} → graph {:?}", lid.0, nid);
    }

    // just the graph node ids
    let graph_nodes: Vec<id::N> = m.values().collect();
}
```

### TranslatedMatch

For richer access to node values and adjacencies, use `translate()`:

```rust
for m in &session {
    let tm = session.translate(&m);

    // node with its value
    if let Some((nid, val)) = tm.node(0) {
        println!("pattern 0 → graph {:?} val={:?}", nid, val);
    }

    // all matched nodes with values
    for (lid, nid, val) in tm.nodes() {
        println!("{} → {:?} = {:?}", lid.0, nid, val);
    }
}
```

## Ban Clusters

Ban clusters define **forbidden substructures**. If the ban pattern matches, the overall match is rejected.

```rust
// find edges whose endpoints do NOT share a common neighbor
let session = search![&g,
    get(Mono) {
        N(0) ^ N(1)
    },
    ban(Mono) {
        n(0) ^ N(2),
        n(1) ^ n(2),
    }
].unwrap();
```

The `n(0)` and `n(1)` in the `ban` cluster refer back to the nodes defined in the `get` cluster. The ban says: "reject this match if nodes 0 and 1 share a common neighbor (node 2)."

## Sequential vs Parallel

The `Session` supports two iteration modes:

```rust
// sequential — lazy iterator, one match at a time
for m in session.iter() { /* ... */ }

// also works via IntoIterator
for m in &session { /* ... */ }

// parallel — uses rayon, collects all matches
let all: Vec<Match> = session.par_iter().collect();
```

**`session.iter()`** is single-threaded lazy backtracking — yields one match at a time. Use this when you want to process matches as a stream or stop early.

**`session.par_iter()`** partitions the search space across threads via rayon. Use this for large graphs where you need all matches and have cores to spare.

## Value Predicates

Filter matches based on node or edge values:

```rust
// find nodes where x > 10.0
let session = search![&g,
    get(Mono) {
        N(0).test(|p: &Point| p.x > 10.0) ^ N(1)
    }
].unwrap();
```

## Context Nodes

Pin a pattern node to a specific graph node:

```rust
// find all neighbors of graph node 5
let session = search![&g,
    get(Mono) {
        X(5) ^ N(0)
    }
].unwrap();
```

`X(5)` is pinned to graph node 5 — the search only looks for nodes connected to it.
