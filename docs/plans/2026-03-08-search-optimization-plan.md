# Search Engine Optimization Plan (2026-03-08)

Ordered by expected impact. Each step: implement, measure, commit or revert.

Benchmark command:
```bash
cargo run --release --features crosscheck --bin verify_patterns -- \
  --graph-dir data/crosscheck/undir/{1458n_6411e,5458n_15117e,10458n_30174e,29374n_81643e} \
  --morphisms subiso:7 \
  --patterns motif_triangle,motif_path3,motif_path4,motif_star4,motif_cycle4,extracted_3,extracted_4,extracted_5,extracted_7 \
  --engine shape_match --timed
```

Correctness check (small graph, all morphisms):
```bash
cargo run --release --features crosscheck --bin verify_patterns -- \
  --graph-dir data/crosscheck/undir/154n_186e \
  --morphisms iso,subiso:7,mono:5 \
  --patterns extracted_3,extracted_4,extracted_5,extracted_7,motif_triangle,motif_clique4,motif_cycle4,motif_star4,motif_path3,motif_path4 \
  --engine shape_match
```

Baseline (2ecd115, shape_match ms, subiso):
```
                  1458n     5458n     10458n    29374n
extracted_3       6.67      18.22     23.28     62.18
extracted_4       58.85     81.81     99.42     307.48
extracted_5       437.95    292.12    575.00    1498.53
extracted_7       433.64    5471.83   17383.0   36435.3
motif_cycle4      33.13     33.68     72.00     198.04
motif_path3       7.80      11.36     23.93     71.19
motif_path4       67.19     57.21     127.59    445.17
motif_star4       48.50     46.32     97.41     304.05
motif_triangle    1.19      1.97      2.99      9.11
```

---

## Phase 1: Symmetry breaking (automorphism pruning)

**Why first:** Patterns with automorphisms count every symmetric mapping redundantly. Triangle has 6 automorphisms, star4 has 6, clique4 has 24, path3 has 2, path4 has 2, cycle4 has 8. For COUNT_ONLY mode, we can compute the automorphism group at plan time and impose canonical ordering constraints that eliminate redundant exploration branches. This is a multiplicative reduction — divides work by the automorphism count.

**Approach:**
- At plan time (`Query` compilation), detect automorphisms of the pattern using the search order. For each pair of nodes that are interchangeable under some automorphism, add an ordering constraint: `mapping[i] < mapping[j]` (or `<=` for homo).
- Store constraints as pairs `(pattern_idx_a, pattern_idx_b)` in the Query.
- In `candidates_for_into` or feasibility, enforce the constraint: skip candidate if it violates ordering relative to already-mapped symmetric partner.
- For `Iterator::next()` (non-count), multiply reported count or enumerate with unfolding — but start with COUNT_ONLY only.

**Expected impact:** Up to 6x for triangle, 2x for paths, 6x for star4, 8x for cycle4. Extracted patterns likely have fewer symmetries. The larger the automorphism group, the bigger the win.

**Measure:** Compare triangle, star4, cycle4, path3, path4 times before/after.

---

## Phase 2: Fused candidate generation + feasibility (eliminate intermediate Vec)

**Why second:** Every depth transition allocates/fills a `Vec<id::N>` of candidates, then iterates it again for feasibility. For small candidate sets this is fine, but for large sets (subiso on high-degree nodes) the allocation + second pass is measurable overhead. Fusing into a single-pass iterator that generates and checks feasibility inline eliminates the Vec entirely.

**Approach:**
- Replace `candidates_for_into` + separate feasibility loop in `advance()` with an inline iterator that yields only feasible candidates directly from the CSR neighbor list.
- The anchor-based path (most common) iterates `csr.neighbors(anchor)`, applies degree/class/injectivity/forward-adjacency filters, then immediately checks feasibility — all in one loop body.
- For the leaf batch optimization (depth N-2 → N-1), fuse similarly: iterate anchor neighbors, check feasibility, count — no intermediate buffer.
- Keep `candidate_pool` for the non-anchor fallback paths (active_group, bindings) where materialization is needed.

**Expected impact:** 5-15% on patterns with large candidate sets at interior depths. Biggest effect on extracted_5/extracted_7 where candidate lists are long.

**Measure:** Compare extracted_5, extracted_7, path4, star4 across graph sizes.

---

## Phase 3: Candidate ordering (fail-first heuristic)

**Why third:** Currently candidates are tried in CSR order (node id). A fail-first ordering — trying candidates most likely to fail feasibility early — prunes the search tree sooner. Particularly effective for patterns where most candidates at a depth fail at the next depth.

