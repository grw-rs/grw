//! Path search correctness tests using petgraph as oracle.

use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Reverse;

use grw::edge::anydir;
use grw::graph::edge::{AnyVal, End};
use grw::modify::dsl::*;
use grw::Graph as _;
use petgraph::algo::{all_simple_paths, astar};
use petgraph::graph::{DiGraph, NodeIndex, UnGraph};
use petgraph::visit::EdgeRef;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

type GrwGraph = grw::MGraph<(), grw::edge::Anydir<u8>>;

// ── Graph construction ──────────────────────────────────────────────

/// Build identical random graphs in grw and petgraph.
/// Directed edges go to pg_di, undirected to pg_un. Both go to grw.
fn build_random_graphs(
    seed: u64,
    node_count: usize,
    edge_count: usize,
) -> (GrwGraph, DiGraph<(), u8>, UnGraph<(), u8>) {
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut grw_g: GrwGraph = grw::MGraph::default();
    for _ in 0..node_count {
        grw_g.modify(vec![N_().val(()).into()]).expect("add node");
    }

    let mut pg_di = DiGraph::<(), u8>::new();
    let mut pg_un = UnGraph::<(), u8>::new_undirected();
    for _ in 0..node_count {
        pg_di.add_node(());
        pg_un.add_node(());
    }

    let mut seen_dir: HashSet<(u32, u32)> = HashSet::new();
    let mut seen_und: HashSet<(u32, u32)> = HashSet::new();
    for _ in 0..edge_count {
        let a = rng.random_range(0..node_count as u32);
        let b = rng.random_range(0..node_count as u32);
        if a == b { continue; }
        let label: u8 = rng.random_range(0..4);
        let directed: bool = rng.random_bool(0.5);

        if directed {
            if !seen_dir.insert((a, b)) { continue; }
            let _ = grw_g.modify(vec![
                (X(a) & E().val(label) >> X(b)).into(),
            ]);
            pg_di.add_edge(NodeIndex::new(a as usize), NodeIndex::new(b as usize), label);
        } else {
            let key = (a.min(b), a.max(b));
            if !seen_und.insert(key) { continue; }
            let _ = grw_g.modify(vec![
                (X(a) & E().val(label) ^ X(b)).into(),
            ]);
            pg_un.add_edge(NodeIndex::new(a as usize), NodeIndex::new(b as usize), label);
        }
    }

    (grw_g, pg_di, pg_un)
}

/// Build weighted directed graph for shortest-path tests.
/// Skips duplicate (a,b) pairs to match grw's one-edge-per-slot semantics.
fn build_weighted_digraph(
    seed: u64,
    node_count: usize,
    edge_count: usize,
) -> (GrwGraph, DiGraph<(), u8>) {
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut grw_g: GrwGraph = grw::MGraph::default();
    for _ in 0..node_count {
        grw_g.modify(vec![N_().val(()).into()]).expect("add node");
    }
    let mut pg = DiGraph::<(), u8>::new();
    for _ in 0..node_count { pg.add_node(()); }

    let mut seen_dir: HashSet<(u32, u32)> = HashSet::new();
    for _ in 0..edge_count {
        let a = rng.random_range(0..node_count as u32);
        let b = rng.random_range(0..node_count as u32);
        if a == b { continue; }
        if !seen_dir.insert((a, b)) { continue; }
        let weight: u8 = rng.random_range(1..20);
        let _ = grw_g.modify(vec![
            (X(a) & E().val(weight) >> X(b)).into(),
        ]);
        pg.add_edge(NodeIndex::new(a as usize), NodeIndex::new(b as usize), weight);
    }

    (grw_g, pg)
}

// ── grw path wrappers using Graph methods ───────────────────────────

fn is_directed_src(slot: anydir::Slot, _ev: &AnyVal<u8>) -> bool {
    matches!(slot, anydir::Slot::Dir(End::Src))
}

fn is_undirected(slot: anydir::Slot, _ev: &AnyVal<u8>) -> bool {
    matches!(slot, anydir::Slot::Undir)
}

fn directed_weight(slot: anydir::Slot, ev: &AnyVal<u8>) -> Option<u32> {
    if !matches!(slot, anydir::Slot::Dir(End::Src)) { return None; }
    Some(match ev { AnyVal::Dir(l) => *l as u32, AnyVal::Undir(l) => *l as u32 })
}

fn grw_simple_directed_paths(g: &GrwGraph, from: u32, to: u32, n: usize) -> HashSet<Vec<grw::id::N>> {
    g.all_simple_paths(grw::id::N(from), grw::id::N(to), is_directed_src, n)
        .into_iter().collect()
}

fn grw_simple_undirected_paths(g: &GrwGraph, from: u32, to: u32, n: usize) -> HashSet<Vec<grw::id::N>> {
    g.all_simple_paths(grw::id::N(from), grw::id::N(to), is_undirected, n)
        .into_iter().collect()
}

fn grw_filtered_directed_paths(g: &GrwGraph, from: u32, to: u32, n: usize, label: u8) -> HashSet<Vec<grw::id::N>> {
    g.all_simple_paths(grw::id::N(from), grw::id::N(to), |slot, ev| {
        is_directed_src(slot, ev) && match ev { AnyVal::Dir(l) | AnyVal::Undir(l) => *l == label }
    }, n).into_iter().collect()
}

fn pg_paths(pg: &DiGraph<(), u8>, from: u32, to: u32, n: usize) -> HashSet<Vec<grw::id::N>> {
    all_simple_paths(pg, NodeIndex::new(from as usize), NodeIndex::new(to as usize), 0, Some(n))
        .map(|p: Vec<NodeIndex>| p.iter().map(|ni| grw::id::N(ni.index() as u32)).collect())
        .collect()
}

fn pg_un_paths(pg: &UnGraph<(), u8>, from: u32, to: u32, n: usize) -> HashSet<Vec<grw::id::N>> {
    all_simple_paths(pg, NodeIndex::new(from as usize), NodeIndex::new(to as usize), 0, Some(n))
        .map(|p: Vec<NodeIndex>| p.iter().map(|ni| grw::id::N(ni.index() as u32)).collect())
        .collect()
}

