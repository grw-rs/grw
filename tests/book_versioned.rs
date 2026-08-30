//! Every snippet in `doc/site/src/versioned-graph.md` is mirrored here, so the
//! chapter cannot drift from the API. Same discipline as `site_story.rs`.

use grw::graph::{self, Graph as _, MGraph, VGraph, VUndir, VUndir0};
use grw::modify::error;
use grw::search::{self, Morphism, RevCsr, Search, Seq};
use grw::{id, mgraph, modify, vgraph};

type ER = graph::edge::Undir<()>;

/// § Quick start
#[test]
fn quick_start() {
    let g: VUndir0 = vgraph![N(0) ^ (N(1) ^ (N(2) ^ n(0)))].unwrap();
    assert_eq!(g.node_count(), 3);
    assert_eq!(g.edge_count(), 3);
    assert_eq!(g.version(), 0);
}

/// § Modifying: `&self` in, a new graph out
#[test]
fn modify_returns_next_and_leaves_original() {
    let g1: VUndir<u32, ()> = vgraph![<u32, graph::edge::Undir<()>>;
        N(0).val(1u32) ^ N(1).val(2u32),
        n(1) ^ N(2).val(3u32)
    ]
    .unwrap();

    let (g2, m) = g1.modify(modify![X(0).val(99u32)]).unwrap();

    assert_eq!(g2.node_val(id::N(0)), Some(&99));
    assert_eq!(g1.node_val(id::N(0)), Some(&1)); // g1 never moved
    assert_eq!(m.swapped_node_vals.len(), 1);
    assert_eq!(g1.version(), 0);
    assert_eq!(g2.version(), 1);
}

/// § Modifying — additions report their ids through `Modification`.
#[test]
fn modification_reports_new_ids() {
    let g0: VUndir0 = VGraph::new();
    let (g1, m) = g0.modify(modify![N(7)]).unwrap();
    assert_eq!(*m.new_node_ids[&modify::LocalId(7)], 0);
    assert_eq!(g1.node_count(), 1);
}

/// § Versions are yours to choose
#[test]
fn version_jump_stamps_generations() {
    let g0: VUndir0 = VGraph::new();
    let (g1, _) = g0.modify(modify![N(0) ^ N(1)]).unwrap();
    assert_eq!(g1.version(), 1);

    let (g7, _) = g1.modify_versioned(modify![N(2) ^ x(0)], 7).unwrap();
    assert_eq!(g7.version(), 7);
    assert_eq!(g7.node_gen(id::N(2)), Some(7)); // born at 7
    assert_eq!(g7.node_gen(id::N(0)), Some(1)); // untouched since 1
}

/// § Versions are yours to choose — the refusal.
#[test]
fn non_monotonic_version_is_refused() {
    let g0: VUndir0 = VGraph::new();
    let (g1, _) = g0.modify(modify![N(0)]).unwrap();
    let (g5, _) = g1.modify_versioned(modify![N(1)], 5).unwrap();

    let Err(err) = g5.modify_versioned(modify![N(2)], 4) else {
        panic!("a version that does not advance must be refused")
    };
    assert!(matches!(
        err,
        error::Modify::Version(error::Version::NotMonotonic { current: 5, requested: 4 })
    ));
    assert_eq!(g5.version(), 5);
    assert_eq!(g5.node_count(), 2);
}

/// § Stable identity: slot + generation
#[test]
fn slot_reuse_bumps_the_generation() {
    let g0: VUndir0 = VGraph::new();
    let (g1, _) = g0.modify(modify![N(0), N(1)]).unwrap();
    let before = g1.stable_id(id::N(1)).unwrap();

    let (g2, _) = g1.modify(modify![!X(1)]).unwrap();
    let (g3, _) = g2.modify(modify![N(9)]).unwrap();
    let after = g3.stable_id(id::N(1)).unwrap();

    assert_eq!(before.0, after.0); // same slot, recycled
    assert_ne!(before.1, after.1); // different birth version
    assert_eq!(after.1, 3);
}

