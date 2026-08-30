//! Brute-force oracle for mixed-morphism semantics (the "(ii)" rule):
//! a match may bind two pattern nodes to the same target iff they are not
//! both injective. VF3 cannot express Homo/Epi, so this enumerator is the
//! ground truth for mixed queries — fuzzed against Seq, Par, and both
//! count paths.

use std::collections::BTreeSet;

use grw::search::dsl;
use grw::search::{self, Morphism, Search, Seq, Par, RevCsr};
use grw::Graph as _;
use rayon::iter::ParallelIterator;


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

struct Case {
    target_n: usize,
    target_edges: Vec<(u32, u32)>,
    pattern_n: usize,
    pattern_edges: Vec<(u32, u32)>,
    cluster_of: Vec<usize>,
    cluster_morphism: Vec<Morphism>,
}

fn gen_case(rng: &mut Rng) -> Case {
    let target_n = 2 + rng.below(4) as usize;
    let mut target_edges = Vec::new();
    for a in 0..target_n as u32 {
        for b in (a + 1)..target_n as u32 {
            if rng.chance(1, 2) { target_edges.push((a, b)); }
        }
    }
    let pattern_n = 1 + rng.below(4) as usize;
    let mut pattern_edges = Vec::new();
    for a in 0..pattern_n as u32 {
        for b in (a + 1)..pattern_n as u32 {
            if rng.chance(1, 2) { pattern_edges.push((a, b)); }
        }
    }
    let cluster_count = 1 + rng.below(3) as usize;
    let cluster_of: Vec<usize> = (0..pattern_n).map(|_| rng.below(cluster_count as u64) as usize).collect();
    let morphs = [Morphism::Mono, Morphism::Homo, Morphism::Epi, Morphism::SubIso];
    let cluster_morphism: Vec<Morphism> = (0..cluster_count).map(|_| morphs[rng.below(4) as usize]).collect();
    Case { target_n, target_edges, pattern_n, pattern_edges, cluster_of, cluster_morphism }
}

/// What the oracle produced: the accepted mappings, plus how many mappings
/// were killed *solely* by the induced rule (they passed edge, distinctness
/// and surjectivity). The counters are the anti-vacuity meter for `SubIso`
/// mixtures — without them the extra rule could be dead weight.
///
/// `cross_induced_rejects` is the sharper of the two: it counts only the
/// kills where the far end of the offending target edge was injective but
/// *not* induced. That is the order-independent half of the ratified rule —
/// the half an arrival-order-only engine gets wrong.
struct Verdict {
    matches: BTreeSet<Vec<u32>>,
    induced_only_rejects: usize,
    cross_induced_rejects: usize,
}

