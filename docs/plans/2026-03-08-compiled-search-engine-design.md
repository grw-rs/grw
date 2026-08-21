# Compiled Search Engine Design

## Goal

Split the search engine into two tiers:
1. **Runtime engine** (`shape_match`) — current engine, for dynamically constructed patterns
2. **Compiled engine** — new, for patterns known at compile time. Uses trait-based feature elimination, const-generic monomorphization, and (eventually) SIMD + loop unrolling.

Also: remove naive engine, establish comprehensive benchmarking.

## Problem Analysis

### Where time goes (extracted_7 subiso on 1458n/6411e)

The hot path is `advance::<true>()`:
- ~8.5M matches found
- Per match: candidate generation → feasibility check → map/unmap → count

The current engine branches on many pattern properties at runtime:
- `has_edge_features` → gates fast vs full feasibility
- `morphism` per node → Iso/SubIso/Mono/Homo dispatch
- `has_negated_connected` / `has_ban_clusters` → post-match checks
- `needs_reverse` → reverse map maintenance
- `is_injective[i]` → per-node injectivity check
- `pattern_adj_bits` → loaded from Vec per call

All of these are **statically known from the pattern**. The compiler cannot eliminate dead branches because they're Vec lookups and bool fields, not const generics.

### What the compiler CAN optimize if given const info

If `HAS_EDGE_FEATURES = false`, the `is_feasible` call resolves to `is_feasible_fast` with no runtime check. If `MORPHISM = SubIso` for all nodes, the morphism match arms collapse. If `HAS_NEGATED_CONNECTED = false`, the post-match check disappears entirely. The compiler already does this with `COUNT_ONLY: bool` via const generics.

## Architecture

```
src/search/
  mod.rs              — Morphism, Decision, search! macro (unchanged)
  dsl/                — DSL types (unchanged)
  query/              — Query, compile() (unchanged)
  engine/
    mod.rs            — SearchEngine trait, SearchState, CsrAdj (unchanged)
    shape_match.rs    — Runtime engine (unchanged)
    shaped.rs         — Simpler runtime engine (unchanged)
    compiled.rs       — NEW: compiled engine with trait-based specialization
```

### Phase 0: Remove naive, add benchmarks

Remove `naive.rs`. It's 354 lines of slower duplicate of shape_match. All tests use `run_search()` which calls `query.run()` → naive. Change `query.run()` to use shape_match.

Build a benchmark suite covering all pattern features:
- Morphisms: iso, subiso, mono, homo
- Predicates: node val, edge val, test closures
- Negation: freestanding, connected
- Ban clusters: shared-only, with ban-only nodes
- Edge types: specific slot, any-slot (%)
- Graph types: undir, dir, anydir
- Pattern sizes: 2-node, 3 (triangle), 4 (clique, star, path, cycle), 5-7 (extracted)

### Phase 1: PatternSpec trait + monomorphized advance

Define a trait that describes all compile-time-knowable pattern properties:

```rust
pub trait PatternSpec {
    const SEARCH_LEN: usize;
    const HAS_EDGE_FEATURES: bool;
    const HAS_NEGATED_CONNECTED: bool;
    const HAS_BAN_CLUSTERS: bool;
    const NEEDS_REVERSE: bool;
    const ALL_INJECTIVE: bool;
    const UNIFORM_MORPHISM: Option<Morphism>;
}
```

Write a monomorphized `advance` that uses these constants:

```rust
fn advance_compiled<const COUNT_ONLY: bool, P: PatternSpec>(&mut self) -> (Option<Match>, usize) {
    // Compiler eliminates dead branches:
    if !P::HAS_EDGE_FEATURES {
        // only is_feasible_fast path exists
    }
    if P::UNIFORM_MORPHISM == Some(Morphism::SubIso) {
        // no morphism match needed, always SubIso
    }
    if !P::HAS_NEGATED_CONNECTED && !P::HAS_BAN_CLUSTERS {
        // skip post-match checks entirely at leaf depth
    }
    // etc.
}
```

This requires NO proc macros. Users implement `PatternSpec` manually for hot patterns, or (later) a proc macro generates it. Even without the proc macro, this gives the full speedup for patterns where someone writes the impl.

For extracting a `PatternSpec` from a compiled `Query` at runtime, we can dispatch dynamically:

```rust
// In shape_match plan():
fn plan(query: Query) -> Plan {
    // Inspect query to determine spec, dispatch to monomorphized version
    match (query.has_edge_features, query.has_negated_connected, ...) {
        (false, false, ...) => plan_with_spec::<SimpleSpec>(query),
        ...
    }
}
```

The key insight: there are only ~8 meaningful boolean combinations. We can dispatch to monomorphized versions at plan time, not per-match.

