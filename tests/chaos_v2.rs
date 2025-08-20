use std::collections::BTreeSet;

use rand::Rng;
use rand::rngs::SmallRng;
use rand::SeedableRng;

use grw::graph::edge;
use grw::graph::{self, Graph};
use grw::modify::{self, LocalId, Node};
use grw::modify::node::{Bind, Exist, New};
use grw::{Id, NR, id};

fn degree_histogram<ER: graph::Edge>(
    (ns, es): (Vec<(Id, ())>, Vec<(ER::Def, ())>),
) -> Vec<(Id, usize)> {
    use std::collections::BTreeMap;
    let mut deg: BTreeMap<Id, Id> = BTreeMap::new();
    for (nid, _) in ns {
        deg.entry(nid).or_insert(0);
    }
    for (edef, _) in es {
        let (nr, _): (NR<id::N>, ER::Slot) = edef.into();
        let (n1, n2) = (**nr.n1(), **nr.n2());
        *deg.entry(n1).or_insert(0) += 1;
        if n1 != n2 {
            *deg.entry(n2).or_insert(0) += 1;
        }
    }
    let mut hist: BTreeMap<Id, usize> = BTreeMap::new();
    for (_, &d) in &deg {
        *hist.entry(d).or_insert(0) += 1;
    }
    let mut result: Vec<_> = hist.into_iter().collect();
    result.sort_by(|a, b| b.0.cmp(&a.0));
    result
}

struct Shadow<ER: graph::Edge> {
    nodes: BTreeSet<Id>,
    edges: BTreeSet<(NR<id::N>, ER::Slot)>,
}

impl<ER: graph::Edge> Shadow<ER> {
    fn new() -> Self {
        Shadow {
            nodes: BTreeSet::new(),
            edges: BTreeSet::new(),
        }
    }

    fn add_node(&mut self, id: Id) {
        self.nodes.insert(id);
    }

    fn remove_node(&mut self, id: Id) {
        self.nodes.remove(&id);
        let n = id::N(id);
        self.edges.retain(|&(nr, _)| *nr.n1() != n && *nr.n2() != n);
    }

    fn add_edge(&mut self, nr: NR<id::N>, slot: ER::Slot) {
        self.edges.insert((nr, slot));
    }

    fn remove_edge(&mut self, nr: NR<id::N>, slot: ER::Slot) {
        self.edges.remove(&(nr, slot));
    }

    fn to_vecs(&self) -> (Vec<(Id, ())>, Vec<(ER::Def, ())>) {
        let ns: Vec<(Id, ())> = self.nodes.iter().map(|&id| (id, ())).collect();
        let es: Vec<(ER::Def, ())> = self
            .edges
            .iter()
            .map(|&(nr, slot)| (ER::edge(slot, (**nr.n1(), **nr.n2())), ()))
            .collect();
        (ns, es)
    }
}

trait ChaosEdge: graph::Edge<Val = ()> + Sized {
    fn random_slot(rng: &mut SmallRng) -> Self::Slot;
}

impl ChaosEdge for edge::Undir<()> {
    fn random_slot(_rng: &mut SmallRng) -> edge::undir::Slot {
        edge::undir::UND
    }
}

impl ChaosEdge for edge::Dir<()> {
    fn random_slot(rng: &mut SmallRng) -> edge::dir::Slot {
        if rng.random_bool(0.5) {
            edge::dir::SRC
        } else {
            edge::dir::TGT
        }
    }
}

impl ChaosEdge for edge::Anydir<()> {
    fn random_slot(rng: &mut SmallRng) -> edge::anydir::Slot {
        match rng.random_range(0u8..3) {
            0 => edge::anydir::UND,
            1 => edge::anydir::SRC,
            _ => edge::anydir::TGT,
        }
    }
}

enum EdgeTarget {
    Existing(Id),
    NewLocal(Id),
}

struct PlannedEdge<S> {
    source_local: Id,
    target: EdgeTarget,
    slot: S,
}

struct ExistEdgePlan<S> {
    source: Id,
    target: Id,
    slot: S,
}

struct RemoveEdgePlan<S> {
    nr: NR<id::N>,
    slot: S,
    source_id: Id,
}