/// § Atomicity: an `Err` produces no version at all
#[test]
fn err_produces_no_version_and_leaks_no_slot() {
    let v: VUndir0 = vgraph![N(0) ^ N(1)].unwrap();
    assert!(v.modify(modify![N(9) ^ x(0), x(0) ^ x(1)]).is_err());

    let (next, m) = v.modify(modify![N(7)]).unwrap();
    assert_eq!(*m.new_node_ids[&modify::LocalId(7)], 2); // slot 2, not 3
    assert_eq!(next.version(), 1);
    assert_eq!(v.version(), 0);
}

/// § The two-way door
#[test]
fn two_way_door() {
    let v: VUndir0 = vgraph![N(0) ^ N(1), n(1) ^ N(2)].unwrap();

    let m = v.to_mgraph();
    assert_eq!(m.node_count(), v.node_count());
    assert_eq!(m.edge_count(), v.edge_count());

    let back = VGraph::from_mgraph(&m);
    assert_eq!(back.node_count(), 3);
    assert_eq!(back.version(), 0); // conversion is not an edit
}

/// § The two-way door — one `.grw` file, either representation.
#[test]
fn save_load_across_representations() {
    let v: VUndir0 = vgraph![N(0) ^ N(1), n(1) ^ N(2)].unwrap();
    let path = std::env::temp_dir().join(format!("grw_book_versioned_{}.grw", std::process::id()));
    v.save(&path).unwrap();

    let m: graph::MUndir0 = MGraph::load(&path).unwrap();
    assert_eq!(m.node_count(), 3);

    let v2: VUndir0 = VUndir0::load(&path).unwrap();
    assert_eq!(v2.node_count(), 3);
    assert_eq!(v2.version(), 0);

    std::fs::remove_file(&path).unwrap();
}

/// § Search is unchanged
#[test]
fn search_reads_a_vgraph_through_the_same_trait() {
    let clusters = vec![search::dsl::get(
        Morphism::Mono,
        vec![
            (search::dsl::N::<(), ER>(0) ^ search::dsl::N::<(), ER>(1)).into(),
            (search::dsl::n::<(), ER>(1) ^ search::dsl::N::<(), ER>(2)).into(),
            (search::dsl::n::<(), ER>(0) ^ search::dsl::n::<(), ER>(2)).into(),
        ],
    )];
    let Search::Resolved(r) = search::compile::<(), ER>(clusters).unwrap() else {
        panic!("triangle is a resolved query")
    };
    let query = r.query();

    let m: graph::MUndir0 = mgraph![N(0) ^ N(1), n(1) ^ N(2), n(2) ^ n(0)].unwrap();
    let v: VUndir0 = vgraph![N(0) ^ N(1), n(1) ^ N(2), n(2) ^ n(0)].unwrap();

    let m_hits = Seq::search(query, &m.index(RevCsr)).count();
    let v_hits = Seq::search(query, &v.index(RevCsr)).count();
    assert_eq!(m_hits, 6); // the triangle, once per Mono labelling
    assert_eq!(v_hits, m_hits);
}

/// § Why a versioned graph — history is just the versions you kept.
#[test]
fn retained_versions_stay_readable() {
    let g0: VUndir0 = VGraph::new();
    let (g1, _) = g0.modify(modify![N(0) ^ N(1)]).unwrap();

    let mut history = vec![g1.clone()];
    let mut g = g1;
    for i in 2..6 as grw::Id {
        let (next, _) = g.modify(modify![N(i) ^ x(0)]).unwrap();
        history.push(next.clone());
        g = next;
    }

    assert_eq!(g.node_count(), 6);
    let counts: Vec<usize> = history.iter().map(|h| h.node_count()).collect();
    assert_eq!(counts, vec![2, 3, 4, 5, 6]);
    let versions: Vec<u64> = history.iter().map(|h| h.version()).collect();
    assert_eq!(versions, vec![1, 2, 3, 4, 5]);
}
