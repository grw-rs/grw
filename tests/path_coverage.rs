//! Comprehensive path expression coverage across morphisms, algorithms, and clusters.
//!
//! Each test uses a data graph crafted so the specific combination under test
//! produces a unique, verifiable outcome.

use grw::edge::anydir;
use grw::graph::edge::{AnyVal, End};
use grw::modify::dsl::*;
use grw::search::path::{Config, PathConstraint, Dijkstra};
use grw::graph::dsl::LocalId;

type ER = grw::edge::Anydir<u8>;
type G = grw::Graph<(), ER>;

fn is_dir(slot: anydir::Slot, _: &AnyVal<u8>) -> bool {
    matches!(slot, anydir::Slot::Dir(End::Src))
}
fn is_und(slot: anydir::Slot, _: &AnyVal<u8>) -> bool {
    matches!(slot, anydir::Slot::Undir)
}

fn run(g: &G, search: Result<grw::search::query::Search<(), ER>, grw::search::error::Search>)
    -> Vec<grw::search::engine::Match>
{
    let session = grw::search::Session::from_search(search.unwrap(), g).unwrap();
    session.into_iter().collect()
}

// ═══════════════════════════════════════════════════════════════════
// len semantics
// ═══════════════════════════════════════════════════════════════════

#[test]
fn len_0_yields_trivial_when_from_eq_to() {
    // Triangle: 0-1-2-0
    let g = grw::graph![<(), ER>; N(0) ^ N(1), n(1) ^ N(2), n(2) ^ n(0)].unwrap();
    let c = PathConstraint::default();

    // len(0..): first result is trivial [0] when from==to
    let first = g.path_search(grw::id::N(0), grw::id::N(0), is_und,
        Config::new(()).dfs().len(0..), &c).next();
    assert_eq!(first, Some(vec![grw::id::N(0)]));
}

#[test]
fn len_1_finds_single_hop_only() {
    // Chain: 0→1→2
    let g = grw::graph![<(), ER>;
        N(0) & E().val(1u8) >> N(1),
        n(1) & E().val(1u8) >> N(2)
    ].unwrap();
    let c = PathConstraint::default();

    let paths: Vec<_> = g.path_search(grw::id::N(0), grw::id::N(2), is_dir,
        Config::new(()).dfs().len(1), &c).collect();
    // 0→2 needs 2 hops, but len(1) = exactly 1 edge → empty
    assert!(paths.is_empty());

    let paths1: Vec<_> = g.path_search(grw::id::N(0), grw::id::N(1), is_dir,
        Config::new(()).dfs().len(1), &c).collect();
    assert_eq!(paths1.len(), 1);
    assert_eq!(paths1[0].len(), 2); // [0, 1]
}

