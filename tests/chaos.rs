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

struct Params {
    shrink_factor: f64,
    growth_factor: f64,
    density_factor: f64,
    graft_factor: f64,
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

    for &id in &removals {
        ops.push(Node::Exist(Exist::Rem { id: id::N(id) }));
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

    let actual = degree_histogram::<ER>(graph.to_vecs());
    let expected = degree_histogram::<ER>(shadow.to_vecs());
    assert_eq!(
        actual, expected,
        "step {step}: degree_histogram mismatch (seed={seed})\n\
         graph:  {} nodes, {} edges\n\
         shadow: {} nodes, {} edges\n\
         actual:   {actual:?}\n\
         expected: {expected:?}",
        graph.node_count(),
        graph.edge_count(),
        shadow.nodes.len(),
        shadow.edges.len(),
    );
}

fn chaos_monkey<ER: ChaosEdge>(
    seed: u64,
    shrink_factor: f64,
    growth_factor: f64,
    density_factor: f64,
    graft_factor: f64,
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
        max_batch,
    };

    for step in 0..steps {
        chaos_step::<ER>(&mut graph, &mut shadow, &mut rng, step, seed, &params);
    }

    eprintln!(
        "chaos(seed={seed}): {steps} steps completed — {} nodes, {} edges",
        graph.node_count(),
        graph.edge_count(),
    );
}

#[test]
fn chaos_undir_balanced() {
    chaos_monkey::<edge::Undir<()>>(42, 0.1, 0.5, 0.3, 0.5, 15, 50);
}

#[test]
fn chaos_undir_growth_heavy() {
    chaos_monkey::<edge::Undir<()>>(123, 0.05, 1.5, 0.3, 0.7, 15, 30);
}

#[test]
fn chaos_undir_shrink_heavy() {
    chaos_monkey::<edge::Undir<()>>(999, 0.4, 0.3, 0.2, 0.3, 15, 40);
}

#[test]
fn chaos_dir_balanced() {
    chaos_monkey::<edge::Dir<()>>(42, 0.1, 0.5, 0.3, 0.5, 15, 50);
}

#[test]
fn chaos_dir_growth_heavy() {
    chaos_monkey::<edge::Dir<()>>(123, 0.05, 1.5, 0.3, 0.7, 15, 30);
}

#[test]
fn chaos_anydir_balanced() {
    chaos_monkey::<edge::Anydir<()>>(42, 0.1, 0.5, 0.3, 0.5, 15, 50);
}

#[test]
fn chaos_anydir_dense() {
    chaos_monkey::<edge::Anydir<()>>(777, 0.1, 0.5, 0.5, 0.8, 10, 30);
}