struct Params {
    shrink_factor: f64,
    growth_factor: f64,
    density_factor: f64,
    graft_factor: f64,
    exist_edge_factor: f64,
    remove_edge_factor: f64,
    max_batch: usize,
}

fn chaos_step<ER: ChaosEdge>(
    graph: &mut Graph<(), ER>,
    shadow: &mut Shadow<ER>,
    rng: &mut SmallRng,
    step: usize,
    seed: u64,
    params: &Params,
) where
    for<'a> Graph<(), ER>: TryFrom<(Vec<(Id, ())>, Vec<(ER::Def, ())>)>,
    <Graph<(), ER> as TryFrom<(Vec<(Id, ())>, Vec<(ER::Def, ())>)>>::Error: std::fmt::Debug,
{
    let existing: Vec<Id> = shadow.nodes.iter().copied().collect();

    let removals: Vec<Id> = existing
        .iter()
        .copied()
        .filter(|_| rng.random_bool(params.shrink_factor))
        .collect();
    let surviving: Vec<Id> = existing
        .iter()
        .copied()
        .filter(|id| !removals.contains(id))
        .collect();

    let n_new = if surviving.is_empty() {
        1usize.max((params.growth_factor * 1.0) as usize)
    } else {
        1usize.max((params.growth_factor * surviving.len() as f64) as usize)
    }
    .min(params.max_batch);

    let mut ops: Vec<Node<(), ER>> = Vec::new();
    let mut edge_plan: Vec<PlannedEdge<ER::Slot>> = Vec::new();
    let mut exist_edge_plan: Vec<ExistEdgePlan<ER::Slot>> = Vec::new();
    let mut remove_edge_plan: Vec<RemoveEdgePlan<ER::Slot>> = Vec::new();

    for &id in &removals {
        ops.push(Node::Exist(Exist::Rem { id: id::N(id) }));
    }

    if surviving.len() >= 2 {
        let mut planned_exist: BTreeSet<(NR<id::N>, ER::Slot)> = BTreeSet::new();
        for i in 0..surviving.len() {
            if rng.random_bool(params.exist_edge_factor) {
                let j = rng.random_range(0..surviving.len());
                if i != j {
                    let src = surviving[i];
                    let tgt = surviving[j];
                    let slot = ER::random_slot(rng);
                    let def: ER::Def = ER::edge(slot, (src, tgt));
                    let (nr, stored_slot): (NR<id::N>, ER::Slot) = def.into();
                    if !shadow.edges.contains(&(nr, stored_slot))
                        && !planned_exist.contains(&(nr, stored_slot))
                    {
                        planned_exist.insert((nr, stored_slot));
                        exist_edge_plan.push(ExistEdgePlan {
                            source: src,
                            target: tgt,
                            slot,
                        });
                    }
                }
            }
        }
    }

    {
        let surviving_set: BTreeSet<Id> = surviving.iter().copied().collect();
        let removable_edges: Vec<(NR<id::N>, ER::Slot)> = shadow
            .edges
            .iter()
            .filter(|(nr, _)| {
                surviving_set.contains(&**nr.n1()) && surviving_set.contains(&**nr.n2())
            })
            .copied()
            .collect();

        for &(nr, slot) in &removable_edges {
            if rng.random_bool(params.remove_edge_factor) {
                remove_edge_plan.push(RemoveEdgePlan {
                    nr,
                    slot,
                    source_id: **nr.n1(),
                });
            }
        }
    }

    {
        use std::collections::BTreeMap;
        let mut exist_ops: BTreeMap<Id, Vec<modify::edge::Edge<(), ER>>> = BTreeMap::new();

        for ee in &exist_edge_plan {
            let edge_op = modify::edge::Edge::New {
                slot: ee.slot,
                val: (),
                target: Node::Exist(Exist::Bind {
                    id: id::N(ee.target),
                    op: Bind::Ref,
                    edges: vec![],
                }),
            };
            exist_ops.entry(ee.source).or_default().push(edge_op);
        }

        for re in &remove_edge_plan {
            let [n1, n2]: [id::N; 2] = re.nr.into();
            let (source_id, target_id) = if *n1 == re.source_id {
                (*n1, *n2)
            } else {
                (*n2, *n1)
            };
            let edge_op = modify::edge::Edge::Exist {
                slot: re.slot,
                op: modify::edge::Exist::Rem,
                target: Node::Exist(Exist::Bind {
                    id: id::N(target_id),
                    op: Bind::Ref,
                    edges: vec![],
                }),
            };
            exist_ops.entry(source_id).or_default().push(edge_op);
        }

        let removal_set: BTreeSet<Id> = removals.iter().copied().collect();
        for (src, edges) in exist_ops {
            if removal_set.contains(&src) {
                continue;
            }
            ops.push(Node::Exist(Exist::Bind {
                id: id::N(src),
                op: Bind::Pass,
                edges,
            }));
        }
    }

    for i in 1..=n_new as Id {
        let mut edges: Vec<modify::edge::Edge<(), ER>> = Vec::new();

        if !surviving.is_empty() && rng.random_bool(params.graft_factor) {
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
            edge_plan.push(PlannedEdge {
                source_local: i,
                target: EdgeTarget::Existing(target_id),
                slot,
            });
        }

        for prev in 1..i {
            if rng.random_bool(params.density_factor) {
                let slot = ER::random_slot(rng);
                edges.push(modify::edge::Edge::New {
                    slot,
                    val: (),
                    target: Node::New(New::Ref { id: LocalId(prev) }, vec![]),
                });
                edge_plan.push(PlannedEdge {
                    source_local: i,
                    target: EdgeTarget::NewLocal(prev),
                    slot,
                });
            }
        }

        ops.push(Node::New(
            New::Add {
                id: Some(LocalId(i)),
                val: (),
            },
            edges,
        ));
    }

    let result = graph.modify(ops).unwrap_or_else(|e| {
        panic!(
            "modify failed at step {step} (seed={seed}): {e:?}\n\
             shadow: {} nodes, {} edges\n\
             graph:  {} nodes, {} edges",
            shadow.nodes.len(),
            shadow.edges.len(),
            graph.node_count(),
            graph.edge_count(),
        )
    });

    for &id in &removals {
        shadow.remove_node(id);
    }
    for ee in &exist_edge_plan {
        let def: ER::Def = ER::edge(ee.slot, (ee.source, ee.target));
        let (nr, slot): (NR<id::N>, ER::Slot) = def.into();
        shadow.add_edge(nr, slot);
    }
    for re in &remove_edge_plan {
        shadow.remove_edge(re.nr, re.slot);
    }
    for i in 1..=n_new as Id {
        let real_id = *result.new_node_ids[&LocalId(i)];
        shadow.add_node(real_id);
    }
    for pe in &edge_plan {
        let src = *result.new_node_ids[&LocalId(pe.source_local)];
        let tgt = match pe.target {
            EdgeTarget::Existing(id) => id,
            EdgeTarget::NewLocal(lid) => *result.new_node_ids[&LocalId(lid)],
        };
        let def: ER::Def = ER::edge(pe.slot, (src, tgt));
        let (nr, slot): (NR<id::N>, ER::Slot) = def.into();
        shadow.add_edge(nr, slot);
    }

    assert_eq!(
        graph.node_count(),
        shadow.nodes.len(),
        "step {step}: node count mismatch (seed={seed})\n\
         graph={}, shadow={}",
        graph.node_count(),
        shadow.nodes.len(),
    );
    assert_eq!(
        graph.edge_count(),
        shadow.edges.len(),
        "step {step}: edge count mismatch (seed={seed})\n\
         graph={}, shadow={}",
        graph.edge_count(),
        shadow.edges.len(),
    );

    let _rebuilt: Graph<(), ER> = shadow.to_vecs().try_into().unwrap_or_else(|_| {
        panic!(
            "step {step}: shadow rebuild failed (seed={seed})\n\
             shadow: {} nodes, {} edges",
            shadow.nodes.len(),
            shadow.edges.len(),
        )
    });

    let actual_hist = degree_histogram::<ER>(graph.to_vecs());
    let expected_hist = degree_histogram::<ER>(shadow.to_vecs());

    assert_eq!(
        actual_hist, expected_hist,
        "step {step}: degree_histogram mismatch (seed={seed})\n\
         graph:  {} nodes, {} edges\n\
         shadow: {} nodes, {} edges\n\
         actual:   {actual_hist:?}\n\
         expected: {expected_hist:?}",
        graph.node_count(),
        graph.edge_count(),
        shadow.nodes.len(),
        shadow.edges.len(),
    );
}