### Phase 2: Const pattern data via static arrays

Move per-depth pattern data from Vec to const arrays where possible:

```rust
pub trait PatternSpec {
    // ... booleans from Phase 1 ...
    const SEARCH_ORDER: &'static [usize];
    const PATTERN_ADJ_BITS: &'static [u64];
    const PATTERN_DEGREES: &'static [usize];
    const NODE_MORPHISMS: &'static [Morphism];
    const IS_INJECTIVE: &'static [bool];
}
```

With these as static slices, the compiler can constant-fold `SEARCH_ORDER[depth]` when `depth` is known, and entire branches like `IS_INJECTIVE[pattern_idx]` become compile-time constants for unrolled loops.

### Phase 3: Proc macro for automatic PatternSpec generation

A proc macro `compiled_pattern!` that:

1. Parses the `search!` DSL syntax (same tokens)
2. Runs flattening logic at compile time (reimplemented in the proc macro crate)
3. Generates a zero-sized struct implementing `PatternSpec`

```rust
compiled_pattern! {
    get(SubIso) {
        N(0) ^ N(1),
        n(1) ^ N(2)
    }
}
// Expands to:
struct _P123;
impl PatternSpec for _P123 {
    const SEARCH_LEN: usize = 3;
    const HAS_EDGE_FEATURES: bool = false;
    const HAS_NEGATED_CONNECTED: bool = false;
    const HAS_BAN_CLUSTERS: bool = false;
    const NEEDS_REVERSE: bool = true;
    const ALL_INJECTIVE: bool = true;
    const UNIFORM_MORPHISM: Option<Morphism> = Some(Morphism::SubIso);
    const SEARCH_ORDER: &'static [usize] = &[1, 0, 2]; // highest degree first
    const PATTERN_ADJ_BITS: &'static [u64] = &[0b010, 0b101, 0b010];
    const PATTERN_DEGREES: &'static [usize] = &[1, 2, 1];
    const NODE_MORPHISMS: &'static [Morphism] = &[SubIso, SubIso, SubIso];
    const IS_INJECTIVE: &'static [bool] = &[true, true, true];
}
```