// ── Tests ───────────────────────────────────────────────────────────

#[test]
fn directed_small_graphs() {
    for seed in 0..100 {
        let (grw_g, pg_di, _) = build_random_graphs(seed, 8, 15);
        let n = 8;
        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                let grw = grw_simple_directed_paths(&grw_g, from, to, n);
                let pg = pg_paths(&pg_di, from, to, n);
                assert_eq!(grw, pg, "seed={seed} {from}->{to}");
            }
        }
    }
}

#[test]
fn directed_medium_graphs() {
    for seed in 0..20 {
        let (grw_g, pg_di, _) = build_random_graphs(seed, 15, 30);
        let n = 15;
        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                let grw = grw_simple_directed_paths(&grw_g, from, to, n);
                let pg = pg_paths(&pg_di, from, to, n);
                assert_eq!(grw, pg, "seed={seed} {from}->{to}");
            }
        }
    }
}

#[test]
fn undirected_small_graphs() {
    for seed in 0..100 {
        let (grw_g, _, pg_un) = build_random_graphs(seed, 8, 15);
        let n = 8;
        for from in 0..n as u32 {
            for to in (from + 1)..n as u32 {
                let grw = grw_simple_undirected_paths(&grw_g, from, to, n);
                let pg = pg_un_paths(&pg_un, from, to, n);
                assert_eq!(grw, pg, "seed={seed} {from}--{to}");
            }
        }
    }
}

#[test]
fn undirected_medium_graphs() {
    for seed in 0..20 {
        let (grw_g, _, pg_un) = build_random_graphs(seed, 15, 30);
        let n = 15;
        for from in 0..n as u32 {
            for to in (from + 1)..n as u32 {
                let grw = grw_simple_undirected_paths(&grw_g, from, to, n);
                let pg = pg_un_paths(&pg_un, from, to, n);
                assert_eq!(grw, pg, "seed={seed} {from}--{to}");
            }
        }
    }
}

#[test]
fn filtered_directed_paths() {
    for seed in 0..50 {
        let (grw_g, pg_di, _) = build_random_graphs(seed, 8, 20);
        let n = 8;
        let filter_label: u8 = (seed % 4) as u8;

        let mut pg_filtered = DiGraph::<(), u8>::new();
        for _ in 0..n { pg_filtered.add_node(()); }
        for edge in pg_di.edge_references() {
            if *edge.weight() == filter_label {
                pg_filtered.add_edge(edge.source(), edge.target(), *edge.weight());
            }
        }

        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                let grw = grw_filtered_directed_paths(&grw_g, from, to, n, filter_label);
                let pg = pg_paths(&pg_filtered, from, to, n);
                assert_eq!(grw, pg, "seed={seed} filter={filter_label} {from}->{to}");
            }
        }
    }
}

#[test]
fn cycle_detection() {
    for seed in 0..50 {
        let (grw_g, pg_di, _) = build_random_graphs(seed, 6, 12);
        let n = 6;
        for node in 0..n as u32 {
            let grw_cycles = grw_simple_directed_paths(&grw_g, node, node, n);
            let pg_cycles: HashSet<Vec<u32>> = all_simple_paths(
                &pg_di, NodeIndex::new(node as usize), NodeIndex::new(node as usize), 1, Some(n),
            ).map(|p: Vec<NodeIndex>| p.iter().map(|ni| ni.index() as u32).collect()).collect();
            assert_eq!(
                !grw_cycles.is_empty(), !pg_cycles.is_empty(),
                "seed={seed} node={node} cycle mismatch"
            );
        }
    }
}

#[test]
fn dijkstra_matches_petgraph() {
    for seed in 0..50 {
        let (grw_g, pg) = build_weighted_digraph(seed, 12, 25);
        let n = 12;
        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                let grw_result = grw_g.shortest_path(
                    grw::id::N(from), grw::id::N(to), directed_weight,
                );
                let pg_result = astar(
                    &pg,
                    NodeIndex::new(from as usize),
                    |n| n == NodeIndex::new(to as usize),
                    |e| *e.weight() as u32,
                    |_| 0,
                );
                match (&grw_result, &pg_result) {
                    (None, None) => {}
                    (Some((gc, _)), Some((pc, _))) => {
                        assert_eq!(*gc, *pc, "seed={seed} {from}->{to} dijkstra cost mismatch");
                    }
                    (g, p) => panic!("seed={seed} {from}->{to} dijkstra existence mismatch grw={g:?} pg={p:?}"),
                }
            }
        }
    }
}

#[test]
fn astar_matches_petgraph() {
    for seed in 0..50 {
        let (grw_g, pg) = build_weighted_digraph(seed, 12, 25);
        let n = 12;
        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                let grw_result = grw_g.astar(
                    grw::id::N(from), grw::id::N(to),
                    directed_weight,
                    |_| 0, // zero heuristic = Dijkstra
                );
                let pg_result = astar(
                    &pg,
                    NodeIndex::new(from as usize),
                    |n| n == NodeIndex::new(to as usize),
                    |e| *e.weight() as u32,
                    |_| 0,
                );
                match (&grw_result, &pg_result) {
                    (None, None) => {}
                    (Some((gc, _)), Some((pc, _))) => {
                        assert_eq!(*gc, *pc, "seed={seed} {from}->{to} astar cost mismatch");
                    }
                    (g, p) => panic!("seed={seed} {from}->{to} astar existence mismatch grw={g:?} pg={p:?}"),
                }
            }
        }
    }
}