fn chaos_v2<ER: ChaosEdge>(
    seed: u64,
    shrink_factor: f64,
    growth_factor: f64,
    density_factor: f64,
    graft_factor: f64,
    exist_edge_factor: f64,
    remove_edge_factor: f64,
    max_batch: usize,
    steps: usize,
) where
    for<'a> Graph<(), ER>: TryFrom<(Vec<(Id, ())>, Vec<(ER::Def, ())>)>,
    <Graph<(), ER> as TryFrom<(Vec<(Id, ())>, Vec<(ER::Def, ())>)>>::Error: std::fmt::Debug,
{
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut graph = Graph::<(), ER>::default();
    let mut shadow = Shadow::<ER>::new();

    let params = Params {
        shrink_factor,
        growth_factor,
        density_factor,
        graft_factor,
        exist_edge_factor,
        remove_edge_factor,
        max_batch,
    };

    for step in 0..steps {
        chaos_step::<ER>(&mut graph, &mut shadow, &mut rng, step, seed, &params);
    }

    eprintln!(
        "chaos_v2(seed={seed}): {steps} steps — {} nodes, {} edges",
        graph.node_count(),
        graph.edge_count(),
    );
}

fn chaos_v2_phased<ER: ChaosEdge>(
    seed: u64,
    warmup_params: Params,
    warmup_steps: usize,
    test_params: Params,
    test_steps: usize,
) where
    for<'a> Graph<(), ER>: TryFrom<(Vec<(Id, ())>, Vec<(ER::Def, ())>)>,
    <Graph<(), ER> as TryFrom<(Vec<(Id, ())>, Vec<(ER::Def, ())>)>>::Error: std::fmt::Debug,
{
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut graph = Graph::<(), ER>::default();
    let mut shadow = Shadow::<ER>::new();

    for step in 0..warmup_steps {
        chaos_step::<ER>(&mut graph, &mut shadow, &mut rng, step, seed, &warmup_params);
    }

    eprintln!(
        "  warmup done: {} nodes, {} edges",
        graph.node_count(), graph.edge_count(),
    );

    for step in 0..test_steps {
        chaos_step::<ER>(
            &mut graph, &mut shadow, &mut rng,
            warmup_steps + step, seed, &test_params,
        );
    }

    eprintln!(
        "chaos_v2_phased(seed={seed}): {}+{} steps — {} nodes, {} edges",
        warmup_steps, test_steps, graph.node_count(), graph.edge_count(),
    );
}

