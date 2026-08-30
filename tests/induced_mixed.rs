//! Induced-ness in mixed-morphism queries.
//!
//! Ratified rule (see `doc/site/src/mixed-morphisms.md`): induced-ness holds
//! between a `SubIso`/`Iso` node and *every* injective binding, in any
//! cluster, and it is order-independent — declaring the clusters the other
//! way round must not change the match set. Bindings from free clusters
//! (`Homo`, `Epi`) are invisible: they neither trigger the check nor are
//! protected by it, and two non-induced injective nodes never constrain each
//! other.

use grw::search::dsl;
use grw::search::{self, Morphism, RevCsr, Search, Seq, Par};
use grw::Graph as _;
use rayon::iter::ParallelIterator;

type GER = grw::graph::edge::Undir<()>;

/// Two nodes joined by one undirected edge.
fn target_edge() -> grw::graph::MUndir0 {
    grw::mgraph![<(), GER>; N(0) ^ N(1)].unwrap()
}

/// Path `0 — 1 — 2`.
fn target_path3() -> grw::graph::MUndir0 {
    grw::mgraph![<(), GER>; N(0) ^ N(1), n(1) ^ N(2)].unwrap()
}

/// Triangle `0 — 1 — 2 — 0`.
fn target_triangle() -> grw::graph::MUndir0 {
    grw::mgraph![<(), GER>; N(0) ^ N(1), n(1) ^ N(2), n(2) ^ n(0)].unwrap()
}

/// Runs the query through every engine path and asserts all four agree, then
/// returns the sorted binding vectors.
fn run(
    g: &grw::graph::MUndir0,
    clusters: Vec<dsl::ClusterOps<(), GER>>,
    pattern_n: usize,
) -> Vec<Vec<u32>> {
    let t = g.index(RevCsr);
    let Search::Resolved(r) = search::compile::<(), GER>(clusters).unwrap() else {
        panic!("query did not resolve")
    };
    let query = r.query();

    let mut out = Vec::new();
    for m in Seq::search(&query, &t) {
        let mut v = vec![u32::MAX; pattern_n];
        for i in 0..pattern_n {
            v[i] = *m.get(grw::graph::dsl::LocalId(i as u32)).expect("bound") as u32;
        }
        out.push(v);
    }
    out.sort();

    let seq_count = Seq::search(&query, &t).count();
    let par_enum: usize = Par::search(&query, &t).map(|_| 1usize).sum();
    let par_count = Par::search(&query, &t).count();
    assert_eq!(seq_count, out.len(), "Seq count path disagrees with Seq enumeration");
    assert_eq!(par_enum, out.len(), "Par enumeration disagrees with Seq enumeration");
    assert_eq!(par_count, out.len(), "Par count path disagrees with Seq enumeration");

    out
}

fn n(i: u32) -> dsl::Op<(), GER> {
    dsl::N::<(), GER>(i).into()
}

fn e(a: u32, b: u32) -> dsl::Op<(), GER> {
    (dsl::n::<(), GER>(a) ^ dsl::n::<(), GER>(b)).into()
}

fn empty() -> Vec<Vec<u32>> {
    Vec::new()
}

// --- one target edge, no pattern edge: the induced node forbids the pair ----

#[test]
fn subiso_then_mono_rejects_unmirrored_edge() {
    let g = target_edge();
    let got = run(
        &g,
        vec![
            dsl::get(Morphism::SubIso, vec![n(0)]),
            dsl::get(Morphism::Mono, vec![n(1)]),
        ],
        2,
    );
    assert_eq!(got, empty());
}

#[test]
fn mono_then_subiso_rejects_unmirrored_edge() {
    // Same query as above with the clusters declared the other way round.
    let g = target_edge();
    let got = run(
        &g,
        vec![
            dsl::get(Morphism::Mono, vec![n(0)]),
            dsl::get(Morphism::SubIso, vec![n(1)]),
        ],
        2,
    );
    assert_eq!(got, empty());
}

#[test]
fn one_subiso_cluster_rejects_unmirrored_edge() {
    let g = target_edge();
    let got = run(&g, vec![dsl::get(Morphism::SubIso, vec![n(0), n(1)])], 2);
    assert_eq!(got, empty());
}

#[test]
fn two_subiso_clusters_reject_unmirrored_edge() {
    let g = target_edge();
    let got = run(
        &g,
        vec![
            dsl::get(Morphism::SubIso, vec![n(0)]),
            dsl::get(Morphism::SubIso, vec![n(1)]),
        ],
        2,
    );
    assert_eq!(got, empty());
}

// --- free bindings are invisible in both declaration orders ----------------

#[test]
fn subiso_then_homo_ignores_free_binding() {
    let g = target_edge();
    let got = run(
        &g,
        vec![
            dsl::get(Morphism::SubIso, vec![n(0)]),
            dsl::get(Morphism::Homo, vec![n(1)]),
        ],
        2,
    );
    assert_eq!(got, vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]]);
}

#[test]
fn homo_then_subiso_ignores_free_binding() {
    let g = target_edge();
    let got = run(
        &g,
        vec![
            dsl::get(Morphism::Homo, vec![n(0)]),
            dsl::get(Morphism::SubIso, vec![n(1)]),
        ],
        2,
    );
    assert_eq!(got, vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]]);
}

// --- a mirrored edge is legal in both declaration orders -------------------

#[test]
fn subiso_then_mono_accepts_mirrored_edge() {
    let g = target_edge();
    let got = run(
        &g,
        vec![
            dsl::get(Morphism::SubIso, vec![n(0), e(0, 1)]),
            dsl::get(Morphism::Mono, vec![n(1)]),
        ],
        2,
    );
    assert_eq!(got, vec![vec![0, 1], vec![1, 0]]);
}

