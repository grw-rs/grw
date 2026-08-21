use std::collections::BTreeSet;

use rand::Rng;
use rand::rngs::SmallRng;
use rand::SeedableRng;

use grw::graph::edge;
use grw::graph::{self, Graph};
use grw::modify::{self, LocalId, Node};
use grw::modify::node::{Bind, Exist, New};
use grw::{Id, NR, id};

fn degree_histogram(
    (ns, es): (Vec<(Id, ())>, Vec<(edge::undir::E<Id>, ())>),
) -> Vec<(Id, usize)> {
    use std::collections::BTreeMap;
    let mut deg: BTreeMap<Id, Id> = BTreeMap::new();
    for (nid, _) in ns {
        deg.entry(nid).or_insert(0);
    }
    for (edef, _) in es {
        let (nr, _): (NR<id::N>, edge::undir::Slot) = edef.into();
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

struct StepStats {
    step: usize,
    nodes_before: usize,
    edges_before: usize,
    removals: usize,
    surviving: usize,
    n_new: usize,
    graft_edges: usize,
    new_to_new_edges: usize,
    exist_edges: usize,
    remove_edges: usize,
    nodes_after: usize,
    edges_after: usize,
    removed_node_ids: Vec<Id>,
}

fn replay_step(
    graph: &mut Graph<(), edge::Undir<()>>,
    shadow: &mut Shadow<edge::Undir<()>>,
    rng: &mut SmallRng,
    step: usize,
) -> StepStats {
    let params = Params {
        shrink_factor: 0.1,
        growth_factor: 0.5,
        density_factor: 0.3,
        graft_factor: 0.5,
        exist_edge_factor: 0.05,
        remove_edge_factor: 0.05,
        max_batch: 15,
    };

    let nodes_before = shadow.nodes.len();
    let edges_before = shadow.edges.len();

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

    let mut ops: Vec<Node<(), edge::Undir<()>>> = Vec::new();
    let mut edge_plan: Vec<PlannedEdge<edge::undir::Slot>> = Vec::new();
    let mut exist_edge_plan: Vec<ExistEdgePlan<edge::undir::Slot>> = Vec::new();
    let mut remove_edge_plan: Vec<RemoveEdgePlan<edge::undir::Slot>> = Vec::new();

    for &id in &removals {
        ops.push(Node::Exist(Exist::Rem { id: id::N(id) }));
    }

    if surviving.len() >= 2 {
        let mut planned_exist: BTreeSet<(NR<id::N>, edge::undir::Slot)> = BTreeSet::new();
        for i in 0..surviving.len() {
            if rng.random_bool(params.exist_edge_factor) {
                let j = rng.random_range(0..surviving.len());
                if i != j {
                    let src = surviving[i];
                    let tgt = surviving[j];
                    let slot = edge::undir::UND;
                    let def: edge::undir::E<Id> = <edge::Undir<()> as graph::Edge>::edge(slot, (src, tgt));
                    let (nr, stored_slot): (NR<id::N>, edge::undir::Slot) = def.into();
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
        let removable_edges: Vec<(NR<id::N>, edge::undir::Slot)> = shadow
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

    let mut graft_count = 0usize;
    let mut new_to_new_count = 0usize;

    {
        use std::collections::BTreeMap;
        let mut exist_ops: BTreeMap<Id, Vec<modify::edge::Edge<(), edge::Undir<()>>>> = BTreeMap::new();

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
        let mut edges: Vec<modify::edge::Edge<(), edge::Undir<()>>> = Vec::new();

        if !surviving.is_empty() && rng.random_bool(params.graft_factor) {
            let target_id = surviving[rng.random_range(0..surviving.len())];
            let slot = edge::undir::UND;
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
            graft_count += 1;
        }

        for prev in 1..i {
            if rng.random_bool(params.density_factor) {
                let slot = edge::undir::UND;
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
                new_to_new_count += 1;
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
            "modify failed at step {step} (seed=42): {e:?}\n\
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
        let def: edge::undir::E<Id> = <edge::Undir<()> as graph::Edge>::edge(ee.slot, (ee.source, ee.target));
        let (nr, slot): (NR<id::N>, edge::undir::Slot) = def.into();
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
        let def: edge::undir::E<Id> = <edge::Undir<()> as graph::Edge>::edge(pe.slot, (src, tgt));
        let (nr, slot): (NR<id::N>, edge::undir::Slot) = def.into();
        shadow.add_edge(nr, slot);
    }

    let nodes_after = shadow.nodes.len();
    let edges_after = shadow.edges.len();

    StepStats {
        step,
        nodes_before,
        edges_before,
        removals: removals.len(),
        surviving: surviving.len(),
        n_new,
        graft_edges: graft_count,
        new_to_new_edges: new_to_new_count,
        exist_edges: exist_edge_plan.len(),
        remove_edges: remove_edge_plan.len(),
        nodes_after,
        edges_after,
        removed_node_ids: removals,
    }
}

fn roundtrip_check(
    graph: &Graph<(), edge::Undir<()>>,
    shadow: &Shadow<edge::Undir<()>>,
    step: usize,
) -> bool {
    assert_eq!(
        graph.node_count(),
        shadow.nodes.len(),
        "step {step}: node count mismatch (seed=42)\n\
         graph={}, shadow={}",
        graph.node_count(),
        shadow.nodes.len(),
    );
    assert_eq!(
        graph.edge_count(),
        shadow.edges.len(),
        "step {step}: edge count mismatch (seed=42)\n\
         graph={}, shadow={}",
        graph.edge_count(),
        shadow.edges.len(),
    );

    let _rebuilt: Graph<(), edge::Undir<()>> = shadow.to_vecs().try_into().unwrap_or_else(|_| {
        panic!(
            "step {step}: shadow rebuild failed (seed=42)\n\
             shadow: {} nodes, {} edges",
            shadow.nodes.len(),
            shadow.edges.len(),
        )
    });

    let actual_hist = degree_histogram(graph.to_vecs());
    let expected_hist = degree_histogram(shadow.to_vecs());

    if actual_hist != expected_hist {
        eprintln!(
            "step {step}: degree_histogram MISMATCH (seed=42)\n\
             graph:  {} nodes, {} edges\n\
             shadow: {} nodes, {} edges\n\
             actual:   {actual_hist:?}\n\
             expected: {expected_hist:?}",
            graph.node_count(),
            graph.edge_count(),
            shadow.nodes.len(),
            shadow.edges.len(),
        );
        return false;
    }

    true
}

#[test]
fn replay_chaos_seed42() {
    let mut rng = SmallRng::seed_from_u64(42);
    let mut graph = Graph::<(), edge::Undir<()>>::default();
    let mut shadow = Shadow::<edge::Undir<()>>::new();

    for step in 0..=15 {
        eprintln!("========== STEP {step} ==========");

        let stats = replay_step(&mut graph, &mut shadow, &mut rng, step);

        eprintln!(
            "  before: {} nodes, {} edges",
            stats.nodes_before, stats.edges_before,
        );
        eprintln!(
            "  removals: {} {:?}",
            stats.removals, stats.removed_node_ids,
        );
        eprintln!("  surviving: {}", stats.surviving);
        eprintln!("  n_new: {}", stats.n_new);
        eprintln!("  graft_edges: {}", stats.graft_edges);
        eprintln!("  new_to_new_edges: {}", stats.new_to_new_edges);
        eprintln!("  exist_edges: {}", stats.exist_edges);
        eprintln!("  remove_edges: {}", stats.remove_edges);
        eprintln!(
            "  after: {} nodes, {} edges",
            stats.nodes_after, stats.edges_after,
        );
        eprintln!(
            "  graph: {} nodes, {} edges",
            graph.node_count(), graph.edge_count(),
        );

        let ok = roundtrip_check(&graph, &shadow, step);

        if !ok {
            eprintln!("DIVERGENCE DETECTED AT STEP {step}");
            eprintln!("shadow nodes: {:?}", shadow.nodes);
            eprintln!("shadow edges: {:?}", shadow.edges);
            panic!(
                "Roundtrip check failed at step {step}. See stderr for details."
            );
        }

        eprintln!("  roundtrip: OK");
    }

    eprintln!("All 16 steps passed roundtrip check.");
}