// ── balanced: moderate growth, moderate churn ────────────────────────────────

#[test]
fn v2_undir_balanced() {
    chaos_v2::<edge::Undir<()>>(42, 0.1, 0.5, 0.3, 0.5, 0.05, 0.05, 15, 80);
}

#[test]
fn v2_dir_balanced() {
    chaos_v2::<edge::Dir<()>>(42, 0.1, 0.5, 0.3, 0.5, 0.05, 0.05, 15, 80);
}

#[test]
fn v2_anydir_balanced() {
    chaos_v2::<edge::Anydir<()>>(42, 0.1, 0.5, 0.3, 0.5, 0.05, 0.05, 15, 80);
}

// ── growth heavy: fast growth, low shrink ───────────────────────────────────

#[test]
fn v2_undir_growth_heavy() {
    chaos_v2::<edge::Undir<()>>(123, 0.05, 1.5, 0.3, 0.7, 0.03, 0.02, 15, 40);
}

#[test]
fn v2_dir_growth_heavy() {
    chaos_v2::<edge::Dir<()>>(123, 0.05, 1.5, 0.3, 0.7, 0.03, 0.02, 15, 40);
}

// ── shrink heavy: aggressive node removal ───────────────────────────────────

#[test]
fn v2_undir_shrink_heavy() {
    chaos_v2::<edge::Undir<()>>(999, 0.4, 0.3, 0.2, 0.3, 0.02, 0.1, 15, 60);
}

// ── dense: high new-to-new and exist-to-exist edge density ──────────────────

#[test]
fn v2_undir_dense() {
    chaos_v2::<edge::Undir<()>>(777, 0.1, 0.5, 0.6, 0.8, 0.1, 0.02, 10, 60);
}

