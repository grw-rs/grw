# Pattern Compiler Architecture Design

**Goal:** Replace the one-size-fits-all search loop with a pattern compiler that generates specialized search strategies based on pattern analysis, eliminating runtime overhead for features the pattern doesn't use.

**Problem:** The current search engine pays for all features (negation, bans, any_slot, morphism dispatch, reverse map) on every iteration, even when the pattern doesn't use them. Shape-based pruning only works at depth 0; deeper depths fall back to generic neighbor iteration. Smart search ordering fights the shape system rather than working with it.

## Layer 1: Pattern Analysis (PatternProfile)

Statically analyze each compiled pattern to produce a profile:

```
PatternProfile {
    morphism: Morphism,          // iso, subiso, mono, homo
    has_negation: bool,
    has_bans: bool,
    has_any_slot: bool,
    has_predicates: bool,
    node_count: usize,
    is_connected: bool,
    shape_class: ShapeClass,     // Clique, Star, Path, Tree, Mixed
}
```

This analysis happens once at `plan()` time and drives all subsequent decisions.

## Layer 2: Typestate Strategy Selection

Use zero-sized types and monomorphization to eliminate runtime branching:

```rust
trait SearchStrategy {
    type ReverseMap: ReverseMapOps;
    type FeasibilityCheck: FeasibilityOps;
    type MatchBuilder: MatchBuilderOps;
}

struct IsoStrategy;
struct SubIsoStrategy;
struct MonoStrategy;

impl SearchStrategy for IsoStrategy {
    type ReverseMap = BitsetReverse;       // dense, SIMD-friendly
    type FeasibilityCheck = IsoFeasibility; // bidirectional degree check
    type MatchBuilder = DirectMatch;        // no allocation
}

impl SearchStrategy for SubIsoStrategy {
    type ReverseMap = VecReverse;           // flat Vec<Option<usize>>
    type FeasibilityCheck = SubIsoFeasibility; // forward-only check
    type MatchBuilder = DirectMatch;
}
```

The search loop is generic over `S: SearchStrategy`, so the compiler monomorphizes separate versions with zero-cost dispatch.

### Separate Loops for Negation/Bans

```rust
trait NegationHandler {
    fn check_negated(&self, state: &SearchState) -> bool;
}

struct NoNegation;  // empty impl, compiles to nothing
struct HasNegation; // actual negation checking

trait BanHandler {
    fn check_bans(&self, state: &SearchState) -> bool;
}

struct NoBans;     // empty impl
struct HasBans;    // actual ban checking
```

Patterns without negation/bans get a search loop that literally doesn't contain the checking code.

## Layer 3: Shape-Aware Search Strategies

### Iso Strategy

For isomorphism, shapes give exact candidate sets at every depth:

- **Depth 0:** Shape group iteration (existing, works well)
- **Depth > 0:** Shape class filtering in neighbor path (CoreMember -> Cores, StarCenter -> Star::Center, etc.)
- **Search order:** Shape-driven (start from most constrained shape, e.g., clique member)

Clique detection example: depth-0 iterates exact-dim cores. If target has no matching cores, instant rejection (0.02ms for clique4 subiso).

### SubIso Strategy

For subisomorphism, shapes provide upper-bound pruning:

