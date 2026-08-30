//! Differential oracle: random op sequences applied to MGraph (mutably, via a
//! clone-and-swap discipline) and VGraph (functionally) must, after every
//! step, present identical Graph-trait views, and every retained VGraph
//! version must remain bit-identical to its snapshot. Also checks generation
//! stamping and slot-reuse monotonicity, and Seq search binding-set parity
//! between MGraph and VGraph over random small graphs.

use std::collections::{BTreeSet, HashMap};

use grw::graph::{self, Graph as _};
use grw::modify::{self, N, X, e, n};
use grw::search::{self, Morphism, RevCsr, Search, Seq};

type ER = graph::edge::Undir<()>;
type MU = graph::MUndir0;
type VU = graph::VUndir0;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        self.0 = x; x
    }
    fn below(&mut self, n: u64) -> u64 { self.next() % n }
    fn chance(&mut self, num: u64, den: u64) -> bool { self.below(den) < num }
}

fn view<G: graph::Graph<(), ER>>(g: &G) -> Vec<(u32, Vec<u32>)> {
    let mut out: Vec<(u32, Vec<u32>)> = g.iter_node_ids().map(|nd| {
        let mut adj: Vec<u32> = g.neighbors(nd).unwrap().map(|(m, _, _)| *m as u32).collect();
        adj.sort_unstable();
        (*nd as u32, adj)
    }).collect();
    out.sort_unstable();
    out
}

fn next_fresh_id(m: &MU) -> u32 {
    let modi = m.clone().modify(vec![N::<(), ER>(0).into()]).expect("probe add cannot fail");
    *modi.new_node_ids[&modify::LocalId(0)] as u32
}

fn random_ops(rng: &mut Rng, m: &MU) -> Vec<modify::Node<(), ER>> {
    let existing: Vec<grw::id::N> = m.iter_node_ids().collect();
    let pick = rng.below(100);

    if pick < 30 || existing.len() < 2 {
        return vec![N::<(), ER>(0).into()];
    }

    if pick < 70 {
        let a = existing[rng.below(existing.len() as u64) as usize];
        let b = existing[rng.below(existing.len() as u64) as usize];
        return vec![(X::<(), ER>(a) ^ X::<(), ER>(b)).into()];
    }

    if pick < 85 {
        let a = existing[rng.below(existing.len() as u64) as usize];
        return vec![(!X::<(), ER>(a)).into()];
    }

    let mut candidates: Vec<(grw::id::N, grw::id::N)> = Vec::new();
    for &nd in &existing {
        if let Some(it) = m.neighbors(nd) {
            for (nb, _, _) in it {
                if nd <= nb { candidates.push((nd, nb)); }
            }
        }
    }
    if candidates.is_empty() {
        return vec![N::<(), ER>(0).into()];
    }
    let (a, b) = candidates[rng.below(candidates.len() as u64) as usize];
    vec![(X::<(), ER>(a) & !e::<(), ER>() ^ X::<(), ER>(b)).into()]
}

#[derive(Clone, Copy)]
enum Poison {
    DupEdge(grw::id::N, grw::id::N),
    MissingEdge(grw::id::N, grw::id::N),
}

fn choose_poison(rng: &mut Rng, m: &MU) -> Option<Poison> {
    let existing: Vec<grw::id::N> = m.iter_node_ids().collect();
    if existing.len() < 2 {
        return None;
    }
    let mut edges: Vec<(grw::id::N, grw::id::N)> = Vec::new();
    for &nd in &existing {
        if let Some(it) = m.neighbors(nd) {
            for (nb, _, _) in it {
                if nd <= nb { edges.push((nd, nb)); }
            }
        }
    }
    if rng.chance(1, 2) && !edges.is_empty() {
        let (a, b) = edges[rng.below(edges.len() as u64) as usize];
        return Some(Poison::DupEdge(a, b));
    }
    for _ in 0..existing.len() * existing.len() + 1 {
        let a = existing[rng.below(existing.len() as u64) as usize];
        let b = existing[rng.below(existing.len() as u64) as usize];
        if a != b && !edges.contains(&(a.min(b), a.max(b))) {
            return Some(Poison::MissingEdge(a, b));
        }
    }
    edges.first().map(|&(a, b)| Poison::DupEdge(a, b))
}