#[test]
fn mono_then_subiso_accepts_mirrored_edge() {
    let g = target_edge();
    let got = run(
        &g,
        vec![
            dsl::get(Morphism::Mono, vec![n(0), e(0, 1)]),
            dsl::get(Morphism::SubIso, vec![n(1)]),
        ],
        2,
    );
    assert_eq!(got, vec![vec![0, 1], vec![1, 0]]);
}

// --- directed edges exercise the multi-slot `is_feasible` arm --------------

type DER = grw::graph::edge::Dir<()>;

fn run_dir(
    g: &grw::graph::MDir0,
    clusters: Vec<dsl::ClusterOps<(), DER>>,
    pattern_n: usize,
) -> Vec<Vec<u32>> {
    let t = g.index(RevCsr);
    let Search::Resolved(r) = search::compile::<(), DER>(clusters).unwrap() else {
        panic!("query did not resolve")
    };
    let query = r.query();

    let mut out = Vec::new();
    for m in Seq::search(&query, &t) {
        let mut v = vec![u32::MAX; pattern_n];
        for i in 0..pattern_n {
            v[i] = *m.get(grw::graph::dsl::LocalId(i as u32)).expect("bound") as u32;
        }
        out.push(v);
    }
    out.sort();

    assert_eq!(Seq::search(&query, &t).count(), out.len());
    assert_eq!(Par::search(&query, &t).map(|_| 1usize).sum::<usize>(), out.len());
    assert_eq!(Par::search(&query, &t).count(), out.len());

    out
}

fn dn(i: u32) -> dsl::Op<(), DER> {
    dsl::N::<(), DER>(i).into()
}

fn de(a: u32, b: u32) -> dsl::Op<(), DER> {
    (dsl::n::<(), DER>(a) >> dsl::n::<(), DER>(b)).into()
}

#[test]
fn directed_unmirrored_edge_rejected_in_both_orders() {
    let g: grw::graph::MDir0 = grw::mgraph![N(0) >> N(1)].unwrap();
    let subiso_first = run_dir(
        &g,
        vec![
            dsl::get(Morphism::SubIso, vec![dn(0)]),
            dsl::get(Morphism::Mono, vec![dn(1)]),
        ],
        2,
    );
    let mono_first = run_dir(
        &g,
        vec![
            dsl::get(Morphism::Mono, vec![dn(0)]),
            dsl::get(Morphism::SubIso, vec![dn(1)]),
        ],
        2,
    );
    assert_eq!(subiso_first, empty());
    assert_eq!(mono_first, empty());
}

#[test]
fn directed_mirrored_edge_accepted_in_both_orders() {
    let g: grw::graph::MDir0 = grw::mgraph![N(0) >> N(1)].unwrap();
    let subiso_first = run_dir(
        &g,
        vec![
            dsl::get(Morphism::SubIso, vec![dn(0), de(0, 1)]),
            dsl::get(Morphism::Mono, vec![dn(1)]),
        ],
        2,
    );
    let mono_first = run_dir(
        &g,
        vec![
            dsl::get(Morphism::Mono, vec![dn(0), de(0, 1)]),
            dsl::get(Morphism::SubIso, vec![dn(1)]),
        ],
        2,
    );
    assert_eq!(subiso_first, vec![vec![0, 1]]);
    assert_eq!(mono_first, vec![vec![0, 1]]);
}

#[test]
fn directed_antiparallel_pair_needs_both_slots_mirrored() {
    // Target carries `0 >> 1` and `1 >> 0`; the pattern mirrors only one of
    // them, so the induced node still sees an unmirrored target edge and both
    // injective placements die — at 2 target slots against 1 pattern edge.
    let g: grw::graph::MDir0 = grw::mgraph![N(0) >> N(1), n(1) >> n(0)].unwrap();
    let mono_first = run_dir(
        &g,
        vec![
            dsl::get(Morphism::Mono, vec![dn(0), de(0, 1)]),
            dsl::get(Morphism::SubIso, vec![dn(1)]),
        ],
        2,
    );
    let subiso_first = run_dir(
        &g,
        vec![
            dsl::get(Morphism::SubIso, vec![dn(1)]),
            dsl::get(Morphism::Mono, vec![dn(0), de(0, 1)]),
        ],
        2,
    );
    assert_eq!(mono_first, empty());
    assert_eq!(subiso_first, empty());
}

// --- three nodes: the induced node is checked against both Mono bindings ---

#[test]
fn subiso_rejects_every_unmirrored_neighbour_on_path3() {
    // No pattern edges at all: every injective placement puts some Mono
    // binding next to the SubIso binding, so nothing survives.
    let g = target_path3();
    let got = run(
        &g,
        vec![
            dsl::get(Morphism::SubIso, vec![n(0)]),
            dsl::get(Morphism::Mono, vec![n(1), n(2)]),
        ],
        3,
    );
    assert_eq!(got, empty());
}

#[test]
fn mono_pair_may_stay_adjacent_next_to_a_subiso_node() {
    // Negative control for over-rejection: `1—2` is a Mono/Mono pair, so
    // their target adjacency in the triangle is none of the induced node's
    // business. All six permutations survive.
    let g = target_triangle();
    let got = run(
        &g,
        vec![
            dsl::get(Morphism::SubIso, vec![n(0), e(0, 1), e(0, 2)]),
            dsl::get(Morphism::Mono, vec![n(1), n(2)]),
        ],
        3,
    );
    assert_eq!(
        got,
        vec![
            vec![0, 1, 2],
            vec![0, 2, 1],
            vec![1, 0, 2],
            vec![1, 2, 0],
            vec![2, 0, 1],
            vec![2, 1, 0],
        ]
    );
}