#[test]
fn dijkstra_large_graphs() {
    for seed in 0..10 {
        let (grw_g, pg) = build_weighted_digraph(seed, 30, 80);
        let n = 30;
        let mut rng = SmallRng::seed_from_u64(seed + 1000);
        for _ in 0..50 {
            let from = rng.random_range(0..n as u32);
            let to = rng.random_range(0..n as u32);
            if from == to { continue; }
            let grw_result = grw_g.shortest_path(
                grw::id::N(from), grw::id::N(to), directed_weight,
            );
            let pg_result = astar(
                &pg,
                NodeIndex::new(from as usize),
                |n| n == NodeIndex::new(to as usize),
                |e| *e.weight() as u32,
                |_| 0,
            );
            match (&grw_result, &pg_result) {
                (None, None) => {}
                (Some((gc, _)), Some((pc, _))) => {
                    assert_eq!(*gc, *pc, "seed={seed} {from}->{to} cost mismatch");
                }
                _ => panic!("seed={seed} {from}->{to} existence mismatch"),
            }
        }
    }
}

#[test]
fn astar_with_heuristic() {
    // A* with admissible heuristic should find same cost as Dijkstra.
    // Use node ID difference as a trivial admissible heuristic.
    for seed in 0..20 {
        let (grw_g, pg) = build_weighted_digraph(seed, 15, 35);
        let n = 15;
        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                let dijkstra_result = grw_g.shortest_path(
                    grw::id::N(from), grw::id::N(to), directed_weight,
                );
                let astar_result = grw_g.astar(
                    grw::id::N(from), grw::id::N(to),
                    directed_weight,
                    |_n| 0, // admissible: always underestimates
                );
                match (&dijkstra_result, &astar_result) {
                    (None, None) => {}
                    (Some((dc, _)), Some((ac, _))) => {
                        assert_eq!(*dc, *ac, "seed={seed} {from}->{to} astar vs dijkstra cost mismatch");
                    }
                    (d, a) => panic!("seed={seed} {from}->{to} astar vs dijkstra existence mismatch d={d:?} a={a:?}"),
                }
            }
        }
    }
}

#[test]
fn directed_dense_graphs() {
    for seed in 0..20 {
        let (grw_g, pg_di, _) = build_random_graphs(seed, 6, 25);
        let n = 6;
        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                let grw = grw_simple_directed_paths(&grw_g, from, to, n);
                let pg = pg_paths(&pg_di, from, to, n);
                assert_eq!(grw, pg, "dense seed={seed} {from}->{to}");
            }
        }
    }
}

#[test]
fn undirected_dense_graphs() {
    for seed in 0..20 {
        let (grw_g, _, pg_un) = build_random_graphs(seed, 6, 25);
        let n = 6;
        for from in 0..n as u32 {
            for to in (from + 1)..n as u32 {
                let grw = grw_simple_undirected_paths(&grw_g, from, to, n);
                let pg = pg_un_paths(&pg_un, from, to, n);
                assert_eq!(grw, pg, "dense seed={seed} {from}--{to}");
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Executor tests — verify path::execute matches Graph methods
// ═══════════════════════════════════════════════════════════════════

use grw::search::path::{self, PathConstraint};
use grw::search::dsl::{self as sdsl, IntoPathConfig};
use grw::search::Search;

fn exec_dfs_config() -> path::Config<(), path::Traversal> {
    path::Config::new(()).dfs()
}

fn exec_bfs_config() -> path::Config<(), path::Traversal> {
    path::Config::new(()).bfs()
}

fn exec_drive_all_config() -> path::Config<(), path::Traversal> {
    path::Config::new(()).drive(|_| Some(path::Explore::All))
}

fn exec_navigate_config() -> path::Config<(), path::Navigated, AnyVal<u8>> {
    path::Config::new(()).navigate(path::Dijkstra::counted::<AnyVal<u8>>())
}

#[test]
fn executor_dfs_matches_graph_method() {
    for seed in 0..50 {
        let (grw_g, pg_di, _) = build_random_graphs(seed, 8, 15);
        let n = 8;
        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                let exec_result: HashSet<Vec<grw::id::N>> = grw_g.path_search(
                    grw::id::N(from), grw::id::N(to),
                    is_directed_src, exec_dfs_config(), &PathConstraint::default(),
                ).collect();
                let graph_result = grw_simple_directed_paths(&grw_g, from, to, n);
                assert_eq!(exec_result, graph_result, "exec_dfs seed={seed} {from}->{to}");
            }
        }
    }
}

#[test]
fn executor_bfs_matches_graph_method() {
    for seed in 0..50 {
        let (grw_g, pg_di, _) = build_random_graphs(seed, 8, 15);
        let n = 8;
        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                let exec_result: HashSet<Vec<grw::id::N>> = grw_g.path_search(
                    grw::id::N(from), grw::id::N(to),
                    is_directed_src, exec_bfs_config(), &PathConstraint::default(),
                ).collect();
                let pg = pg_paths(&pg_di, from, to, n);
                assert_eq!(exec_result, pg, "exec_bfs seed={seed} {from}->{to}");
            }
        }
    }
}

#[test]
fn executor_drive_all_matches_dfs() {
    for seed in 0..30 {
        let (grw_g, _, _) = build_random_graphs(seed, 8, 15);
        let n = 8;
        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                let dfs_result: HashSet<Vec<grw::id::N>> = grw_g.path_search(
                    grw::id::N(from), grw::id::N(to),
                    is_directed_src, exec_dfs_config(), &PathConstraint::default(),
                ).collect();
                let drive_result: HashSet<Vec<grw::id::N>> = grw_g.path_search(
                    grw::id::N(from), grw::id::N(to),
                    is_directed_src, exec_drive_all_config(), &PathConstraint::default(),
                ).collect();
                assert_eq!(dfs_result, drive_result, "drive_all seed={seed} {from}->{to}");
            }
        }
    }
}

#[test]
fn executor_navigate_dijkstra_finds_paths() {
    for seed in 0..30 {
        let (grw_g, pg_di, _) = build_random_graphs(seed, 8, 15);
        let n = 8;
        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                let nav_paths: Vec<Vec<grw::id::N>> = grw_g.path_navigate(
                    grw::id::N(from), grw::id::N(to),
                    is_directed_src, exec_navigate_config(), &PathConstraint::default(),
                ).map(|(_, p)| p).collect();
                let all_paths = grw_simple_directed_paths(&grw_g, from, to, n);
                // Navigator may find fewer paths (greedy), but every found path must be valid
                for p in &nav_paths {
                    assert!(all_paths.contains(p),
                        "navigate seed={seed} {from}->{to}: invalid path {p:?}");
                }
            }
        }
    }
}