#[test]
fn v2_anydir_dense() {
    chaos_v2::<edge::Anydir<()>>(777, 0.1, 0.5, 0.5, 0.8, 0.1, 0.02, 10, 60);
}

// ── edge churn: heavy edge addition and removal between existing nodes ──────

#[test]
fn v2_undir_edge_churn() {
    chaos_v2::<edge::Undir<()>>(555, 0.05, 0.3, 0.2, 0.5, 0.15, 0.15, 12, 80);
}

#[test]
fn v2_dir_edge_churn() {
    chaos_v2::<edge::Dir<()>>(555, 0.05, 0.3, 0.2, 0.5, 0.15, 0.15, 12, 80);
}

#[test]
fn v2_anydir_edge_churn() {
    chaos_v2::<edge::Anydir<()>>(555, 0.05, 0.3, 0.2, 0.5, 0.15, 0.15, 12, 80);
}

// ── graft heavy: many new nodes connecting to existing structure ─────────────

#[test]
fn v2_undir_graft_heavy() {
    chaos_v2::<edge::Undir<()>>(314, 0.08, 0.8, 0.2, 0.95, 0.02, 0.01, 20, 60);
}

// ── mixed stress: all operations at moderate rates, many steps ──────────────

#[test]
fn v2_undir_mixed_stress() {
    chaos_v2::<edge::Undir<()>>(2024, 0.15, 0.6, 0.3, 0.6, 0.08, 0.08, 15, 100);
}

#[test]
fn v2_dir_mixed_stress() {
    chaos_v2::<edge::Dir<()>>(2024, 0.15, 0.6, 0.3, 0.6, 0.08, 0.08, 15, 100);
}

#[test]
fn v2_anydir_mixed_stress() {
    chaos_v2::<edge::Anydir<()>>(2024, 0.15, 0.6, 0.3, 0.6, 0.08, 0.08, 15, 100);
}

// ── multi-seed sweep: same params, different seeds ──────────────────────────

#[test]
fn v2_undir_seed_sweep() {
    for seed in [1, 7, 13, 42, 99, 256, 1000, 9999] {
        chaos_v2::<edge::Undir<()>>(seed, 0.12, 0.5, 0.3, 0.6, 0.08, 0.06, 15, 60);
    }
}

#[test]
fn v2_anydir_seed_sweep() {
    for seed in [1, 7, 13, 42, 99, 256, 1000, 9999] {
        chaos_v2::<edge::Anydir<()>>(seed, 0.12, 0.5, 0.3, 0.6, 0.08, 0.06, 15, 60);
    }
}

// ── large batch: wide modifications per step ─────────────────────────────────

#[test]
fn v2_undir_large_batch_50() {
    chaos_v2::<edge::Undir<()>>(42, 0.08, 1.0, 0.25, 0.6, 0.05, 0.03, 50, 60);
}

#[test]
fn v2_undir_large_batch_100() {
    chaos_v2::<edge::Undir<()>>(42, 0.05, 0.8, 0.2, 0.5, 0.03, 0.02, 100, 40);
}

#[test]
fn v2_anydir_large_batch_50() {
    chaos_v2::<edge::Anydir<()>>(42, 0.08, 1.0, 0.25, 0.6, 0.05, 0.03, 50, 60);
}

// ── ultra dense: extreme new-to-new edge density ─────────────────────────────

#[test]
fn v2_undir_ultra_dense_07() {
    chaos_v2::<edge::Undir<()>>(777, 0.1, 0.5, 0.7, 0.8, 0.1, 0.02, 10, 60);
}

#[test]
fn v2_undir_ultra_dense_09() {
    chaos_v2::<edge::Undir<()>>(777, 0.1, 0.4, 0.9, 0.8, 0.1, 0.02, 8, 50);
}

#[test]
fn v2_anydir_ultra_dense_08() {
    chaos_v2::<edge::Anydir<()>>(777, 0.1, 0.5, 0.8, 0.8, 0.1, 0.02, 10, 60);
}

// ── marathon: long-running incremental stability ─────────────────────────────

#[test]
fn v2_undir_marathon_200() {
    chaos_v2::<edge::Undir<()>>(2024, 0.12, 0.5, 0.3, 0.6, 0.08, 0.06, 15, 200);
}