fn poison_node(p: Poison) -> modify::Node<(), ER> {
    match p {
        Poison::DupEdge(a, b) => (X::<(), ER>(a) ^ X::<(), ER>(b)).into(),
        Poison::MissingEdge(a, b) => (X::<(), ER>(a) & !e::<(), ER>() ^ X::<(), ER>(b)).into(),
    }
}

#[test]
fn differential_oracle() {
    let mut rng = Rng(0xA5A5_5A5A_1234_5678);
    let mut total_accepted: u64 = 0;
    let mut poisoned_batches: u64 = 0;
    let mut atomicity_checks: u64 = 0;
    let mut max_nodes: usize = 0;
    for case in 0..200 {
        let mut m: MU = grw::mgraph![N(0)].unwrap();
        let mut v: VU = VU::from_mgraph(&m);
        let mut kept: Vec<(VU, Vec<(u32, Vec<u32>)>)> = Vec::new();
        let mut gens: HashMap<u32, u64> = HashMap::new();
        for nd in v.iter_node_ids() {
            gens.insert(*nd as u32, v.node_gen(nd).unwrap());
        }

        for step in 0..60 {
            let seed_before = rng.0;
            let mut ops = random_ops(&mut rng, &m);
            let mut rng2 = Rng(seed_before);
            let mut ops2 = random_ops(&mut rng2, &m);

            if rng.chance(15, 100) {
                if let Some(p) = choose_poison(&mut rng, &m) {
                    poisoned_batches += 1;
                    ops.push(poison_node(p));
                    ops2.push(poison_node(p));
                }
            }

            let before_m = view(&m);
            let mut m_next = m.clone();
            let m_result = m_next.modify(ops);
            let v_result = v.modify(ops2);

            match (m_result, v_result) {
                (Ok(_), Ok((v_next, v_mod))) => {
                    total_accepted += 1;
                    m = m_next;
                    v = v_next;
                    let vm = view(&m);
                    max_nodes = max_nodes.max(vm.len());
                    assert_eq!(vm, view(&v), "case {case} step {step}");

                    for (_, id) in v_mod.new_node_ids.iter() {
                        let slot = **id as u32;
                        let node_gen = v.node_gen(*id).unwrap();
                        assert_eq!(node_gen, v.version(), "case {case} step {step}: new node gen mismatch");
                        if let Some(&prior) = gens.get(&slot) {
                            assert!(node_gen > prior, "case {case} step {step}: slot {slot} reused with non-increasing gen ({prior} -> {node_gen})");
                        }
                        gens.insert(slot, node_gen);
                    }

                    if step % 10 == 0 { kept.push((v.clone(), view(&v))); }
                }
                (Ok(_), Err(e)) => panic!("case {case} step {step}: mgraph accepted but vgraph rejected: {e:?}"),
                (Err(e), Ok(_)) => panic!("case {case} step {step}: vgraph accepted but mgraph rejected: {e:?}"),
                (Err(_), Err(_)) => {
                    atomicity_checks += 1;
                    assert_eq!(view(&m_next), before_m, "case {case} step {step}: mgraph clone mutated on Err");
                    assert_eq!(next_fresh_id(&m_next), next_fresh_id(&m), "case {case} step {step}: mgraph id space leaked on Err");
                    continue;
                }
            }
        }

        for (old, snap) in &kept {
            assert_eq!(&view(old), snap, "case {case}: retained version mutated");
        }
        for nd in v.iter_node_ids() {
            let node_gen = v.node_gen(nd).unwrap();
            assert!(node_gen <= v.version());
        }
    }
    // Seeds are fixed, so this count is deterministic. Measured identical (9180) at
    // 8afc6fd, before the validation hoist: any drift in WHICH batches are accepted
    // must fail here, not merely dip under an anti-vacuity floor.
    assert_eq!(total_accepted, 9180, "accept/reject drift: batches accepted across 200 cases changed");
    assert!(poisoned_batches > 500, "anti-vacuity floor: only {poisoned_batches} poisoned batches across 200 cases");
    assert!(max_nodes > 15, "anti-vacuity floor: reached state too small, max {max_nodes} nodes across 200 cases");
    assert!(atomicity_checks > 1000, "anti-vacuity floor: only {atomicity_checks} (Err,Err) atomicity checks across 200 cases");
}