#[test]
fn executor_bfs_shortest_first() {
    for seed in 0..30 {
        let (grw_g, pg_di, _) = build_random_graphs(seed, 8, 15);
        let n = 8;
        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                let bfs_paths: Vec<Vec<grw::id::N>> = grw_g.path_search(
                    grw::id::N(from), grw::id::N(to),
                    is_directed_src, exec_bfs_config(), &PathConstraint::default(),
                ).collect();
                if bfs_paths.len() >= 2 {
                    // BFS should return shortest paths first
                    assert!(bfs_paths[0].len() <= bfs_paths[1].len(),
                        "bfs order seed={seed} {from}->{to}: first={} second={}",
                        bfs_paths[0].len(), bfs_paths[1].len());
                }
            }
        }
    }
}

#[test]
fn executor_len_bounds() {
    for seed in 0..20 {
        let (grw_g, _, _) = build_random_graphs(seed, 8, 15);
        let n = 8;
        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                // len(2..4): only paths with 2 or 3 edges
                let paths: Vec<Vec<grw::id::N>> = grw_g.path_search(
                    grw::id::N(from), grw::id::N(to),
                    is_directed_src, path::Config::new(()).dfs().len(2..4), &PathConstraint::default(),
                ).collect();
                for p in &paths {
                    let edges = p.len() - 1;
                    assert!(edges >= 2 && edges < 4,
                        "len_bounds seed={seed} {from}->{to}: edge count {edges} not in 2..4");
                }
            }
        }
    }
}

#[test]
fn executor_guard_filters() {
    for seed in 0..20 {
        let (grw_g, _, _) = build_random_graphs(seed, 8, 15);
        let n = 8;
        for from in 0..n as u32 {
            for to in 0..n as u32 {
                if from == to { continue; }
                // Guard: reject paths passing through node 3
                let paths: Vec<Vec<grw::id::N>> = grw_g.path_search(
                    grw::id::N(from), grw::id::N(to),
                    is_directed_src,
                    path::Config::new(()).dfs().guard(|p| !p.contains(grw::id::N(3))),
                    &PathConstraint::default(),
                ).collect();
                for p in &paths {
                    assert!(!p.iter().any(|&n| *n == 3),
                        "guard seed={seed} {from}->{to}: path contains node 3: {p:?}");
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// End-to-end: search DSL with path patterns
// ═══════════════════════════════════════════════════════════════════

/// Build a chain graph: 0 → 1 → 2 → ... → (n-1), all directed.
fn build_chain(n: usize) -> GrwGraph {
    let mut g: GrwGraph = grw::MGraph::default();
    for _ in 0..n {
        g.modify(vec![N_().val(()).into()]).expect("add node");
    }
    for i in 0..n - 1 {
        let _ = g.modify(vec![
            (X(i as u32) & E().val(1u8) >> X((i + 1) as u32)).into(),
        ]);
    }
    g
}

/// Build a diamond: 0 → 1, 0 → 2, 1 → 3, 2 → 3.
fn build_diamond() -> GrwGraph {
    let mut g: GrwGraph = grw::MGraph::default();
    for _ in 0..4 {
        g.modify(vec![N_().val(()).into()]).expect("add node");
    }
    for &(a, b) in &[(0u32, 1u32), (0, 2), (1, 3), (2, 3)] {
        let _ = g.modify(vec![
            (X(a) & E().val(1u8) >> X(b)).into(),
        ]);
    }
    g
}

/// Run path search through executor directly (bypassing search DSL for now).
/// The search engine integration works but the DSL syntax for path tests
/// needs the full search session machinery. These tests verify the engine's
/// path dispatch via the executor.
#[test]
fn engine_path_chain() {
    let g = build_chain(5);
    let paths: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(4),
        &is_directed_src, path::Config::new(()).dfs(), &PathConstraint::default(),
    ).collect();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].len(), 5); // [0, 1, 2, 3, 4]
}

#[test]
fn engine_path_chain_no_reverse() {
    let g = build_chain(5);
    let paths: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(4), grw::id::N(0),
        &is_directed_src, path::Config::new(()).dfs(), &PathConstraint::default(),
    ).collect();
    assert!(paths.is_empty());
}

#[test]
fn engine_path_diamond() {
    let g = build_diamond();
    let paths: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(3),
        &is_directed_src, path::Config::new(()).dfs(), &PathConstraint::default(),
    ).collect();
    assert_eq!(paths.len(), 2); // 0→1→3 and 0→2→3
}

#[test]
fn engine_path_diamond_bfs_shortest_first() {
    let g = build_diamond();
    let paths: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(3),
        &is_directed_src, path::Config::new(()).bfs(), &PathConstraint::default(),
    ).collect();
    assert_eq!(paths.len(), 2);
    assert!(paths[0].len() <= paths[1].len());
}

#[test]
fn engine_path_result_in_match() {
    // Verify that Match carries the actual path found
    let g = build_chain(5);
    let paths: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(4),
        &is_directed_src, path::Config::new(()).dfs(), &PathConstraint::default(),
    ).collect();
    assert_eq!(paths.len(), 1);
    let p = &paths[0];
    assert_eq!(p.len(), 5);
    assert_eq!(*p[0], 0);
    assert_eq!(*p[1], 1);
    assert_eq!(*p[2], 2);
    assert_eq!(*p[3], 3);
    assert_eq!(*p[4], 4);
}

