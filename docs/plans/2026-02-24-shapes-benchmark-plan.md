# Shapes Benchmark Harness Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build an A/B benchmarking harness that compares two incremental shape update algorithms (v1=current, v2=new) with full instrumentation metrics, correctness verification, and Rayon tuning.

**Architecture:** Add a `bench` cargo feature that gates instrumented shape update methods and a standalone binary. The binary runs chaos scenarios at logarithmic scale (10..100K nodes), applying identical ops to two graph copies via RNG cloning (SmallRng: Clone), collecting per-step ShapesMetrics as CSV. Both algorithms are verified against batch-rebuilt shapes at every step.

**Tech Stack:** Rust, rayon (existing), rand (promoted from dev-dep to optional dep behind `bench` feature), std::env for CLI args.

**Key constraint:** `apply_ops_raw` and `update_shapes` are `pub(crate)` -- not accessible from `src/bin/`. All instrumented methods must be exposed via public API gated behind `#[cfg(feature = "bench")]`.

---

### Task 1: Add `bench` feature and promote `rand`

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add bench feature and rand as optional dep**

In `Cargo.toml`, add:

```toml
[features]
default = ["id32"]
id32 = []
id64 = []
bench = ["rand"]

[dependencies]
# ... existing deps ...
rand = { version = "0.9", optional = true }

[dev-dependencies]
trybuild = "1"
rand = "0.9"
```

Note: `rand` stays in `[dev-dependencies]` too for tests. The `[dependencies]` entry is optional, only activated by `bench` feature. Both can coexist.

**Step 2: Add bin target**

```toml
[[bin]]
name = "bench_shapes"
path = "src/bin/bench_shapes.rs"
required-features = ["bench"]
```

**Step 3: Verify**

