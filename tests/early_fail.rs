use std::collections::BTreeSet;

use rand::Rng;
use rand::rngs::SmallRng;
use rand::SeedableRng;

use grw::graph::edge;
use grw::graph::{self, Undir0};
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

struct Shadow {
    nodes: BTreeSet<Id>,
    edges: BTreeSet<(NR<id::N>, edge::undir::Slot)>,
}

impl Shadow {
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

    fn add_edge(&mut self, nr: NR<id::N>, slot: edge::undir::Slot) {
        self.edges.insert((nr, slot));
    }

    fn remove_edge(&mut self, nr: NR<id::N>, slot: edge::undir::Slot) {
        self.edges.remove(&(nr, slot));
    }

    fn to_vecs(&self) -> (Vec<(Id, ())>, Vec<(edge::undir::E<Id>, ())>) {
        let ns: Vec<(Id, ())> = self.nodes.iter().map(|&id| (id, ())).collect();
        let es: Vec<(edge::undir::E<Id>, ())> = self
            .edges
            .iter()
            .map(|&(nr, slot)| (<edge::Undir<()> as graph::Edge>::edge(slot, (**nr.n1(), **nr.n2())), ()))
            .collect();
        (ns, es)
    }
}

struct EdgeTarget {
    existing: Option<Id>,
    new_local: Option<Id>,
}

struct PlannedEdge {
    source_local: Id,
    target: EdgeTarget,
    slot: edge::undir::Slot,
}

struct ExistEdgePlan {
    source: Id,
    target: Id,
    slot: edge::undir::Slot,
}

struct RemoveEdgePlan {
    nr: NR<id::N>,
    slot: edge::undir::Slot,
    source_id: Id,
}

fn chaos_step(
    graph: &mut Undir0,
    shadow: &mut Shadow,
    rng: &mut SmallRng,
    step: usize,
    seed: u64,
    shrink_factor: f64,
    growth_factor: f64,
    density_factor: f64,
    graft_factor: f64,
    exist_edge_factor: f64,
    remove_edge_factor: f64,
    max_batch: usize,
) {
    let existing: Vec<Id> = shadow.nodes.iter().copied().collect();

    let removals: Vec<Id> = existing
        .iter()
        .copied()
        .filter(|_| rng.random_bool(shrink_factor))
        .collect();
    let surviving: Vec<Id> = existing
        .iter()
        .copied()
        .filter(|id| !removals.contains(id))
        .collect();

    let n_new = if surviving.is_empty() {
        1usize.max((growth_factor * 1.0) as usize)
    } else {
        1usize.max((growth_factor * surviving.len() as f64) as usize)
    }
    .min(max_batch);

    let mut ops: Vec<Node<(), edge::Undir<()>>> = Vec::new();
    let mut edge_plan: Vec<PlannedEdge> = Vec::new();
    let mut exist_edge_plan: Vec<ExistEdgePlan> = Vec::new();
    let mut remove_edge_plan: Vec<RemoveEdgePlan> = Vec::new();

    for &id in &removals {
        ops.push(Node::Exist(Exist::Rem { id: id::N(id) }));
    }

    if surviving.len() >= 2 {
        let mut planned_exist: BTreeSet<(NR<id::N>, edge::undir::Slot)> = BTreeSet::new();
        for i in 0..surviving.len() {
            if rng.random_bool(exist_edge_factor) {
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
            if rng.random_bool(remove_edge_factor) {
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

        if !surviving.is_empty() && rng.random_bool(graft_factor) {
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
                target: EdgeTarget { existing: Some(target_id), new_local: None },
                slot,
            });
        }

        for prev in 1..i {
            if rng.random_bool(density_factor) {
                let slot = edge::undir::UND;
                edges.push(modify::edge::Edge::New {
                    slot,
                    val: (),
                    target: Node::New(New::Ref { id: LocalId(prev) }, vec![]),
                });
                edge_plan.push(PlannedEdge {
                    source_local: i,
                    target: EdgeTarget { existing: None, new_local: Some(prev) },
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

    let result = graph.modify(ops).unwrap();

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
        let tgt = match (&pe.target.existing, &pe.target.new_local) {
            (Some(id), _) => *id,
            (_, Some(lid)) => *result.new_node_ids[&LocalId(*lid)],
            _ => unreachable!(),
        };
        let def: edge::undir::E<Id> = <edge::Undir<()> as graph::Edge>::edge(pe.slot, (src, tgt));
        let (nr, slot): (NR<id::N>, edge::undir::Slot) = def.into();
        shadow.add_edge(nr, slot);
    }

    assert_eq!(graph.node_count(), shadow.nodes.len(), "step {step}: node count");
    assert_eq!(graph.edge_count(), shadow.edges.len(), "step {step}: edge count");

    let _rebuilt: Undir0 = shadow.to_vecs().try_into().unwrap();

    let actual_hist = degree_histogram(graph.to_vecs());
    let expected_hist = degree_histogram(shadow.to_vecs());
    assert_eq!(
        actual_hist, expected_hist,
        "step {step}: degree_histogram mismatch (seed={seed})"
    );
}

#[test]
fn find_early_fail() {
    let seed = 13u64;
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut g = Undir0::default();
    let mut s = Shadow::new();
    for step in 0..60 {
        chaos_step(
            &mut g, &mut s, &mut rng, step, seed,
            0.12, 0.5, 0.3, 0.6, 0.08, 0.06, 15,
        );
    }
}