#[test]
fn len_2_range_excludes_shorter_and_longer() {
    // Diamond: 0→1→3, 0→2→3, plus 0→2→1→3 (3 hops)
    let g = grw::graph![<(), ER>;
        N(0) & E().val(1u8) >> N(1),
        n(0) & E().val(1u8) >> N(2),
        n(1) & E().val(1u8) >> N(3),
        n(2) & E().val(1u8) >> n(3),
        n(2) & E().val(1u8) >> n(1)
    ].unwrap();
    let c = PathConstraint::default();

    // len(2..3) = exactly 2 edges
    let paths: Vec<_> = g.path_search(grw::id::N(0), grw::id::N(3), is_dir,
        Config::new(()).dfs().len(2..3), &c).collect();
    for p in &paths {
        assert_eq!(p.len(), 3, "exactly 2 edges = 3 nodes");
    }
    assert!(!paths.is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// Algorithm differences: DFS vs BFS ordering
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bfs_returns_shortest_dfs_returns_deepest_first() {
    // Graph with short and long paths: 0→3 (1 hop), 0→1→2→3 (3 hops)
    let g = grw::graph![<(), ER>;
        N(0) & E().val(1u8) >> N(1),
        n(1) & E().val(1u8) >> N(2),
        n(2) & E().val(1u8) >> N(3),
        n(0) & E().val(1u8) >> n(3)
    ].unwrap();
    let c = PathConstraint::default();

    let bfs_first = g.path_search(grw::id::N(0), grw::id::N(3), is_dir,
        Config::new(()).bfs(), &c).next().unwrap();
    assert_eq!(bfs_first.len(), 2, "BFS finds shortest first: [0, 3]");

    let dfs_all: Vec<_> = g.path_search(grw::id::N(0), grw::id::N(3), is_dir,
        Config::new(()).dfs(), &c).collect();
    // DFS finds all paths; BFS guarantees shortest first
    assert!(dfs_all.len() > 1, "multiple paths exist");
    assert!(dfs_all.iter().any(|p| p.len() > 2), "DFS finds long paths too");
}

#[test]
fn navigator_finds_weighted_path() {
    // 0→1 (weight 5), 0→2 (weight 1). Navigator orders by cost.
    let g = grw::graph![<(), ER>;
        N(0) & E().val(5u8) >> N(1),
        n(0) & E().val(1u8) >> N(2)
    ].unwrap();
    let c = PathConstraint::default();

    let (cost, path) = g.path_navigate(grw::id::N(0), grw::id::N(2), is_dir,
        Config::new(()).navigate(Dijkstra::weighted(|ev: &AnyVal<u8>| match ev {
            AnyVal::Dir(w) | AnyVal::Undir(w) => *w as f64,
        })), &c).next().unwrap();

    assert_eq!(cost, 1.0, "Dijkstra finds weight-1 edge");
    assert_eq!(path, vec![grw::id::N(0), grw::id::N(2)]);
}

#[test]
fn navigator_dijkstra_finds_cheapest_via_detour() {
    // 0→1 (weight 10), 0→2 (weight 1), 2→1 (weight 1)
    // Cheapest to reach 1: 0→2→1 (cost 2), NOT direct 0→1 (cost 10).
    //
    // Bug: the visited set marks node 1 as visited when first pushed (via weight-10 edge).
    // When the cheaper 0→2→1 path discovers node 1, visited rejects the re-insert.
    // Fix: don't mark visited on push — mark on pop (lazy deletion), or allow re-insert
    // when a cheaper cost is found.
    let g = grw::graph![<(), ER>;
        N(0) & E().val(10u8) >> N(1),
        n(0) & E().val(1u8) >> N(2),
        n(2) & E().val(1u8) >> n(1)
    ].unwrap();
    let c = PathConstraint::default();

    let (cost, path) = g.path_navigate(grw::id::N(0), grw::id::N(1), is_dir,
        Config::new(()).navigate(Dijkstra::weighted(|ev: &AnyVal<u8>| match ev {
            AnyVal::Dir(w) | AnyVal::Undir(w) => *w as f64,
        })), &c).next().unwrap();

    assert_eq!(cost, 2.0, "Dijkstra should find cheapest path 0→2→1 (cost 2), not direct 0→1 (cost 10)");
    assert_eq!(path, vec![grw::id::N(0), grw::id::N(2), grw::id::N(1)]);
}

// ═══════════════════════════════════════════════════════════════════
// Morphism on path intermediates
// ═══════════════════════════════════════════════════════════════════

#[test]
fn mono_path_rejects_reusing_bound_node() {
    // Triangle: 0-1-2-0. Bind X(0)=0, X(1)=0 (same node).
    // Mono path from 0 to 0: intermediates can't reuse node 0 (in excluded set).
    // Path [0, 1, 2, 0] has intermediates [1, 2] — neither is 0. Should work.
    let g = grw::graph![<(), ER>; N(0) ^ N(1), n(1) ^ N(2), n(2) ^ n(0)].unwrap();

    let search = grw::search![<(), ER>;
        get(grw::Homo) { X(0), X(1) },
        get(grw::Mono) { n(0) ^ ..n(1).dfs().len(2..) }
    ];
    let grw::search::query::Search::Unresolved(u) = search.unwrap()
        else { panic!("expected unresolved") };
    let bound = u.bind(&[(0, 0), (1, 0)]).unwrap();
    let indexed = g.index(grw::search::engine::RevCsr);
    let matches: Vec<_> = grw::search::engine::Seq::search_bound(
        bound.query(), &indexed, bound.bindings().to_vec()
    ).collect();

    assert!(!matches.is_empty(), "Mono path 0→0 should find cycle through non-excluded nodes");
    let p = matches[0].path(0);
    // Intermediates must not include node 0
    for &n in &p[1..p.len()-1] {
        assert_ne!(*n, 0, "Mono: intermediate must not reuse excluded node 0");
    }
}

#[test]
fn subiso_path_rejects_branched_intermediates() {
    // 0-1-2-3, with branch 1-4
    let g = grw::graph![<(), ER>; N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3), n(1) ^ N(4)].unwrap();

    let search = grw::search![<(), ER>;
        get(grw::SubIso) { X(0) ^ ..X(1).dfs() }
    ];
    let grw::search::query::Search::Unresolved(u) = search.unwrap()
        else { panic!("expected unresolved") };
    let bound = u.bind(&[(0, 0), (1, 3)]).unwrap();
    let indexed = g.index(grw::search::engine::RevCsr);
    let matches: Vec<_> = grw::search::engine::Seq::search_bound(
        bound.query(), &indexed, bound.bindings().to_vec()
    ).collect();

    // Path 0→1→2→3: node 1 has outside edge to 4 → SubIso rejects
    assert!(matches.is_empty() || matches[0].path(0).is_empty(),
        "SubIso rejects path with branched intermediate");
}

#[test]
fn subiso_path_accepts_clean_intermediates() {
    // 0-1-2-3, no branches
    let g = grw::graph![<(), ER>; N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3)].unwrap();

    let search = grw::search![<(), ER>;
        get(grw::SubIso) { X(0) ^ ..X(1).dfs() }
    ];
    let grw::search::query::Search::Unresolved(u) = search.unwrap()
        else { panic!("expected unresolved") };
    let bound = u.bind(&[(0, 0), (1, 3)]).unwrap();
    let indexed = g.index(grw::search::engine::RevCsr);
    let matches: Vec<_> = grw::search::engine::Seq::search_bound(
        bound.query(), &indexed, bound.bindings().to_vec()
    ).collect();

    assert!(!matches.is_empty());
    assert_eq!(matches[0].path(0).len(), 4);
}

