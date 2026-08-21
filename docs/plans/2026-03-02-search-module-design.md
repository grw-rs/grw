# Search Module Design

## Goal

A graph morphism search engine for shagra that leverages the shape system (cores, stars, paths) for fast substructure matching. Supports composed graph patterns — multiple morphism types within a single pattern via clusters. Engine-only, no UI.

## Composed Graph Patterns

Traditional graph pattern matching forces one morphism type across the entire pattern. This is fundamentally limiting when different parts of a pattern need different matching semantics.

A composed graph pattern is a single pattern graph partitioned into overlapping clusters. Each cluster defines:

- **Elements**: a set of nodes and edges (non-exclusive — elements can belong to multiple clusters)
- **Morphism**: Iso, SubIso, or Mono (per-cluster, not global)
- **Decision**: `get` (accept) or `ban` (reject / NAC)

### Morphism Types

| Type | Semantics |
|------|-----------|
| Iso | Bijective structure-preserving map. Target graph must have same size as pattern. |
| SubIso | Induced subgraph isomorphism. All edges between matched nodes must correspond exactly — no extra, no missing. |
| Mono | Monomorphism. Pattern edges must exist in target, but extra target edges between matched nodes are allowed. |

Matching power hierarchy: Iso &sube; SubIso &sube; Mono. Each finds everything the previous finds, plus more.

### Cluster Types

**`get`** (accept): Required structure. Mapping contributes to the result.

**`ban`** (reject / NAC): If the cluster's elements are found via its morphism, the entire match is rejected. The morphism type controls rejection breadth:

| Ban morphism | Rejection breadth |
|--------------|-------------------|
| Iso | Narrowest — only exact matches rejected |
| SubIso | Medium |
| Mono | Broadest — any injective match triggers rejection |

### "Exactly N" Patterns

Modeled with N `get` clusters + 1 `ban` cluster, all using distinct pattern node IDs. Global injectivity ensures each `get` maps to different target nodes. The `ban` checks for an (N+1)th instance.

Example — "exactly 2 triangles":

```rust
let q = search![
    get(SubIso) {
        N(0) >> E() >> N(1) >> E() >> N(2),
        n(0) >> E() >> n(2),
    },
    get(SubIso) {
        N(3) >> E() >> N(4) >> E() >> N(5),
        n(3) >> E() >> n(5),
    },
    ban(SubIso) {
        N(6) >> E() >> N(7) >> E() >> N(8),
        n(6) >> E() >> n(8),
    },
];
```

The iterator yields all valid assignments, including permutations of which `get` maps to which target triangle.

### Invalid Pattern Detection

Two clusters invalidate the pattern when:

- Both clusters share the same elements (via `n()` references)
- The `ban` cluster's morphism is &ge; the `get` cluster's morphism in matching power

Because every match found by the stricter morphism is also found by the broader one, so `ban` always fires.

| Ban cluster | Get cluster | Valid? |
|-------------|-------------|--------|
| Mono/ban | Mono/get | INVALID |
| Mono/ban | SubIso/get | INVALID |
| Mono/ban | Iso/get | INVALID |
| SubIso/ban | SubIso/get | INVALID |
| SubIso/ban | Iso/get | INVALID |
| SubIso/ban | Mono/get | VALID |
| Iso/ban | Iso/get | INVALID |
| Iso/ban | SubIso/get | VALID |
| Iso/ban | Mono/get | VALID |

Clusters with the same structure but different pattern node IDs are always valid — no shared elements, no conflict.

### Why Not `once` (accept-distinct / PAC)

A `once` cluster pins its mapping for the rest of the search. If multiple mappings exist, which gets pinned depends on search order — non-deterministic from the user's perspective. This produces surprising, order-dependent results. Dropped in favor of `get` + `ban` combinations which are fully deterministic and exhaustively enumerable.

## Context Nodes

Pattern nodes pre-mapped to specific target graph nodes. Analogous to `X`/`x` in the modify DSL.

- `X(id)` — declares a context node (mapped to a specific target node at search time)
- `x(id)` — references a context node