Run: `cargo check`
Expected: PASS (no code changes yet, binary doesn't exist but required-features prevents it from being built without the feature)

**Step 4: Commit**

```
feat: add bench feature with optional rand dependency
```

---

### Task 2: Add ShapesMetrics struct

**Files:**
- Create: `src/modify/bench.rs`
- Modify: `src/modify/mod.rs`

**Step 1: Create `src/modify/bench.rs` with ShapesMetrics**

```rust
use std::time::Duration;

#[derive(Debug, Default)]
pub struct ShapesMetrics {
    pub added_nodes: usize,
    pub removed_nodes: usize,
    pub added_edges: usize,
    pub removed_edges: usize,
    pub graft_edges: usize,
    pub exist_edges: usize,
    pub new_to_new_edges: usize,

    pub total: Duration,
    pub descension: Duration,
    pub ascension: Duration,
    pub populate_index: Duration,

    pub dirty_seed_size: usize,
    pub dirty_expanded_size: usize,
    pub expand_calls: u32,
    pub recompute_fallback: bool,

    pub scope_size: usize,
    pub promoted_dot_stars: usize,

    pub solver_targets: usize,
    pub cores_dim3: usize,
    pub path_inners: usize,
    pub endpoints: usize,
    pub scoped_dots: usize,

    pub cores_created: usize,
    pub stars_created: usize,
    pub paths_created: usize,
    pub shapes_freed: usize,

    pub graph_nodes: usize,
    pub graph_edges: usize,
}

impl ShapesMetrics {
    pub fn csv_header() -> &'static str {
        "added_nodes,removed_nodes,added_edges,removed_edges,graft_edges,exist_edges,new_to_new_edges,\
         total_us,descension_us,ascension_us,populate_index_us,\
         dirty_seed_size,dirty_expanded_size,expand_calls,recompute_fallback,\
         scope_size,promoted_dot_stars,\
         solver_targets,cores_dim3,path_inners,endpoints,scoped_dots,\
         cores_created,stars_created,paths_created,shapes_freed,\
         graph_nodes,graph_edges"
    }

    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.added_nodes, self.removed_nodes, self.added_edges, self.removed_edges,
            self.graft_edges, self.exist_edges, self.new_to_new_edges,
            self.total.as_micros(), self.descension.as_micros(),
            self.ascension.as_micros(), self.populate_index.as_micros(),
            self.dirty_seed_size, self.dirty_expanded_size,
            self.expand_calls, self.recompute_fallback as u8,
            self.scope_size, self.promoted_dot_stars,
            self.solver_targets, self.cores_dim3, self.path_inners,
            self.endpoints, self.scoped_dots,
            self.cores_created, self.stars_created, self.paths_created, self.shapes_freed,
            self.graph_nodes, self.graph_edges,
        )
    }

    pub(crate) fn from_diff(diff: &super::apply::Diff, graph_nodes: usize, graph_edges: usize) -> Self {
        ShapesMetrics {
            added_nodes: diff.added_nodes.len() as usize,
            removed_nodes: diff.removed_nodes.len() as usize,
            added_edges: diff.added_edges.len(),
            removed_edges: diff.removed_edges.len(),
            graft_edges: diff.graft_edges.len(),
            exist_edges: diff.exist_edges.len(),
            new_to_new_edges: diff.new_to_new_edges.len(),
            graph_nodes,
            graph_edges,
            ..ShapesMetrics::default()
        }
    }
}
```

**Step 2: Wire module in `src/modify/mod.rs`**

Add after `mod validate;` line:

```rust
#[cfg(feature = "bench")]
pub mod bench;
```

And add public re-export:

```rust
#[cfg(feature = "bench")]
pub use bench::ShapesMetrics;
```

**Step 3: Verify**

Run: `cargo check --features bench`
Expected: PASS

**Step 4: Commit**

```
feat: add ShapesMetrics struct for benchmark instrumentation
```

---

### Task 3: Instrument v1 algorithm

**Files:**
- Modify: `src/modify/apply.rs`

This is the largest task. We copy the existing `update_shapes` logic into `update_shapes_v1_instrumented`, adding `Instant` timing at phase boundaries and incrementing counters at key points. The original `update_shapes` stays untouched.

**Step 1: Add the instrumented method**

In `src/modify/apply.rs`, inside the `impl<NV: Sync, ER: graph::Edge> Graph<NV, ER>` block (after `update_shapes`, before `expand_dirty_zone`), add:

```rust
#[cfg(feature = "bench")]
pub(crate) fn update_shapes_v1_instrumented(&mut self, mut diff: Diff) -> super::bench::ShapesMetrics {
    use crate::adj;
    use crate::shape::solver::Classification;
    use std::time::Instant;

    let mut m = super::bench::ShapesMetrics::from_diff(
        &diff,
        self.nodes.by_id.len(),
        self.edges.by_rel.len(),
    );

    let t_total = Instant::now();

    // ── early returns ───────────────────────────────────────────────
    if diff.added_nodes.is_empty()
        && diff.removed_nodes.is_empty()
        && diff.added_edges.is_empty()
        && diff.removed_edges.is_empty()
    {
        m.total = t_total.elapsed();
        return m;
    }

    if diff.added_edges.is_empty()
        && diff.removed_edges.is_empty()
        && diff.removed_nodes.is_empty()
    {
        self.shapes.dots |= &diff.added_nodes;
        m.total = t_total.elapsed();
        return m;
    }

    // ── descension phase ────────────────────────────────────────────
    let t_desc = Instant::now();

    // (paste entire descension section from update_shapes lines 531-621)
    // ... with these instrumentation additions:
    // - after dirty_removed is built: m.dirty_seed_size = dirty_removed.len() as usize;
    // - after each expand_dirty_zone call: m.expand_calls += 1; m.dirty_expanded_size += expanded.len() as usize;
    // - on recompute_shapes fallback: m.recompute_fallback = true;
    // - after promoted_dot_stars: m.promoted_dot_stars = promoted_dot_stars.len() as usize;
    // ... (copy the exact same logic from lines 531-691)

    m.descension = t_desc.elapsed();

    // ── ascension phase ─────────────────────────────────────────────
    let t_asc = Instant::now();

    // (paste entire ascension section from update_shapes lines 692-856)
    // ... with these instrumentation additions:
    // - after scope is computed: m.scope_size = scope.len() as usize;
    // - after classification: m.solver_targets = classification.solver_targets.len(); etc.
    // - after core loop: m.cores_created = new_cores.len();
    // - after star loop: m.stars_created = new_star_centers.len() as usize;
    // - after paths block: m.paths_created = (count paths);
    // ... (copy the exact same logic from lines 692-856)

    m.ascension = t_asc.elapsed();

    // ── populate index ──────────────────────────────────────────────
    let t_idx = Instant::now();
    self.shapes.populate_index(&mut self.nodes);
    m.populate_index = t_idx.elapsed();

    m.total = t_total.elapsed();
    m
}
```

The full method body is a copy of `update_shapes` (lines 511-858) with `Instant` timing and counter increments inserted at the phase boundaries described above. Every `recompute_shapes(); return;` becomes `self.recompute_shapes(); m.recompute_fallback = true; m.total = t_total.elapsed(); return m;`.

Track `shapes_freed` by counting how many IDs are returned to `free_ids` (each `self.shapes.free_ids.push_id(...)` call increments `m.shapes_freed += 1`).

**Step 2: Verify**

Run: `cargo check --features bench`
Expected: PASS

**Step 3: Write a quick sanity test**

In `src/modify/tests.rs` (or a new test), add:

```rust
#[cfg(feature = "bench")]
#[test]
fn v1_instrumented_matches_update_shapes() {
    use crate::graph::{Undir0, edge};
    use crate::modify::{self, N_, x};

    let mut g1: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(1, 2),
        edge::undir::E::U(2, 0),
    ].try_into().unwrap();
    let mut g2 = /* build identical graph */;

    let ops1 = vec![(N_() ^ x(crate::id::N(0)) ^ x(crate::id::N(1)) ^ x(crate::id::N(2))).into()];
    let ops2 = /* identical ops */;

    g1.modify(ops1).unwrap();
    let fragment = crate::modify::Fragment::new(ops2).validate().unwrap();
    let (_, diff) = g2.apply_ops_raw(fragment.ops).unwrap();
    let metrics = g2.update_shapes_v1_instrumented(diff);

    assert_eq!(g1.shapes_dims(), g2.shapes_dims());
    assert!(metrics.total.as_nanos() > 0);
}
```

Since we can't clone ops (no Clone impl), build two identical graphs and two identical ops Vecs from scratch.

**Step 4: Run test**

Run: `cargo test --features bench --lib v1_instrumented`
Expected: PASS

**Step 5: Commit**

```
feat: add update_shapes_v1_instrumented with per-phase timing and counters
```

---

### Task 4: Add public bench API

**Files:**
- Modify: `src/modify/mod.rs`

**Step 1: Add public instrumented modify methods**

In `src/modify/mod.rs`, inside the existing `impl<NV: Sync, ER: graph::Edge> graph::Graph<NV, ER>` block (after `modify_timed`), add:

```rust
#[cfg(feature = "bench")]
pub fn modify_v1_bench(
    &mut self,
    ops: Vec<Node<NV, ER>>,
) -> Result<(Modification<NV, ER>, bench::ShapesMetrics), error::Modification> {
    let fragment = Fragment::new(ops).validate()?;
    let (result, diff) = self.apply_ops_raw(fragment.ops)?;
    let metrics = self.update_shapes_v1_instrumented(diff);
    Ok((result, metrics))
}

#[cfg(feature = "bench")]
pub fn modify_v2_bench(
    &mut self,
    ops: Vec<Node<NV, ER>>,
) -> Result<(Modification<NV, ER>, bench::ShapesMetrics), error::Modification> {
    let fragment = Fragment::new(ops).validate()?;
    let (result, diff) = self.apply_ops_raw(fragment.ops)?;
    let metrics = self.update_shapes_v2_instrumented(diff);
    Ok((result, metrics))
}
```

**Step 2: Verify**

Run: `cargo check --features bench`
Expected: will fail because `update_shapes_v2_instrumented` doesn't exist yet. That's fine -- proceed to Task 5 first or stub it.

Actually, stub v2 immediately as a copy of v1:

In `src/modify/apply.rs`, add after `update_shapes_v1_instrumented`:

```rust
#[cfg(feature = "bench")]
pub(crate) fn update_shapes_v2_instrumented(&mut self, diff: Diff) -> super::bench::ShapesMetrics {
    self.update_shapes_v1_instrumented(diff)
}
```

**Step 3: Verify**

Run: `cargo check --features bench`
Expected: PASS

**Step 4: Commit**

```
feat: add public modify_v1_bench/modify_v2_bench API behind bench feature
```

---

### Task 5: Create benchmark binary

**Files:**
- Create: `src/bin/bench_shapes.rs`

This is the largest single file. It contains:
1. CLI arg parsing (std::env)
2. Shadow state tracking (duplicated from chaos.rs, ~50 lines)
3. Chaos ops generation (extracted from chaos.rs pattern)
4. A/B benchmark orchestration with RNG cloning
5. CSV output
6. Correctness verification

**Step 1: Create `src/bin/bench_shapes.rs`**

```rust
use std::collections::BTreeSet;

use rand::Rng;
use rand::rngs::SmallRng;
use rand::SeedableRng;

use shagra::graph::edge;
use shagra::graph::{self, Graph};
use shagra::modify::{self, LocalId, Node, ShapesMetrics};
use shagra::modify::node::{Bind, Exist, New};
use shagra::{Id, NR, id};

// ── CLI args ───────────────────────────────────────────────────────────────

struct Args {
    num_threads: usize,
    min_len: usize,
    max_len: usize,
    seed: u64,
    scenarios: Vec<String>,
}

fn parse_args() -> Args {
    let mut args = Args {
        num_threads: 0, // 0 = rayon default
        min_len: 100,
        max_len: 0, // 0 = no limit
        seed: 42,
        scenarios: vec![],
    };

    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        if let Some(val) = arg.strip_prefix("--num-threads=") {
            args.num_threads = val.parse().expect("invalid --num-threads");
        } else if let Some(val) = arg.strip_prefix("--min-len=") {
            args.min_len = val.parse().expect("invalid --min-len");
        } else if let Some(val) = arg.strip_prefix("--max-len=") {
            args.max_len = val.parse().expect("invalid --max-len");
        } else if let Some(val) = arg.strip_prefix("--seed=") {
            args.seed = val.parse().expect("invalid --seed");
        } else if arg == "--help" {
            eprintln!("Usage: bench_shapes [OPTIONS]");
            eprintln!("  --num-threads=N   Rayon thread count (0=default)");
            eprintln!("  --min-len=N       Rayon min chunk len (default: 100)");
            eprintln!("  --max-len=N       Rayon max chunk len (0=no limit)");
            eprintln!("  --seed=N          RNG seed (default: 42)");
            eprintln!("  --scenario=NAME   Run only named scenario (tiny/small/medium/large/huge)");
            std::process::exit(0);
        } else if let Some(val) = arg.strip_prefix("--scenario=") {
            args.scenarios.push(val.to_string());
        }
    }
    args
}

// ── Shadow state ───────────────────────────────────────────────────────────

struct Shadow<ER: graph::Edge> {
    nodes: BTreeSet<Id>,
    edges: BTreeSet<(NR<id::N>, ER::Slot)>,
}

impl<ER: graph::Edge> Shadow<ER> {
    fn new() -> Self {
        Shadow { nodes: BTreeSet::new(), edges: BTreeSet::new() }
    }
    fn add_node(&mut self, id: Id) { self.nodes.insert(id); }
    fn remove_node(&mut self, id: Id) {
        self.nodes.remove(&id);
        let n = id::N(id);
        self.edges.retain(|&(nr, _)| *nr.n1() != n && *nr.n2() != n);
    }
    fn add_edge(&mut self, nr: NR<id::N>, slot: ER::Slot) { self.edges.insert((nr, slot)); }
    fn to_vecs(&self) -> (Vec<(Id, ())>, Vec<(ER::Def, ())>) {
        let ns: Vec<(Id, ())> = self.nodes.iter().map(|&id| (id, ())).collect();
        let es: Vec<(ER::Def, ())> = self.edges.iter()
            .map(|&(nr, slot)| (ER::edge(slot, (**nr.n1(), **nr.n2())), ()))
            .collect();
        (ns, es)
    }
}

// ── ChaosEdge trait ────────────────────────────────────────────────────────

trait ChaosEdge: graph::Edge<Val = ()> + Sized {
    fn random_slot(rng: &mut SmallRng) -> Self::Slot;
    fn type_name() -> &'static str;
}

impl ChaosEdge for edge::Undir<()> {
    fn random_slot(_rng: &mut SmallRng) -> edge::undir::Slot { edge::undir::UND }
    fn type_name() -> &'static str { "undir" }
}
impl ChaosEdge for edge::Dir<()> {
    fn random_slot(rng: &mut SmallRng) -> edge::dir::Slot {
        if rng.random_bool(0.5) { edge::dir::SRC } else { edge::dir::TGT }
    }
    fn type_name() -> &'static str { "dir" }
}
impl ChaosEdge for edge::Anydir<()> {
    fn random_slot(rng: &mut SmallRng) -> edge::anydir::Slot {
        match rng.random_range(0u8..3) {
            0 => edge::anydir::UND, 1 => edge::anydir::SRC, _ => edge::anydir::TGT,
        }
    }
    fn type_name() -> &'static str { "anydir" }
}

// ── Scenario params ────────────────────────────────────────────────────────

struct Scenario {
    name: &'static str,
    steps: usize,
    max_batch: usize,
    growth: f64,
    shrink: f64,
    density: f64,
    graft: f64,
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario { name: "tiny",   steps: 20,  max_batch: 5,    growth: 0.5,  shrink: 0.1,  density: 0.3, graft: 0.5 },
        Scenario { name: "small",  steps: 50,  max_batch: 15,   growth: 0.5,  shrink: 0.1,  density: 0.3, graft: 0.5 },
        Scenario { name: "medium", steps: 100, max_batch: 50,   growth: 0.3,  shrink: 0.05, density: 0.3, graft: 0.5 },
        Scenario { name: "large",  steps: 200, max_batch: 200,  growth: 0.2,  shrink: 0.03, density: 0.3, graft: 0.5 },
        Scenario { name: "huge",   steps: 300, max_batch: 1000, growth: 0.15, shrink: 0.02, density: 0.2, graft: 0.5 },
    ]
}

// ── Ops generation ─────────────────────────────────────────────────────────

fn generate_ops<ER: ChaosEdge>(
    shadow: &Shadow<ER>,
    rng: &mut SmallRng,
    scenario: &Scenario,
) -> Vec<Node<(), ER>> {
    let existing: Vec<Id> = shadow.nodes.iter().copied().collect();
    let removals: Vec<Id> = existing.iter().copied()
        .filter(|_| rng.random_bool(scenario.shrink))
        .collect();
    let surviving: Vec<Id> = existing.iter().copied()
        .filter(|id| !removals.contains(id))
        .collect();

    let n_new = if surviving.is_empty() {
        1usize.max((scenario.growth * 1.0) as usize)
    } else {
        1usize.max((scenario.growth * surviving.len() as f64) as usize)
    }.min(scenario.max_batch);

    let mut ops: Vec<Node<(), ER>> = Vec::new();

    for &id in &removals {
        ops.push(Node::Exist(Exist::Rem { id: id::N(id) }));
    }

    for i in 1..=n_new as Id {
        let mut edges: Vec<modify::edge::Edge<(), ER>> = Vec::new();

        if !surviving.is_empty() && rng.random_bool(scenario.graft) {
            let target_id = surviving[rng.random_range(0..surviving.len())];
            let slot = ER::random_slot(rng);
            edges.push(modify::edge::Edge::New {
                slot,
                val: (),
                target: Node::Exist(Exist::Bind {
                    id: id::N(target_id),
                    op: Bind::Ref,
                    edges: vec![],
                }),
            });
        }

        for prev in 1..i {
            if rng.random_bool(scenario.density) {
                let slot = ER::random_slot(rng);
                edges.push(modify::edge::Edge::New {
                    slot,
                    val: (),
                    target: Node::New(New::Ref { id: LocalId(prev) }, vec![]),
                });
            }
        }

        ops.push(Node::New(
            New::Add { id: Some(LocalId(i)), val: () },
            edges,
        ));
    }

    ops
}

// ── Edge plan for shadow update ────────────────────────────────────────────

enum EdgeTarget { Existing(Id), NewLocal(Id) }

struct PlannedEdge<S> {
    source_local: Id,
    target: EdgeTarget,
    slot: S,
}

fn plan_edges<ER: ChaosEdge>(
    ops: &[Node<(), ER>],
) -> (Vec<Id>, Vec<PlannedEdge<ER::Slot>>, usize) {
    let mut removals = Vec::new();
    let mut edges = Vec::new();
    let mut n_new: Id = 0;

    for op in ops {
        match op {
            Node::Exist(Exist::Rem { id }) => { removals.push(**id); }
            Node::New(New::Add { id: Some(local), .. }, op_edges) => {
                n_new += 1;
                for edge in op_edges {
                    if let modify::edge::Edge::New { slot, target, .. } = edge {
                        let tgt = match target {
                            Node::Exist(Exist::Bind { id, .. }) => EdgeTarget::Existing(**id),
                            Node::New(New::Ref { id }, _) => EdgeTarget::NewLocal(id.0),
                            _ => continue,
                        };
                        edges.push(PlannedEdge { source_local: local.0, target: tgt, slot: *slot });
                    }
                }
            }
            _ => {}
        }
    }

    (removals, edges, n_new as usize)
}

fn update_shadow<ER: ChaosEdge>(
    shadow: &mut Shadow<ER>,
    result: &modify::Modification<(), ER>,
    removals: &[Id],
    edges: &[PlannedEdge<ER::Slot>],
) {
    for &id in removals { shadow.remove_node(id); }
    for (local, real) in &result.new_node_ids {
        shadow.add_node(**real);
    }
    for pe in edges {
        let src = *result.new_node_ids[&LocalId(pe.source_local)];
        let tgt = match pe.target {
            EdgeTarget::Existing(id) => id,
            EdgeTarget::NewLocal(lid) => *result.new_node_ids[&LocalId(lid)],
        };
        let def: ER::Def = ER::edge(pe.slot, (src, tgt));
        let (nr, slot): (NR<id::N>, ER::Slot) = def.into();
        shadow.add_edge(nr, slot);
    }
}

// ── Benchmark runner ───────────────────────────────────────────────────────

fn bench_scenario<ER: ChaosEdge>(
    scenario: &Scenario,
    seed: u64,
) where
    for<'a> Graph<(), ER>: TryFrom<(Vec<(Id, ())>, Vec<(ER::Def, ())>)>,
    <Graph<(), ER> as TryFrom<(Vec<(Id, ())>, Vec<(ER::Def, ())>)>>::Error: std::fmt::Debug,
{
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut graph_a = Graph::<(), ER>::default();
    let mut graph_b = Graph::<(), ER>::default();
    let mut shadow = Shadow::<ER>::new();

    for step in 0..scenario.steps {
        let rng_snapshot = rng.clone();

        // Generate and apply to graph_a (v1)
        let ops_a = generate_ops(&shadow, &mut rng, scenario);
        let (removals, edge_plan, _) = plan_edges(&ops_a);
        let (result_a, metrics_a) = graph_a.modify_v1_bench(ops_a)
            .unwrap_or_else(|e| panic!("{}/{} step {step}: v1 failed: {e:?}", scenario.name, ER::type_name()));

        // Generate identical ops for graph_b (v2) from cloned RNG
        let mut rng_b = rng_snapshot;
        let ops_b = generate_ops(&shadow, &mut rng_b, scenario);
        let (result_b, metrics_b) = graph_b.modify_v2_bench(ops_b)
            .unwrap_or_else(|e| panic!("{}/{} step {step}: v2 failed: {e:?}", scenario.name, ER::type_name()));

        // Update shadow (both results should be identical)
        update_shadow(&mut shadow, &result_a, &removals, &edge_plan);

        // Verify correctness against batch rebuild
        let (ns, es) = shadow.to_vecs();
        let rebuilt: Graph<(), ER> = (ns, es).try_into().unwrap_or_else(|_| {
            panic!("{}/{} step {step}: rebuild failed", scenario.name, ER::type_name())
        });

        let expected = rebuilt.shapes_dims();
        let actual_a = graph_a.shapes_dims();
        let actual_b = graph_b.shapes_dims();

        assert_eq!(actual_a, expected,
            "{}/{} step {step}: v1 shapes_dims mismatch (seed={seed})\n  actual:   {actual_a:?}\n  expected: {expected:?}",
            scenario.name, ER::type_name());
        assert_eq!(actual_b, expected,
            "{}/{} step {step}: v2 shapes_dims mismatch (seed={seed})\n  actual:   {actual_b:?}\n  expected: {expected:?}",
            scenario.name, ER::type_name());

        // Emit CSV rows
        println!("{},{},{step},v1,{}",
            scenario.name, ER::type_name(), metrics_a.to_csv_row());
        println!("{},{},{step},v2,{}",
            scenario.name, ER::type_name(), metrics_b.to_csv_row());
    }

    eprintln!("{}/{}: {} steps completed -- {} nodes, {} edges",
        scenario.name, ER::type_name(), scenario.steps,
        graph_a.node_count(), graph_a.edge_count());
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    let args = parse_args();

    if args.num_threads > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(args.num_threads)
            .build_global()
            .expect("failed to configure rayon thread pool");
    }

    // CSV header
    println!("scenario,edge_type,step,algo,{}", ShapesMetrics::csv_header());

    let all_scenarios = scenarios();
    let active: Vec<&Scenario> = if args.scenarios.is_empty() {
        all_scenarios.iter().collect()
    } else {
        all_scenarios.iter()
            .filter(|s| args.scenarios.iter().any(|a| a == s.name))
            .collect()
    };

    for scenario in &active {
        bench_scenario::<edge::Undir<()>>(scenario, args.seed);
        bench_scenario::<edge::Dir<()>>(scenario, args.seed);
        bench_scenario::<edge::Anydir<()>>(scenario, args.seed);
    }
}
```

**Step 2: Verify compilation**

Run: `cargo check --features bench`
Expected: PASS

**Step 3: Run tiny scenario**

Run: `cargo run --features bench --release --bin bench_shapes -- --scenario=tiny 2>/dev/null | head -5`
Expected: CSV header + first few data rows

**Step 4: Run full benchmark and inspect**

Run: `cargo run --features bench --release --bin bench_shapes -- --scenario=small 2>bench_stderr.txt | tee bench_small.csv | wc -l`
Expected: 301 lines (1 header + 50 steps * 2 algos * 3 edge types)

**Step 5: Commit**

```
feat: add bench_shapes binary with A/B comparison harness
```

---

### Task 6: Verify end-to-end correctness

**Files:** None (verification only)

**Step 1: Run all existing tests (no bench feature)**

Run: `cargo test --lib && cargo test --test harness && cargo test --test chaos && cargo test --test compile_fail`
Expected: All pass. The bench feature should not affect normal compilation.

**Step 2: Run benchmark small + medium**

Run: `cargo run --features bench --release --bin bench_shapes -- --scenario=small --scenario=medium 2>&1 >/dev/null`
Expected: No assertion failures. stderr shows completion messages.

**Step 3: Run benchmark large**

Run: `cargo run --features bench --release --bin bench_shapes -- --scenario=large 2>&1 >/dev/null`
Expected: No assertion failures. May take a while.

**Step 4: Commit (if any fixes were needed)**

---

### Task 7: Rayon tuning infrastructure

**Files:**
- Modify: `src/bin/bench_shapes.rs`

This task wires the `--min-len` and `--max-len` CLI args into the benchmark. Currently the batch `compute_shapes` in solver.rs uses `.with_min_len(100)`. For the v2 algorithm (future), we'll pass these values through. For now, just ensure the args are parsed and printed in stderr so we can verify they work.

**Step 1: Add env-based Rayon tuning logging**

At startup in `main()`, after thread pool setup:

```rust
eprintln!("config: num_threads={}, min_len={}, max_len={}, seed={}",
    if args.num_threads == 0 { rayon::current_num_threads() } else { args.num_threads },
    args.min_len,
    args.max_len,
    args.seed);
```

**Step 2: Verify**

Run: `cargo run --features bench --release --bin bench_shapes -- --min-len=50 --max-len=500 --num-threads=4 --scenario=tiny 2>&1 >/dev/null | head -1`
Expected: `config: num_threads=4, min_len=50, max_len=500, seed=42`

**Step 3: Commit**

```
feat: wire Rayon tuning args into benchmark binary
```

---

## Summary

After completing all tasks:

1. `cargo test` (no features) -- all existing tests pass, bench code is not compiled
2. `cargo run --features bench --release --bin bench_shapes` -- runs full A/B benchmark
3. v1 and v2 currently produce identical results (v2 is a stub)
4. CSV output ready for analysis
5. Rayon tuning args ready for sweeping
6. Next step: implement actual v2 algorithm (descension/ascension from the plan at `docs/plans/warm-yawning-scott.md`) inside `update_shapes_v2_instrumented`, iterate and compare against v1