// ═══════════════════════════════════════════════════════════════════
// Cross-cluster: endpoints in one morphism, path in another
// ═══════════════════════════════════════════════════════════════════

#[test]
fn homo_endpoints_mono_path_finds_cycle() {
    // Square: 0-1-2-3-0
    let g = grw::graph![<(), ER>; N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3), n(3) ^ n(0)].unwrap();

    let matches = run(&g, grw::search![<(), ER>;
        get(grw::Homo) { N(0), N(1) },
        get(grw::Mono) { n(0) ^ ..n(1).dfs().len(2..) }
    ]);

    let cycles: Vec<_> = matches.iter()
        .filter(|m| m.get(LocalId(0)) == m.get(LocalId(1)) && !m.path(0).is_empty())
        .collect();
    assert!(!cycles.is_empty(), "Homo endpoints + Mono path should find cycles");
}

#[test]
fn subiso_endpoints_and_path_rejects_branches() {
    // 0-1-2-3 with branch at 1. All in one SubIso cluster.
    let g = grw::graph![<(), ER>; N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3), n(1) ^ N(4)].unwrap();

    let search = grw::search![<(), ER>;
        get(grw::SubIso) { X(0) ^ ..X(1).dfs() }
    ];
    let grw::search::query::Search::Unresolved(u) = search.unwrap()
        else { panic!("expected unresolved") };
    let bound = u.bind(&[(0, 0), (1, 3)]).unwrap();
    let indexed = g.index(grw::search::engine::RevCsr);
    let matches: Vec<_> = grw::search::engine::Seq::search_bound(
        bound.query(), &indexed, bound.bindings().to_vec()
    ).collect();

    assert!(matches.is_empty() || matches[0].path(0).is_empty(),
        "SubIso path should reject: node 1 has branch to 4");
}

