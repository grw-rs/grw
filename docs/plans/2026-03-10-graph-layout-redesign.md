# Graph Layout Redesign

## Motivation

Shapes (cores, stars, paths, dots) were designed to accelerate search by pruning candidate nodes based on structural classification. Analysis of crosscheck benchmarks at 25k nodes shows:

- **Unvalued SubIso:** shagra 5-19x faster than VF3 — but this comes from CSR cache layout + count_leaf_fused optimization, not shapes
- **Valued SubIso:** roughly at parity with VF3 after removing CSR construction from timing
- **Shape pruning for SubIso:** only CoreMember and StarCenter roles use shape-based candidates at depth-0. StarMember, PathNode, Dot, Free all fall through to scanning all nodes.
- **Modify pipeline:** desc/bat/mrg/lnk/idx phases exist solely to maintain shapes. At 25k nodes this takes ~28ms per modification batch.

Conclusion: shapes add significant modify complexity for negligible search benefit on SubIso (the dominant use case). Degree-sorted indexing provides equivalent filtering at near-zero maintenance cost.

## Current Layout (problems)

| Component | Container | Issue |
|-----------|-----------|-------|
| Nodes | `FxHashMap<id::N, Node>` | Unordered, no degree access, pointer-chasing |
| Adjacency | Roaring bitmap per node | Poor iteration cache locality, overkill for avg degree 4-10 |
| Edges | `BTreeMap<(NR, Slot), Val>` | O(log m) per lookup, scattered memory, NR key construction on every access |
| Shapes | Cores/Stars/Paths stores + indices | Expensive to maintain, barely helps SubIso search |

## Proposed Layout

```rust
struct Node<NV> {
    val: NV,
    adj: SmallVec<[(u32, u32); 8]>,  // (neighbor_id, edge_idx) sorted by neighbor_id
}

struct Graph<NV, EV> {
    nodes: Vec<Option<Node<NV>>>,
    node_ids: IdSpace,

    edges: Vec<Option<EV>>,  // EV determined by edge type (see Edge Storage below)
    edge_ids: IdSpace,

    degree_buckets: Vec<(u32, IdSet<N>)>,  // sorted by degree descending
}
```

### Node Storage

- `Vec<Option<Node>>` indexed by node ID. Removing node at index 0 leaves `None`, not a shift of 1M elements.
- `IdSpace` tracks free node IDs for reuse after removal.
- Node value and adjacency list are collocated: accessing a node for search gives both val (for predicate check) and adj (for neighbor traversal) in the same cache line(s).
- SmallVec inline capacity of 8 entries = 64 bytes, covers avg degree 4-10 without heap allocation.

### Adjacency Representation

Sorted `SmallVec<[(u32, u32); 8]>` per node replaces roaring bitmaps.

At avg degree 4-10:
- Contains check: binary search on 4-10 elements, 2-3 comparisons
- Iteration: sequential scan, perfectly cache-friendly
- Insert/remove: sorted insert with memmove of a few u32 pairs
- SIMD potential: can compare 8 neighbor IDs at once with AVX2

Roaring bitmaps shine at high cardinality (thousands of bits). At degree 4-10 they're overhead.

### Edge Storage

Flat `Vec<Option<EV>>` indexed by edge_idx, allocated from `edge_ids: IdSpace`.

One edge entry per node pair. Both endpoint nodes store the same `edge_idx` in their adjacency lists. Edge value type varies by edge kind:

**Undirected (`edge::Undir<V>`):**
- Single value per edge. `edges[edge_idx] = Some(val)`.

**Directed (`edge::Dir<V>`):**
- Up to two independent directed edges per node pair (A->B and B->A).
- Internal slots: fwd (lo->hi) and rev (hi->lo), determined by normalization convention `min(A,B) < max(A,B)`.
- `edges[edge_idx] = Some(DirSlots { fwd: Option<V>, rev: Option<V> })`

**Anydir (`edge::Anydir<V>`):**
- Up to three slots per node pair: fwd directed, rev directed, undirected.
- `edges[edge_idx] = Some(AnydirSlots { fwd: Option<V>, rev: Option<V>, und: Option<V> })`

