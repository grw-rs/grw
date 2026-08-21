# Tiered Search Index Design

## Problem

The search engine currently hard-codes CsrAdj as the only target graph representation. This forces O(|V| + |E|) upfront precomputation even when the user wants a quick one-off search on a dynamic graph. It also leaks CSR-specific types (CsrStore) into the engine interface and requires `NV: Clone` for all searches.

## Design

Four tiers of precomputation. Each builds on the previous. The user chooses how much to invest upfront. Trait bounds gate at compile time — using a tier that requires `NV: Hash` when `NV: !Hash` is a compile error, not a runtime surprise.

```
Tier        Precomputes                 Bounds required
────        ───────────                 ───────────────
Raw         nothing                     NV: Sync
Rev         + dense reverse array       NV: Sync
RevCsr      + CSR adjacency + edges     NV: Sync + Clone, ER::Val: Clone
RevCsrVal   + value-grouped node index  NV: Sync + Clone + Eq + Hash, ER::Val: Clone
```

### API

```rust
// Raw — zero precompute, search directly on graph
let session = Session::new(query, &graph);

// Indexed — user opts into precomputation
let idx = graph.index(Rev);
let idx = graph.index(RevCsr);
let idx = graph.index(RevCsrVal);
let session = Session::new(query, &idx);

// Iteration
session.iter();          // sequential
session.par_iter();      // parallel
```

`graph.index(RevCsr)` when `NV: !Clone` → compile error.
`graph.index(RevCsrVal)` when `NV: !Hash` → compile error.

### The `search::Index` trait

The backtracking engine operates on this trait. Each tier implements it. The compiler monomorphizes — no vtable, no runtime dispatch.

```rust
pub trait Index<NV: Sync, ER: graph::Edge>: Sync {
    type Neighbors<'a>: ExactSizeIterator<Item = u32> where Self: 'a;
    type Reverse: ReverseLookup;

    fn degree(&self, n: u32) -> u32;
    fn is_adjacent(&self, n1: u32, n2: u32) -> bool;
    fn neighbors(&self, n: u32) -> Self::Neighbors<'_>;
    fn node_val(&self, n: u32) -> &NV;
    fn create_reverse(&self) -> Self::Reverse;

    fn check_edge(
        &self, n1: u32, n2: u32,
        slot: ER::Slot, any_slot: bool, negated: bool,
        pred: Option<&dyn Fn(&ER::Val) -> bool>,
    ) -> bool;

    fn candidates(
        &self,
        pred: Option<&dyn Fn(&NV) -> bool>,
        min_degree: usize,
        morphism: Morphism,
    ) -> Vec<id::N>;

    fn search_order(&self, query: &Query<NV, ER>) -> (Vec<usize>, Vec<usize>);
}

pub trait ReverseLookup {
    fn get(&self, n: u32) -> u32;
    fn set(&mut self, n: u32, val: u32);
    fn clear(&mut self, n: u32);
}
```

### Tier implementations

```rust
// Raw: graph itself implements search::Index
impl<NV: Sync, ER: graph::Edge> Index<NV, ER> for graph::Graph<NV, ER> {
    type Neighbors<'a> = /* SmallVec iterator */ where Self: 'a;
    type Reverse = FxHashMap<u32, u32>;
    // All methods delegate to live graph. Zero precomputation.
    // search_order: returns pre-compiled order from query, no scanning.
    // candidates: iterates sparse node storage on-the-fly.
    // check_edge: calls graph.edges_between / graph.get_edge_val.
    // create_reverse: returns empty FxHashMap.
}
```

```rust
// Indexed tiers: wrapper struct
pub struct Indexed<'g, NV: Sync, ER: graph::Edge, T: Tier<NV, ER>> {
    graph: &'g graph::Graph<NV, ER>,
    data: T::Data,
}
```

Each tier is a zero-sized marker type with a `Tier` trait impl that gates bounds:

