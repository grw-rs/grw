//! Tests for path search iterator API + bug fixes.
//!
//! Each test targets a specific fix or improvement:
//! 1. A* heuristic actually affects frontier priority (was broken — ignored heuristic)
//! 2. Lazy iterators (DFS/BFS yield one path at a time)
//! 3. Navigated search returns cost alongside path
//! 4. PathConstraint::default() is unconstrained
//! 5. Dijkstra::counted() works for any edge value type
//! 6. BFS with parent pointers matches DFS on random graphs
//! 7. NaN weight panics in debug mode

use std::collections::HashSet;

use grw::edge::anydir;
use grw::graph::edge::{AnyVal, End};
use grw::modify::dsl::*;
use grw::search::path::{self, AStar, Config, Dijkstra, PathConstraint};

type GrwGraph = grw::Graph<(), grw::edge::Anydir<u8>>;

fn is_directed_src(slot: anydir::Slot, _ev: &AnyVal<u8>) -> bool {
    matches!(slot, anydir::Slot::Dir(End::Src))
}

// ── Graph builders ─────────────────────────────────────────────────

/// A→B (w=1) → D, A→C (w=10) → D.  Two paths with different costs.
fn build_astar_graph() -> GrwGraph {
    let mut g: GrwGraph = grw::Graph::default();
    // 0=A, 1=B, 2=C, 3=D
    for _ in 0..4 {
        g.modify(vec![N_().val(()).into()]).unwrap();
    }
    // A→B weight 1
    g.modify(vec![(X(0u32) & E().val(1u8) >> X(1u32)).into()]).unwrap();
    // B→D weight 1
    g.modify(vec![(X(1u32) & E().val(1u8) >> X(3u32)).into()]).unwrap();
    // A→C weight 10
    g.modify(vec![(X(0u32) & E().val(10u8) >> X(2u32)).into()]).unwrap();
    // C→D weight 1
    g.modify(vec![(X(2u32) & E().val(1u8) >> X(3u32)).into()]).unwrap();
    g
}

fn build_chain(n: usize) -> GrwGraph {
    let mut g: GrwGraph = grw::Graph::default();
    for _ in 0..n {
        g.modify(vec![N_().val(()).into()]).unwrap();
    }
    for i in 0..n - 1 {
        let _ = g.modify(vec![
            (X(i as u32) & E().val(1u8) >> X((i + 1) as u32)).into(),
        ]);
    }
    g
}

fn build_diamond() -> GrwGraph {
    let mut g: GrwGraph = grw::Graph::default();
    for _ in 0..4 {
        g.modify(vec![N_().val(()).into()]).unwrap();
    }
    for &(a, b) in &[(0u32, 1u32), (0, 2), (1, 3), (2, 3)] {
        let _ = g.modify(vec![
            (X(a) & E().val(1u8) >> X(b)).into(),
        ]);
    }
    g
}

fn weight_fn(ev: &AnyVal<u8>) -> f64 {
    match ev {
        AnyVal::Dir(w) | AnyVal::Undir(w) => *w as f64,
    }
}

// ═══════════════════════════════════════════════════════════════════
// 1. A* heuristic fix
// ═══════════════════════════════════════════════════════════════════

#[test]
fn astar_heuristic_affects_priority() {
    // Graph: A→B→D (cost 2), A→C→D (cost 11)
    // Non-admissible heuristic: h(B)=100, h(C)=0, h(D)=0
    // With heuristic active: A* explores C first (f=10+0=10 < f(B)=1+100=101)
    //   → finds A→C→D with cost 11 first
    // If heuristic were ignored (old bug): A* = Dijkstra, finds A→B→D with cost 2
    let g = build_astar_graph();
    let constraint = PathConstraint::default();

    let config = Config::new(()).navigate(AStar::new(
        |ev: &AnyVal<u8>| weight_fn(ev),
        |node: grw::id::N| if *node == 1 { 100.0 } else { 0.0 },
    ));
    let result = g
        .path_navigate(grw::id::N(0), grw::id::N(3), is_directed_src, config, &constraint)
        .next();

    let (cost, path) = result.expect("should find a path");
    // Heuristic misleads A* to explore C before B → finds the expensive route
    assert_eq!(cost, 11.0, "A* should use heuristic, finding cost-11 path first");
    assert_eq!(
        path,
        vec![grw::id::N(0), grw::id::N(2), grw::id::N(3)],
        "path should go through C, not B"
    );
}

