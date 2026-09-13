//! Differential oracle: random op sequences applied to MGraph (mutably, via a
//! clone-and-swap discipline) and VGraph (functionally) must, after every
//! step, present identical Graph-trait views, and every retained VGraph
//! version must remain bit-identical to its snapshot. Also checks generation
//! stamping and slot-reuse monotonicity, Seq search binding-set parity
//! between MGraph and VGraph over random small graphs, and — since T2 — that
//! a unique index on `v % 97` and a multi index on `v % 7` (the same two used
//! by `index_oracle.rs`) answer every possible key identically on both graphs
//! after every accepted step.

use std::collections::{BTreeSet, HashMap};

use grw::graph::index::{Cardinality, IndexDecl, IndexHit, IndexName, KeyBytes, KeyTag};
use grw::graph::{self, Graph as _};
use grw::modify::{self, N, X, e, n};
use grw::search::{self, Morphism, RevCsr, Search, Seq};

type ER = graph::edge::Undir<()>;
type MU = graph::MUndirN<u32>;
type VU = graph::VUndirN<u32>;

const UNIQUE: IndexName = IndexName("u_mod97");
const MULTI: IndexName = IndexName("m_mod7");
const UNIQUE_MOD: u32 = 97;
const MULTI_MOD: u32 = 7;
const VAL_RANGE: u64 = 300;

fn decls() -> Vec<IndexDecl<u32>> {
    vec![
        IndexDecl::new(UNIQUE, Cardinality::Unique, |v: &u32| Some(*v % UNIQUE_MOD)),
        IndexDecl::new(MULTI, Cardinality::Multi, |v: &u32| Some(*v % MULTI_MOD)),
    ]
}

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

fn view<G: graph::Graph<u32, ER>>(g: &G) -> Vec<(u32, Vec<u32>)> {
    let mut out: Vec<(u32, Vec<u32>)> = g.iter_node_ids().map(|nd| {
        let mut adj: Vec<u32> = g.neighbors(nd).unwrap().map(|(m, _, _)| *m as u32).collect();
        adj.sort_unstable();
        (*nd as u32, adj)
    }).collect();
    out.sort_unstable();
    out
}

/// Probes the lowest free node id by inserting a throwaway node on a clone,
/// choosing a value whose `% UNIQUE_MOD` residue is currently unoccupied so
/// the probe itself can never be refused by the unique index. `None` only
/// when every one of the 97 residues is already taken (the unique index caps
/// live node count at `UNIQUE_MOD`) — in that case there is nothing to probe
/// with, and callers compare `None == None` on both sides, which still holds.
fn next_fresh_id(m: &MU) -> Option<u32> {
    let tag = KeyTag::of::<u32>();
    for residue in 0..UNIQUE_MOD {
        if matches!(m.index_hit(UNIQUE, &KeyBytes::of(&residue), tag).unwrap(), IndexHit::None) {
            let modi = m.clone().modify(vec![N::<u32, ER>(0).val(residue).into()]).expect("probe add cannot fail");
            return Some(*modi.new_node_ids[&modify::LocalId(0)] as u32);
        }
    }
    None
}

fn random_val(rng: &mut Rng) -> u32 {
    rng.below(VAL_RANGE) as u32
}

