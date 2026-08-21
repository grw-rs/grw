# Shapes Update Benchmark Design

## Goal

Build a benchmarking harness to A/B compare incremental shape update algorithms. The current algorithm (v1) is the reference. A new descension/ascension algorithm (v2) will be developed and tuned against it. Both must produce identical results to batch-rebuilt shapes.

## Architecture

### Clone-and-Diverge

At each chaos step, both graphs receive identical ops:

```
ops = generate_chaos_ops(shadow, rng, params)

graph_a.apply_ops_raw(ops.clone()) -> diff_a
metrics_a = graph_a.update_shapes_v1_instrumented(diff_a)

graph_b.apply_ops_raw(ops.clone()) -> diff_b
metrics_b = graph_b.update_shapes_v2_instrumented(diff_b)

rebuilt = batch_rebuild(shadow)
assert graph_a.shapes_dims() == rebuilt.shapes_dims()
assert graph_b.shapes_dims() == rebuilt.shapes_dims()
```

Both graphs start empty, receive the same mutation sequence, diverge only in shape update strategy. Correctness verified against batch rebuild at every step.

### ShapesMetrics

```rust
pub struct ShapesMetrics {
    // Diff stats (input characterization)
    pub added_nodes: usize,
    pub removed_nodes: usize,
    pub added_edges: usize,
    pub removed_edges: usize,
    pub graft_edges: usize,
    pub exist_edges: usize,
    pub new_to_new_edges: usize,

    // Timing
    pub total: Duration,
    pub descension: Duration,
    pub ascension: Duration,
    pub populate_index: Duration,

    // Dirty zone
    pub dirty_seed_size: usize,
    pub dirty_expanded_size: usize,
    pub expand_calls: u32,
    pub recompute_fallback: bool,

    // Scope
    pub scope_size: usize,
    pub promoted_dot_stars: usize,

    // Classification results
    pub solver_targets: usize,
    pub cores_dim3: usize,
    pub path_inners: usize,
    pub endpoints: usize,
    pub scoped_dots: usize,

    // Shape allocations
    pub cores_created: usize,
    pub stars_created: usize,
    pub paths_created: usize,
    pub shapes_freed: usize,

    // Graph state (post-mutation, pre-shapes)
    pub graph_nodes: usize,
    pub graph_edges: usize,
}
```

### Instrumented Methods

Each algorithm variant is a concrete method on `Graph`:

- `update_shapes_v1_instrumented(diff: Diff) -> ShapesMetrics` — current algorithm with timing/counters
- `update_shapes_v2_instrumented(diff: Diff) -> ShapesMetrics` — new algorithm (starts as copy of v1)

Original `update_shapes` stays untouched for production use.

### Rayon Tuning

No hand-rolled `if scope.len() > threshold` branching. Always use `par_iter()` with Rayon's `IndexedParallelIterator::with_min_len(N)` / `with_max_len(N)`. Rayon handles splitting decisions internally.

The benchmark binary accepts CLI args for tuning:
- `--min-len=N` — Rayon minimum chunk length
- `--max-len=N` — Rayon maximum chunk length
- `--num-threads=N` — Rayon thread pool size

Sweep values (50, 100, 200, 500, 1000) across scenario matrix to find optimal granularity per graph scale.

## Benchmark Binary

`src/bin/bench_shapes.rs` — standalone binary run with `cargo run --bin bench_shapes --release`.

### Scenario Matrix (Logarithmic Sweep)

| Target nodes | Steps | max_batch | growth | shrink | density | graft |
|---|---|---|---|---|---|---|
| ~10 | 20 | 5 | 0.5 | 0.1 | 0.3 | 0.5 |
| ~100 | 50 | 15 | 0.5 | 0.1 | 0.3 | 0.5 |
| ~1K | 100 | 50 | 0.3 | 0.05 | 0.3 | 0.5 |
| ~10K | 200 | 200 | 0.2 | 0.03 | 0.3 | 0.5 |
| ~100K | 300 | 1000 | 0.15 | 0.02 | 0.2 | 0.5 |

Each scenario runs for all three edge types: Undir, Dir, Anydir.

### CSV Output

One row per (scenario, step, algorithm). All ShapesMetrics fields as columns. Output to stdout, pipe to file for analysis.

## Implementation Order

1. **Make ops Clone-able** — derive Clone on Node, Edge, New, Exist, Bind types in src/modify/
2. **ShapesMetrics struct** — add to src/modify/apply.rs (or src/modify/metrics.rs)
3. **Instrument current algorithm** — add update_shapes_v1_instrumented, same logic as update_shapes with Instant timing and counters
4. **Benchmark binary** — src/bin/bench_shapes.rs with chaos generation, clone-and-diverge, CSV output, CLI args
5. **Stub v2** — copy of v1 initially, becomes target for descension/ascension refactor
6. **Rayon tuning** — ThreadPoolBuilder for --num-threads, with_min_len/with_max_len wired to CLI args

## Files

- `src/modify/apply.rs` — ShapesMetrics, v1/v2 instrumented methods
- `src/modify/mod.rs` — derive Clone on op types, expose instrumented methods
- `src/bin/bench_shapes.rs` — benchmark binary
- `Cargo.toml` — add clap or argh for CLI parsing (optional, could use env vars)