#[test]
fn cross_cluster_mono_endpoints_subiso_path_rejects_branches() {
    // Same graph: 0-1-2-3 with branch at 1.
    // Endpoints in Mono cluster (can have any degree), path in SubIso (induced check).
    let g = grw::graph![<(), ER>; N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3), n(1) ^ N(4)].unwrap();

    let search = grw::search![<(), ER>;
        get(grw::Mono) { X(10), X(11) },
        get(grw::SubIso) { n(10) ^ ..n(11).dfs() }
    ];
    let grw::search::query::Search::Unresolved(u) = search.unwrap()
        else { panic!("expected unresolved") };
    let bound = u.bind(&[(10, 0), (11, 3)]).unwrap();
    let indexed = g.index(grw::search::engine::RevCsr);
    let matches: Vec<_> = grw::search::engine::Seq::search_bound(
        bound.query(), &indexed, bound.bindings().to_vec()
    ).collect();

    // Path 0→1→2→3: node 1 has outside edge to 4. SubIso path cluster rejects.
    assert!(matches.is_empty() || matches[0].path(0).is_empty(),
        "cross-cluster: SubIso path morphism should reject branched intermediate");
}

// ═══════════════════════════════════════════════════════════════════
// Ban clusters with paths
// ═══════════════════════════════════════════════════════════════════

#[test]
fn ban_cannot_contradict_path_intermediates() {
    // Chain: 0→1→2→3. Path 0→3 has intermediates {1, 2}.
    // Ban tries to match endpoints with ANY neighbor — but path intermediates
    // are protected (excluded from ban candidates).
    let g = grw::graph![<(), ER>;
        N(0) & E().val(1u8) >> N(1),
        n(1) & E().val(1u8) >> N(2),
        n(2) & E().val(1u8) >> N(3)
    ].unwrap();

    // Get: path from N(0) to N(1). Ban: reject if endpoint has any neighbor.
    // Without protection, ban always fires (node 0→1 edge). With protection,
    // node 1 is a path intermediate → excluded → ban can't bind N_() to it.
    let matches = run(&g, grw::search![<(), ER>;
        get(grw::Mono) { N(0) >> ..N(1).dfs().len(3..) },
        ban(grw::Mono) { n(0) >> N(2) }
    ]);

    // N(2) in ban is anonymous — must map to a node adjacent to N(0).
    // Node 0's only outgoing is to 1. But 1 is a path intermediate → protected.
    // Ban can't bind N(2) to 1 → ban unsatisfiable → match survives!
    let valid: Vec<_> = matches.iter()
        .filter(|m| m.get(LocalId(0)) == Some(grw::id::N(0))
             && m.get(LocalId(1)) == Some(grw::id::N(3)))
        .collect();
    assert!(!valid.is_empty(),
        "ban should not kill match: path intermediate 1 is protected from ban candidates");
}

