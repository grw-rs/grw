# Index Tiers

Before the search engine can match patterns, it needs a data structure for fast neighbor lookups. When you use `search![&g, ...]`, the graph is indexed automatically with `RevCsr`. For manual control, you can index explicitly.

## Tiers

| Tier | What it builds | Memory | Speed | When to use |
|------|---------------|--------|-------|-------------|
| `Raw` | Sorted node id list | Minimal | Baseline | Tiny graphs, memory constrained |
| `Rev` | Reverse adjacency map | Low | Good | Small graphs |
| `RevCsr` | CSR-compressed reverse adjacency | Medium | Fast | **Default choice** — good balance |
| `RevCsrVal` | CSR + node values grouped by value | Higher | Fastest for valued | Graphs with node value predicates (`.val`/`.test`) |

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
let count1 = Seq::search(r.query(), &indexed).unwrap().count();
let count2 = Seq::search(r.query(), &indexed).unwrap().count();
```

## RevCsrVal for Node Value Predicates

When your search pattern uses node value predicates (`.val(..)`, `.test(..)`), `RevCsrVal` pre-groups nodes by their value in the CSR structure (`value_groups: NV -> [id::N]`), so evaluating a predicate only visits the value groups that actually pass it instead of every node:

```rust
let indexed = g.index(RevCsrVal);
```

This trades more memory for faster node-predicate evaluation during search.

## Key Predicates Bypass The Scan

A [`.key`/`.key_in`](./search.md#key-predicates) predicate never touches `value_groups` at all, on any tier: it resolves straight through the graph's own [index](./indices.md) tables (`Graph::index_hit`), producing a candidate pool directly instead of scanning node values.