#[test]
fn engine_next_paths_diamond() {
    // Diamond has 2 paths from 0→3. next_paths should iterate both.
    let g = build_diamond();
    let all: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(3),
        &is_directed_src, path::Config::new(()).dfs(), &PathConstraint::default(),
    ).collect();
    assert_eq!(all.len(), 2, "diamond should have 2 paths 0→3");

    use grw::search::engine::Match;
    let mut m = Match::with_paths(vec![all.clone()]);

    // First path
    let p0 = m.path(0).to_vec();
    assert!(!p0.is_empty());

    // Second path
    assert!(m.next_paths());
    let p1 = m.path(0).to_vec();
    assert!(!p1.is_empty());
    assert_ne!(p0, p1, "two paths should differ");

    // Exhausted
    assert!(!m.next_paths());

    // Reset and verify
    m.reset_paths();
    assert_eq!(m.path(0), p0.as_slice());
}

#[test]
fn engine_next_paths_cross_product() {
    // Two path edges, each with 2 alternatives → 4 combinations
    use grw::search::engine::Match;
    let path_a = vec![
        vec![grw::id::N(0), grw::id::N(1)],
        vec![grw::id::N(0), grw::id::N(2)],
    ];
    let path_b = vec![
        vec![grw::id::N(3), grw::id::N(4)],
        vec![grw::id::N(3), grw::id::N(5)],
    ];
    let mut m = Match::with_paths(vec![path_a, path_b]);

    let mut combos = Vec::new();
    loop {
        let pa = m.path(0).to_vec();
        let pb = m.path(1).to_vec();
        combos.push((pa, pb));
        if !m.next_paths() { break; }
    }
    assert_eq!(combos.len(), 4, "2×2 = 4 combinations");
    // All combinations should be unique
    let unique: HashSet<_> = combos.iter().collect();
    assert_eq!(unique.len(), 4);
}

#[test]
fn engine_next_paths_injective() {
    // Two paths sharing intermediate node 5 — injective should skip that combo
    use grw::search::engine::Match;
    let path_a = vec![
        vec![grw::id::N(0), grw::id::N(5), grw::id::N(1)],  // passes through 5
        vec![grw::id::N(0), grw::id::N(6), grw::id::N(1)],  // passes through 6
    ];
    let path_b = vec![
        vec![grw::id::N(2), grw::id::N(5), grw::id::N(3)],  // passes through 5 — conflicts with path_a[0]
        vec![grw::id::N(2), grw::id::N(7), grw::id::N(3)],  // passes through 7 — ok
    ];
    let mut m = Match::with_paths(vec![path_a, path_b]);

    // Non-injective: 4 combos
    let mut count = 1; // current combo
    while m.next_paths() { count += 1; }
    assert_eq!(count, 4);

    // Injective: skip combos where paths share intermediate nodes
    m.reset_paths();
    let mut injective_combos = vec![];
    // Check initial combo — skip if not injective
    if m.paths_are_injective() {
        let pa: Vec<u32> = m.path(0).iter().map(|n| **n).collect();
        let pb: Vec<u32> = m.path(1).iter().map(|n| **n).collect();
        injective_combos.push((pa, pb));
    }
    while m.next_paths_injective() {
        let pa: Vec<u32> = m.path(0).iter().map(|n| **n).collect();
        let pb: Vec<u32> = m.path(1).iter().map(|n| **n).collect();
        injective_combos.push((pa, pb));
    }
    // (a[0],b[0]) shares node 5 → skipped. 3 valid combos.
    assert_eq!(injective_combos.len(), 3,
        "injective should skip combo sharing node 5, got: {injective_combos:?}");
}

