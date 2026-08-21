# Index Tiers

Before the search engine can match patterns, it needs a data structure for fast neighbor lookups. When you use `search![&g, ...]`, the graph is indexed automatically with `RevCsr`. For manual control, you can index explicitly.

## Tiers

| Tier | What it builds | Memory | Speed | When to use |
|------|---------------|--------|-------|-------------|
| `Raw` | Sorted node id list | Minimal | Baseline | Tiny graphs, memory constrained |
| `Rev` | Reverse adjacency map | Low | Good | Small graphs |
| `RevCsr` | CSR-compressed reverse adjacency | Medium | Fast | **Default choice** — good balance |
| `RevCsrVal` | CSR + cached edge values | Higher | Fastest for valued | Graphs with edge predicates |

## Automatic Indexing

The `search!` macro with a graph reference handles indexing for you:

```rust
// RevCsr index is built automatically
let session = search![&g,
    get(Mono) { N(0) ^ N(1) }
].unwrap();
```

## Manual Indexing

For repeated searches on the same graph, index once and reuse:

```rust
use grw::search::{Search, Seq, Par, RevCsr, RevCsrVal};

let Search::Resolved(r) = search![<(), edge::Undir<()>>;
    get(Mono) { N(0) ^ N(1) }
].unwrap() else { panic!() };

// index once
let indexed = g.index(RevCsr);

// search many times
let count1 = Seq::search(&r.query, &indexed).count();
let count2 = Seq::search(&r.query, &indexed).count();
```

## RevCsrVal for Edge Predicates

When your search pattern uses edge value predicates, `RevCsrVal` pre-caches edge values in the CSR structure for faster lookups:

```rust
let indexed = g.index(RevCsrVal);
```

This trades more memory for faster edge predicate evaluation during search.