fn oracle(case: &Case) -> Verdict {
    let has_edge = |a: u32, b: u32| -> bool {
        a != b && case.target_edges.iter().any(|&(x, y)| (x, y) == (a.min(b), a.max(b)))
    };
    // Undir: the oracle's pattern edges are stored a<b, so adjacency is read
    // symmetrically and the induced rule needs no direction case.
    let pattern_adjacent = |a: usize, b: usize| -> bool {
        let (lo, hi) = (a.min(b) as u32, a.max(b) as u32);
        case.pattern_edges.iter().any(|&(x, y)| (x, y) == (lo, hi))
    };
    let injective: Vec<bool> = (0..case.pattern_n)
        .map(|i| case.cluster_morphism[case.cluster_of[i]].is_injective())
        .collect();
    let induced: Vec<bool> = (0..case.pattern_n)
        .map(|i| case.cluster_morphism[case.cluster_of[i]].is_induced())
        .collect();
    // Only clusters that actually own a node exist in the compiled query.
    let surjective = case.cluster_morphism.iter().enumerate()
        .any(|(c, m)| m.is_surjective() && case.cluster_of.contains(&c));

    let mut out = BTreeSet::new();
    let mut induced_only_rejects = 0usize;
    let mut cross_induced_rejects = 0usize;
    let t = case.target_n as u32;
    let total = (t as u64).pow(case.pattern_n as u32);
    'outer: for mut code in 0..total {
        let mut m = vec![0u32; case.pattern_n];
        for slot in m.iter_mut() { *slot = (code % t as u64) as u32; code /= t as u64; }
        for &(a, b) in &case.pattern_edges {
            if !has_edge(m[a as usize], m[b as usize]) { continue 'outer; }
        }
        for i in 0..case.pattern_n {
            for j in (i + 1)..case.pattern_n {
                if injective[i] && injective[j] && m[i] == m[j] { continue 'outer; }
            }
        }
        if surjective {
            let covered: BTreeSet<u32> = m.iter().copied().collect();
            if covered.len() != case.target_n { continue 'outer; }
        }
        // Ratified rule: induced-ness holds between a SubIso/Iso node and
        // every injective binding, whatever cluster that binding came from.
        // Free bindings are invisible to it.
        let mut killed = false;
        let mut crossed = false;
        for i in 0..case.pattern_n {
            if !induced[i] { continue; }
            for j in 0..case.pattern_n {
                if i == j || !injective[j] { continue; }
                if has_edge(m[i], m[j]) && !pattern_adjacent(i, j) {
                    killed = true;
                    if !induced[j] { crossed = true; }
                }
            }
        }
        if killed {
            induced_only_rejects += 1;
            if crossed { cross_induced_rejects += 1; }
            continue 'outer;
        }
        out.insert(m);
    }
    Verdict { matches: out, induced_only_rejects, cross_induced_rejects }
}

fn build_and_run(case: &Case) -> Option<(BTreeSet<Vec<u32>>, usize, usize, usize)> {
    use grw::graph::dsl as gdsl;
    type GER = grw::graph::edge::Undir<()>;
    let mut ops: Vec<gdsl::Op<(), GER>> = Vec::new();
    for i in 0..case.target_n as u32 {
        ops.push(gdsl::N::<(), GER>(i).into());
    }
    for &(a, b) in &case.target_edges {
        ops.push((gdsl::n::<(), GER>(a) ^ gdsl::n::<(), GER>(b)).into());
    }
    let g: grw::graph::MUndir0 = gdsl::from_fragment::<(), GER>(ops).unwrap();
    let t = g.index(RevCsr);

    // Every node is DEFINED in its assigned cluster (bare N), so the
    // node's morphism follows cluster_of exactly; edges are pure references
    // and never move a definition.
    let mut cluster_ops: Vec<Vec<dsl::Op<(), GER>>> =
        (0..case.cluster_morphism.len()).map(|_| Vec::new()).collect();
    for i in 0..case.pattern_n {
        cluster_ops[case.cluster_of[i]].push(dsl::N::<(), GER>(i as u32).into());
    }
    for &(a, b) in &case.pattern_edges {
        cluster_ops[case.cluster_of[a as usize]]
            .push((dsl::n::<(), GER>(a) ^ dsl::n::<(), GER>(b)).into());
    }
    let clusters: Vec<dsl::ClusterOps<(), GER>> = cluster_ops.into_iter()
        .zip(case.cluster_morphism.iter())
        .filter(|(ops, _)| !ops.is_empty())
        .map(|(ops, &m)| dsl::get(m, ops))
        .collect();
    if clusters.is_empty() { return None; }

    let Search::Resolved(r) = search::compile::<(), GER>(clusters).unwrap() else { return None; };
    let query = r.query();

    let mut engine_set = BTreeSet::new();
    for m in Seq::search(&query, &t) {
        let mut v = vec![u32::MAX; case.pattern_n];
        for i in 0..case.pattern_n {
            let n = m.get(grw::graph::dsl::LocalId(i as u32)).expect("bound");
            v[i] = *n as u32;
        }
        engine_set.insert(v);
    }
    let seq_count = Seq::search(&query, &t).count();
    let par_enum: usize = Par::search(&query, &t).map(|_m| 1usize).sum();
    let par_count = Par::search(&query, &t).count();
    Some((engine_set, seq_count, par_enum, par_count))
}

#[test]
fn fuzz_mixed_morphisms_vs_oracle() {
    let mut rng = Rng(0x9e3779b97f4a7c15);
    let mut ran = 0usize;
    let mut induced_bite = 0usize;
    let mut free_bite = 0usize;
    for case_idx in 0..8000 {
        let case = gen_case(&mut rng);
        let Verdict { matches: expected, induced_only_rejects, cross_induced_rejects } = oracle(&case);
        let Some((engine_set, seq_count, par_enum, par_count)) = build_and_run(&case) else { continue; };
        ran += 1;
        let used = |pred: fn(Morphism) -> bool| case.cluster_morphism.iter().enumerate()
            .any(|(c, &m)| pred(m) && case.cluster_of.contains(&c));
        if cross_induced_rejects > 0 {
            induced_bite += 1;
        }
        if induced_only_rejects > 0 && used(|m| m.is_induced()) && used(|m| !m.is_injective()) {
            free_bite += 1;
        }
        let ok = engine_set == expected
            && seq_count == expected.len()
            && par_enum == expected.len()
            && par_count == expected.len();
        if !ok {
            eprintln!("case {case_idx}: target_n={} target_edges={:?}", case.target_n, case.target_edges);
            eprintln!("  pattern_n={} pattern_edges={:?}", case.pattern_n, case.pattern_edges);
            eprintln!("  cluster_of={:?} morphisms={:?}", case.cluster_of, case.cluster_morphism);
            eprintln!("  oracle={} engine={} seq_count={seq_count} par_enum={par_enum} par_count={par_count}",
                expected.len(), engine_set.len());
            for m in expected.difference(&engine_set) { eprintln!("  missing: {m:?}"); }
            for m in engine_set.difference(&expected) { eprintln!("  extra:   {m:?}"); }
            panic!("mixed-morphism mismatch at case {case_idx}");
        }
    }
    println!("oracle fuzz: {ran} cases verified, {induced_bite} where induced-ness bit an injective non-induced binding, {free_bite} with a live induced-vs-free mixture");
    assert!(ran > 6000);
    assert!(induced_bite > 100, "induced rule was near-vacuous: only {induced_bite} biting mixtures");
    assert!(free_bite > 50, "free-invisibility was near-vacuous: only {free_bite} induced-vs-free mixtures");
}