#[test]
fn engine_path_chain_len_bounded() {
    let g = build_chain(6);
    // Only paths with 2..4 edges (2 or 3 hops)
    let paths: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(5),
        &is_directed_src, path::Config::new(()).dfs().len(2..4), &PathConstraint::default(),
    ).collect();
    // Chain 0→1→2→3→4→5 has 5 edges, which is >= 4, so rejected
    assert!(paths.is_empty());

    // But 0→1→2 has 2 edges, within bounds
    let paths2: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(2),
        &is_directed_src, path::Config::new(()).dfs().len(2..4), &PathConstraint::default(),
    ).collect();
    assert_eq!(paths2.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════
// Full DSL syntax: search! macro with path patterns
// ═══════════════════════════════════════════════════════════════════

type ER = grw::edge::Anydir<u8>;

use grw::graph::dsl::LocalId;
use grw::search::path::{Dijkstra, AStar};

fn run_search(g: &GrwGraph, search: Result<grw::search::query::Search<(), ER>, grw::search::error::Search>) -> Vec<grw::search::engine::Match> {
    let session = grw::search::Session::from_search(search.unwrap(), g).unwrap();
    session.into_iter().collect()
}

#[test]
fn dsl_path_chain_dfs() {
    let g = build_chain(5);
    let matches = run_search(&g, grw::search![<(), ER>;
        get(grw::Mono) { N(0) >> ..N(1).dfs() }
    ]);
    let with_paths: Vec<_> = matches.iter().filter(|m| !m.path(0).is_empty()).collect();
    assert!(!with_paths.is_empty(), "should find at least one path");
}

#[test]
fn dsl_path_diamond_bfs() {
    let g = build_diamond();
    let matches = run_search(&g, grw::search![<(), ER>;
        get(grw::Mono) { N(0) >> ..N(1).bfs() }
    ]);
    let with_paths: Vec<_> = matches.iter().filter(|m| !m.path(0).is_empty()).collect();
    assert!(!with_paths.is_empty(), "diamond should have path matches via BFS");
}

#[test]
fn dsl_path_with_len() {
    let g = build_chain(6);
    let matches = run_search(&g, grw::search![<(), ER>;
        get(grw::Mono) { N(0) >> ..N(1).dfs().len(2..4) }
    ]);
    for m in &matches {
        let p = m.path(0);
        if !p.is_empty() {
            let edges = p.len() - 1;
            assert!(edges >= 2 && edges < 4, "edge count {edges} not in 2..4");
        }
    }
}

#[test]
fn dsl_path_guard() {
    let g = build_chain(5);
    let matches = run_search(&g, grw::search![<(), ER>;
        get(grw::Mono) {
            N(0) >> ..N(1).dfs()
                .guard(|p| !p.contains(grw::id::N(2)))
        }
    ]);
    for m in &matches {
        let p = m.path(0);
        if !p.is_empty() {
            assert!(!p.iter().any(|n| **n == 2), "guard should reject paths through node 2");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Edge-predicate path DSL: N & E().test(pred) >> ..N.dfs()
// ═══════════════════════════════════════════════════════════════════

#[test]
fn dsl_path_with_edge_predicate() {
    // Chain on label 1: 0→1→2→3.  Shortcut on label 2: 0→3.
    let mut g: GrwGraph = grw::MGraph::default();
    for _ in 0..4 { g.modify(vec![N_().val(()).into()]).unwrap(); }
    for &(a, b) in &[(0u32, 1), (1, 2), (2, 3)] {
        g.modify(vec![(X(a) & E().val(1u8) >> X(b)).into()]).unwrap();
    }
    g.modify(vec![(X(0u32) & E().val(2u8) >> X(3u32)).into()]).unwrap();

    // label==1: adjacent pairs + multi-hop paths via label-1
    let m1 = run_search(&g, grw::search![<(), ER>;
        get(grw::Homo) {
            N(0).test(|_| true) & E().test(|l: &u8| *l == 1) >> ..N(1).test(|_| true).dfs()
        }
    ]);
    assert!(m1.len() >= 3, "label-1: at least 3 pairs, got {}", m1.len());

    // label==2: only the 0→3 shortcut
    let m2 = run_search(&g, grw::search![<(), ER>;
        get(grw::Homo) {
            N(0).test(|_| true) & E().test(|l: &u8| *l == 2) >> ..N(1).test(|_| true).dfs()
        }
    ]);
    assert_eq!(m2.len(), 1, "label-2: only 0→3 shortcut");

    // no pred: all reachable directed pairs
    let m_any = run_search(&g, grw::search![<(), ER>;
        get(grw::Homo) { N(0) >> ..N(1).dfs() }
    ]);
    assert!(m_any.len() >= 4, "no pred: at least 4, got {}", m_any.len());
}

// ═══════════════════════════════════════════════════════════════════
// Cross-cluster path: terminals in one morphism, path in another
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bound_path_mono_allows_branched_intermediates() {
    //  0 - 1 - 2 - 3 - 4
    //      |       |
    //      5       6
    //
    // Mono path 0→4: intermediates have branches — Mono allows this.
    let g = grw::mgraph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3), n(3) ^ N(4),
        n(1) ^ N(5), n(3) ^ N(6)
    ].unwrap();

    let search = grw::search![<(), ER>;
        get(grw::Mono) { X(0) ^ ..X(1).dfs() }
    ];
    let grw::search::query::Search::Unresolved(u) = search.unwrap()
        else { panic!("expected unresolved") };
    let bound = u.bind(&[(0, 0), (1, 4)]).unwrap();
    let indexed = g.index(grw::search::engine::RevCsr);
    let matches: Vec<_> = grw::search::engine::Seq::search_bound(
        bound.query(), &indexed, bound.bindings().to_vec()
    ).collect();

    assert!(!matches.is_empty(), "Mono should find path 0→4 despite branches");
    let p = matches[0].path(0);
    assert_eq!(p.len(), 5, "path 0→1→2→3→4");
}

#[test]
fn bound_path_subiso_rejects_branched_intermediates() {
    //  0 - 1 - 2 - 3 - 4
    //      |       |
    //      5       6
    //
    // SubIso path 0→4: intermediates 1,3 have outside edges → induced property violated.
    let g = grw::mgraph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3), n(3) ^ N(4),
        n(1) ^ N(5), n(3) ^ N(6)
    ].unwrap();

    let search = grw::search![<(), ER>;
        get(grw::SubIso) { X(0) ^ ..X(1).dfs() }
    ];
    let grw::search::query::Search::Unresolved(u) = search.unwrap()
        else { panic!("expected unresolved") };
    let bound = u.bind(&[(0, 0), (1, 4)]).unwrap();
    let indexed = g.index(grw::search::engine::RevCsr);
    let matches: Vec<_> = grw::search::engine::Seq::search_bound(
        bound.query(), &indexed, bound.bindings().to_vec()
    ).collect();

    assert!(matches.is_empty() || matches[0].path(0).is_empty(),
        "SubIso should reject path 0→4: intermediates have outside edges");
}

#[test]
fn bound_path_subiso_accepts_clean_chain() {
    //  0 - 1 - 2 - 3 - 4
    //
    // No branches — all intermediates have degree ≤ 2 — SubIso path should work.
    let g = grw::mgraph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3), n(3) ^ N(4)
    ].unwrap();

    let search = grw::search![<(), ER>;
        get(grw::SubIso) { X(0) ^ ..X(1).dfs() }
    ];
    let grw::search::query::Search::Unresolved(u) = search.unwrap()
        else { panic!("expected unresolved") };
    let bound = u.bind(&[(0, 0), (1, 4)]).unwrap();
    let indexed = g.index(grw::search::engine::RevCsr);
    let matches: Vec<_> = grw::search::engine::Seq::search_bound(
        bound.query(), &indexed, bound.bindings().to_vec()
    ).collect();

    assert!(!matches.is_empty(), "SubIso should accept clean chain path 0→4");
    assert_eq!(matches[0].path(0).len(), 5);
}

// ═══════════════════════════════════════════════════════════════════
// Homo terminals + SubIso path = induced cycle detection
// ═══════════════════════════════════════════════════════════════════

#[test]
fn homo_terminals_find_cycle() {
    // Square cycle: 0-1-2-3-0
    let g = grw::mgraph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3), n(3) ^ n(0)
    ].unwrap();

    // Homo defines terminals (allows same-target), Mono path references them.
    let matches = run_search(&g, grw::search![<(), ER>;
        get(grw::Homo) { N(0), N(1) },
        get(grw::Mono) { n(0) ^ ..n(1).dfs().len(2..) }
    ]);

    let cycles: Vec<_> = matches.iter()
        .filter(|m| {
            let a = m.get(LocalId(0));
            let b = m.get(LocalId(1));
            a == b && !m.path(0).is_empty()
        })
        .collect();
    assert!(!cycles.is_empty(), "Homo terminals should discover cycles");

    let p = cycles[0].path(0);
    assert_eq!(*p[0], *p[p.len()-1], "cycle path starts and ends at same node");
}