**Approach:**
- After generating candidates for a depth, sort by ascending target degree. Lower-degree candidates have fewer neighbors → fewer valid extensions at deeper depths → prune earlier.
- Only sort when candidate count exceeds a threshold (e.g., >16) to avoid sort overhead on small sets.
- For COUNT_ONLY this doesn't change correctness, only visit order.
- Alternative/complement: sort by number of already-mapped neighbors (more constrained = fewer candidates at next depth = try first).

**Expected impact:** Hard to predict — depends on pattern structure. Could be 10-30% on tree-like patterns (path4, extracted_5) where wrong early choices lead to large wasted subtrees. May be negligible on dense patterns (triangle, clique4) where all candidates are equally constrained.

**Measure:** Compare extracted_5, extracted_7, path4 times. If regression on some patterns, make it conditional.

---

## Phase 4: Per-node compact adjacency hash (O(1) is_adjacent)

**Why fourth:** `is_adjacent` uses binary search on sorted CSR neighbors — O(log d). Called millions of times in feasibility checks. Replacing with O(1) lookup would help on high-degree nodes.

**Approach:**
- For each node with degree > threshold (e.g., 32), build a compact hash set of its neighbors alongside the CSR array. Use a simple open-addressing hash (power-of-2 size, 70% load factor).
- Store as `Option<Vec<u32>>` per node (None for low-degree, Some(hash_table) for high-degree). The hash table is a flat `Vec<u32>` with sentinel values for empty slots.
- `is_adjacent` dispatches: if degree <= threshold, binary search on CSR (already fast for small d). If degree > threshold, hash probe.
- Memory cost: for a node with degree d, hash table = ~1.4d × 4 bytes. At 29k nodes, only ~10% have degree > 32, so overhead is modest (~50-100KB extra).

**Expected impact:** 10-20% on graphs with high-degree hubs where the hot path is feasibility checking. Less impact on uniform-degree graphs. Binary search at degree 32 is ~5 comparisons, hash is ~1.4 probes — 3-4x faster per call.

**Measure:** Compare across all graph sizes. Expect bigger wins on denser graphs (the `_6411e` / `_30174e` / `_81643e` variants).

---

## Phase 5: CSR neighbor intersection for multi-mapped feasibility

**Why fifth:** `is_feasible_fast` checks adjacency to each already-mapped pattern neighbor independently — k binary searches (or hash probes after Phase 4). When k >= 3, a single sorted merge-intersection of the candidate's neighbor list with the set of mapped nodes could be faster.

**Approach:**
- In `is_feasible_fast`, when the number of mapped neighbors (popcount of `adj_bits & mapped_bits`) >= 3, switch to intersection mode: collect mapped target node ids into a small sorted array, then do a single linear scan of `csr.neighbors(candidate)` checking membership in the sorted array.
- For k < 3, keep the current per-neighbor check (overhead of intersection setup not worth it for 1-2 checks).

**Expected impact:** 5-10% on deep patterns (extracted_7) where inner depths have 3+ mapped neighbors. Negligible on shallow patterns.

**Measure:** Compare extracted_7 times specifically.

---

## Phase 6: Shape-aware subiso candidate tightening

**Why sixth:** For subiso, several shape roles fall back to "all target nodes" as candidates (StarMember, PathNode, Dot, Free). Using shape dimension constraints could tighten these sets.

**Approach:**
- StarMember subiso: instead of all nodes, collect nodes from stars with total >= star_total. A 3-arm pattern star member can't appear in a 2-arm target star.
- PathNode subiso: collect nodes from paths with total >= path_total.
- Dot subiso: only dot-classified nodes (degree 1 nodes not in paths).
- These are depth-0 candidates only — interior depths use anchor-based generation which is already tight.

**Expected impact:** Depends on how many false candidates currently reach depth-0 and get filtered at depth-1. Could be significant for patterns starting with a loosely-constrained node. Likely modest (5-10%) since shape filtering already happens for CoreMember and StarCenter.

**Measure:** Compare star4, path3, path4, extracted_3 subiso times.

---

## Phase 7: Bitset reverse mapping for injectivity

**Why last:** The current `reverse[n] != UNMAPPED` check is already a single array lookup — hard to beat. A bitset (29k nodes = 3.6KB) would be more cache-friendly but the improvement is marginal since `reverse[]` at 116KB still fits in L2.

**Approach:**
- Add a `mapped_bitset: Vec<u64>` alongside `reverse[]`. Set/clear bits on map/unmap. Check with `(mapped_bitset[n/64] >> (n%64)) & 1`.
- Keep `reverse[]` for the actual reverse-only feasibility checks (needs the mapped pattern index, not just a boolean).

**Expected impact:** 1-5%. Only helps if `reverse[]` is causing cache misses, which is unlikely at these graph sizes. More useful at 100k+ nodes.

**Measure:** Compare across 29374n and larger graphs. If negligible, don't merge.