The proc macro crate needs to reimplement the flattening/compilation logic from `compile.rs` without depending on `shagra` types (proc macro crates can't depend on the crate they generate code for). This means duplicating the structural logic (node/edge flattening, search order computation, etc.) — roughly 200 lines of pure logic.

Node/edge **predicates** (closures) remain runtime. The proc macro only computes structural constants. The `Query` still stores predicates at runtime; the `PatternSpec` just provides the static layout.

### Phase 4: Loop unrolling for small patterns

For patterns with SEARCH_LEN <= 7, generate unrolled nested loops instead of a stack-based loop:

```rust
// Triangle SubIso COUNT_ONLY (3 nested loops, no stack)
fn count_triangle_subiso(state: &mut SearchState) -> usize {
    let mut total = 0;
    for &c0 in &candidates_depth0 {
        state.mapping[SEARCH_ORDER[0]] = *c0;
        state.reverse[*c0 as usize] = SEARCH_ORDER[0] as u32;
        for &c1 in state.csr.neighbors(*c0) {
            // inline degree filter + injectivity
            if state.reverse[c1 as usize] != UNMAPPED { continue; }
            if state.csr.degree(c1) < 2 { continue; }
            state.mapping[SEARCH_ORDER[1]] = c1;
            state.reverse[c1 as usize] = SEARCH_ORDER[1] as u32;
            for &c2 in state.csr.neighbors(*c0) {
                if state.reverse[c2 as usize] != UNMAPPED { continue; }
                if !state.csr.is_adjacent(c1, c2) { continue; }
                // reverse check for SubIso
                if !is_feasible_reverse_only_inline(state, SEARCH_ORDER[2], c2) { continue; }
                total += 1;
            }
            state.mapping[SEARCH_ORDER[1]] = UNMAPPED;
            state.reverse[c1 as usize] = UNMAPPED;
        }
        state.mapping[SEARCH_ORDER[0]] = UNMAPPED;
        state.reverse[*c0 as usize] = UNMAPPED;
    }
    total
}
```

Benefits:
- No stack frame allocation/deallocation
- No candidate Vec allocation (iterate CSR neighbors directly)
- Compiler can register-allocate loop variables
- Branch prediction improves (fixed loop structure)

This is the biggest potential win for small patterns. The stack-based approach has per-candidate overhead from frame management that dominates when individual candidate checks are cheap.

The proc macro from Phase 3 can generate these unrolled versions automatically. For SEARCH_LEN > 7, fall back to the stack-based approach.

### Phase 5: SIMD

Two SIMD opportunities:

**1. Batch adjacency check**: Check if a candidate is adjacent to multiple mapped nodes simultaneously.

Current `is_feasible_reverse_only` iterates CSR neighbors one by one:
```rust
for &raw_neighbor in self.csr.neighbors(candidate_id) {
    let mpi = self.reverse[raw_neighbor as usize];
    if mpi != UNMAPPED {
        if (pattern_adj_bits[pattern_idx] >> mpi) & 1 == 0 {
            return false;
        }
    }
}
```

With SIMD, load 8 neighbor IDs at once, gather 8 reverse entries, compare all against UNMAPPED, and check adj_bits in batch:

```rust
// Pseudocode with portable_simd (nightly) or std::simd
let neighbors = u32x8::from_slice(&csr_neighbors[offset..]);
let reverse_vals = u32x8::gather(&self.reverse, neighbors);  // gather
let mapped_mask = reverse_vals.simd_ne(u32x8::splat(UNMAPPED));
// For mapped entries, check pattern_adj_bits
if mapped_mask.any() {
    // shift and mask check
}
```

Feasibility: depends on nightly `portable_simd` or manual intrinsics. The `gather` operation is key — `vpgatherdd` on AVX2 is 4-12 cycles depending on cache. Whether this beats scalar depends on average neighbor count.

**2. Batch candidate filtering**: Filter candidates by degree and injectivity using SIMD.

Load 8 candidate IDs, gather their degrees from CSR offsets, compare against pattern_degree, gather reverse entries to check injectivity — produce a bitmask of valid candidates.

This is most valuable when candidate sets are large (>32 entries), which happens for depth-0 and disconnected components.

**Assessment**: SIMD is high-effort, nightly-dependent (for portable_simd), and the wins are uncertain without profiling. Should be last priority. Profile first after Phase 1-2.

### Phase 6: Cache-friendly structures

The current CSR (offsets + flat neighbors) is already cache-friendly for sequential neighbor iteration. The main cache concern is the `reverse` array — it's indexed by target node ID, which can be sparse and large (up to max_id entries).

Possible improvements:
- **reverse as HashMap for sparse graphs**: If max_id >> node_count, the reverse array wastes memory and cache. But for dense ID ranges (contiguous IDs), the array is optimal.
- **Prefetching**: When iterating candidates, prefetch `reverse[candidate]` before the feasibility check.
- **SoA for SearchState**: Currently mapping and reverse are separate Vec<u32>. Could interleave for better locality if they're accessed together — but they're not (mapping indexed by pattern_idx, reverse indexed by target_id).

Assessment: Current structures are already near-optimal for the access patterns. Biggest win is probably software prefetching in the candidate loop.

## Phasing

| Phase | Effort | Expected Speedup | Dependencies |
|-------|--------|-------------------|-------------|
| 0: Remove naive + benchmarks | 1 day | 0% (infrastructure) | None |
| 1: PatternSpec trait + monomorphized advance | 2-3 days | 10-25% | Phase 0 |
| 2: Const pattern data | 1 day | 5-10% | Phase 1 |
| 3: Proc macro | 3-5 days | 0% (ergonomics for 1+2) | Phase 1+2 |
| 4: Loop unrolling | 2-3 days | 15-40% for small patterns | Phase 1 |
| 5: SIMD | 3-5 days | Unknown, profile first | Phase 1 |
| 6: Cache optimization | 1-2 days | 5-10% | Phase 1 |

Recommended order: 0 → 1 → 4 → 2 → 6 → 3 → 5

Phase 4 (unrolling) likely gives the biggest speedup and doesn't need the proc macro — can start with hand-written unrolled versions for the 3-5 most common pattern shapes, then automate with the proc macro later.

## Key Decisions Needed

1. **PatternSpec as trait vs const generics**: Trait gives named constants (readable), const generics give finer monomorphization. Start with trait, can add const generics later for specific hot paths.

2. **Proc macro crate structure**: Separate crate `shagra-macros` (required for proc macros). Needs to duplicate compile.rs logic without shagra dependency.

3. **Loop unrolling scope**: Hand-written for top-N patterns first, or proc-macro generated from the start? Hand-written first validates the approach and benchmarks the gains. Proc macro automates once the shape is proven.

4. **SIMD stability**: `portable_simd` is nightly-only. Alternative: use `std::arch` intrinsics behind `#[cfg(target_feature = "avx2")]`. Or use a stable SIMD crate like `packed_simd` (unmaintained) or `wide` (limited). Recommend: skip SIMD until after profiling Phases 1-4.

5. **Shaped engine fate**: Keep or remove? It's simpler than shape_match but slower. If shape_match always wins, remove it. If there are edge cases where shaped is faster (very small graphs?), keep it. Benchmark will tell.