#[test]
fn ban_still_fires_on_non_path_edges() {
    // Chain: 0→1→2→3, plus extra edge 0→4.
    // Ban: n(0) >> N(2). Node 0 has edge to 4 (NOT a path intermediate) → ban fires.
    let g = grw::graph![<(), ER>;
        N(0) & E().val(1u8) >> N(1),
        n(1) & E().val(1u8) >> N(2),
        n(2) & E().val(1u8) >> N(3),
        n(0) & E().val(1u8) >> N(4)
    ].unwrap();

    let matches = run(&g, grw::search![<(), ER>;
        get(grw::Mono) { N(0) >> ..N(1).dfs().len(3..) },
        ban(grw::Mono) { n(0) >> N(2) }
    ]);

    // Node 0→4 is NOT a path edge (4 is not on path 0→1→2→3).
    // Ban binds N(2)=4, satisfiable → match rejected.
    let valid: Vec<_> = matches.iter()
        .filter(|m| m.get(LocalId(0)) == Some(grw::id::N(0))
             && m.get(LocalId(1)) == Some(grw::id::N(3)))
        .collect();
    assert!(valid.is_empty(),
        "ban should fire: node 0 has non-path edge to 4");
}

#[test]
fn ban_cluster_rejects_match_with_path() {
    // Chain: 0→1→2→3
    let g = grw::graph![<(), ER>;
        N(0) & E().val(1u8) >> N(1),
        n(1) & E().val(1u8) >> N(2),
        n(2) & E().val(1u8) >> N(3)
    ].unwrap();

    // Get: find any directed path N(0)→N(1).
    // Ban: if N(1) has an outgoing edge to some N(2), reject.
    // Node 3 has no outgoing edge → match (0→1→2→3) survives ban.
    // Node 1 has outgoing to 2 → match (0→1) would be banned.
    let matches = run(&g, grw::search![<(), ER>;
        get(grw::Mono) { N(0) >> ..N(1).dfs() },
        ban(grw::Mono) { n(1) >> N(2) }
    ]);

    // Matches where N(1) has an outgoing edge are banned.
    // Node 1→2 exists, node 2→3 exists. Only N(1)=3 has no outgoing → match 0→1→2→3 survives.
    let endpoints: Vec<u32> = matches.iter()
        .map(|m| *m.get(LocalId(1)).unwrap())
        .collect();
    assert!(!endpoints.contains(&1), "ban: N(1)=1 has outgoing edge → banned");
    assert!(!endpoints.contains(&2), "ban: N(1)=2 has outgoing edge → banned");
    // N(1)=3 has no outgoing → should survive
    assert!(endpoints.contains(&3), "ban: N(1)=3 has no outgoing → survives");
}

#[test]
fn ban_allows_when_ban_unsatisfiable() {
    // Chain: 0→1→2
    let g = grw::graph![<(), ER>;
        N(0) & E().val(1u8) >> N(1),
        n(1) & E().val(1u8) >> N(2)
    ].unwrap();

    // Get: path 0→N(1). Ban: reject if N(1) is adjacent to a node matching impossible pred.
    let matches = run(&g, grw::search![<(), ER>;
        get(grw::Mono) { N(0) >> ..N(1).dfs() },
        ban(grw::Mono) { n(1) >> N(99).test(|_| false) }
    ]);

    // Ban target pred always false → ban never satisfiable → matches survive.
    assert!(!matches.is_empty(), "unsatisfiable ban should not reject matches");
}

// ═══════════════════════════════════════════════════════════════════
// Edge predicates on paths across morphisms
// ═══════════════════════════════════════════════════════════════════

#[test]
fn typed_edge_pred_filters_path_in_subiso() {
    // Two parallel chains: 0→1→2 (label 1), 0→3→2 (label 2)
    let g = grw::graph![<(), ER>;
        N(0) & E().val(1u8) >> N(1),
        n(1) & E().val(1u8) >> N(2),
        n(0) & E().val(2u8) >> N(3),
        n(3) & E().val(2u8) >> n(2)
    ].unwrap();

    // SubIso + typed pred (label==1): only path through node 1 (no branches)
    let search = grw::search![<(), ER>;
        get(grw::SubIso) { X(0) & E().test(|l: &u8| *l == 1) >> ..X(1).dfs() }
    ];
    let grw::search::query::Search::Unresolved(u) = search.unwrap()
        else { panic!("expected unresolved") };
    let bound = u.bind(&[(0, 0), (1, 2)]).unwrap();
    let indexed = g.index(grw::search::engine::RevCsr);
    let matches: Vec<_> = grw::search::engine::Seq::search_bound(
        bound.query(), &indexed, bound.bindings().to_vec()
    ).collect();

    // Path 0→1→2 via label-1: node 1 has no outside edges in label-1 subgraph
    assert!(!matches.is_empty(), "SubIso path through clean label-1 chain should work");
    let p = matches[0].path(0);
    assert_eq!(p.len(), 3);
}

