# Search Engine Profiling Results (2026-03-11)

## Setup

- **Binary**: `prof_search` (no VF3, shagra-only)
- **Graph**: 500k nodes, 1M edges, avg degree 4, Anydir<i32> (75% undirected, 25% directed)
- **Node labels**: i32, 5 distinct values (0-4)
- **Edge values**: i32 (0-9)
- **Pattern**: 6 nodes, 5 edges, path-like (two hubs), seed=1
- **Collection**: `.collect()` (not count-only)
- **Matches**: 233 per iteration, ~145ms/iter
- **Profiling**: `perf record -g --call-graph fp`, no LTO, `debuginfo=2`, `force-frame-pointers=yes`
- **Samples**: 17k over 20 iterations (~3s search time)

## Build command

```bash
CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_DEBUG=2 RUSTFLAGS="-C force-frame-pointers=yes" \
  cargo build --release --bin prof_search

perf record -o /tmp/shagra_perf.data -g --call-graph fp \
  target/release/prof_search 500000 6 20 1
```

## Top Functions (self-time, % of total)

| Function | Self % | Description |
|---|---|---|
| `is_feasible` | 19.0% | Binary search adjacency + edge slot checks + edge predicates |
| `FnMut::call_mut` | 9.7% | Indirect call overhead for `Box<dyn Fn>` node/edge predicates |
| `lookahead_ok` | 7.7% | Bitwise tzcnt loop counting mapped neighbors |
| `SearchState::new` | 7.5% | `compute_search_order`: scans ALL 500k nodes per pattern node via dyn Fn |
| `from_iter` (collect) | 5.9% | Vec allocation for match collection |
| `Iterator::next` | 5.1% | Main backtracking loop (advance) |
| `Free::val::{{closure}}` | 3.1% | Node value equality body: `\|nv\| nv == &expected` |
| `BTreeMap::insert` (×2) | 3.6% + 3.4% | Graph construction (not search) |
| `candidates_for_into` | 2.6% | Candidate generation |
| `build_csr_store` | 2.6% | CSR construction (not search) |
| `initial_candidates` | 2.5% | First-depth candidate set |
| `CsrAdj::build` | 1.1% | CSR construction (not search) |
| `best_mapped_neighbor` | 0.8% | Anchor selection |

## Inclusive overhead (children)

| Function | Inclusive % | Self % |
|---|---|---|
| `Iterator::next` (search loop) | 49.5% | 5.1% |
| `is_feasible` | 34.2% | 19.0% |
| `FnMut::call_mut` | 24.7% | 9.7% |
| `candidates_for_into` | 21.4% | 2.6% |
| `Free::val::{{closure}}` | 14.0% | 3.1% |
| `SearchState::new` | 8.3% | 7.5% |
| `lookahead_ok` | 8.2% | 7.7% |

## Non-search overhead (~21%)

Graph construction, BTreeMap inserts, CSR building, from_nodes_edges — not in the search hot path.

## Key Findings

### 1. Node predicate vtable overhead (12.8% combined)

`FnMut::call_mut` (9.7%) + `Free::val::{{closure}}` (3.1%) = 12.8% total for node value filtering.

The closure body is trivial (`|nv| nv == &expected_val`), but it's dispatched through `Box<dyn Fn(&NV) -> bool>` vtable indirection. Called millions of times during candidate filtering in `candidates_for_into`.

**Fix**: Monomorphize node predicates — replace `Box<dyn Fn>` with enum-dispatched or generic predicates.

### 2. compute_search_order full scan (7.5%)

`SearchState::new` → `compute_search_order` (mod.rs:275-354) iterates ALL 500k target nodes for each of the 6 pattern nodes, calling the `dyn Fn` predicate each time. That's 3M+ indirect calls just to count candidates for search ordering.

**Fix**: Pre-compute candidate counts using a value-indexed structure (`HashMap<NV, Vec<Id>>` or `HashMap<NV, usize>`) during CSR build. Then `compute_search_order` becomes O(pattern_nodes) lookups instead of O(pattern_nodes × target_nodes) scans.

### 3. is_feasible (19%) — mixed costs

Not just binary search (which was the SIMD target). Breakdown within `is_feasible`:
- Binary search loop (`binary_search` on sorted neighbors): ~4-5%
- Edge slot computation (reverse_slot, actual_slot): ~2-3%
- Edge predicate calls (`edge_check` → `csr_any_match` → `dyn Fn`): ~2-3%
- Adjacency lookup overhead (CSR offset loads, bounds checks): ~3-4%
- Loop overhead (iterating `query.adj[pattern_idx]`): ~3-4%

SIMD on the binary search loop would only save ~4-5% of the 19% — explaining why it showed no measurable improvement.

### 4. lookahead_ok (7.7%) — tight bitwise loop

The inner loop uses `tzcnt` to iterate set bits of `pattern_adj_bits`, checking `mapping[neighbor_idx] != UNMAPPED` for each. Very tight code, hard to optimize further without algorithmic changes. Called frequently as a pre-filter before `is_feasible`.

### 5. Match collection overhead (5.9%)

`from_iter` / Vec allocation for `.collect()` is 5.9%. This is inherent to the collect-all workflow. The count-only path (`count_leaf_fused`) avoids this entirely.

## Optimization Priority (by expected impact)

1. **Monomorphize node predicates** — eliminate 9.7% vtable overhead → ~1.1x speedup on search time
2. **Index-based candidate counting** — eliminate 7.5% full-scan → ~1.08x on total time (one-time cost but runs every search)
3. **Monomorphize edge predicates** — reduce is_feasible edge check overhead → ~1.03x
4. **Pre-filter candidate sets by value** — reduce candidates_for_into work → compound benefit with #1

## Pattern used

```
pattern edges: [(749, 52436, false), (52436, 139389, true), (52436, 237692, false), (237692, 356265, false), (237692, 469525, true)]
```

Two hubs (52436 with 3 edges, 237692 with 3 edges), path-like topology, mix of directed and undirected edges.
