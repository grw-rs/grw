//! Inference-preservation guard for the path API.
//!
//! Every expression here is written exactly as path users write it today —
//! with no more type annotations than current tests carry. If a change to the
//! path type-states (marker traits, method re-homing) makes any of these
//! require a new annotation, this file stops compiling and the change must be
//! retracted. Compile-only: runtime behavior is covered by the oracle suites.

use grw::edge::anydir;
use grw::graph::edge::{AnyVal, End};
use grw::search::path::{AStar, Config, Dijkstra, PathConstraint};

type ER = grw::edge::Anydir<u8>;

fn is_dir(slot: anydir::Slot, _: &AnyVal<u8>) -> bool {
    matches!(slot, anydir::Slot::Dir(End::Src))
}

fn diamond() -> grw::MGraph<(), ER> {
    // 0 → 1 → 3, 0 → 2 → 3
    grw::mgraph![<(), ER>;
        N(0) >> N(1), n(1) >> N(3),
        n(0) >> N(2), n(2) >> n(3)
    ]
    .unwrap()
}

// ── standalone Config forms ─────────────────────────────────────────

#[test]
fn traversal_configs_annotation_free() {
    let g = diamond();
    let pc = PathConstraint::default();

    let dfs = Config::new(()).dfs();
    assert!(g.path_search(grw::id::N(0), grw::id::N(3), is_dir, dfs, &pc).next().is_some());

    let bfs_len = Config::new(()).bfs().len(2..5);
    assert!(g.path_search(grw::id::N(0), grw::id::N(3), is_dir, bfs_len, &pc).next().is_some());

    let dfs_open = Config::new(()).dfs().len(0..);
    assert!(g.path_search(grw::id::N(0), grw::id::N(0), is_dir, dfs_open, &pc).next().is_some());
}

#[test]
fn navigated_configs_annotation_free() {
    let g = diamond();
    let pc = PathConstraint::default();

    // EV pinned by the navigator, as today: turbofish on counted…
    let counted = Config::new(()).navigate(Dijkstra::counted::<AnyVal<u8>>());
    assert!(g.path_navigate(grw::id::N(0), grw::id::N(3), is_dir, counted, &pc).next().is_some());

    // …or a closure-annotated weight fn.
    let weighted = Config::new(()).navigate(Dijkstra::weighted(|_: &AnyVal<u8>| 1.0));
    assert!(g.path_navigate(grw::id::N(0), grw::id::N(3), is_dir, weighted, &pc).next().is_some());

    // .one()/.all() chain directly off navigate, no annotations.
    let all = Config::new(()).navigate(Dijkstra::counted::<AnyVal<u8>>()).all();
    assert!(g.path_navigate(grw::id::N(0), grw::id::N(3), is_dir, all, &pc).next().is_some());

    let one = Config::new(())
        .navigate(AStar::new(|_: &AnyVal<u8>| 1.0, |_n: grw::id::N| 0.0))
        .one();
    assert!(g.path_navigate(grw::id::N(0), grw::id::N(3), is_dir, one, &pc).next().is_some());
}

// ── in-search! forms ────────────────────────────────────────────────

#[test]
fn search_macro_paths_annotation_free() {
    let _g = grw::mgraph![<(), ER>; N(0) ^ N(1), n(1) ^ N(2), n(2) ^ n(0)].unwrap();

    // free-node refs on a path, defined in a sibling cluster (as in path_coverage)
    let search = grw::search![<(), ER>;
        get(grw::Homo) { X(0), X(1) },
        get(grw::Mono) { n(0) ^ ..n(1).dfs().len(2..) }
    ];
    assert!(search.is_ok());

    // context-node path, bare dfs
    let search = grw::search![<(), ER>;
        get(grw::SubIso) { X(0) ^ ..X(1).dfs() }
    ];
    assert!(search.is_ok());
}