Context nodes must appear in at least one `get` cluster. An `X` node appearing only in `ban` clusters is nonsensical (banning a node you're explicitly providing) and rejected at validation time.

```rust
let q = search![
    get(SubIso) {
        X(0) >> E() >> N(1) >> E() >> N(2),
        x(0) >> E() >> n(2),
    },
    ban(Mono) {
        x(0) >> E() >> N(3),
    },
];
```

### API: Typestate for Context

```rust
pub struct Unbound;
pub struct Ready;

pub struct Query<NV, ER: Edge, State> { ... }

impl<NV, ER: Edge> Query<NV, ER, Ready> {
    pub fn find_in<'g>(&self, graph: &'g Graph<NV, ER>) -> Matches<'g, NV, ER>;
}

impl<NV, ER: Edge> Query<NV, ER, Unbound> {
    pub fn bind(&self, ctx: Context) -> BoundQuery<NV, ER>;
}

impl<NV, ER: Edge> BoundQuery<'_, NV, ER> {
    pub fn find_in<'g>(&self, graph: &'g Graph<NV, ER>) -> Matches<'g, NV, ER>;
}
```

The compiled `Query` is reusable — bind different contexts, search different graphs:

```rust
let q = search![ get(Mono) { X(0) >> E() >> N(1) } ];

for center in interesting_nodes {
    let bound = q.bind(Context::from([(0, center)]));
    for m in bound.find_in(&graph) { ... }
}
```

## Constraint Expressivity

Supported from v1:

- **Structure**: topology, connectivity, edge direction (directed/undirected/any)
- **Attribute predicates**: `N(0).where(|v| v > 5)`, `E().where(|e| e.weight < 10)`
- **Wildcards**: pattern nodes that match any target node regardless of attributes
- **Negative structures**: `ban` clusters (more general than individual negative edges)

## Engine Architecture

### Shape-Aware Domain Computation

Shapes provide pre-computed structural information that narrows candidate sets per pattern node. Domain computation is not strict shape-type-to-type — compatibility depends on the cluster's morphism:

| Cluster morphism | Domain rule |
|------------------|------------|
| SubIso | Target node must have exactly compatible local structure — degree match, edge directions match, no extra edges between already-matched neighbors |
| Mono | Target node must have at least the pattern's local structure — degree &ge; pattern degree, required edges exist, extra edges irrelevant |
| Iso | SubIso rules + target graph size = pattern size |

Cross-type matching: a mono star pattern node can match target core members (core nodes are all-connected, which satisfies spoke adjacency plus allows extra inter-spoke edges under mono). Shapes serve as guardrails for fast candidate enumeration, not as strict type filters.

Domain computation runs at `find_in` time (target graph not known until then). The compiled `Query` precomputes what structural properties each pattern node requires; `domain.rs` maps those to target candidates using the shape index.

### Ban Cluster Scheduling

Ban clusters participate in the search plan based on their connectivity:

**Connected ban** (shares nodes with `get` clusters via `n()`/`x()` references): Checked eagerly as soon as all the ban cluster's nodes have assignments in the partial mapping. Prunes the search space immediately.

**Disconnected ban** (no shared nodes): Runs first as a precondition. If the banned structure exists anywhere in the graph, the entire query yields zero results. Early termination.

### Search Plan

The search plan is a schedule of pattern node assignments and constraint checkpoints:

1. **Disconnected ban preconditions** — check first, abort early if any fires
2. **Context nodes** — domain size = 1, instant assignment, propagate constraints
3. **Free nodes: most-constrained-first** — scored by (cluster morphism restrictiveness x domain size x connected constraint count). Smallest domain after propagation goes next.
4. **Ban-only nodes** — nodes belonging solely to `ban` clusters are searched maximal-to-minimal compatibility (find the forbidden structure fast)

At each step, all checkable constraints for the current partial mapping are evaluated:

- Edge feasibility for newly connected pattern nodes
- SubIso exactness checks (no extra edges between matched nodes)
- Partial ban cluster checks (can we already determine the ban will/won't fire?)
- Attribute predicates on the newly assigned node
- Injectivity across the global mapping

### Backtracking Engine

Sequential stack-based backtracking. Each `next()` call on the `Matches` iterator resumes from the stack, tries the next candidate, runs checkpoints, yields or continues.

```rust
pub struct Matches<'g, NV, ER: Edge> {
    query: &'g Query<NV, ER>,
    target: &'g Graph<NV, ER>,
    domains: Vec<Vec<id::N>>,
    mapping: FxHashMap<Id, id::N>,
    reverse: FxHashMap<id::N, Id>,
    stack: Vec<StackFrame>,
}

impl<'g, NV, ER: Edge> Iterator for Matches<'g, NV, ER> {
    type Item = Match;
}
```

### Future: Parallel Search

Sequential backtracking first. Future upgrade: worker threads explore different branches of the search tree, exchanging partial mappings via channels. The search plan and constraint checking logic are identical — only the exploration strategy changes from sequential stack to parallel work-stealing. The `Matches` iterator interface stays the same.

## Core Types

```rust
pub enum Morphism {
    Iso,
    SubIso,
    Mono,
}

pub enum Decision {
    Get,
    Ban,
}

pub struct Cluster {
    nodes: IdSet<id::P>,
    edges: Vec<NR<id::P>>,
    morphism: Morphism,
    decision: Decision,
}

pub struct Query<NV, ER: Edge, State> {
    pattern: Graph<NV, ER>,
    clusters: Vec<Cluster>,
    ordering: Vec<PatternNode>,
    constraints: Vec<Constraint>,
    _state: PhantomData<State>,
}

pub struct Match {
    node_map: FxHashMap<Id, id::N>,
    edge_map: Vec<(NR<id::N>, NR<id::N>)>,
}

pub struct Context {
    bindings: FxHashMap<Id, id::N>,
}
```

## DSL Syntax

```rust
let q = search![<NV, ER>
    get(SubIso) {
        N(0).val("S") >> E().val(1) >> N(1).val("X"),
        n(0) >> E().val(2) >> N(2).val("X"),
    },
    get(Mono) {
        N(3).val("E") >> E() >> n(2),
    },
    ban(Mono) {
        n(0) >> E().val(3) >> N(5).val("X"),
    },
];
```

- `get(Morphism) { ... }` — required structure, mapping in result
- `ban(Morphism) { ... }` — if found, reject match
- `N(id)` / `n(id)` — free pattern node / reference
- `X(id)` / `x(id)` — context node / reference
- `E()` — edge builder, same operator overloading as `graph!` and `modify!`
- Every node/edge must belong to at least one cluster
- `n()`/`x()` references cross cluster boundaries (shared elements)

## Module Structure

```
src/search/
    mod.rs              -- search! macro, Query, Match, re-exports
    dsl/
        mod.rs          -- cluster builders, N/n/X/x/E constructors
        node.rs         -- pattern node types (Free, Context), From impls, operators
        edge.rs         -- pattern edge types, operator overloading
        validate.rs     -- invalidity detection, X-only-in-ban check
    query/
        mod.rs          -- Query<NV, ER, State>, BoundQuery
        compile.rs      -- cluster IR -> pattern Graph + cluster set + search plan
        domain.rs       -- shape-aware candidate domain computation
        plan.rs         -- search ordering, ban scheduling, checkpoint generation
    engine/
        mod.rs          -- Matches iterator, Match result type
        backtrack.rs    -- sequential stack-based backtracking
        feasibility.rs  -- per-cluster morphism checks
        context.rs      -- Context, bind(), BoundQuery
    error.rs            -- InvalidPattern, BindError
```

## Cross-Validation

External solvers for correctness verification and comparative benchmarking. Behind `feature = "cross-validate"`.

### Reference Solvers

**Glasgow subgraph solver**: fastest known for subgraph iso. CLI with DIMACS input. Subprocess wrapper.

**vf3lib**: latest VF2-family evolution. CLI wrapper. Supports iso, sub-iso, mono.

Neither supports mixed graphs. Anydirected cross-validation runs directed and undirected edges separately as partial checks. Full mixed-graph correctness through hand-verified test cases.

### Test Structure

```
tests/
    search/
        mod.rs              -- test utilities, graph generators
        correctness.rs      -- shagra results == reference results
        benchmarks.rs       -- comparative timing
    search_refs/
        mod.rs              -- ReferenceSolver trait
        glasgow.rs          -- Glasgow subprocess wrapper
        vf3.rs              -- vf3lib subprocess wrapper
        format.rs           -- DIMACS/VF format serialization
```

### Correctness Properties

- For random graph pairs, shagra's match count equals reference solver's match count
- Mono results are a superset of sub-iso results (every sub-iso match is also a mono match)
- All three solvers agree on graph isomorphism (yes/no)
- Ban clusters correctly reduce match count compared to get-only queries
- Context nodes produce a subset of non-context results (pinning narrows, never expands)

### Benchmark Scenarios

- Sparse target (1000 nodes, avg degree 3), small pattern (10 nodes)
- Dense target (200 nodes, avg degree 20), clique-heavy pattern (8 nodes)
- Shape-structured target (clear cores + stars + paths), shape-aligned pattern (demonstrates domain pruning advantage)

## Algorithm Approach

Shape-domain VF2: VF2-style backtracking where the shape index provides pre-computed candidate domains per pattern node. Shapes capture global structural invariants (cliques, hubs, path topology) that local arc-consistency propagation cannot discover. This gives shagra tighter initial domains than Glasgow computes at runtime.

Upgrade path: add constraint propagation (Glasgow-style) within the domain-based architecture if simple backtracking proves insufficient on hard instances.