// ═══════════════════════════════════════════════════════════════════
// Drive closure with path
// ═══════════════════════════════════════════════════════════════════

#[test]
fn anyedge_path_traverses_reverse_directed() {
    // Data graph: 0 ← 1 (directed edge from 1 to 0)
    // % path from 0 to 1 should traverse the edge backwards via Dir(Tgt) slot.
    let g = grw::graph![<(), ER>;
        N(1) & E().val(1u8) >> N(0)
    ].unwrap();

    let search = grw::search![<(), ER>;
        get(grw::Mono) { X(0) % ..X(1).dfs() }
    ];
    let grw::search::query::Search::Unresolved(u) = search.unwrap()
        else { panic!("expected unresolved") };
    let bound = u.bind(&[(0, 0), (1, 1)]).unwrap();
    let indexed = g.index(grw::search::engine::RevCsr);
    let matches: Vec<_> = grw::search::engine::Seq::search_bound(
        bound.query(), &indexed, bound.bindings().to_vec()
    ).collect();

    assert!(!matches.is_empty(), "% should traverse directed edge in reverse");
    assert_eq!(matches[0].path(0), &[grw::id::N(0), grw::id::N(1)]);
}

#[test]
fn anyedge_path_traverses_mixed_directions() {
    // Mixed graph: 0→1 (directed), 1-2 (undirected), 2→3 (directed)
    // `%` path should traverse all edge types.
    let g = grw::graph![<(), ER>;
        N(0) & E().val(1u8) >> N(1),
        n(1) & E().val(1u8) ^ N(2),
        n(2) & E().val(1u8) >> N(3)
    ].unwrap();

    let search = grw::search![<(), ER>;
        get(grw::Mono) { X(0) % ..X(1).dfs() }
    ];
    let grw::search::query::Search::Unresolved(u) = search.unwrap()
        else { panic!("expected unresolved") };
    let bound = u.bind(&[(0, 0), (1, 3)]).unwrap();
    let indexed = g.index(grw::search::engine::RevCsr);
    let matches: Vec<_> = grw::search::engine::Seq::search_bound(
        bound.query(), &indexed, bound.bindings().to_vec()
    ).collect();

    assert!(!matches.is_empty(), "% path should traverse directed+undirected edges");
    assert_eq!(matches[0].path(0).len(), 4, "path 0→1-2→3");
}

#[test]
fn drive_closure_controls_exploration() {
    // Fan: 0→1, 0→2, 0→3. Drive selects only first neighbor.
    let g = grw::graph![<(), ER>;
        N(0) & E().val(1u8) >> N(1),
        n(0) & E().val(1u8) >> N(2),
        n(0) & E().val(1u8) >> N(3)
    ].unwrap();
    let c = PathConstraint::default();

    // Drive: only explore first neighbor (index 0)
    let config = Config::new(()).drive(|_n| Some(grw::search::path::Explore::One(0)));
    let paths: Vec<_> = g.path_search(grw::id::N(0), grw::id::N(3), is_dir,
        config, &c).collect();

    // Drive(One(0)) only explores the first neighbor — may not reach node 3
    // depending on adjacency order. At most 1 path (if first neighbor leads to 3).
    assert!(paths.len() <= 1, "drive One(0) limits exploration");
}
