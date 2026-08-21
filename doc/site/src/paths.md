# Variable-Length Paths

Prefix a pattern node with `..` and the edge becomes a **variable-length
path**. Everything else about the pattern stays the same — a path drops in
anywhere an edge can.

```rust,ignore
..n(1).dfs()                                              // deepest-first trails
..n(1).bfs().len(3..10)                                   // shortest-first, 3–9 edges
..n(1).navigate(Dijkstra::counted())                      // cheapest by hop count
..n(1).navigate(Dijkstra::weighted(|ev: &EV| ev.cost))    // cheapest by edge weight
..n(1).navigate(AStar::new(|ev| ev.cost, |n| h(n))).all() // every trail, cost-ordered
..n(1).dfs().guard(|p| !p.contains(N(2)))                 // prune paths mid-walk
..n(1).drive(|n| Some(Explore::One(0)))                   // steer expansion per depth
```

## Traversal: `dfs` / `bfs` / `drive`

`dfs()` yields deepest trails first, `bfs()` shortest first. Both are **lazy**
— each `next()` computes exactly one complete path by backtracking.

`drive(f)` steers a DFS: at each expansion step, the closure receives the
number of candidate continuations `n` and answers with an
[`Explore`](#steering-with-explore) decision — or `None` to prune the tip.

## Length bounds: `len`

`.len(bounds)` constrains the number of **edges** in the path. Any `usize`
range works, and a bare `usize` means an exact length:

```rust,ignore
.len(3)       // exactly 3 edges
.len(2..5)    // 2, 3 or 4
.len(2..=4)   // same, inclusive
.len(1..)     // at least one (the default)
.len(0..)     // include the trivial self-path when from == to
```

## Navigation: `Dijkstra` / `AStar`

`navigate(nav)` orders results by **cost** instead of traversal order. The
navigator supplies a non-negative per-edge cost; A\* adds an admissible
per-node heuristic that steers exploration without changing which path is
cheapest.

```rust,ignore
let cfg = Config::new(())
    .navigate(Dijkstra::weighted(|ms: &u32| *ms as f64));

let (cost, path) = g
    .path_navigate(N(0), N(3), outgoing, cfg, &constraint)
    .next()
    .unwrap();
```

Two result modes:

| Mode | Meaning |
|------|---------|
| `.one()` *(default)* | The single cheapest path. Textbook Dijkstra/A\* node dominance — `O((V+E) log V)`. |
| `.all()` | Every trail, in non-decreasing cost. No dominance; the frontier can grow — you decide when to stop pulling. |

## Guards

`.guard(pred)` rejects paths (including partial ones) during the walk. The
predicate receives the path **tip-first** as a lazy, allocation-free view —
decide from a short suffix and return early:

```rust,ignore
.dfs().guard(|p| !p.contains(id::N(2)))
```

## Steering with `Explore`

The `drive` closure picks which continuation indices `0..n` to expand:

| Decision | Meaning |
|----------|---------|
| `Explore::All` | expand every candidate |
| `Explore::One(i)` | only the `i`-th |
| `Explore::Len(l)` | the first `min(l, n)` |
| `Explore::Steps(s)` | exactly the listed indices |
| `None` | prune this tip |

## Paths inside `search!`

Inside a pattern, the path's endpoints are pattern nodes and the morphism
governs the **intermediates**: a `Mono` path can't reuse nodes bound
elsewhere in the match, a `SubIso` path additionally rejects branched
intermediates. Bind both endpoints to the same node and paths become
**cycle detection**:

```rust,ignore
let session = search![&g,
    get(Homo) { N(0), N(1) },            // endpoints may collapse
    get(Mono) { n(0) >> ..n(1).dfs().len(1..) }
].unwrap();

let cycles = session.iter()
    .filter(|m| m[0] == m[1] && !m.path(0).is_empty())
    .count();
```

## Consuming path results

One `Match` fixes the endpoints — but several concrete paths may realize the
same binding. They hang off the match, per path edge:

```rust,ignore
m.path(0)          // &[id::N] — full node sequence of the first path edge
m.path_count(0)    // how many alternatives were found
m.next_paths()     // step to the next combination (odometer, rightmost first)
```

`next_paths_injective()` additionally keeps intermediates **across different
path edges** mutually distinct — that is how `Mono`/`SubIso` semantics extend
to multi-path matches.

Standalone iterators are plain: `g.path_search(..)` yields `Vec<id::N>` per
`next()`, `g.path_navigate(..)` yields `(cost, Vec<id::N>)` in non-decreasing
cost order.