#[test]
fn astar_admissible_finds_optimal() {
    // Same graph, admissible heuristic h(n)=0 → A* = Dijkstra → finds optimal
    let g = build_astar_graph();
    let constraint = PathConstraint::default();

    let config = Config::new(()).navigate(AStar::new(
        |ev: &AnyVal<u8>| weight_fn(ev),
        |_node: grw::id::N| 0.0,
    ));
    let (cost, path) = g
        .path_navigate(grw::id::N(0), grw::id::N(3), is_directed_src, config, &constraint)
        .next()
        .expect("should find a path");

    assert_eq!(cost, 2.0, "admissible A* should find optimal cost");
    assert_eq!(path, vec![grw::id::N(0), grw::id::N(1), grw::id::N(3)]);
}

// ═══════════════════════════════════════════════════════════════════
// 2. Lazy iterators
// ═══════════════════════════════════════════════════════════════════

#[test]
fn lazy_dfs_next_yields_one_path() {
    let g = build_diamond(); // 2 paths from 0→3
    let constraint = PathConstraint::default();
    let config = Config::new(()).dfs();
    let mut iter = g.path_search(
        grw::id::N(0), grw::id::N(3), is_directed_src, config, &constraint,
    );
    let first = iter.next();
    assert!(first.is_some(), "should yield at least one path");
    let second = iter.next();
    assert!(second.is_some(), "diamond has two paths");
    let third = iter.next();
    assert!(third.is_none(), "diamond has exactly two paths");
}

#[test]
fn lazy_bfs_shortest_first() {
    // Build graph with short path (0→3) and long path (0→1→2→3)
    let mut g: GrwGraph = grw::Graph::default();
    for _ in 0..4 {
        g.modify(vec![N_().val(()).into()]).unwrap();
    }
    // 0→1→2→3 (long)
    for &(a, b) in &[(0u32, 1), (1, 2), (2, 3)] {
        g.modify(vec![(X(a) & E().val(1u8) >> X(b)).into()]).unwrap();
    }
    // 0→3 (short)
    g.modify(vec![(X(0u32) & E().val(1u8) >> X(3u32)).into()]).unwrap();

    let constraint = PathConstraint::default();
    let config = Config::new(()).bfs();
    let first = g
        .path_search(grw::id::N(0), grw::id::N(3), is_directed_src, config, &constraint)
        .next()
        .expect("should find a path");

    assert_eq!(first.len(), 2, "BFS first result should be shortest (2 nodes = 1 edge)");
    assert_eq!(first, vec![grw::id::N(0), grw::id::N(3)]);
}

#[test]
fn lazy_collect_matches_exhaustive() {
    // Collect all paths via iterator should match original behavior
    let g = build_diamond();
    let constraint = PathConstraint::default();
    let config = Config::new(()).dfs();
    let all: HashSet<Vec<grw::id::N>> = g
        .path_search(grw::id::N(0), grw::id::N(3), is_directed_src, config, &constraint)
        .collect();

    assert_eq!(all.len(), 2);
    assert!(all.contains(&vec![grw::id::N(0), grw::id::N(1), grw::id::N(3)]));
    assert!(all.contains(&vec![grw::id::N(0), grw::id::N(2), grw::id::N(3)]));
}

#[test]
fn trivial_self_path_with_len_zero() {
    let g = build_chain(3);
    let constraint = PathConstraint::default();

    // len(0) on from==to: yields trivial [A]
    let config = Config::new(()).dfs().len(0..);
    let first = g
        .path_search(grw::id::N(1), grw::id::N(1), is_directed_src, config, &constraint)
        .next();
    assert_eq!(first, Some(vec![grw::id::N(1)]), "len(0..) from==to should yield trivial [A]");

    // len(1..) on from==to: skips trivial, finds actual cycle (if any)
    let config2 = Config::new(()).dfs().len(1..);
    let first2 = g
        .path_search(grw::id::N(1), grw::id::N(1), is_directed_src, config2, &constraint)
        .next();
    // Chain 0→1→2 has no cycle back to 1
    assert_eq!(first2, None, "len(1..) on acyclic chain should find nothing");

    // Default (no .len()) has min_len=1, so no trivial
    let config3 = Config::new(()).dfs();
    let first3 = g
        .path_search(grw::id::N(1), grw::id::N(1), is_directed_src, config3, &constraint)
        .next();
    assert_eq!(first3, None, "default min_len=1 excludes trivial self-path");
}