### Edge Operations

**Adding edge A-B:**
```
edge_idx = edge_ids.alloc()
edges[edge_idx] = Some(value)
adj[A].sorted_insert((B, edge_idx))
adj[B].sorted_insert((A, edge_idx))
degree_buckets: move A from bucket d to d+1, same for B
```

**Looking up edge between A and B:**
```
binary_search adj[A] for B → (B, edge_idx) → edges[edge_idx]
                                                ↑ O(1)
                              ↑ O(log degree)
```

No NR type, no BTreeMap. The adjacency list IS the (node, node) -> edge_id mapping.

**Removing edge A-B:**
```
binary_search adj[A] for B → get edge_idx, remove entry
binary_search adj[B] for A → remove entry
edges[edge_idx] = None
edge_ids.free(edge_idx)
degree_buckets: move A from bucket d to d-1, same for B
```

### Normalization and Slots

Normalization (which node ID is lower) remains as an internal convention for interpreting directed edge slots. The user never sees fwd/rev/slots.

**User API (unchanged):**
- `D(5, 3)` — directed from 5 to 3. Always presented as `D(5, 3)`.
- `U(5, 3)` — undirected. Always normalized to `U(3, 5)` (lo, hi) on both input and output.

**Internal slot resolution:**
```
D(5, 3):
  pair normalized to (3, 5) since 3 < 5
  source=5 is the higher node → "rev" slot
  edges[edge_idx].rev = Some(val)

  Lookup from node 5, neighbor 3: 5 > 3 → read rev slot
  Lookup from node 3, neighbor 5: 3 < 5 → read rev slot
  Both reconstruct D(5, 3)
```

The Edge trait handles the translation. `edge::Dir`, `edge::Undir`, `edge::Anydir` each know their slot layout and how to map `(from, to)` to the internal slot.

### Degree Index

`Vec<(u32, IdSet<N>)>` sorted by degree descending. Bucketed by degree value.

**Modification:** degree changes from d to d+1:
```
degree_buckets[d].remove(node_id)    // O(1) bitmap op
degree_buckets[d+1].insert(node_id)  // O(1) bitmap op
```
If bucket d+1 doesn't exist, insert new `(d+1, IdSet)` — binary search + insert into small Vec (number of distinct degrees is bounded, ~50-100 unique values even at 1M nodes).

**Search query** "all nodes with degree >= k": binary search for k in the bucket list, iterate buckets from there. Each bucket provides an IdSet that can be intersected with val_filtered if needed.

## What Goes Away

- Shape decomposition (cores, stars, paths, dots classification)
- Entire modify pipeline: desc/bat/mrg/lnk/idx phases
- NR type (normalization is just a `<` comparison at access time)
- BTreeMap for edges
- Roaring bitmaps for adjacency
- CSR rebuild cost (CSR can still be built lazily for search if beneficial, or search can work directly on the adj lists)

## Search Engine Impact

The search engine currently builds a CSR (`GraphIndex`) from the graph for cache-friendly access. With the new layout:

- `adj` is already sorted and contiguous per node — similar cache profile to CSR for neighbor iteration
- Edge values are O(1) array access instead of O(log m) BTreeMap
- Degree-based candidate filtering via degree_buckets replaces shape-based filtering
- val_filtered pre-scan works unchanged on contiguous node_vals (accessed via `nodes[i].val`)
- count_leaf_fused optimization works unchanged

Whether a separate CSR is still worth building depends on benchmarks. The new layout may be fast enough that CSR becomes unnecessary.

## SIMD Opportunities

With sorted `Vec<u32>` adjacency and contiguous arrays:

1. **Adjacency check:** load 8 neighbor IDs with AVX2, broadcast target, compare — single SIMD op for degree <= 8
2. **Neighbor intersection:** SIMD shuffle-based sorted merge intersection (~4x over scalar)
3. **Candidate filtering:** SIMD scan of contiguous degree array for degree >= k
4. **val_filtered:** SIMD broadcast+compare for equality predicates on contiguous node_vals