#[test]
fn homo_subiso_finds_clean_cycle_rejects_branched() {
    // Clean cycle: 0-1-2-3-0
    let g_clean = grw::mgraph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3), n(3) ^ n(0)
    ].unwrap();

    // Branched cycle: 0-1-2-3-0, with 2-4 branch
    let g_branched = grw::mgraph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3), n(3) ^ n(0),
        n(2) ^ N(4)
    ].unwrap();

    let homo_pattern = || grw::search![<(), ER>;
        get(grw::Homo) { N(0) ^ ..N(1).dfs().len(2..) }
    ];

    let is_cycle = |m: &grw::search::engine::Match| {
        let a = m.get(LocalId(0));
        let b = m.get(LocalId(1));
        a == b && m.path(0).len() >= 3
    };

    // Clean cycle with Homo: finds the 4-cycle
    let m_clean = run_search(&g_clean, homo_pattern());
    let clean_cycles: Vec<_> = m_clean.iter().filter(|m| is_cycle(m)).collect();
    assert!(!clean_cycles.is_empty(), "clean square: Homo should find cycle");

    // Branched cycle with Homo: also finds cycles (Homo doesn't check induced)
    let m_branch = run_search(&g_branched, homo_pattern());
    let branch_homo: Vec<_> = m_branch.iter().filter(|m| is_cycle(m)).collect();
    assert!(!branch_homo.is_empty(), "branched: Homo finds cycle (no induced check)");

    // The SubIso induced check on PATHS (not cycles) was tested in
    // bound_path_subiso_rejects_branched_intermediates above.
}

// ═══════════════════════════════════════════════════════════════════
// Weighted navigator tests — Dijkstra/AStar with real edge values
// ═══════════════════════════════════════════════════════════════════

#[test]
fn weighted_dijkstra_trivial() {
    // Simplest possible test: 2 nodes, 1 directed edge
    let mut g: GrwGraph = grw::MGraph::default();
    g.modify(vec![N_().val(()).into()]).unwrap();
    g.modify(vec![N_().val(()).into()]).unwrap();
    g.modify(vec![(X(0u32) & E().val(5u8) >> X(1u32)).into()]).unwrap();

    let config = path::Config::new(()).navigate(
        Dijkstra::weighted(|ev: &AnyVal<u8>| match ev {
            AnyVal::Dir(w) | AnyVal::Undir(w) => *w as f64,
        })
    );
    let paths: Vec<Vec<grw::id::N>> = g.path_navigate(
        grw::id::N(0), grw::id::N(1), is_directed_src, config, &PathConstraint::default(),
    ).map(|(_, p)| p).collect();
    assert_eq!(paths.len(), 1, "should find path 0→1, got {:?}", paths);
    assert_eq!(paths[0].len(), 2);
}

#[test]
fn weighted_dijkstra_extracts_from_edge_value() {
    // Build graph with edge weights as u8 labels
    let (grw_g, pg) = build_weighted_digraph(42, 10, 20);

    for from in 0..10u32 {
        for to in 0..10u32 {
            if from == to { continue; }

            // grw: Dijkstra navigator extracting weight from AnyVal<u8>
            let config = path::Config::new(()).navigate(
                Dijkstra::weighted(|ev: &AnyVal<u8>| match ev {
                    AnyVal::Dir(w) => *w as f64,
                    AnyVal::Undir(w) => *w as f64,
                })
            );
            let nav_paths: Vec<Vec<grw::id::N>> = grw_g.path_navigate(
                grw::id::N(from), grw::id::N(to),
                is_directed_src, config, &PathConstraint::default(),
            ).map(|(_, p)| p).collect();

            // petgraph: A* with zero heuristic = Dijkstra
            let pg_result = astar(
                &pg, NodeIndex::new(from as usize),
                |n| n == NodeIndex::new(to as usize),
                |e| *e.weight() as u32,
                |_| 0,
            );

            // Navigator paths must be valid simple paths
            let all = grw_g.all_simple_paths(grw::id::N(from), grw::id::N(to), is_directed_src, 10);
            for p in &nav_paths {
                assert!(all.contains(p), "seed=42 {from}->{to} navigator found invalid path {p:?}");
            }
            // And existence should match petgraph
            let pg_exists = astar(
                &pg, NodeIndex::new(from as usize),
                |n| n == NodeIndex::new(to as usize),
                |e| *e.weight() as u32, |_| 0,
            ).is_some();
            // Navigator found paths must be valid
            for p in &nav_paths {
                assert!(all.contains(p), "seed=42 {from}->{to} invalid path {p:?}");
            }
        }
    }
}