#[test]
fn lazy_chain_no_path_returns_none() {
    let g = build_chain(5);
    let constraint = PathConstraint::default();
    let config = Config::new(()).dfs();
    // Reverse direction: no path from 4→0 in directed chain
    let result = g
        .path_search(grw::id::N(4), grw::id::N(0), is_directed_src, config, &constraint)
        .next();
    assert!(result.is_none());
}

// ═══════════════════════════════════════════════════════════════════
// 3. Navigated search returns cost
// ═══════════════════════════════════════════════════════════════════

#[test]
fn navigated_dijkstra_returns_cost() {
    let g = build_astar_graph(); // A→B→D cost 2, A→C→D cost 11
    let constraint = PathConstraint::default();
    let config = Config::new(()).navigate(Dijkstra::weighted(|ev: &AnyVal<u8>| weight_fn(ev)));
    let (cost, path) = g
        .path_navigate(grw::id::N(0), grw::id::N(3), is_directed_src, config, &constraint)
        .next()
        .expect("should find a path");

    assert_eq!(cost, 2.0, "Dijkstra should find optimal cost");
    assert_eq!(path, vec![grw::id::N(0), grw::id::N(1), grw::id::N(3)]);
}

#[test]
fn navigated_counted_returns_hop_count() {
    let g = build_chain(5); // 0→1→2→3→4
    let constraint = PathConstraint::default();
    let config = Config::new(()).navigate(Dijkstra::counted::<AnyVal<u8>>());
    let (cost, path) = g
        .path_navigate(grw::id::N(0), grw::id::N(4), is_directed_src, config, &constraint)
        .next()
        .expect("should find a path");

    assert_eq!(cost, 4.0, "counted gives hop count as cost");
    assert_eq!(path.len(), 5);
}

// ═══════════════════════════════════════════════════════════════════
// 4. PathConstraint default
// ═══════════════════════════════════════════════════════════════════

#[test]
fn constraint_default_is_unconstrained() {
    let def = PathConstraint::default();
    let unc = PathConstraint::unconstrained();
    // Both should accept any node
    assert!(def.accept_node(grw::id::N(42)));
    assert!(unc.accept_node(grw::id::N(42)));
    // And produce the same search results
    let g = build_chain(5);
    let config_d = Config::new(()).dfs();
    let config_u = Config::new(()).dfs();
    let paths_d: Vec<_> = g
        .path_search(grw::id::N(0), grw::id::N(4), is_directed_src, config_d, &def)
        .collect();
    let paths_u: Vec<_> = g
        .path_search(grw::id::N(0), grw::id::N(4), is_directed_src, config_u, &unc)
        .collect();
    assert_eq!(paths_d, paths_u);
}

// ═══════════════════════════════════════════════════════════════════
// 5. Dijkstra::counted with typed edges
// ═══════════════════════════════════════════════════════════════════

#[test]
fn dijkstra_counted_works_with_any_edge_type() {
    // counted() should compile and work even when EV != ()
    let g = build_diamond();
    let constraint = PathConstraint::default();
    // EV = AnyVal<u8>, not ()
    let config = Config::new(()).navigate(Dijkstra::counted::<AnyVal<u8>>());
    let result = g
        .path_navigate(grw::id::N(0), grw::id::N(3), is_directed_src, config, &constraint)
        .next();
    assert!(result.is_some(), "counted() should work with typed edge values");
    let (cost, _path) = result.unwrap();
    assert_eq!(cost, 2.0, "diamond shortest is 2 hops");
}

// ═══════════════════════════════════════════════════════════════════
// 6. BFS correctness (parent-pointer impl matches DFS)
// ═══════════════════════════════════════════════════════════════════

fn build_random_directed(seed: u64, nodes: usize, edges: usize) -> GrwGraph {
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};
    use std::collections::HashSet;

    let mut rng = SmallRng::seed_from_u64(seed);
    let mut g: GrwGraph = grw::Graph::default();
    for _ in 0..nodes {
        g.modify(vec![N_().val(()).into()]).unwrap();
    }
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    for _ in 0..edges {
        let a = rng.random_range(0..nodes as u32);
        let b = rng.random_range(0..nodes as u32);
        if a == b || !seen.insert((a, b)) {
            continue;
        }
        let label: u8 = rng.random_range(0..4);
        let _ = g.modify(vec![
            (X(a) & E().val(label) >> X(b)).into(),
        ]);
    }
    g
}