fn random_ops(rng: &mut Rng, m: &MU) -> Vec<modify::Node<u32, ER>> {
    let existing: Vec<grw::id::N> = m.iter_node_ids().collect();
    let pick = rng.below(100);

    if pick < 30 || existing.len() < 2 {
        return vec![N::<u32, ER>(0).val(random_val(rng)).into()];
    }

    if pick < 70 {
        let a = existing[rng.below(existing.len() as u64) as usize];
        let b = existing[rng.below(existing.len() as u64) as usize];
        return vec![(X::<u32, ER>(a) ^ X::<u32, ER>(b)).into()];
    }

    if pick < 85 {
        let a = existing[rng.below(existing.len() as u64) as usize];
        return vec![(!X::<u32, ER>(a)).into()];
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
        return vec![N::<u32, ER>(0).val(random_val(rng)).into()];
    }
    let (a, b) = candidates[rng.below(candidates.len() as u64) as usize];
    vec![(X::<u32, ER>(a) & !e::<u32, ER>() ^ X::<u32, ER>(b)).into()]
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

fn poison_node(p: Poison) -> modify::Node<u32, ER> {
    match p {
        Poison::DupEdge(a, b) => (X::<u32, ER>(a) ^ X::<u32, ER>(b)).into(),
        Poison::MissingEdge(a, b) => (X::<u32, ER>(a) & !e::<u32, ER>() ^ X::<u32, ER>(b)).into(),
    }
}

/// Scans every possible key of both indices and asserts `index_hit` agrees
/// between `m` and `v`.
fn index_parity_check(m: &MU, v: &VU) {
    let tag = KeyTag::of::<u32>();
    for key in 0..UNIQUE_MOD {
        let mh = m.index_hit(UNIQUE, &KeyBytes::of(&key), tag).unwrap();
        let vh = v.index_hit(UNIQUE, &KeyBytes::of(&key), tag).unwrap();
        match (mh, vh) {
            (IndexHit::One(a), IndexHit::One(b)) => assert_eq!(a, b, "unique key {key} mismatch"),
            (IndexHit::None, IndexHit::None) => {}
            _ => panic!("unique key {key}: m/v index_hit shape mismatch"),
        }
    }
    for key in 0..MULTI_MOD {
        let mh = m.index_hit(MULTI, &KeyBytes::of(&key), tag).unwrap();
        let vh = v.index_hit(MULTI, &KeyBytes::of(&key), tag).unwrap();
        let m_set: BTreeSet<u32> = match mh {
            IndexHit::Many(s) => s.iter().map(|n| *n).collect(),
            IndexHit::None => BTreeSet::new(),
            IndexHit::One(_) => panic!("multi key {key}: m returned One"),
        };
        let v_set: BTreeSet<u32> = match vh {
            IndexHit::Many(s) => s.iter().map(|n| *n).collect(),
            IndexHit::None => BTreeSet::new(),
            IndexHit::One(_) => panic!("multi key {key}: v returned One"),
        };
        assert_eq!(m_set, v_set, "multi key {key} mismatch");
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
        let mut m: MU = grw::mgraph![N(0).val(0u32)].unwrap().with_indices(decls()).unwrap();
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
                    index_parity_check(&m, &v);

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
    // Seeds are fixed, so this count is deterministic. Recomputed for T2's
    // added value assignment + unique-index saturation (which now also
    // refuses batches that were previously accepted): any further drift in
    // WHICH batches are accepted must fail here, not merely dip under an
    // anti-vacuity floor.
    assert_eq!(total_accepted, TOTAL_ACCEPTED, "accept/reject drift: batches accepted across 200 cases changed");
    assert!(poisoned_batches > 500, "anti-vacuity floor: only {poisoned_batches} poisoned batches across 200 cases");
    assert!(max_nodes > 15, "anti-vacuity floor: reached state too small, max {max_nodes} nodes across 200 cases");
    assert!(atomicity_checks > 1000, "anti-vacuity floor: only {atomicity_checks} (Err,Err) atomicity checks across 200 cases");
}

const TOTAL_ACCEPTED: u64 = 8847;

#[test]
fn differential_oracle_error_atomicity() {
    let mut rng = Rng(0xDEAD_BEEF_1357_9BDF);
    let mut errored_batches: u64 = 0;
    for case in 0..200 {
        let mut m: MU = grw::mgraph![N(0).val(0u32)].unwrap();

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
    let mut ops: Vec<modify::Node<u32, ER>> = (0..n_nodes).map(|i| N::<u32, ER>(i).val(0u32).into()).collect();
    for a in 0..n_nodes {
        for b in (a + 1)..n_nodes {
            if rng.chance(1, 3) {
                ops.push((n::<u32, ER>(a) ^ n::<u32, ER>(b)).into());
            }
        }
    }
    let mut g: MU = grw::mgraph![].unwrap();
    g.modify(ops).unwrap();
    g
}

fn pattern_wedge() -> Vec<search::dsl::Op<u32, ER>> {
    vec![
        (search::dsl::N::<u32, ER>(0) ^ search::dsl::N::<u32, ER>(1)).into(),
        (search::dsl::n::<u32, ER>(1) ^ search::dsl::N::<u32, ER>(2)).into(),
    ]
}

fn pattern_triangle() -> Vec<search::dsl::Op<u32, ER>> {
    vec![
        (search::dsl::N::<u32, ER>(0) ^ search::dsl::N::<u32, ER>(1)).into(),
        (search::dsl::n::<u32, ER>(1) ^ search::dsl::N::<u32, ER>(2)).into(),
        (search::dsl::n::<u32, ER>(0) ^ search::dsl::n::<u32, ER>(2)).into(),
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
            let Search::Resolved(r) = search::compile::<u32, ER>(clusters).unwrap() else {
                panic!("case {case} pattern {name}: unexpected unresolved query");
            };
            let query = r.query();
            let n_pattern = query.node_count() as u32;

            let mi = mg.index(RevCsr);
            let vi = vg.index(RevCsr);

            let m_set: BTreeSet<Vec<u32>> = Seq::search(query, &mi).unwrap()
                .map(|mm| (0..n_pattern).map(|i| *mm.get(i).expect("bound") as u32).collect::<Vec<u32>>())
                .collect();
            let v_set: BTreeSet<Vec<u32>> = Seq::search(query, &vi).unwrap()
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

    key_parity(&mut rng);
}

const OBY_VAL: IndexName = IndexName("oracle_by_val");
const OBY_BUCKET: IndexName = IndexName("oracle_by_bucket");

fn oracle_decls() -> Vec<IndexDecl<u32>> {
    vec![
        IndexDecl::new(OBY_VAL, Cardinality::Unique, |v: &u32| Some(*v)),
        IndexDecl::new(OBY_BUCKET, Cardinality::Multi, |v: &u32| Some(*v % 5)),
    ]
}

/// Node values are their own ids, so `oracle_by_val` is a genuine unique key
/// and `oracle_by_bucket` (`v % 5`) groups roughly a fifth of the graph.
fn random_valued_graph(rng: &mut Rng) -> MU {
    let n_nodes = 3 + rng.below(18) as u32;
    let mut ops: Vec<modify::Node<u32, ER>> =
        (0..n_nodes).map(|i| N::<u32, ER>(i).val(i).into()).collect();
    for a in 0..n_nodes {
        for b in (a + 1)..n_nodes {
            if rng.chance(1, 3) {
                ops.push((n::<u32, ER>(a) ^ n::<u32, ER>(b)).into());
            }
        }
    }
    let mut g: MU = grw::mgraph![].unwrap();
    g.modify(ops).unwrap();
    g
}

#[derive(Clone, Copy)]
enum Sel {
    Unconstrained,
    Key(u32),
    KeyIn(u32, u32),
    Test(u32),
    KeyAndTest(u32, u32),
}

/// The same constraint twice: once addressed through an index, once as the
/// closure that decides the same thing by scanning. The oracle asserts the
/// two select identically.
fn sel_op(sel: Sel, keyed: bool) -> search::dsl::Op<u32, ER> {
    use search::dsl::N as SN;
    match (sel, keyed) {
        (Sel::Unconstrained, _) => SN::<u32, ER>(0).into(),
        (Sel::Key(k), true) => SN::<u32, ER>(0).key(OBY_VAL, k).into(),
        (Sel::Key(k), false) => SN::<u32, ER>(0).test(move |v: &u32| *v == k).into(),
        (Sel::KeyIn(a, b), true) => SN::<u32, ER>(0).key_in(OBY_BUCKET, [a, b]).into(),
        (Sel::KeyIn(a, b), false) => {
            SN::<u32, ER>(0).test(move |v: &u32| *v % 5 == a || *v % 5 == b).into()
        }
        (Sel::Test(c), _) => SN::<u32, ER>(0).test(move |v: &u32| *v > c).into(),
        (Sel::KeyAndTest(a, c), true) => {
            SN::<u32, ER>(0).key_in(OBY_BUCKET, [a]).test(move |v: &u32| *v > c).into()
        }
        (Sel::KeyAndTest(a, c), false) => {
            SN::<u32, ER>(0).test(move |v: &u32| *v % 5 == a && *v > c).into()
        }
    }
}

fn ban_op(key: u32, keyed: bool) -> search::dsl::ClusterOps<u32, ER> {
    use search::dsl::{N as SN, n as sn};
    let target: search::dsl::Op<u32, ER> = if keyed {
        SN::<u32, ER>(9).key(OBY_VAL, key).into()
    } else {
        SN::<u32, ER>(9).test(move |v: &u32| *v == key).into()
    };
    search::dsl::ban(Morphism::Mono, vec![target, (sn::<u32, ER>(0) ^ sn::<u32, ER>(9)).into()])
}

fn clusters(shape: u64, sel: Sel, ban: Option<u32>, keyed: bool) -> Vec<search::dsl::ClusterOps<u32, ER>> {
    use search::dsl::{E as SE, N as SN, n as sn};
    let mut ops: Vec<search::dsl::Op<u32, ER>> = vec![
        sel_op(sel, keyed),
        (sn::<u32, ER>(0) ^ SN::<u32, ER>(1)).into(),
        (sn::<u32, ER>(1) ^ SN::<u32, ER>(2)).into(),
    ];
    match shape {
        1 => ops.push((sn::<u32, ER>(0) ^ sn::<u32, ER>(2)).into()),
        2 => ops.push((sn::<u32, ER>(0) & !SE::<u32, ER>() ^ sn::<u32, ER>(2)).into()),
        _ => {}
    }
    let mut out = vec![search::dsl::get(Morphism::Mono, ops)];
    if let Some(key) = ban {
        out.push(ban_op(key, keyed));
    }
    out
}

/// 300 random (graph, query) pairs: every query is compiled twice — once with
/// `.key`/`.key_in` against graphs carrying the two oracle indices, once as
/// the equivalent closure against the same graphs without indices — and all
/// four runs (MGraph/VGraph x keyed/unkeyed) must produce the same bindings.
fn key_parity(rng: &mut Rng) {
    let mut total: usize = 0;
    let mut keyed_cases: usize = 0;
    for case in 0..300 {
        let plain_m = random_valued_graph(rng);
        let plain_v: VU = VU::from_mgraph(&plain_m);
        let idx_m = plain_m.clone().with_indices(oracle_decls()).unwrap();
        let idx_v = VU::from_mgraph(&plain_m).with_indices(oracle_decls()).unwrap();
        let live = plain_m.node_count() as u32;

        let shape = rng.below(3);
        let sel = match rng.below(5) {
            0 => Sel::Unconstrained,
            1 => Sel::Key(rng.below(live as u64 + 4) as u32),
            2 => Sel::KeyIn(rng.below(5) as u32, rng.below(5) as u32),
            3 => Sel::Test(rng.below(live as u64 + 1) as u32),
            _ => Sel::KeyAndTest(rng.below(5) as u32, rng.below(live as u64 + 1) as u32),
        };
        if !matches!(sel, Sel::Unconstrained | Sel::Test(_)) {
            keyed_cases += 1;
        }
        let ban = rng.chance(1, 3).then(|| rng.below(live as u64 + 2) as u32);
        let pin = rng.chance(1, 3).then(|| (rng.below(3) as usize, rng.below(live as u64) as u32));

        let mut sets: Vec<BTreeSet<Vec<u32>>> = Vec::new();
        for keyed in [true, false] {
            let search::Search::Resolved(r) = search::compile::<u32, ER>(clusters(shape, sel, ban, keyed)).unwrap() else {
                panic!("case {case}: unexpected unresolved query");
            };
            let query = r.query();
            let mut bindings: Vec<Option<grw::id::N>> = vec![None; query.node_count()];
            if let Some((idx, node)) = pin {
                bindings[idx] = Some(grw::id::N(node));
            }
            if keyed {
                let mi = idx_m.index(RevCsr);
                let vi = idx_v.index(RevCsr);
                sets.push(collect_triples(query, Seq::search_bound(query, &mi, bindings.clone()).unwrap()));
                sets.push(collect_triples(query, Seq::search_bound(query, &vi, bindings).unwrap()));
            } else {
                let mi = plain_m.index(RevCsr);
                let vi = plain_v.index(RevCsr);
                sets.push(collect_triples(query, Seq::search_bound(query, &mi, bindings.clone()).unwrap()));
                sets.push(collect_triples(query, Seq::search_bound(query, &vi, bindings).unwrap()));
            }
        }
        for other in &sets[1..] {
            assert_eq!(&sets[0], other, "case {case}: binding sets differ across keyed/unkeyed or MGraph/VGraph");
        }
        total += sets[0].len();
    }
    assert!(total > 500, "key parity non-empty guard: total {total}");
    assert!(keyed_cases > 100, "key parity coverage guard: keyed cases {keyed_cases}");
}

fn collect_triples<I: Iterator<Item = search::Match>>(
    query: &search::Query<u32, ER>,
    it: I,
) -> BTreeSet<Vec<u32>> {
    let lids: Vec<grw::search::dsl::LocalId> = (0..query.node_count())
        .map(|i| query.node_local_id(i))
        .filter(|lid| lid.0 < 3)
        .collect();
    it.map(|m| lids.iter().map(|lid| *m.get(*lid).expect("bound") as u32).collect::<Vec<u32>>())
        .collect()
}