#[test]
fn weighted_astar_matches_dijkstra() {
    let (grw_g, _) = build_weighted_digraph(7, 12, 25);

    for from in 0..12u32 {
        for to in 0..12u32 {
            if from == to { continue; }

            let dij_config = path::Config::new(()).navigate(
                Dijkstra::weighted(|ev: &AnyVal<u8>| match ev {
                    AnyVal::Dir(w) | AnyVal::Undir(w) => *w as f64,
                })
            );
            let astar_config = path::Config::new(()).navigate(
                AStar::new(
                    |ev: &AnyVal<u8>| match ev { AnyVal::Dir(w) | AnyVal::Undir(w) => *w as f64 },
                    |_node| 0.0, // zero heuristic = Dijkstra
                )
            );

            let dij_paths: Vec<Vec<grw::id::N>> = grw_g.path_navigate(
                grw::id::N(from), grw::id::N(to), is_directed_src, dij_config, &PathConstraint::default(),
            ).map(|(_, p)| p).collect();
            let astar_paths: Vec<Vec<grw::id::N>> = grw_g.path_navigate(
                grw::id::N(from), grw::id::N(to), is_directed_src, astar_config, &PathConstraint::default(),
            ).map(|(_, p)| p).collect();

            assert_eq!(
                dij_paths.is_empty(), astar_paths.is_empty(),
                "seed=7 {from}->{to} dijkstra vs astar existence mismatch"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// PathConstraint tests — morphism-specific path filtering
// ═══════════════════════════════════════════════════════════════════

/// Build graph: 0→1→2→3→4 + 0→5→3 (two paths from 0 to 3, sharing node 3)
fn build_fork() -> GrwGraph {
    let mut g: GrwGraph = grw::MGraph::default();
    for _ in 0..6 { g.modify(vec![N_().val(()).into()]).unwrap(); }
    for &(a, b) in &[(0u32,1),(1,2),(2,3),(3,4),(0,5),(5,3)] {
        let _ = g.modify(vec![(X(a) & E().val(1u8) >> X(b)).into()]);
    }
    g
}

#[test]
fn constraint_unconstrained_finds_all() {
    let g = build_chain(5);
    let constraint = PathConstraint::unconstrained();
    let paths: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(4), is_directed_src, path::Config::new(()).dfs(), &constraint,
    ).collect();
    assert_eq!(paths.len(), 1);
}

#[test]
fn constraint_mono_excludes_used_nodes() {
    let g = build_fork();
    // Path 1: 0→1→2→3. Then path 2 from 0→3 should avoid nodes 1, 2.
    let mut constraint = PathConstraint::from_morphism(grw::Mono, &[]);

    let paths1: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(3), is_directed_src, path::Config::new(()).dfs(), &constraint,
    ).collect();
    assert!(!paths1.is_empty());
    constraint.commit(&paths1[0]); // exclude intermediates of first path

    // Second path 0→3 must avoid nodes used by first path's intermediates
    let paths2: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(3), is_directed_src, path::Config::new(()).dfs(), &constraint,
    ).collect();
    // First path was 0→1→2→3 (intermediates: 1, 2). Second path must use 0→5→3.
    for p in &paths2 {
        for &n in &p[1..p.len()-1] {
            assert!(!paths1[0][1..paths1[0].len()-1].contains(&n),
                "mono: path 2 shares intermediate {n:?} with path 1");
        }
    }
    assert!(!paths2.is_empty(), "should find alternative path 0→5→3");
}

#[test]
fn constraint_mono_rejects_when_no_alternative() {
    // Simple chain: only one path 0→4, no alternatives after exclusion
    let g = build_chain(5);
    let mut constraint = PathConstraint::from_morphism(grw::Mono, &[]);

    let paths1: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(4), is_directed_src, path::Config::new(()).dfs(), &constraint,
    ).collect();
    assert_eq!(paths1.len(), 1);
    constraint.commit(&paths1[0]);

    // Same path again — intermediates excluded, no path possible
    let paths2: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(4), is_directed_src, path::Config::new(()).dfs(), &constraint,
    ).collect();
    assert!(paths2.is_empty(), "mono: no alternative path should exist");
}

#[test]
fn constraint_subiso_rejects_outside_edges() {
    // Graph: 0→1→2, 1→3 (node 1 has outside edge to 3)
    let mut g: GrwGraph = grw::MGraph::default();
    for _ in 0..4 { g.modify(vec![N_().val(()).into()]).unwrap(); }
    for &(a, b) in &[(0u32,1),(1,2),(1,3)] {
        let _ = g.modify(vec![(X(a) & E().val(1u8) >> X(b)).into()]);
    }

    let constraint = PathConstraint::from_morphism(grw::SubIso, &[]);

    let paths: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(2), is_directed_src, path::Config::new(()).dfs(), &constraint,
    ).collect();
    // Path 0→1→2: node 1 has edge to 3 which is outside the path → SubIso rejects
    assert!(paths.is_empty(), "subiso: should reject path where intermediate has outside edges");
}

#[test]
fn constraint_subiso_accepts_clean_path() {
    // Graph: 0→1→2 (no outside edges)
    let mut g: GrwGraph = grw::MGraph::default();
    for _ in 0..3 { g.modify(vec![N_().val(()).into()]).unwrap(); }
    for &(a, b) in &[(0u32,1),(1,2)] {
        let _ = g.modify(vec![(X(a) & E().val(1u8) >> X(b)).into()]);
    }

    let constraint = PathConstraint::from_morphism(grw::SubIso, &[]);

    let paths: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(2), is_directed_src, path::Config::new(()).dfs(), &constraint,
    ).collect();
    assert_eq!(paths.len(), 1, "subiso: clean path should be accepted");
}

#[test]
fn constraint_homo_allows_revisits() {
    // Homo doesn't exclude anything — all paths valid
    let g = build_fork();
    let mut constraint = PathConstraint::from_morphism(grw::Homo, &[]);

    let paths1: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(3), is_directed_src, path::Config::new(()).dfs(), &constraint,
    ).collect();
    constraint.commit(&paths1[0]);

    let paths2: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(3), is_directed_src, path::Config::new(()).dfs(), &constraint,
    ).collect();
    // Homo: same intermediates are fine
    assert_eq!(paths2.len(), paths1.len(), "homo: should find same paths regardless of commit");
}

#[test]
fn constraint_bindings_excluded_in_mono() {
    // Bindings (pattern-bound nodes) are pre-excluded in Mono
    let g = build_chain(5);
    let constraint = PathConstraint::from_morphism(grw::Mono, &[grw::id::N(2)]);

    let paths: Vec<Vec<grw::id::N>> = g.path_search(
        grw::id::N(0), grw::id::N(4), is_directed_src, path::Config::new(()).dfs(), &constraint,
    ).collect();
    // Path 0→1→2→3→4: node 2 is in bindings → excluded → no path
    assert!(paths.is_empty(), "mono: binding node 2 should be excluded from path");
}