#[test]
fn bfs_finds_same_paths_as_dfs() {
    let constraint = PathConstraint::default();
    for seed in 0..50 {
        let g = build_random_directed(seed, 8, 15);
        for from in 0..8u32 {
            for to in 0..8u32 {
                if from == to { continue; }
                let dfs_config = Config::new(()).dfs();
                let bfs_config = Config::new(()).bfs();
                let dfs: HashSet<Vec<grw::id::N>> = g
                    .path_search(grw::id::N(from), grw::id::N(to), is_directed_src, dfs_config, &constraint)
                    .collect();
                let bfs: HashSet<Vec<grw::id::N>> = g
                    .path_search(grw::id::N(from), grw::id::N(to), is_directed_src, bfs_config, &constraint)
                    .collect();
                assert_eq!(dfs, bfs, "seed={seed} {from}->{to} DFS vs BFS path sets differ");
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// 7. NaN weight panics
// ═══════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "edge cost produced NaN")]
fn nan_weight_panics() {
    let g = build_chain(3);
    let constraint = PathConstraint::default();
    let config = Config::new(()).navigate(Dijkstra::weighted(|_: &AnyVal<u8>| f64::NAN));
    // Consuming the iterator triggers the search, which hits the NaN assert
    let _ = g
        .path_navigate(grw::id::N(0), grw::id::N(2), is_directed_src, config, &constraint)
        .next();
}

// ═══════════════════════════════════════════════════════════════════
// 8. Guard and length bounds work with iterators
// ═══════════════════════════════════════════════════════════════════

#[test]
fn iterator_respects_len_bounds() {
    let g = build_chain(6);
    let constraint = PathConstraint::default();
    // Only 2..4 edges
    let config = Config::new(()).dfs().len(2..4);
    let paths: Vec<_> = g
        .path_search(grw::id::N(0), grw::id::N(5), is_directed_src, config, &constraint)
        .collect();
    // Chain 0→1→2→3→4→5 has 5 edges → outside 2..4 → no result
    assert!(paths.is_empty());

    let config2 = Config::new(()).dfs().len(2..4);
    let paths2: Vec<_> = g
        .path_search(grw::id::N(0), grw::id::N(2), is_directed_src, config2, &constraint)
        .collect();
    assert_eq!(paths2.len(), 1, "0→1→2 has 2 edges, within 2..4");
}

#[test]
fn iterator_respects_guard() {
    let g = build_chain(5);
    let constraint = PathConstraint::default();
    let config = Config::new(())
        .dfs()
        .guard(|p| !p.contains(grw::id::N(2)));
    let paths: Vec<_> = g
        .path_search(grw::id::N(0), grw::id::N(4), is_directed_src, config, &constraint)
        .collect();
    // Only path 0→1→2→3→4 passes through node 2 → guard rejects → empty
    assert!(paths.is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// 9. Constraint works with unified API
// ═══════════════════════════════════════════════════════════════════

#[test]
fn constraint_mono_via_unified_api() {
    // Build fork: 0→1→2→3, 0→5→3
    let mut g: GrwGraph = grw::Graph::default();
    for _ in 0..6 {
        g.modify(vec![N_().val(()).into()]).unwrap();
    }
    for &(a, b) in &[(0u32, 1), (1, 2), (2, 3), (3, 4), (0, 5), (5, 3)] {
        let _ = g.modify(vec![(X(a) & E().val(1u8) >> X(b)).into()]);
    }

    // First path: 0→1→2→3, commit intermediates
    let mut constraint = PathConstraint::from_morphism(grw::Mono, &[]);
    let config = Config::new(()).dfs();
    let paths1: Vec<_> = g
        .path_search(grw::id::N(0), grw::id::N(3), is_directed_src, config, &constraint)
        .collect();
    assert!(!paths1.is_empty());
    constraint.commit(&paths1[0]);

    // Second search: intermediates of first path excluded
    let config2 = Config::new(()).dfs();
    let paths2: Vec<_> = g
        .path_search(grw::id::N(0), grw::id::N(3), is_directed_src, config2, &constraint)
        .collect();
    for p in &paths2 {
        for &n in &p[1..p.len() - 1] {
            assert!(
                !paths1[0][1..paths1[0].len() - 1].contains(&n),
                "mono: path 2 shares intermediate {n:?} with path 1"
            );
        }
    }
    assert!(!paths2.is_empty(), "should find alternative path 0→5→3");
}

// ═══════════════════════════════════════════════════════════════════
// 10. Algorithm behavior comparison — each algo finds different path
// ═══════════════════════════════════════════════════════════════════

/// Graph where DFS, BFS, Dijkstra each find a different first path:
///
///   S →(w=2)→ D →(w=2)→ E →(w=2)→ T    (3 hops, cost 6)
///   S →(w=1)→ B →(w=1)→ C →(w=1)→ T    (3 hops, cost 3 — cheapest)
///   S →(w=100)→ A →(w=1)→ T             (2 hops, cost 101 — fewest hops)
///
/// Edge insertion order: D-path first → DFS explores it first (LIFO).
fn build_algo_comparison_graph() -> GrwGraph {
    // S=0, D=1, E=2, T=3, B=4, C=5, A=6
    let mut g: GrwGraph = grw::Graph::default();
    for _ in 0..7 {
        g.modify(vec![N_().val(()).into()]).unwrap();
    }
    // D-path (inserted first → first neighbor of S → DFS explores first)
    g.modify(vec![(X(0u32) & E().val(2u8) >> X(1u32)).into()]).unwrap();
    g.modify(vec![(X(1u32) & E().val(2u8) >> X(2u32)).into()]).unwrap();
    g.modify(vec![(X(2u32) & E().val(2u8) >> X(3u32)).into()]).unwrap();
    // B-path (cheapest by weight)
    g.modify(vec![(X(0u32) & E().val(1u8) >> X(4u32)).into()]).unwrap();
    g.modify(vec![(X(4u32) & E().val(1u8) >> X(5u32)).into()]).unwrap();
    g.modify(vec![(X(5u32) & E().val(1u8) >> X(3u32)).into()]).unwrap();
    // A-path (fewest hops but most expensive)
    g.modify(vec![(X(0u32) & E().val(100u8) >> X(6u32)).into()]).unwrap();
    g.modify(vec![(X(6u32) & E().val(1u8) >> X(3u32)).into()]).unwrap();
    g
}

#[test]
fn each_algorithm_finds_different_first_path() {
    let g = build_algo_comparison_graph();
    let constraint = PathConstraint::default();
    let s = grw::id::N(0);
    let t = grw::id::N(3);

    let dfs_first = g.path_search(s, t, is_directed_src, Config::new(()).dfs(), &constraint)
        .next().unwrap();
    let bfs_first = g.path_search(s, t, is_directed_src, Config::new(()).bfs(), &constraint)
        .next().unwrap();
    let (dij_cost, dij_path) = g.path_navigate(s, t, is_directed_src,
        Config::new(()).navigate(Dijkstra::weighted(|ev: &AnyVal<u8>| weight_fn(ev))),
        &constraint,
    ).next().unwrap();
    let (astar_cost, astar_path) = g.path_navigate(s, t, is_directed_src,
        Config::new(()).navigate(AStar::new(
            |ev: &AnyVal<u8>| weight_fn(ev),
            |_: grw::id::N| 0.0,
        )),
        &constraint,
    ).next().unwrap();

    // DFS: S(0)→D(1)→E(2)→T(3) — first neighbor explored first
    assert_eq!(dfs_first, vec![s, grw::id::N(1), grw::id::N(2), t],
        "DFS should find D-path (first inserted neighbor)");

    // BFS: S(0)→A(6)→T(3) — fewest hops (2)
    assert_eq!(bfs_first, vec![s, grw::id::N(6), t],
        "BFS should find A-path (fewest hops)");
    assert_eq!(bfs_first.len(), 3, "BFS path has 2 edges");

    // Dijkstra: S(0)→B(4)→C(5)→T(3) — lowest cost (3)
    assert_eq!(dij_cost, 3.0, "Dijkstra should find cost-3 path");
    assert_eq!(dij_path, vec![s, grw::id::N(4), grw::id::N(5), t],
        "Dijkstra should find B-path (cheapest)");

    // A* with h=0 matches Dijkstra exactly
    assert_eq!(astar_cost, dij_cost);
    assert_eq!(astar_path, dij_path);

    // All three first-paths are distinct
    assert_ne!(dfs_first, bfs_first);
    assert_ne!(bfs_first, dij_path);
    assert_ne!(dfs_first, dij_path);
}

// ═══════════════════════════════════════════════════════════════════
// 11. Spatial grid: A* explores fewer nodes than Dijkstra
// ═══════════════════════════════════════════════════════════════════

const GRID: u32 = 40;

fn grid_id(x: u32, y: u32) -> u32 { y * GRID + x }
fn grid_pos(id: u32) -> (f64, f64) { ((id % GRID) as f64, (id / GRID) as f64) }

/// 40×40 grid with a cheap "highway" corridor along the diagonal (|x-y| ≤ 3)
/// and expensive terrain (weight=3) everywhere else.
///
/// A* with Euclidean heuristic discovers the cheap diagonal corridor quickly
/// and stays near it, while Dijkstra explores expensive terrain uniformly
/// in all directions before reaching the target.
fn build_highway_grid() -> GrwGraph {
    let mut g: GrwGraph = grw::Graph::default();
    for _ in 0..(GRID * GRID) {
        g.modify(vec![N_().val(()).into()]).unwrap();
    }
    let near_diag = |x: u32, y: u32| (x as i32 - y as i32).unsigned_abs() <= 3;
    for y in 0..GRID {
        for x in 0..GRID {
            let id = grid_id(x, y);
            if x + 1 < GRID {
                let right = grid_id(x + 1, y);
                let w: u8 = if near_diag(x, y) && near_diag(x + 1, y) { 1 } else { 3 };
                g.modify(vec![(X(id) & E().val(w) >> X(right)).into()]).unwrap();
                g.modify(vec![(X(right) & E().val(w) >> X(id)).into()]).unwrap();
            }
            if y + 1 < GRID {
                let below = grid_id(x, y + 1);
                let w: u8 = if near_diag(x, y) && near_diag(x, y + 1) { 1 } else { 3 };
                g.modify(vec![(X(id) & E().val(w) >> X(below)).into()]).unwrap();
                g.modify(vec![(X(below) & E().val(w) >> X(id)).into()]).unwrap();
            }
        }
    }
    g
}

#[test]
fn astar_explores_fewer_edges_than_dijkstra_on_spatial_grid() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let g = build_highway_grid();
    let constraint = PathConstraint::default();
    let start = grw::id::N(grid_id(0, 0));
    let target = grw::id::N(grid_id(GRID - 1, GRID - 1));
    let target_pos = grid_pos(*target);

    // Dijkstra — count edge evaluations via weight function calls
    let dij_evals = Arc::new(AtomicUsize::new(0));
    let de = dij_evals.clone();
    let dij_config = Config::new(()).navigate(Dijkstra::weighted(move |ev: &AnyVal<u8>| {
        de.fetch_add(1, Ordering::Relaxed);
        match ev { AnyVal::Dir(w) | AnyVal::Undir(w) => *w as f64 }
    }));
    let (dij_cost, _) = g.path_navigate(start, target, is_directed_src, dij_config, &constraint)
        .next().expect("Dijkstra should find path on grid");
    let dij_count = dij_evals.load(Ordering::Relaxed);

    // A* with Euclidean heuristic — count edge evaluations
    let astar_evals = Arc::new(AtomicUsize::new(0));
    let ae = astar_evals.clone();
    let astar_config = Config::new(()).navigate(AStar::new(
        move |ev: &AnyVal<u8>| {
            ae.fetch_add(1, Ordering::Relaxed);
            match ev { AnyVal::Dir(w) | AnyVal::Undir(w) => *w as f64 }
        },
        move |node: grw::id::N| {
            let p = grid_pos(*node);
            ((p.0 - target_pos.0).powi(2) + (p.1 - target_pos.1).powi(2)).sqrt()
        },
    ));
    let (astar_cost, _) = g.path_navigate(start, target, is_directed_src, astar_config, &constraint)
        .next().expect("A* should find path on grid");
    let astar_count = astar_evals.load(Ordering::Relaxed);

    // Same optimal cost
    assert_eq!(dij_cost, astar_cost,
        "A* and Dijkstra must find same optimal cost: dij={dij_cost} astar={astar_cost}");

    // A* explores strictly fewer edges — at least 1.5x advantage on open grid
    let ratio = dij_count as f64 / astar_count as f64;
    assert!(ratio > 1.5,
        "A* should evaluate significantly fewer edges than Dijkstra on 40x40 grid: \
         A*={astar_count} Dijkstra={dij_count} ratio={ratio:.2}x");

    // Typical ratio: ~2x on this 40×40 highway grid
    eprintln!("spatial grid: Dijkstra={dij_count} A*={astar_count} ratio={ratio:.1}x cost={dij_cost}");
}
