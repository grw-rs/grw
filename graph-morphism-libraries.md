# Graph Morphism Libraries Survey

**Researched:** 2026-03-02
**Scope:** Libraries supporting graph morphism search (isomorphism, subgraph isomorphism, monomorphism, homomorphism) across all major languages, with emphasis on what Shagra can learn from, interface with, or surpass.

---

## Table of Contents

1. [Comparison Matrix](#comparison-matrix)
2. [Rust Libraries](#rust-libraries)
3. [C/C++ Libraries](#cc-libraries)
4. [Python Libraries](#python-libraries)
5. [Java Libraries](#java-libraries)
6. [Graph Databases / Query Languages](#graph-databases--query-languages)
7. [Isomorphism-Only Tools (Canonical Labeling)](#isomorphism-only-tools)
8. [Algorithm Taxonomy](#algorithm-taxonomy)
9. [Expressivity Analysis](#expressivity-analysis)
10. [Gaps in the Ecosystem](#gaps-in-the-ecosystem)
11. [Implications for Shagra](#implications-for-shagra)

---

## Comparison Matrix

### Morphism Type Support

| Library | Lang | Iso | SubIso (Induced) | SubIso (Non-Induced) / Mono | Homo | Algorithm |
|---------|------|-----|-------------------|------------------------------|------|-----------|
| **petgraph** | Rust | Y | Y | **N** | N | VF2 |
| **vf2 crate** | Rust | Y | Y | **N** | N | VF2 |
| **igraph** | C | Y | Y | Y (LAD) | N | VF2, LAD, BLISS |
| **Boost.Graph** | C++ | Y | Y | Y (`vf2_graph_mono`) | N | VF2 |
| **LEMON** | C++ | Y | Y | Y | N | VF2++ |
| **vf3lib** | C++ | Y | Y (node-induced) | Y (mono mode) | N | VF3, VF3L, VF3P |
| **RI** | C++ | Y | Y (induced) | Y (mono mode) | N | RI |
| **Glasgow** | C++ | N | Y | Y | N | CP + inference |
| **NetworkX** | Py | Y | Y | Y (`subgraph_monomorphisms_iter`) | N | VF2, VF2++, ISMAGS |
| **grandiso** | Py | N | Y | Y (default mode) | N | Grand-Iso |
| **JGraphT** | Java | Y | Y (induced only) | **N** | N | VF2 |
| **Neo4j/Cypher** | Query | N/A | non-induced (default) | Y (pattern match) | Y (default) | custom |
| **nauty/Traces** | C | Y | N | N | N | partition refinement |
| **BLISS** | C | Y | N | N | N | partition refinement |

### Pattern Expressivity

| Library | Node Attr | Edge Attr | Wildcards | Negative Pat | Direction (Mixed) | Named Bindings | Induced/Non-Ind |
|---------|-----------|-----------|-----------|--------------|-------------------|----------------|-----------------|
| **petgraph** | Y (weight eq) | Y (weight eq) | N | N | Dir or Undir, not mixed | N (index map) | Induced only |
| **vf2 crate** | Y (node_eq) | Y (edge_eq) | N | N | Dir or Undir, not mixed | N (index map) | Both |
| **igraph** | Y (colors) | Y (colors) | N | N | Dir or Undir, not mixed | N (index map) | Both (LAD) |
| **Boost.Graph** | Y (predicate) | Y (predicate) | N | N | Dir or Undir | N (callback) | Both |
| **LEMON** | Y (labels) | Y (labels) | N | N | Dir or Undir | N | Both |
| **vf3lib** | Y (labels) | Y (labels) | N | N | Dir or Undir | N | Both |
| **RI** | Y (attrs) | Y (attrs) | N | N | Dir or Undir, multigraph | N (index map) | Both |
| **Glasgow** | Y (labels) | Y (labels) | N | N | Dir or Undir | N | Both |
| **NetworkX** | Y (semantic) | Y (semantic) | N | N | Dir or Undir, multi | N (dict map) | Both (VF2) |
| **grandiso** | Y | Y | N | N | Dir or Undir | N | Both |
| **JGraphT** | Y (comparator) | Y (comparator) | N | N | Dir or Undir | N (mapping) | Induced only |
| **Neo4j/Cypher** | Y (property) | Y (property) | Y (`%`) | Y (WHERE NOT) | Y (mixed arrows) | Y (variables) | Non-induced default |

---

## Rust Libraries

### petgraph (isomorphism module)

**Source:** [docs.rs/petgraph/latest/petgraph/algo/isomorphism](https://docs.rs/petgraph/latest/petgraph/algo/isomorphism/index.html)
**Confidence:** HIGH (official docs verified)

**Maturity:** The most widely-used Rust graph library. ~16M total downloads on crates.io. Actively maintained.

**Morphism types:**
- Graph isomorphism: `is_isomorphic()`, `is_isomorphic_matching()`
- Subgraph isomorphism (induced): `is_isomorphic_subgraph()`, `is_isomorphic_subgraph_matching()`
- Enumerate all mappings: `subgraph_isomorphisms_iter()`
- **No monomorphism** (non-induced subgraph matching)
- **No homomorphism**

**Algorithm:** VF2 (original Cordella et al. 2004)

**Expressivity:**
- Node attribute matching: Yes, via custom matching function in `_matching` variants
- Edge attribute matching: Yes, same mechanism
- Wildcards: No
- Negative patterns: No
- Mixed graphs (dir+undir edges): **No** -- graphs are either all-directed or all-undirected
- Named bindings: No -- returns `Vec<(usize, usize)>` index mappings
- Induced vs non-induced: **Induced only** -- "In petgraph's context, 'subgraph' always means a 'node-induced subgraph'"

**Key limitation for Shagra:** No support for mixed (anydirected) graphs. No monomorphism. The VF2 implementation is basic (no VF2++ cutting rules). No query DSL.

---

### vf2 crate (OwenTrokeBillard/vf2)

**Source:** [github.com/OwenTrokeBillard/vf2](https://github.com/OwenTrokeBillard/vf2), [docs.rs/vf2/latest/vf2](https://docs.rs/vf2/latest/vf2/)
**Confidence:** HIGH (official docs + GitHub verified)

**Maturity:** 67 stars, 28 commits, last updated Sep 2024. v1.0. Small but focused.

**Morphism types:**
- Graph isomorphism: `isomorphisms()`
- Subgraph isomorphism (induced): `induced_subgraph_isomorphisms()`
- Subgraph isomorphism (non-induced): `subgraph_isomorphisms()`
- **Distinguishes induced vs non-induced** -- this is notable vs petgraph
- **No monomorphism explicitly named, but non-induced subgraph iso is functionally equivalent**
- **No homomorphism**

**Algorithm:** VF2 (without VF2++ cutting rules, noted as future improvement)

**API design:** Builder pattern:
```rust
let isomorphisms = vf2::isomorphisms(&query, &data)
    .node_eq(|n1, n2| n1 == n2)
    .edge_eq(|e1, e2| e1 == e2)
    .first();  // or .vec() or .iter()
```

**Expressivity:**
- Node attribute matching: Yes, via `node_eq()` closure
- Edge attribute matching: Yes, via `edge_eq()` closure
- Wildcards: No
- Negative patterns: No
- Mixed graphs: **No** -- directed or undirected, not mixed
- Named bindings: No -- returns index mappings
- Induced vs non-induced: **Both** (separate functions)

**Key advantage over petgraph:** Supports both induced and non-induced subgraph isomorphism. Cleaner builder API. Can work without petgraph (has own `Graph` trait).

**Key limitation for Shagra:** No mixed graph support. No query DSL. Performance likely behind VF2++/VF3/Glasgow on hard instances.

---

### subgraph-matching crate

**Source:** [crates.io/crates/subgraph-matching](https://crates.io/crates/subgraph-matching)
**Confidence:** LOW (minimal information available)

Marked as "work in progress and unstable." Not enough information to evaluate. Likely abandoned or very early stage.

---

### igraph-sys crate (Rust FFI bindings to igraph C)

**Source:** [crates.io/crates/igraph-sys](https://crates.io/crates/igraph-sys)
**Confidence:** MEDIUM

Raw FFI bindings to the igraph C library exist. However:
- The crate appears minimally maintained
- The `igraph` crate on crates.io (v0.1.1) is **not** a wrapper around C igraph -- it's an unrelated Rust-native graph data structure
- Using igraph via FFI would require building the C library and writing safe Rust wrappers around the subgraph isomorphism functions

**Feasibility for Shagra:** Possible but high effort. Would get access to VF2, LAD, BLISS algorithms. LAD is particularly interesting as it supports both induced and non-induced matching with vertex domain restrictions.

---

## C/C++ Libraries

### igraph (C library)

**Source:** [igraph.org/c/doc/igraph-Isomorphism.html](https://igraph.org/c/doc/igraph-Isomorphism.html)
**Confidence:** HIGH (official C documentation verified)

**Maturity:** Very mature, actively maintained. Widely used across Python, R, C interfaces. One of the most complete graph libraries.

**Morphism types:**
- Graph isomorphism: `igraph_isomorphic()`, `igraph_isomorphic_vf2()`, `igraph_isomorphic_bliss()`
- Subgraph isomorphism: `igraph_subisomorphic()`, `igraph_subisomorphic_vf2()`, `igraph_subisomorphic_lad()`
- Induced subgraph isomorphism: LAD with `induced=true`
- Non-induced subgraph isomorphism: LAD with `induced=false`
- Automorphism group: `igraph_automorphism_group()`, `igraph_count_automorphisms()`
- Canonical labeling: `igraph_canonical_permutation()`
- **No explicit homomorphism**

**Algorithms:**
- **VF2:** Full graph iso + subgraph iso. Supports vertex/edge colors and custom compatibility functions.
- **LAD (Solnon):** Subgraph iso with constraint propagation. Default for subgraph problems. Supports matching domain restrictions per vertex.
- **BLISS:** Fast isomorphism and canonical labeling. Successor to nauty.

**Expressivity:**
- Node attribute matching: Yes (vertex colors, compatibility functions)
- Edge attribute matching: Yes (edge colors, compatibility functions)
- Vertex domain restrictions: Yes (LAD) -- can specify allowed target vertices per pattern vertex
- Wildcards: No
- Negative patterns: No
- Mixed graphs: **No** -- directed or undirected
- Named bindings: No -- returns vertex mappings as arrays
- Induced vs non-induced: **Both** (LAD parameter)

**Rust FFI feasibility:** The C API is clean and well-documented. Building igraph from source via `cc` crate is feasible. The isomorphism API uses simple C types (arrays, function pointers).

---

### Boost.Graph (C++)

**Source:** [boost.org/doc/libs/latest/libs/graph/doc/vf2_sub_graph_iso.html](https://www.boost.org/doc/libs/latest/libs/graph/doc/vf2_sub_graph_iso.html)
**Confidence:** HIGH (official Boost docs verified)

**Maturity:** Part of Boost. Extremely stable, widely used. Not graph-specialized but comprehensive.

**Morphism types:**
- Graph isomorphism: `vf2_graph_iso()`
- Subgraph isomorphism (induced): `vf2_subgraph_iso()` -- "all edges of G which have both endpoints in V' are in E'"
- Monomorphism (non-induced): `vf2_graph_mono()` -- "these subgraphs need not be induced subgraphs"
- **All three variants explicitly supported**
- **No homomorphism**

**Algorithm:** VF2 (Cordella et al.)

**Expressivity:**
- Node attribute matching: Yes (VertexEquivalencePredicate)
- Edge attribute matching: Yes (EdgeEquivalencePredicate)
- Wildcards: No
- Negative patterns: No
- Mixed graphs: No
- Named bindings: Callback-based (user_callback receives mapping)
- Induced vs non-induced: **Both** (separate functions)

**Rust FFI feasibility:** Boost is C++ with heavy template usage. FFI from Rust is extremely painful. Not recommended.

---

### LEMON (C++) with VF2++

**Source:** [lemon.cs.elte.hu](https://lemon.cs.elte.hu/trac/lemon/ticket/597), [VF2++ paper](https://www.sciencedirect.com/science/article/pii/S0166218X18300829)
**Confidence:** MEDIUM (paper + project page, not direct API verification)

**Maturity:** Academic library, Boost license. The VF2++ implementation is the reference implementation from the algorithm's authors.

**Morphism types:**
- Graph isomorphism
- Induced subgraph isomorphism
- Non-induced subgraph isomorphism
- Specialized algorithms for each variant

**Algorithm:** VF2++ -- improved VF2 with:
- Better node matching order (based on graph structure analysis)
- More efficient cutting rules
- ~10x speedup over VF2 for induced subgraph isomorphism
- Better asymptotic behavior for graph isomorphism

**Expressivity:**
- Node/edge labels: Yes
- The API follows LEMON's graph template style

**Rust FFI feasibility:** LEMON is C++ with templates. FFI difficult but more feasible than Boost (cleaner codebase). Could potentially port the algorithm logic to Rust rather than wrapping.

**Key insight for Shagra:** VF2++ represents a significant improvement over VF2. If implementing a VF2-family algorithm, VF2++ cutting rules and node ordering are worth incorporating. The paper is the best reference for these improvements.

---

### vf3lib (MiviaLab)

**Source:** [github.com/MiviaLab/vf3lib](https://github.com/MiviaLab/vf3lib)
**Confidence:** HIGH (GitHub README verified)

**Maturity:** 40 commits, academic origin. Official implementations from the VF3 paper authors.

**Morphism types:**
- Graph isomorphism
- Subgraph isomorphism (node-induced)
- Monomorphism (non-induced, edge-induced)
- **All three explicitly supported and named**

**Algorithm variants:**
- **VF3:** Full algorithm with all heuristics. Best for large/dense graphs.
- **VF3L (Light):** Look-ahead disabled. Better for sparse/small graphs.
- **VF3P (Parallel):** Multi-core parallel VF3L.

**Expressivity:**
- Node/edge attributes: Yes (via file format labels)
- Directed: Yes (default)
- Undirected: Yes (via `-u` flag)
- Mixed: **No**

**Interface:** Command-line only. No library API designed for embedding.

**Rust FFI feasibility:** C++11, could be wrapped but the lack of a library API makes it awkward. Better to port the algorithm concepts.

**Key insight for Shagra:** VF3 claims to be fastest on large/dense graphs. The parallel variant (VF3P) is interesting for large pattern matching. However, the Glasgow Subgraph Solver has been shown to outperform VF3 on many hard instances.

**Python bindings:** vf3py (PyPI) provides Python/NetworkX interface as of 2025.

---

### RI (Reduced Inference)

**Source:** [github.com/InfOmics/RI](https://github.com/InfOmics/RI)
**Confidence:** HIGH (GitHub README verified)

**Maturity:** v3.6, released April 2020. 17 commits. Academic. MIT license.

**Morphism types:**
- Graph isomorphism (`iso` mode)
- Induced subgraph isomorphism (`ind` mode)
- Monomorphism (`mono` mode)
- **All three explicitly supported**

**Algorithm:** RI algorithm -- focuses on powerful pattern vertex ordering dependent on pattern topology, combined with light constraint verification. The key insight: a good search order + light checks beats heavy inference.

**Expressivity:**
- Node attributes: Yes
- Edge attributes: Yes
- Directed: Yes
- Undirected: Yes
- Multigraphs: Yes
- Mixed: Not explicitly stated but supports both dir and undir

**Interface:** Both C++ library API and command-line tool. No external dependencies.

**Rust FFI feasibility:** Pure C++ with no dependencies. Clean enough for FFI wrapping or algorithm porting.

**Key insight for Shagra:** RI's philosophy of "smart ordering + light checks beats heavy inference" is relevant to Shagra's shape-based approach. The shape decomposition (cores, stars, paths) could inform a search ordering strategy.

---

### Glasgow Subgraph Solver

**Source:** [github.com/ciaranm/glasgow-subgraph-solver](https://github.com/ciaranm/glasgow-subgraph-solver)
**Confidence:** HIGH (GitHub + paper verified)

**Maturity:** Active research project. MIT license. State-of-the-art for hard instances.

**Morphism types:**
- Induced subgraph isomorphism
- Non-induced subgraph isomorphism
- Maximum common subgraph
- Clique finding
- **No explicit graph isomorphism mode (would need both graphs same size)**
- **No homomorphism**

**Algorithm:** Constraint programming approach with:
- Bit-parallel adjacency constraint propagation
- Path-based inference
- Neighbourhood degree sequence pruning
- Restarts with learned nogoods
- Parallel search option

**Expressivity:**
- Node labels: Yes (labelled LAD format)
- Edge labels: Yes (labelled LAD format)
- Directed: Yes
- Undirected: Yes
- Side constraints: Extensible framework for additional constraints

**Interface:** Command-line only. No library API.

**Input formats:** LAD, Labelled LAD, CSV, DIMACS 2 (auto-detected).

**Rust FFI feasibility:** C++, command-line only. Would need to be invoked as subprocess or algorithm ported. Not suitable for tight integration.

**Key insight for Shagra:** The Glasgow solver is the gold standard for correctness and hard instances. Its use of constraint propagation with bit-parallel operations is conceptually similar to what Shagra could do with its shape index. The solver's approach of combining structural inference (paths, degree sequences) with systematic search is the most relevant paradigm for Shagra's future search module.

---

## Python Libraries

### NetworkX (isomorphism module)

**Source:** [networkx.org/documentation/stable/reference/algorithms/isomorphism.html](https://networkx.org/documentation/stable/reference/algorithms/isomorphism.html)
**Confidence:** HIGH (official docs verified, v3.6.1)

**Maturity:** The standard Python graph library. Extremely widely used. Actively maintained.

**Algorithms available:**
1. **VF2:** Full implementation with GraphMatcher/DiGraphMatcher classes
2. **VF2++:** Improved version with better node ordering and cutting rules (added in recent versions)
3. **ISMAGS:** Index-based Subgraph Matching Algorithm with General Symmetries -- symmetry-aware, avoids redundant isomorphisms

**Morphism types:**
- Graph isomorphism: `is_isomorphic()`, `vf2pp_is_isomorphic()`
- Subgraph isomorphism (induced): `GraphMatcher.subgraph_isomorphisms_iter()`
- Monomorphism (non-induced): `GraphMatcher.subgraph_monomorphisms_iter()`
- **Both induced and non-induced supported**
- **No homomorphism**

**Expressivity:**
- Node attribute matching: Yes (`node_match` parameter, semantic feasibility)
- Edge attribute matching: Yes (`edge_match` parameter)
- Wildcards: No
- Negative patterns: No
- Mixed graphs: No (DiGraph or Graph, not mixed)
- Named bindings: Returns dict mappings `{pattern_node: target_node}`
- Multigraphs: Yes (DiMultiGraphMatcher)

**ISMAGS notable feature:** Exploits symmetry in the query graph to avoid enumerating equivalent isomorphisms. If the pattern has automorphisms, ISMAGS only returns one representative per symmetry class. This is a significant optimization when the pattern is symmetric.

**Rust relevance:** Reference implementation quality. Good for correctness testing.

---

### grandiso-networkx

**Source:** [github.com/aplbrain/grandiso-networkx](https://github.com/aplbrain/grandiso-networkx)
**Confidence:** MEDIUM (GitHub verified)

**Maturity:** Pure Python. Focused on motif search in neuroscience. Actively maintained.

**Morphism types:**
- Subgraph monomorphism: Default mode
- Subgraph isomorphism: Optional strict mode
- **No graph isomorphism**

**Key innovation:** Queue-based state space (vs tree-based in VF2), enabling trivial parallelization. Claims to outperform NetworkX's VF2 on larger graphs with lower memory overhead.

---

## Java Libraries

### JGraphT

**Source:** [jgrapht.org](https://jgrapht.org/javadoc/org.jgrapht.core/org/jgrapht/alg/isomorphism/VF2SubgraphIsomorphismInspector.html)
**Confidence:** HIGH (official javadoc verified)

**Maturity:** The standard Java graph library. Very mature, actively maintained.

**Morphism types:**
- Graph isomorphism: `VF2GraphIsomorphismInspector`
- Subgraph isomorphism (induced): `VF2SubgraphIsomorphismInspector`
- **No monomorphism (non-induced)**
- **No homomorphism**

**Algorithm:** VF2 (Cordella et al.)

**Limitations:**
- Does not support multigraphs
- Induced subgraph isomorphism only

**Expressivity:**
- Node attribute matching: Yes (Comparator-based)
- Edge attribute matching: Yes (Comparator-based)
- Otherwise similar limitations to petgraph

---

## Graph Databases / Query Languages

### Neo4j / Cypher

**Source:** [neo4j.com/docs/cypher-manual/current/patterns/reference/](https://neo4j.com/docs/cypher-manual/current/patterns/reference/)
**Confidence:** HIGH (official docs verified)

Cypher is not a library but a query language. It represents the most expressive pattern matching system in this survey.

**Pattern matching semantics:**
- Default: **Homomorphism with relationship uniqueness** (nodes can repeat, edges cannot)
- `DIFFERENT RELATIONSHIPS` (default): edges unique per match
- `REPEATABLE ELEMENTS` (Cypher 25+): edges can repeat
- No node-isomorphism enforcement by default (two pattern variables can match same node)
- **Not subgraph isomorphism** -- it's more like path-based pattern matching

**Expressivity (BEST IN CLASS):**
- Node property constraints: `({name: "Alice", age: 30})`
- Edge property constraints: `-[:KNOWS {since: 2020}]->`
- Label expressions with boolean logic: `(n:Person&!Employee)`, `(n:Person|Company)`
- Wildcards: `%` matches any label
- **Negative patterns:** `WHERE NOT (a)-[:KNOWS]->(b)` and `NONE()` predicates
- **Direction constraints for mixed queries:** `<-[]-`, `-[]->`, `-[]-` (bidirectional)
- **Named bindings:** Every node/edge can be bound to a variable: `(person:Person)-[r:KNOWS]->(friend)`
- Variable-length paths: `-[:KNOWS*1..5]->`
- Quantified path patterns: `((a)-[:KNOWS]->(b)){2,5}`

**What Cypher teaches us about expressivity:**
1. Negative patterns are essential for real queries ("find triangles that are NOT part of a clique")
2. Property constraints on matched elements are basic expectations
3. Named bindings make results usable
4. Direction mixing in queries is natural for mixed graphs
5. Variable-length paths enable reachability queries alongside structural matching

**Limitation:** Cypher is not doing strict subgraph isomorphism. It's doing pattern matching with homomorphism-like semantics. This means it can return duplicate results where different pattern nodes map to the same graph node.

---

## Isomorphism-Only Tools

These tools solve graph isomorphism (are two graphs the same?) but NOT subgraph isomorphism. Included for completeness since igraph uses BLISS internally.

### nauty / Traces (Brendan McKay)

**Source:** [pallini.di.uniroma1.it](https://pallini.di.uniroma1.it/), [users.cecs.anu.edu.au/~bdm/nauty](https://users.cecs.anu.edu.au/~bdm/nauty/)
**Confidence:** HIGH

The gold standard for graph isomorphism and automorphism group computation. Portable C. v2.8.9 latest.

- Computes automorphism groups
- Produces canonical labels (two graphs are isomorphic iff their canonical labels are identical)
- Extremely fast for most practical graphs
- **No subgraph isomorphism support**

### BLISS (Junttila & Kaski)

**Source:** [users.aalto.fi/~tjunttil/bliss](https://users.aalto.fi/~tjunttil/bliss/)
**Confidence:** HIGH

Successor to nauty with better heuristics. Used inside igraph.

- Canonical labeling
- Automorphism groups
- Directed and undirected graphs
- Self-loops but not multi-edges
- **No subgraph isomorphism support**

---

## Algorithm Taxonomy

### Backtracking Family (VF2, VF2++, VF3, RI)

All based on the same fundamental approach:
1. Choose a pattern vertex
2. Try mapping it to a candidate target vertex
3. Check feasibility (syntactic + semantic)
4. Recurse to next pattern vertex
5. Backtrack on failure

**Differences are in:**
- **Matching order:** Which pattern vertex to try next (VF2++ and RI have the best ordering strategies)
- **Cutting rules:** How aggressively to prune the search space (VF2++ adds more rules)
- **Look-ahead:** How far to look ahead for conflicts (VF3 has configurable look-ahead depth)
- **Data structures:** Bit-parallel operations (Glasgow), hash-based (RI)

**Relative performance (from benchmarks in literature):**
- Small/sparse graphs: VF2, VF3L, RI all similar
- Large/dense graphs: VF3 > VF2++ > VF2
- Hard instances (many partial matches): Glasgow >> VF3 > VF2++
- Symmetric patterns: ISMAGS avoids redundant exploration

### Constraint Programming (Glasgow)

Fundamentally different approach:
1. Model as constraint satisfaction problem
2. Propagate constraints (arc consistency, path consistency)
3. Search with restarts and learned nogoods
4. Bit-parallel domain operations

**Advantages:** Best for hard instances. Can incorporate arbitrary side constraints. Proof logging.
**Disadvantages:** Higher constant overhead. Setup cost amortized over hard searches.

### LAD (igraph)

Hybrid approach:
1. Constraint propagation (like Glasgow, but lighter)
2. Backtracking search
3. Per-vertex domain restrictions

**Advantages:** Good balance of pruning power and overhead. Supports non-induced matching. Domain restrictions enable attribute-based filtering as constraint propagation rather than post-hoc checking.

---

## Expressivity Analysis

### What Shagra's search module would need (compared to ecosystem)

| Feature | Who Has It | Who Doesn't | Shagra Priority |
|---------|-----------|-------------|-----------------|
| Node attribute constraints | Everyone | -- | Must have |
| Edge attribute constraints | Everyone | -- | Must have |
| Induced subgraph iso | Most | JGraphT (only this) | Must have |
| Non-induced / monomorphism | igraph, Boost, LEMON, VF3, RI, NetworkX, Glasgow | petgraph, JGraphT | Must have |
| Mixed graph (dir+undir) | **Nobody as library** (Cypher queries only) | ALL libraries | **Shagra's differentiator** |
| Named binding extraction | Cypher | All libraries (return indices) | Should have |
| Negative patterns | Cypher | ALL libraries | Could have (future) |
| Wildcards | Cypher | ALL libraries | Could have (future) |
| Variable-length paths | Cypher | ALL libraries | Could have (future) |
| Symmetry-aware matching | ISMAGS | Everyone else | Nice to have |
| Parallel search | Glasgow, VF3P | Most others | Nice to have |

### The Mixed Graph Gap

**This is the most significant finding.** No graph morphism library natively supports mixed graphs (graphs with both directed and undirected edges in the same structure). Every library requires choosing directed OR undirected at graph creation time.

The academic literature on mixed graph isomorphism exists (Mohammed 2017, "Mixed Graph Representation and Mixed Graph Isomorphism") but no production software implements it.

Shagra's "anydirected" graph model is genuinely novel in the software library space. Any search module Shagra builds for mixed graphs would be the first of its kind as a library.

**Implication:** Cannot reuse existing algorithms directly. Must extend VF2/VF2++/CP to handle edge directionality as a constraint dimension. The feasibility check must consider: directed-directed (must match direction), undirected-undirected (always matches), directed-undirected (configurable: strict=no match, relaxed=match).

---

## Gaps in the Ecosystem

### 1. No mixed-graph morphism library exists
As documented above. Shagra fills this gap.

### 2. No Rust library supports monomorphism
petgraph: induced only. vf2 crate: has non-induced but calls it "subgraph isomorphism" not monomorphism. Neither has the formal monomorphism that maps every pattern edge to exactly one target edge without the induced constraint.

### 3. No library provides a pattern query DSL
All libraries require constructing a pattern graph programmatically. None offer a query language for expressing patterns (Cypher is a database query language, not a library feature). A Rust macro-based pattern DSL would be novel.

### 4. No library integrates structural indexing with morphism search
All libraries perform search from scratch. None pre-index the target graph's structure to accelerate repeated queries. Shagra's shape decomposition (cores, stars, paths) could serve as a search index, narrowing candidates before backtracking begins.

### 5. Homomorphism is largely unsupported
Only Cypher supports it (as its default semantics). No combinatorial library offers graph homomorphism search. This is a real gap -- homomorphisms are useful for pattern queries where you care about structure, not identity.

---

## Implications for Shagra

### What to learn from

1. **VF2++ (LEMON):** Best node ordering strategy and cutting rules in the VF2 family. The paper is essential reading for any VF2-based implementation.

2. **Glasgow Solver:** Constraint propagation approach. Shagra's shape index could serve as pre-computed domain information, similar to how Glasgow computes domains at search time. This is the most architecturally aligned approach.

3. **RI algorithm:** The insight that smart ordering beats heavy inference is relevant. Pattern vertex ordering based on shape topology (e.g., start from core vertices, expand through stars) could be very effective.

4. **igraph LAD:** Domain restrictions per pattern vertex. Shagra's shape decomposition naturally provides this: "this pattern core can only match target cores with same/similar dimension."

5. **ISMAGS:** Symmetry exploitation. If Shagra's patterns have automorphisms (which structural patterns often do), avoiding redundant exploration is important.

6. **Cypher:** Expressivity model. Named bindings, negative patterns, property constraints, direction control -- these define the user-facing API expectations.

### What Shagra can uniquely offer

1. **Mixed-graph morphism search:** First library to natively handle directed + undirected edges
2. **Shape-indexed search:** Pre-computed structural decomposition accelerates repeated pattern queries
3. **Rust-native performance:** No FFI overhead, no GC, SIMD-friendly bit operations
4. **Type-safe pattern construction:** Leveraging Rust's type system and Shagra's existing `graph!` macro for compile-time safe patterns

### Recommended algorithm approach

**Phase 1:** VF2-based algorithm extended for mixed graphs, using shape index for candidate filtering.
- Reason: VF2 is well-understood, easy to implement correctly, and the extensions for mixed graphs are straightforward.

**Phase 2:** Incorporate VF2++ improvements (node ordering, cutting rules).
- Reason: ~10x performance improvement for induced subgraph isomorphism.

**Phase 3:** Add constraint propagation (Glasgow-style) using shape index as pre-computed domains.
- Reason: This is where Shagra's architecture pays off -- the shape decomposition IS domain information.

### Recommended morphism support order

1. Subgraph isomorphism (induced) -- most common use case
2. Graph isomorphism -- simplest, good for testing
3. Monomorphism (non-induced) -- needed for practical pattern matching
4. Homomorphism -- least common but fills ecosystem gap

---

## Sources

### Official Documentation (HIGH confidence)
- [petgraph isomorphism module](https://docs.rs/petgraph/latest/petgraph/algo/isomorphism/index.html)
- [vf2 crate docs](https://docs.rs/vf2/latest/vf2/)
- [igraph C isomorphism reference](https://igraph.org/c/doc/igraph-Isomorphism.html)
- [Boost.Graph VF2 docs](https://www.boost.org/doc/libs/latest/libs/graph/doc/vf2_sub_graph_iso.html)
- [NetworkX isomorphism module](https://networkx.org/documentation/stable/reference/algorithms/isomorphism.html)
- [NetworkX VF2 docs](https://networkx.org/documentation/stable/reference/algorithms/isomorphism.vf2.html)
- [JGraphT VF2SubgraphIsomorphismInspector](https://jgrapht.org/javadoc/org.jgrapht.core/org/jgrapht/alg/isomorphism/VF2SubgraphIsomorphismInspector.html)
- [Neo4j Cypher patterns reference](https://neo4j.com/docs/cypher-manual/current/patterns/reference/)
- [nauty/Traces home](https://pallini.di.uniroma1.it/)
- [BLISS documentation](https://users.aalto.fi/~tjunttil/bliss/)

### GitHub Repositories (HIGH confidence)
- [Glasgow Subgraph Solver](https://github.com/ciaranm/glasgow-subgraph-solver)
- [vf3lib](https://github.com/MiviaLab/vf3lib)
- [RI](https://github.com/InfOmics/RI)
- [vf2 Rust crate](https://github.com/OwenTrokeBillard/vf2)
- [grandiso-networkx](https://github.com/aplbrain/grandiso-networkx)

### Academic Papers (referenced but not directly verified)
- Cordella et al. "A (sub)graph isomorphism algorithm for matching large graphs" (2004) -- VF2
- Juttner & Madarasi "VF2++ -- An improved subgraph isomorphism algorithm" (2018) -- VF2++
- Carletti et al. "Introducing VF3: A New Algorithm for Subgraph Isomorphism" (2017) -- VF3
- Bonnici et al. "A subgraph isomorphism algorithm and its application to biochemical data" (2013) -- RI
- McCreesh et al. "The Glasgow Subgraph Solver" (2020) -- Glasgow
- Solnon "AllDifferent-based filtering for subgraph isomorphism" (2010) -- LAD