- **Depth 0:** Shape-filtered candidates (cores with >= dim, stars with >= dim)
- **Depth > 0:** Degree filtering + selective class filtering (CoreMember only; StarCenter too aggressive)
- **Search order:** Connectivity-first within shape constraints (don't fight the shape system)

### Candidate Generation

Flip the subiso adjacency check: instead of iterating ALL target neighbors and checking if they're mapped, iterate MAPPED pattern neighbors and check target adjacency. For patterns with few edges relative to target degree, this is significantly faster:

```rust
// Current (slow for high-degree targets):
for target_neighbor in target.neighbors(candidate) {
    if let Some(mapped) = reverse[target_neighbor] { ... }
}

// Proposed (fast for sparse patterns):
for &(pattern_neighbor, slot, ..) in &query.adj[pattern_idx] {
    if let Some(target_neighbor) = mapping[pattern_neighbor] {
        if !target.is_adjacent(candidate, target_neighbor) { return false; }
    }
}
```

## Layer 4: Data Structure Optimizations

### CSR Adjacency Format

Replace `BTreeMap<id::N, Node>` random access with Compressed Sparse Row:

```rust
struct CsrAdjacency {
    offsets: Vec<u32>,     // offsets[node_id] = start of neighbors
    neighbors: Vec<u32>,   // flat packed neighbor IDs
}
```

Benefits: cache-friendly iteration, no pointer chasing, SIMD-scannable.

### Flat Arrays

- `mapping: Vec<Option<id::N>>` -> `mapping: Vec<u32>` with sentinel (already nearly there)
- `reverse: Vec<Option<usize>>` -> `Vec<u32>` with `u32::MAX` sentinel (avoid Option overhead)
- Pre-sort pattern adjacency by mapped-first order for branch prediction

## Layer 5: SIMD Opportunities

### Bitset Adjacency (for iso)

For isomorphism on moderate graphs, represent adjacency as bitsets:

```rust
struct BitsetAdj {
    bits: Vec<u64>,  // bits[node_id * stride + word] = neighbor bitmap
    stride: usize,   // words per row = ceil(max_node / 64)
}
```

Feasibility check becomes AND + popcount:
```
candidate_adj AND mapped_mask == expected_pattern
```

### Candidate Filtering

SIMD comparison for degree filtering across candidate arrays:

```rust
// Filter candidates where degree >= pattern_degree
let threshold = _mm256_set1_epi32(pattern_degree);
let mask = _mm256_cmpgt_epi32(degrees_chunk, threshold);
```

## Layer 6: Pattern Compilation Tiers

### Tier 0: Interpreted (current)

Runtime-constructed patterns use the generic search loop. No compilation. This is the fallback for patterns built dynamically.

### Tier 1: Specialized (typestate dispatch)

`plan()` analyzes the pattern and selects the best combination of strategy traits. Monomorphization generates specialized code. Available immediately at plan time.

### Tier 2: Proc Macro (compile-time)

For patterns known at compile time (DSL macros), generate a completely specialized search function:

```rust
#[derive(PatternSearch)]
struct TrianglePattern;

// Generates:
impl TrianglePattern {
    fn search(graph: &Graph) -> impl Iterator<Item = Match> {
        // Unrolled 3-depth loop, inline feasibility, no allocation
    }
}
```

### Tier 3: JIT (Cranelift)

For patterns constructed at runtime that will be searched many times, JIT-compile a specialized search function using Cranelift:

```rust
let compiled = pattern.jit_compile()?;
let matches = compiled.search(&graph);
```

This eliminates interpretation overhead while supporting dynamic patterns.

## Implementation Phases

### Phase 1: Foundation (typestate + CSR)
- PatternProfile analysis
- Typestate search loop (IsoStrategy, SubIsoStrategy, MonoStrategy)
- Separate negation/ban handlers
- CSR adjacency for target graph
- Flipped subiso adjacency check
- **Benchmark target:** Close the 2x gap vs VF3 on tree patterns

### Phase 2: Shape Integration (bitset + SIMD)
- Shape anchor analysis (determine optimal search order per shape class)
- Bitset reverse map for iso
- SIMD candidate filtering
- **Benchmark target:** 2x faster than VF3 on structured patterns

### Phase 3: Compilation (proc macros + JIT)
- Proc macro for compile-time patterns
- Bitset adjacency for moderate graphs
- Cranelift JIT for runtime patterns
- **Benchmark target:** 5-10x faster than VF3 on hot patterns

## Verification

Each phase must pass the full crosscheck suite (shape_match == vf2 == vf3) on all pattern types and morphisms before proceeding.

```bash
cargo run --release --features crosscheck --bin verify_patterns -- \
  --graph-dir data/crosscheck/undir/154n_186e \
  --morphisms iso,subiso:7,mono:5 \
  --patterns extracted_3,extracted_4,extracted_5,extracted_7,motif_triangle,motif_clique4,motif_cycle4,motif_star4,motif_path3,motif_path4 \
  --engine shape_match --timed
```