```rust
pub struct Rev;
pub struct RevCsr;
pub struct RevCsrVal;

pub trait Tier<NV: Sync, ER: graph::Edge> {
    type Data;
    fn build(graph: &graph::Graph<NV, ER>) -> Self::Data;
}

impl<NV: Sync, ER: graph::Edge> Tier<NV, ER> for Rev {
    type Data = RevData;
    // RevData stores reverse_len (= node count) for dense Vec allocation
}

impl<NV: Sync + Clone, ER: graph::Edge> Tier<NV, ER> for RevCsr
where ER::Val: Clone {
    type Data = CsrData<NV, ER>;
    // CsrData wraps existing CsrAdj
}

impl<NV: Sync + Clone + Eq + Hash, ER: graph::Edge> Tier<NV, ER> for RevCsrVal
where ER::Val: Clone {
    type Data = CsrValData<NV, ER>;
    // CsrValData = CsrAdj + HashMap<NV, Vec<id::N>>
}
```

### Per-tier behavior

| Method | Raw | Rev | RevCsr | RevCsrVal |
|---|---|---|---|---|
| `neighbors` | iterate SmallVec | same | slice contiguous u32 array | same as RevCsr |
| `is_adjacent` | `Adjacents::contains` (partition_point) | same | binary search on u32 array | same as RevCsr |
| `node_val` | `graph.nodes.get(n)` | same | `csr.node_vals[n]` contiguous | same as RevCsr |
| `check_edge` | `graph.edges_between` / `graph.get_edge_val` | same | CsrStore + `ER::csr_any_match` | same as RevCsr |
| `candidates` | iterate sparse node storage O(\|V\|) | same | iterate sorted `all_node_ids` O(\|V\|) | iterate value groups O(\|distinct\|) |
| `search_order` | pre-compiled from query (no scan) | same | scan + reorder by selectivity O(\|P\|×\|V\|) | scan value groups O(\|P\|×\|distinct\|) |
| `create_reverse` | `FxHashMap::new()` | `vec![UNMAPPED; \|V\|]` | `vec![UNMAPPED; \|V\|]` | `vec![UNMAPPED; \|V\|]` |

### Engine changes

`Ctx` simplifies from three references to two:

```rust
pub(crate) struct Ctx<'a, NV: Sync, ER: graph::Edge, T: Index<NV, ER>> {
    pub(crate) query: &'a Query<NV, ER>,
    pub(crate) index: &'a T,
}
```

`State` becomes generic over reverse map:

```rust
pub(crate) struct State<R: ReverseLookup> {
    bindings: Vec<Option<id::N>>,
    mapping: Vec<u32>,
    reverse: R,
    search_order: Vec<usize>,
    search_depth_of: Vec<usize>,
    stack: Vec<StackFrame>,
    exhausted: bool,
    candidate_pool: Vec<Vec<id::N>>,
    forward_verified_depths: u64,
}
```

Eliminated:
- `val_filtered` field (replaced by `index.candidates()`)
- `Shared` struct (replaced by `index.search_order()` + trait methods)
- `CsrStore` leak into engine interface (hidden behind `index.check_edge()`)

### Parallel engine

`par::Iter` becomes generic over `T: Index<NV, ER>`. Each rayon worker calls `index.create_reverse()` to get its own reverse map — `FxHashMap` for Raw (cheap), dense `Vec` for Rev+ (user opted in).

The `search_order` is computed once and shared by reference across workers. No per-worker precomputation beyond the reverse map.

Two unsafe pointer casts instead of three: `query_addr` and `index_addr`.

### Migration

| Current | New | Behavior change |
|---|---|---|
| `graph.index()` | `graph.index(RevCsr)` | None |
| `Session::from_search(search, &graph)` | `Session::new(query, &idx)` | query first |
| `search::Graph` wrapper | `search::Indexed<T>` | Parameterized by tier |
| `Engine` trait | Removed | Par already didn't implement it |
| `Shared` struct | Removed | Absorbed by Index trait |
| `val_filtered` on State | Removed | Absorbed by `index.candidates()` |

### Performance invariant

No regression for existing workloads. The RevCsr tier produces identical code to today's CsrAdj path after monomorphization. New tiers add capability without affecting existing code paths.