#[test]
fn differential_oracle_error_atomicity() {
    let mut rng = Rng(0xDEAD_BEEF_1357_9BDF);
    let mut errored_batches: u64 = 0;
    for case in 0..200 {
        let mut m: MU = grw::mgraph![N(0)].unwrap();

        for step in 0..60 {
            let mut ops = random_ops(&mut rng, &m);
            let poisoned = choose_poison(&mut rng, &m).inspect(|&p| ops.push(poison_node(p))).is_some();

            let before = view(&m);
            let mut m_next = m.clone();
            match m_next.modify(ops) {
                Ok(_) => m = m_next,
                Err(_) => {
                    if poisoned { errored_batches += 1; }
                    assert_eq!(view(&m_next), before, "case {case} step {step}: clone mutated on Err");
                }
            }
        }
    }
    assert!(errored_batches > 500, "anti-vacuity floor: only {errored_batches} poisoned-and-rejected batches across 200 cases");
}

fn random_small_graph(rng: &mut Rng) -> MU {
    let n_nodes = 1 + rng.below(30) as u32;
    let mut ops: Vec<modify::Node<(), ER>> = (0..n_nodes).map(|i| N::<(), ER>(i).into()).collect();
    for a in 0..n_nodes {
        for b in (a + 1)..n_nodes {
            if rng.chance(1, 3) {
                ops.push((n::<(), ER>(a) ^ n::<(), ER>(b)).into());
            }
        }
    }
    let mut g: MU = grw::mgraph![].unwrap();
    g.modify(ops).unwrap();
    g
}

fn pattern_wedge() -> Vec<search::dsl::Op<(), ER>> {
    vec![
        (search::dsl::N::<(), ER>(0) ^ search::dsl::N::<(), ER>(1)).into(),
        (search::dsl::n::<(), ER>(1) ^ search::dsl::N::<(), ER>(2)).into(),
    ]
}

fn pattern_triangle() -> Vec<search::dsl::Op<(), ER>> {
    vec![
        (search::dsl::N::<(), ER>(0) ^ search::dsl::N::<(), ER>(1)).into(),
        (search::dsl::n::<(), ER>(1) ^ search::dsl::N::<(), ER>(2)).into(),
        (search::dsl::n::<(), ER>(0) ^ search::dsl::n::<(), ER>(2)).into(),
    ]
}

#[test]
fn search_parity_vs_vgraph() {
    let mut rng = Rng(0xC0FFEE_u64.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let mut wedge_total: usize = 0;
    let mut triangle_total: usize = 0;
    for case in 0..30 {
        let mg = random_small_graph(&mut rng);
        let vg: VU = VU::from_mgraph(&mg);

        for (name, ops) in [("wedge", pattern_wedge()), ("triangle", pattern_triangle())] {
            let clusters = vec![search::dsl::get(Morphism::Mono, ops)];
            let Search::Resolved(r) = search::compile::<(), ER>(clusters).unwrap() else {
                panic!("case {case} pattern {name}: unexpected unresolved query");
            };
            let query = r.query();
            let n_pattern = query.node_count() as u32;

            let mi = mg.index(RevCsr);
            let vi = vg.index(RevCsr);

            let m_set: BTreeSet<Vec<u32>> = Seq::search(query, &mi)
                .map(|mm| (0..n_pattern).map(|i| *mm.get(i).expect("bound") as u32).collect::<Vec<u32>>())
                .collect();
            let v_set: BTreeSet<Vec<u32>> = Seq::search(query, &vi)
                .map(|mm| (0..n_pattern).map(|i| *mm.get(i).expect("bound") as u32).collect::<Vec<u32>>())
                .collect();

            assert_eq!(m_set, v_set, "case {case} pattern {name}: binding sets differ");
            match name {
                "wedge" => wedge_total += m_set.len(),
                "triangle" => triangle_total += m_set.len(),
                _ => unreachable!(),
            }
        }
    }
    assert!(wedge_total > 1000, "search parity non-empty guard: wedge total {wedge_total}");
    assert!(triangle_total > 100, "search parity non-empty guard: triangle total {triangle_total}");
}