#[test]
fn v2_undir_marathon_300() {
    chaos_v2::<edge::Undir<()>>(2024, 0.15, 0.4, 0.25, 0.5, 0.06, 0.06, 12, 300);
}

#[test]
fn v2_anydir_marathon_200() {
    chaos_v2::<edge::Anydir<()>>(2024, 0.12, 0.5, 0.3, 0.6, 0.08, 0.06, 15, 200);
}

// ── warmup + hammer: build large graph then stress test ──────────────────────

#[test]
fn v2_undir_warmup_hammer() {
    chaos_v2_phased::<edge::Undir<()>>(
        42,
        Params {
            shrink_factor: 0.0,
            growth_factor: 2.0,
            density_factor: 0.15,
            graft_factor: 0.5,
            exist_edge_factor: 0.02,
            remove_edge_factor: 0.0,
            max_batch: 50,
        },
        80,
        Params {
            shrink_factor: 0.2,
            growth_factor: 0.3,
            density_factor: 0.4,
            graft_factor: 0.7,
            exist_edge_factor: 0.15,
            remove_edge_factor: 0.15,
            max_batch: 20,
        },
        120,
    );
}

#[test]
fn v2_anydir_warmup_hammer() {
    chaos_v2_phased::<edge::Anydir<()>>(
        42,
        Params {
            shrink_factor: 0.0,
            growth_factor: 2.0,
            density_factor: 0.15,
            graft_factor: 0.5,
            exist_edge_factor: 0.02,
            remove_edge_factor: 0.0,
            max_batch: 50,
        },
        80,
        Params {
            shrink_factor: 0.2,
            growth_factor: 0.3,
            density_factor: 0.4,
            graft_factor: 0.7,
            exist_edge_factor: 0.15,
            remove_edge_factor: 0.15,
            max_batch: 20,
        },
        120,
    );
}

#[test]
fn v2_undir_warmup_dense_hammer() {
    chaos_v2_phased::<edge::Undir<()>>(
        123,
        Params {
            shrink_factor: 0.0,
            growth_factor: 1.5,
            density_factor: 0.3,
            graft_factor: 0.6,
            exist_edge_factor: 0.05,
            remove_edge_factor: 0.0,
            max_batch: 40,
        },
        100,
        Params {
            shrink_factor: 0.1,
            growth_factor: 0.2,
            density_factor: 0.7,
            graft_factor: 0.8,
            exist_edge_factor: 0.12,
            remove_edge_factor: 0.1,
            max_batch: 15,
        },
        150,
    );
}

// ── extreme edge churn: heavy edge add/remove between existing nodes ─────────

#[test]
fn v2_undir_extreme_churn() {
    chaos_v2::<edge::Undir<()>>(555, 0.05, 0.3, 0.2, 0.5, 0.25, 0.25, 12, 80);
}

#[test]
fn v2_anydir_extreme_churn() {
    chaos_v2::<edge::Anydir<()>>(555, 0.05, 0.3, 0.2, 0.5, 0.25, 0.25, 12, 80);
}

#[test]
fn v2_undir_churn_dense() {
    chaos_v2::<edge::Undir<()>>(555, 0.08, 0.4, 0.5, 0.6, 0.2, 0.2, 15, 80);
}

// ── multi-seed sweep at extreme params ───────────────────────────────────────

#[test]
fn v2_undir_seed_sweep_large() {
    for seed in [1, 7, 13, 42, 99, 256, 1000, 9999] {
        chaos_v2::<edge::Undir<()>>(seed, 0.08, 1.0, 0.3, 0.6, 0.05, 0.03, 50, 40);
    }
}

#[test]
fn v2_undir_seed_sweep_dense() {
    for seed in [1, 7, 13, 42, 99, 256, 1000, 9999] {
        chaos_v2::<edge::Undir<()>>(seed, 0.1, 0.5, 0.7, 0.8, 0.1, 0.02, 10, 50);
    }
}

#[test]
fn v2_anydir_seed_sweep_extreme() {
    for seed in [1, 7, 13, 42, 99, 256, 1000, 9999] {
        chaos_v2::<edge::Anydir<()>>(seed, 0.1, 0.5, 0.6, 0.7, 0.15, 0.1, 15, 60);
    }
}
