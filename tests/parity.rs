#![cfg(feature = "parity")]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use petgraph::graph::{NodeIndex, UnGraph};
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::SmallRng;

use grw::graph::edge;
use grw::graph::edge::undir;
use grw::search::Morphism;
use grw::search::dsl;
use grw::search::engine::Match;
use grw::search::{self, Search};
use grw::Id;

type ER = edge::Undir<()>;
type ShagraGraph = grw::graph::Undir0;

// ── Graph construction helpers ───────────────────────────────────────

fn random_graph(seed: u64, nodes: Id, avg_degree: Id) -> ShagraGraph {
    let target_edges = (nodes as u64 * avg_degree as u64) / 2;
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut seen = BTreeSet::new();

    while (seen.len() as u64) < target_edges {
        let a: Id = rng.random_range(0..nodes);
        let b: Id = rng.random_range(0..nodes);
        if a == b {
            continue;
        }
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        seen.insert((lo, hi));
    }

    let edges: Vec<undir::E<Id>> = seen.into_iter().map(|(a, b)| undir::E::U(a, b)).collect();
    let node_ids: Vec<Id> = (0..nodes).collect();
    ShagraGraph::try_from((node_ids, edges)).unwrap()
}

fn extract_connected_subgraph(seed: u64, graph: &ShagraGraph, k: usize) -> ShagraGraph {
    let (all_nodes, all_edges) = graph.to_vecs();

    let mut rng = SmallRng::seed_from_u64(seed);
    let start_idx = rng.random_range(0..all_nodes.len());
    let start = all_nodes[start_idx].0;

    let mut adj: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
    for (edef, _) in &all_edges {
        let undir::E::U(a, b) = edef;
        adj.entry(*a).or_default().push(*b);
        adj.entry(*b).or_default().push(*a);
    }

    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();
    visited.insert(start);
    queue.push_back(start);

    while visited.len() < k {
        let Some(current) = queue.pop_front() else {
            break;
        };
        if let Some(neighbors) = adj.get(&current) {
            for &n in neighbors {
                if visited.len() >= k {
                    break;
                }
                if visited.insert(n) {
                    queue.push_back(n);
                }
            }
        }
    }

    let sub_nodes: Vec<Id> = visited.iter().copied().collect();
    let sub_edges: Vec<undir::E<Id>> = all_edges
        .iter()
        .filter_map(|(edef, _)| {
            let undir::E::U(a, b) = edef;
            if visited.contains(a) && visited.contains(b) {
                Some(undir::E::U(*a, *b))
            } else {
                None
            }
        })
        .collect();

    ShagraGraph::try_from((sub_nodes, sub_edges)).unwrap()
}

// ── Extraction helpers ───────────────────────────────────────────────

fn extract_edge_pairs(g: &ShagraGraph) -> Vec<(Id, Id)> {
    let (_, edges) = g.to_vecs();
    edges
        .iter()
        .map(|(edef, _)| {
            let undir::E::U(a, b) = edef;
            (*a, *b)
        })
        .collect()
}

fn extract_node_ids(g: &ShagraGraph) -> Vec<Id> {
    let (nodes, _) = g.to_vecs();
    nodes.iter().map(|(id, _)| *id).collect()
}

// ── Shagra query helpers ─────────────────────────────────────────────

fn normalize_grw(m: &Match, pattern_local_ids: &[Id]) -> BTreeMap<Id, Id> {
    let mut result = BTreeMap::new();
    for &lid in pattern_local_ids {
        result.insert(lid, *m.node(lid));
    }
    result
}

fn build_pattern(
    morphism: Morphism,
    edges: &[(Id, Id)],
    all_node_ids: &[Id],
) -> Vec<dsl::ClusterOps<(), ER>> {
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<(), ER>> = Vec::new();

    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);

        let op: dsl::Op<(), ER> = match (a_new, b_new) {
            (true, true) => (dsl::N::<(), ER>(a) ^ dsl::N::<(), ER>(b)).into(),
            (true, false) => (dsl::N::<(), ER>(a) ^ dsl::n::<(), ER>(b)).into(),
            (false, true) => (dsl::n::<(), ER>(a) ^ dsl::N::<(), ER>(b)).into(),
            (false, false) => (dsl::n::<(), ER>(a) ^ dsl::n::<(), ER>(b)).into(),
        };
        ops.push(op);
    }

    for &id in all_node_ids {
        if seen.insert(id) {
            ops.push(dsl::N::<(), ER>(id).into());
        }
    }

    vec![dsl::get(morphism, ops)]
}

fn grw_matches_with(
    morphism: Morphism,
    edges: &[(Id, Id)],
    target: &ShagraGraph,
    pattern_local_ids: &[Id],
) -> BTreeSet<BTreeMap<Id, Id>> {
    let clusters = build_pattern(morphism, edges, pattern_local_ids);
    let Search::Resolved(r) = search::compile::<(), ER>(clusters).unwrap()
    else { panic!("unexpected context nodes") };
    let query = r.into_query();
    let indexed = target.index(search::RevCsr);
    search::Seq::search(&query, &indexed)
        .map(|m| normalize_grw(&m, pattern_local_ids))
        .collect()
}

// ── Petgraph helpers ─────────────────────────────────────────────────

fn to_petgraph(g: &ShagraGraph) -> (UnGraph<(), ()>, Vec<Id>) {
    let (nodes, edges) = g.to_vecs();
    let mut pg = UnGraph::<(), ()>::new_undirected();
    let mut grw_ids: Vec<Id> = Vec::new();
    let mut id_to_pg: BTreeMap<Id, NodeIndex> = BTreeMap::new();

    let mut sorted_nodes: Vec<Id> = nodes.into_iter().map(|(id, _)| id).collect();
    sorted_nodes.sort();

    for id in sorted_nodes {
        let idx = pg.add_node(());
        grw_ids.push(id);
        id_to_pg.insert(id, idx);
    }

    for (edef, _) in &edges {
        let undir::E::U(a, b) = edef;
        pg.add_edge(id_to_pg[a], id_to_pg[b], ());
    }

    (pg, grw_ids)
}

fn normalize_petgraph(
    mapping: &[usize],
    pattern_ids: &[Id],
    target_ids: &[Id],
) -> BTreeMap<Id, Id> {
    let mut result = BTreeMap::new();
    for (i, &target_idx) in mapping.iter().enumerate() {
        result.insert(pattern_ids[i], target_ids[target_idx]);
    }
    result
}

fn petgraph_matches(
    pattern: &UnGraph<(), ()>,
    target: &UnGraph<(), ()>,
    pattern_ids: &[Id],
    target_ids: &[Id],
) -> BTreeSet<BTreeMap<Id, Id>> {
    let mut nm = |_: &(), _: &()| true;
    let mut em = |_: &(), _: &()| true;
    let pattern_ref = pattern;
    let target_ref = target;
    match petgraph::algo::isomorphism::subgraph_isomorphisms_iter(
        &pattern_ref, &target_ref, &mut nm, &mut em,
    ) {
        Some(iter) => iter
            .map(|mapping| normalize_petgraph(&mapping, pattern_ids, target_ids))
            .collect(),
        None => BTreeSet::new(),
    }
}

// ── Brute-force oracle ───────────────────────────────────────────────

fn oracle_mappings(
    morphism: Morphism,
    pattern_nodes: &[Id],
    pattern_edges: &[(Id, Id)],
    target: &ShagraGraph,
) -> BTreeSet<BTreeMap<Id, Id>> {
    let target_nodes = extract_node_ids(target);
    let is_injective = matches!(morphism, Morphism::Iso | Morphism::SubIso | Morphism::Mono);
    let check_non_edges = matches!(morphism, Morphism::Iso | Morphism::SubIso);

    if morphism == Morphism::Iso && pattern_nodes.len() != target_nodes.len() {
        return BTreeSet::new();
    }

    let pattern_edge_set: BTreeSet<(Id, Id)> = pattern_edges
        .iter()
        .map(|&(a, b)| (a.min(b), a.max(b)))
        .collect();

    let mut results = BTreeSet::new();
    let mut mapping = BTreeMap::new();
    let mut used = BTreeSet::new();

    oracle_backtrack(
        pattern_nodes,
        &pattern_edge_set,
        &target_nodes,
        target,
        is_injective,
        check_non_edges,
        0,
        &mut mapping,
        &mut used,
        &mut results,
    );

    results
}

fn oracle_backtrack(
    pattern_nodes: &[Id],
    pattern_edges: &BTreeSet<(Id, Id)>,
    target_nodes: &[Id],
    target: &ShagraGraph,
    is_injective: bool,
    check_non_edges: bool,
    depth: usize,
    mapping: &mut BTreeMap<Id, Id>,
    used: &mut BTreeSet<Id>,
    results: &mut BTreeSet<BTreeMap<Id, Id>>,
) {
    if depth == pattern_nodes.len() {
        if oracle_validate(mapping, pattern_nodes, pattern_edges, target, check_non_edges) {
            results.insert(mapping.clone());
        }
        return;
    }

    let pnode = pattern_nodes[depth];

    for &tnode in target_nodes {
        if is_injective && used.contains(&tnode) {
            continue;
        }

        mapping.insert(pnode, tnode);
        if is_injective {
            used.insert(tnode);
        }

        oracle_backtrack(
            pattern_nodes,
            pattern_edges,
            target_nodes,
            target,
            is_injective,
            check_non_edges,
            depth + 1,
            mapping,
            used,
            results,
        );

        mapping.remove(&pnode);
        if is_injective {
            used.remove(&tnode);
        }
    }
}

fn oracle_validate(
    mapping: &BTreeMap<Id, Id>,
    pattern_nodes: &[Id],
    pattern_edges: &BTreeSet<(Id, Id)>,
    target: &ShagraGraph,
    check_non_edges: bool,
) -> bool {
    for &(pa, pb) in pattern_edges {
        let ta = mapping[&pa];
        let tb = mapping[&pb];
        if !target.has(undir::E::U(ta, tb)) {
            return false;
        }
    }

    if check_non_edges {
        for i in 0..pattern_nodes.len() {
            for j in (i + 1)..pattern_nodes.len() {
                let pa = pattern_nodes[i];
                let pb = pattern_nodes[j];
                let key = (pa.min(pb), pa.max(pb));
                if !pattern_edges.contains(&key) {
                    let ta = mapping[&pa];
                    let tb = mapping[&pb];
                    if target.has(undir::E::U(ta, tb)) {
                        return false;
                    }
                }
            }
        }
    }

    true
}

fn oracle_neg_mappings(
    morphism: Morphism,
    pattern_nodes: &[Id],
    pattern_edges: &[(Id, Id)],
    negated_edges: &[(Id, Id)],
    target: &ShagraGraph,
) -> BTreeSet<BTreeMap<Id, Id>> {
    let target_nodes = extract_node_ids(target);
    let is_injective = matches!(morphism, Morphism::Iso | Morphism::SubIso | Morphism::Mono);
    let check_non_edges = matches!(morphism, Morphism::Iso | Morphism::SubIso);

    if morphism == Morphism::Iso && pattern_nodes.len() != target_nodes.len() {
        return BTreeSet::new();
    }

    let pattern_edge_set: BTreeSet<(Id, Id)> = pattern_edges
        .iter()
        .map(|&(a, b)| (a.min(b), a.max(b)))
        .collect();

    let negated_edge_set: BTreeSet<(Id, Id)> = negated_edges
        .iter()
        .map(|&(a, b)| (a.min(b), a.max(b)))
        .collect();

    let mut results = BTreeSet::new();
    let mut mapping = BTreeMap::new();
    let mut used = BTreeSet::new();

    oracle_neg_backtrack(
        pattern_nodes,
        &pattern_edge_set,
        &negated_edge_set,
        &target_nodes,
        target,
        is_injective,
        check_non_edges,
        0,
        &mut mapping,
        &mut used,
        &mut results,
    );

    results
}

fn oracle_neg_backtrack(
    pattern_nodes: &[Id],
    pattern_edges: &BTreeSet<(Id, Id)>,
    negated_edges: &BTreeSet<(Id, Id)>,
    target_nodes: &[Id],
    target: &ShagraGraph,
    is_injective: bool,
    check_non_edges: bool,
    depth: usize,
    mapping: &mut BTreeMap<Id, Id>,
    used: &mut BTreeSet<Id>,
    results: &mut BTreeSet<BTreeMap<Id, Id>>,
) {
    if depth == pattern_nodes.len() {
        if oracle_neg_validate(
            mapping,
            pattern_nodes,
            pattern_edges,
            negated_edges,
            target,
            check_non_edges,
        ) {
            results.insert(mapping.clone());
        }
        return;
    }

    let pnode = pattern_nodes[depth];

    for &tnode in target_nodes {
        if is_injective && used.contains(&tnode) {
            continue;
        }

        mapping.insert(pnode, tnode);
        if is_injective {
            used.insert(tnode);
        }

        oracle_neg_backtrack(
            pattern_nodes,
            pattern_edges,
            negated_edges,
            target_nodes,
            target,
            is_injective,
            check_non_edges,
            depth + 1,
            mapping,
            used,
            results,
        );

        mapping.remove(&pnode);
        if is_injective {
            used.remove(&tnode);
        }
    }
}

fn oracle_neg_validate(
    mapping: &BTreeMap<Id, Id>,
    pattern_nodes: &[Id],
    pattern_edges: &BTreeSet<(Id, Id)>,
    negated_edges: &BTreeSet<(Id, Id)>,
    target: &ShagraGraph,
    check_non_edges: bool,
) -> bool {
    for &(pa, pb) in pattern_edges {
        let ta = mapping[&pa];
        let tb = mapping[&pb];
        if !target.has(undir::E::U(ta, tb)) {
            return false;
        }
    }

    for &(pa, pb) in negated_edges {
        let ta = mapping[&pa];
        let tb = mapping[&pb];
        if target.has(undir::E::U(ta, tb)) {
            return false;
        }
    }

    if check_non_edges {
        for i in 0..pattern_nodes.len() {
            for j in (i + 1)..pattern_nodes.len() {
                let pa = pattern_nodes[i];
                let pb = pattern_nodes[j];
                let key = (pa.min(pb), pa.max(pb));
                if !pattern_edges.contains(&key) && !negated_edges.contains(&key) {
                    let ta = mapping[&pa];
                    let tb = mapping[&pb];
                    if target.has(undir::E::U(ta, tb)) {
                        return false;
                    }
                }
            }
        }
    }

    true
}

fn build_neg_pattern(
    morphism: Morphism,
    edges: &[(Id, Id)],
    negated_edges: &[(Id, Id)],
    all_node_ids: &[Id],
) -> Vec<dsl::ClusterOps<(), ER>> {
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<(), ER>> = Vec::new();

    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);

        let op: dsl::Op<(), ER> = match (a_new, b_new) {
            (true, true) => (dsl::N::<(), ER>(a) ^ dsl::N::<(), ER>(b)).into(),
            (true, false) => (dsl::N::<(), ER>(a) ^ dsl::n::<(), ER>(b)).into(),
            (false, true) => (dsl::n::<(), ER>(a) ^ dsl::N::<(), ER>(b)).into(),
            (false, false) => (dsl::n::<(), ER>(a) ^ dsl::n::<(), ER>(b)).into(),
        };
        ops.push(op);
    }

    for &(a, b) in negated_edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);

        let op: dsl::Op<(), ER> = match (a_new, b_new) {
            (true, true) => (dsl::N::<(), ER>(a) & !dsl::E::<(), ER>() ^ dsl::N::<(), ER>(b)).into(),
            (true, false) => (dsl::N::<(), ER>(a) & !dsl::E::<(), ER>() ^ dsl::n::<(), ER>(b)).into(),
            (false, true) => (dsl::n::<(), ER>(a) & !dsl::E::<(), ER>() ^ dsl::N::<(), ER>(b)).into(),
            (false, false) => (dsl::n::<(), ER>(a) & !dsl::E::<(), ER>() ^ dsl::n::<(), ER>(b)).into(),
        };
        ops.push(op);
    }

    for &id in all_node_ids {
        if seen.insert(id) {
            ops.push(dsl::N::<(), ER>(id).into());
        }
    }

    vec![dsl::get(morphism, ops)]
}

fn grw_neg_matches_with(
    morphism: Morphism,
    edges: &[(Id, Id)],
    negated_edges: &[(Id, Id)],
    target: &ShagraGraph,
    pattern_local_ids: &[Id],
) -> BTreeSet<BTreeMap<Id, Id>> {
    let clusters = build_neg_pattern(morphism, edges, negated_edges, pattern_local_ids);
    let Search::Resolved(r) = search::compile::<(), ER>(clusters).unwrap()
    else { panic!("unexpected context nodes") };
    let query = r.into_query();
    let indexed = target.index(search::RevCsr);
    search::Seq::search(&query, &indexed)
        .map(|m| normalize_grw(&m, pattern_local_ids))
        .collect()
}

// ── Structural validators ────────────────────────────────────────────

fn validate_match_edges(
    mapping: &BTreeMap<Id, Id>,
    pattern_edges: &[(Id, Id)],
    target: &ShagraGraph,
) -> bool {
    for &(pa, pb) in pattern_edges {
        let ta = mapping[&pa];
        let tb = mapping[&pb];
        if !target.is_adjacent(ta, tb) {
            return false;
        }
    }
    true
}


// ═════════════════════════════════════════════════════════════════════
// Tier 0 — Fixed-case smoke tests
// ═════════════════════════════════════════════════════════════════════

// ── Iso ────────────────────────────────────────────────────────────────

#[test]
fn iso_triangle_six_automorphisms() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(0) ^ n(2)
    ].unwrap();

    let edge_pairs = extract_edge_pairs(&target);
    let pattern_ids = extract_node_ids(&target);

    let grw_results = grw_matches_with(Morphism::Iso, &edge_pairs, &target, &pattern_ids);

    let (pg_pattern, pg_pattern_ids) = to_petgraph(&target);
    let (pg_target, pg_target_ids) = to_petgraph(&target);
    let pg_results = petgraph_matches(&pg_pattern, &pg_target, &pg_pattern_ids, &pg_target_ids);

    assert_eq!(grw_results.len(), 6);
    assert_eq!(grw_results, pg_results);
}

#[test]
fn iso_no_match_different_structure() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2)
    ].unwrap();

    let pattern_ids: Vec<Id> = vec![10, 11, 12];
    let pattern_edges = vec![(10, 11), (11, 12), (10, 12)];
    let grw_results = grw_matches_with(Morphism::Iso, &pattern_edges, &target, &pattern_ids);

    let pattern_graph = grw::graph![<(), ER>;
        N(10) ^ N(11), n(11) ^ N(12), n(10) ^ n(12)
    ].unwrap();
    let (pg_pattern, pg_pattern_ids) = to_petgraph(&pattern_graph);
    let (pg_target, pg_target_ids) = to_petgraph(&target);
    let pg_results = petgraph_matches(&pg_pattern, &pg_target, &pg_pattern_ids, &pg_target_ids);

    assert!(grw_results.is_empty());
    assert_eq!(grw_results, pg_results);
}

// ── SubIso ─────────────────────────────────────────────────────────────

#[test]
fn subiso_path_in_triangle_zero() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(0) ^ n(2)
    ].unwrap();

    let pattern_edges = vec![(10, 11), (11, 12)];
    let pattern_ids: Vec<Id> = vec![10, 11, 12];

    let grw_results = grw_matches_with(Morphism::SubIso, &pattern_edges, &target, &pattern_ids);

    let pattern_graph = grw::graph![<(), ER>;
        N(10) ^ N(11), n(11) ^ N(12)
    ].unwrap();
    let (pg_pattern, pg_pattern_ids) = to_petgraph(&pattern_graph);
    let (pg_target, pg_target_ids) = to_petgraph(&target);
    let pg_results = petgraph_matches(&pg_pattern, &pg_target, &pg_pattern_ids, &pg_target_ids);

    assert_eq!(grw_results.len(), 0);
    assert_eq!(grw_results, pg_results);
}

#[test]
fn subiso_edge_in_path() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2)
    ].unwrap();

    let pattern_edges = vec![(10, 11)];
    let pattern_ids: Vec<Id> = vec![10, 11];

    let grw_results = grw_matches_with(Morphism::SubIso, &pattern_edges, &target, &pattern_ids);

    let pattern_graph = grw::graph![<(), ER>; N(10) ^ N(11)].unwrap();
    let (pg_pattern, pg_pattern_ids) = to_petgraph(&pattern_graph);
    let (pg_target, pg_target_ids) = to_petgraph(&target);
    let pg_results = petgraph_matches(&pg_pattern, &pg_target, &pg_pattern_ids, &pg_target_ids);

    assert_eq!(grw_results.len(), 4);
    assert_eq!(grw_results, pg_results);
}

// ── Mono ───────────────────────────────────────────────────────────────

#[test]
fn mono_edge_in_triangle() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(0) ^ n(2)
    ].unwrap();

    let pattern_edges = vec![(10, 11)];
    let pattern_ids: Vec<Id> = vec![10, 11];

    let grw_results = grw_matches_with(Morphism::Mono, &pattern_edges, &target, &pattern_ids);

    let (pg_pattern, pg_pattern_ids) = to_petgraph(
        &grw::graph![<(), ER>; N(10) ^ N(11)].unwrap(),
    );
    let (pg_target, pg_target_ids) = to_petgraph(&target);
    let pg_results = petgraph_matches(&pg_pattern, &pg_target, &pg_pattern_ids, &pg_target_ids);

    assert_eq!(grw_results.len(), 6);
    assert_eq!(grw_results, pg_results);
}

#[test]
fn mono_path_in_triangle_finds_more_than_subiso() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(0) ^ n(2)
    ].unwrap();

    let pattern_edges = vec![(10, 11), (11, 12)];
    let pattern_ids: Vec<Id> = vec![10, 11, 12];

    let mono = grw_matches_with(Morphism::Mono, &pattern_edges, &target, &pattern_ids);
    let subiso = grw_matches_with(Morphism::SubIso, &pattern_edges, &target, &pattern_ids);

    assert_eq!(mono.len(), 6);
    assert_eq!(subiso.len(), 0);
    assert!(subiso.is_subset(&mono));
}

// ── Homo ───────────────────────────────────────────────────────────────

#[test]
fn homo_allows_non_injective_mapping() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1)
    ].unwrap();

    let pattern_edges = vec![(10, 11)];
    let pattern_ids: Vec<Id> = vec![10, 11];

    let mono = grw_matches_with(Morphism::Mono, &pattern_edges, &target, &pattern_ids);
    let homo = grw_matches_with(Morphism::Homo, &pattern_edges, &target, &pattern_ids);

    assert_eq!(mono.len(), 2);
    assert!(homo.len() >= 2);
    assert!(mono.is_subset(&homo));

    for m in &homo {
        assert!(
            validate_match_edges(m, &pattern_edges, &target),
            "homo: invalid match {m:?}"
        );
    }
}

// ═════════════════════════════════════════════════════════════════════
// Tier 1 — Brute-force oracle sweep (all 4 morphism types)
// ═════════════════════════════════════════════════════════════════════

#[test]
fn oracle_iso_sweep() {
    for seed in 0..10 {
        for nodes in 6..=9 {
            let target = random_graph(seed * 1000 + nodes as u64, nodes, 3);
            let edge_pairs = extract_edge_pairs(&target);
            let pattern_ids = extract_node_ids(&target);

            let grw = grw_matches_with(Morphism::Iso, &edge_pairs, &target, &pattern_ids);
            let oracle = oracle_mappings(Morphism::Iso, &pattern_ids, &edge_pairs, &target);

            assert_eq!(
                grw, oracle,
                "oracle_iso: seed={seed} nodes={nodes} grw={} oracle={}",
                grw.len(),
                oracle.len()
            );
        }
    }
}

#[test]
fn oracle_subiso_sweep() {
    for seed in 0..10 {
        for nodes in 8..=12 {
            for pattern_k in 3..=5 {
                let target = random_graph(seed * 1000 + nodes as u64, nodes, 3);
                let subgraph = extract_connected_subgraph(seed, &target, pattern_k);

                let edge_pairs = extract_edge_pairs(&subgraph);
                if edge_pairs.is_empty() {
                    continue;
                }
                let pattern_ids = extract_node_ids(&subgraph);

                let grw =
                    grw_matches_with(Morphism::SubIso, &edge_pairs, &target, &pattern_ids);
                let oracle =
                    oracle_mappings(Morphism::SubIso, &pattern_ids, &edge_pairs, &target);

                assert_eq!(
                    grw, oracle,
                    "oracle_subiso: seed={seed} nodes={nodes} pattern_k={pattern_k} \
                     grw={} oracle={}",
                    grw.len(),
                    oracle.len()
                );
            }
        }
    }
}

#[test]
fn oracle_mono_sweep() {
    for seed in 0..10 {
        for nodes in 8..=12 {
            for pattern_k in 3..=5 {
                let target = random_graph(seed * 1000 + nodes as u64, nodes, 3);
                let subgraph = extract_connected_subgraph(seed, &target, pattern_k);

                let edge_pairs = extract_edge_pairs(&subgraph);
                if edge_pairs.is_empty() {
                    continue;
                }
                let pattern_ids = extract_node_ids(&subgraph);

                let grw =
                    grw_matches_with(Morphism::Mono, &edge_pairs, &target, &pattern_ids);
                let oracle =
                    oracle_mappings(Morphism::Mono, &pattern_ids, &edge_pairs, &target);

                assert_eq!(
                    grw, oracle,
                    "oracle_mono: seed={seed} nodes={nodes} pattern_k={pattern_k} \
                     grw={} oracle={}",
                    grw.len(),
                    oracle.len()
                );
            }
        }
    }
}

#[test]
fn oracle_homo_sweep() {
    for seed in 0..10 {
        for nodes in 8..=10 {
            for pattern_k in 2..=4 {
                let target = random_graph(seed * 1000 + nodes as u64, nodes, 3);
                let subgraph = extract_connected_subgraph(seed, &target, pattern_k);

                let edge_pairs = extract_edge_pairs(&subgraph);
                if edge_pairs.is_empty() {
                    continue;
                }
                let pattern_ids = extract_node_ids(&subgraph);

                let grw =
                    grw_matches_with(Morphism::Homo, &edge_pairs, &target, &pattern_ids);
                let oracle =
                    oracle_mappings(Morphism::Homo, &pattern_ids, &edge_pairs, &target);

                assert_eq!(
                    grw, oracle,
                    "oracle_homo: seed={seed} nodes={nodes} pattern_k={pattern_k} \
                     grw={} oracle={}",
                    grw.len(),
                    oracle.len()
                );
            }
        }
    }
}

#[test]
fn oracle_containment() {
    for seed in 0..10 {
        let target = random_graph(seed * 1000 + 8, 8, 3);
        let subgraph = extract_connected_subgraph(seed, &target, 4);

        let edge_pairs = extract_edge_pairs(&subgraph);
        if edge_pairs.is_empty() {
            continue;
        }
        let pattern_ids = extract_node_ids(&subgraph);

        let subiso = oracle_mappings(Morphism::SubIso, &pattern_ids, &edge_pairs, &target);
        let mono = oracle_mappings(Morphism::Mono, &pattern_ids, &edge_pairs, &target);
        let homo = oracle_mappings(Morphism::Homo, &pattern_ids, &edge_pairs, &target);

        let grw_subiso =
            grw_matches_with(Morphism::SubIso, &edge_pairs, &target, &pattern_ids);
        let grw_mono =
            grw_matches_with(Morphism::Mono, &edge_pairs, &target, &pattern_ids);
        let grw_homo =
            grw_matches_with(Morphism::Homo, &edge_pairs, &target, &pattern_ids);

        assert_eq!(grw_subiso, subiso, "seed={seed}: subiso mismatch");
        assert_eq!(grw_mono, mono, "seed={seed}: mono mismatch");
        assert_eq!(grw_homo, homo, "seed={seed}: homo mismatch");

        assert!(
            subiso.is_subset(&mono),
            "seed={seed}: oracle subiso ⊄ mono (subiso={} mono={})",
            subiso.len(),
            mono.len()
        );
        assert!(
            mono.is_subset(&homo),
            "seed={seed}: oracle mono ⊄ homo (mono={} homo={})",
            mono.len(),
            homo.len()
        );
    }
}

// ═════════════════════════════════════════════════════════════════════
// Tier 2 — Petgraph large-scale parity (Iso + SubIso)
// ═════════════════════════════════════════════════════════════════════

#[test]
fn petgraph_iso_large() {
    for seed in 0..5 {
        for nodes in (50..=100).step_by(10) {
            let target = random_graph(seed * 10000 + nodes as u64, nodes, 4);
            let edge_pairs = extract_edge_pairs(&target);
            let pattern_ids = extract_node_ids(&target);

            let grw =
                grw_matches_with(Morphism::Iso, &edge_pairs, &target, &pattern_ids);

            let (pg_g, pg_ids) = to_petgraph(&target);
            let pg = petgraph_matches(&pg_g, &pg_g, &pg_ids, &pg_ids);

            assert_eq!(
                grw, pg,
                "petgraph_iso_large: seed={seed} nodes={nodes} grw={} petgraph={}",
                grw.len(),
                pg.len()
            );
        }
    }
}

#[test]
fn petgraph_subiso_large() {
    for seed in 0..5 {
        for nodes in (100..=150).step_by(25) {
            for pattern_k in 4..=7 {
                let target = random_graph(seed * 10000 + nodes as u64, nodes, 4);
                let subgraph =
                    extract_connected_subgraph(seed + 100, &target, pattern_k);

                let edge_pairs = extract_edge_pairs(&subgraph);
                if edge_pairs.is_empty() {
                    continue;
                }
                let pattern_ids = extract_node_ids(&subgraph);

                let grw =
                    grw_matches_with(Morphism::SubIso, &edge_pairs, &target, &pattern_ids);

                let (pg_pattern, pg_pattern_ids) = to_petgraph(&subgraph);
                let (pg_target, pg_target_ids) = to_petgraph(&target);
                let pg = petgraph_matches(
                    &pg_pattern, &pg_target, &pg_pattern_ids, &pg_target_ids,
                );

                assert_eq!(
                    grw, pg,
                    "petgraph_subiso_large: seed={seed} nodes={nodes} pattern_k={pattern_k} \
                     grw={} petgraph={}",
                    grw.len(),
                    pg.len()
                );
            }
        }
    }
}

// ═════════════════════════════════════════════════════════════════════
// Tier 3 — Negated edges
// ═════════════════════════════════════════════════════════════════════

// ── Smoke tests ─────────────────────────────────────────────────────

#[test]
fn neg_mono_edge_excludes_adjacent() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(0) ^ n(2)
    ].unwrap();

    let pattern_ids: Vec<Id> = vec![10, 11];
    let pos_edges = vec![(10, 11)];
    let neg_edges = vec![];

    let without_neg = grw_neg_matches_with(Morphism::Mono, &pos_edges, &neg_edges, &target, &pattern_ids);
    assert_eq!(without_neg.len(), 6);

    let pattern_ids3: Vec<Id> = vec![10, 11, 12];
    let pos_edges3 = vec![(10, 11)];
    let neg_edges3 = vec![(10, 12)];
    let with_neg = grw_neg_matches_with(Morphism::Mono, &pos_edges3, &neg_edges3, &target, &pattern_ids3);

    let oracle = oracle_neg_mappings(Morphism::Mono, &pattern_ids3, &pos_edges3, &neg_edges3, &target);
    assert_eq!(with_neg, oracle);
    assert_eq!(with_neg.len(), 0);
}

#[test]
fn neg_mono_path_not_triangle() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3)
    ].unwrap();

    let pattern_ids: Vec<Id> = vec![10, 11, 12];
    let pos_edges = vec![(10, 11), (11, 12)];
    let neg_edges = vec![(10, 12)];

    let grw = grw_neg_matches_with(Morphism::Mono, &pos_edges, &neg_edges, &target, &pattern_ids);
    let oracle = oracle_neg_mappings(Morphism::Mono, &pattern_ids, &pos_edges, &neg_edges, &target);

    assert_eq!(grw, oracle);
    assert_eq!(grw.len(), 4);
}

#[test]
fn neg_mono_negated_only() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2)
    ].unwrap();

    let pattern_ids: Vec<Id> = vec![10, 11];
    let pos_edges: Vec<(Id, Id)> = vec![];
    let neg_edges = vec![(10, 11)];

    let grw = grw_neg_matches_with(Morphism::Mono, &pos_edges, &neg_edges, &target, &pattern_ids);
    let oracle = oracle_neg_mappings(Morphism::Mono, &pattern_ids, &pos_edges, &neg_edges, &target);

    assert_eq!(grw, oracle);
    for m in &grw {
        let ta = m[&10];
        let tb = m[&11];
        assert!(!target.is_adjacent(ta, tb), "negated edge violated: {m:?}");
    }
}

#[test]
fn neg_homo_negated_edge() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2)
    ].unwrap();

    let pattern_ids: Vec<Id> = vec![10, 11];
    let pos_edges: Vec<(Id, Id)> = vec![];
    let neg_edges = vec![(10, 11)];

    let grw = grw_neg_matches_with(Morphism::Homo, &pos_edges, &neg_edges, &target, &pattern_ids);
    let oracle = oracle_neg_mappings(Morphism::Homo, &pattern_ids, &pos_edges, &neg_edges, &target);

    assert_eq!(grw, oracle);
}

#[test]
fn neg_iso_with_negated_edges() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2)
    ].unwrap();

    let pattern_ids: Vec<Id> = vec![10, 11, 12];
    let pos_edges = vec![(10, 11), (11, 12)];
    let neg_edges = vec![(10, 12)];

    let grw = grw_neg_matches_with(Morphism::Iso, &pos_edges, &neg_edges, &target, &pattern_ids);
    let oracle = oracle_neg_mappings(Morphism::Iso, &pattern_ids, &pos_edges, &neg_edges, &target);

    assert_eq!(grw, oracle);
    assert_eq!(grw.len(), 2);
}

#[test]
fn neg_subiso_with_negated_edges() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3)
    ].unwrap();

    let pattern_ids: Vec<Id> = vec![10, 11, 12];
    let pos_edges = vec![(10, 11), (11, 12)];
    let neg_edges = vec![(10, 12)];

    let grw = grw_neg_matches_with(Morphism::SubIso, &pos_edges, &neg_edges, &target, &pattern_ids);
    let oracle = oracle_neg_mappings(Morphism::SubIso, &pattern_ids, &pos_edges, &neg_edges, &target);

    assert_eq!(grw, oracle);
}

// ── Oracle sweep (negated edges) ────────────────────────────────────

fn pick_non_edges(
    rng: &mut SmallRng,
    nodes: &[Id],
    edges: &BTreeSet<(Id, Id)>,
    count: usize,
) -> Vec<(Id, Id)> {
    let mut non_edges = Vec::new();
    let mut seen = BTreeSet::new();
    let mut attempts = 0;
    while non_edges.len() < count && attempts < count * 10 {
        let a = nodes[rng.random_range(0..nodes.len())];
        let b = nodes[rng.random_range(0..nodes.len())];
        if a == b {
            attempts += 1;
            continue;
        }
        let key = (a.min(b), a.max(b));
        if !edges.contains(&key) && seen.insert(key) {
            non_edges.push((a, b));
        }
        attempts += 1;
    }
    non_edges
}

#[test]
fn oracle_neg_mono_sweep() {
    for seed in 0..10 {
        for nodes in 8..=10 {
            for pattern_k in 3..=4 {
                let target = random_graph(seed * 1000 + nodes as u64, nodes, 3);
                let subgraph = extract_connected_subgraph(seed, &target, pattern_k);

                let edge_pairs = extract_edge_pairs(&subgraph);
                if edge_pairs.is_empty() {
                    continue;
                }
                let pattern_ids = extract_node_ids(&subgraph);

                let edge_set: BTreeSet<(Id, Id)> = edge_pairs
                    .iter()
                    .map(|&(a, b)| (a.min(b), a.max(b)))
                    .collect();

                let mut rng = SmallRng::seed_from_u64(seed * 7 + nodes as u64);
                let neg = pick_non_edges(&mut rng, &pattern_ids, &edge_set, 2);

                let grw = grw_neg_matches_with(
                    Morphism::Mono, &edge_pairs, &neg, &target, &pattern_ids,
                );
                let oracle = oracle_neg_mappings(
                    Morphism::Mono, &pattern_ids, &edge_pairs, &neg, &target,
                );

                assert_eq!(
                    grw, oracle,
                    "oracle_neg_mono: seed={seed} nodes={nodes} k={pattern_k} \
                     neg={neg:?} grw={} oracle={}",
                    grw.len(),
                    oracle.len()
                );
            }
        }
    }
}

#[test]
fn oracle_neg_homo_sweep() {
    for seed in 0..10 {
        for nodes in 8..=10 {
            for pattern_k in 2..=3 {
                let target = random_graph(seed * 1000 + nodes as u64, nodes, 3);
                let subgraph = extract_connected_subgraph(seed, &target, pattern_k);

                let edge_pairs = extract_edge_pairs(&subgraph);
                if edge_pairs.is_empty() {
                    continue;
                }
                let pattern_ids = extract_node_ids(&subgraph);

                let edge_set: BTreeSet<(Id, Id)> = edge_pairs
                    .iter()
                    .map(|&(a, b)| (a.min(b), a.max(b)))
                    .collect();

                let mut rng = SmallRng::seed_from_u64(seed * 11 + nodes as u64);
                let neg = pick_non_edges(&mut rng, &pattern_ids, &edge_set, 1);

                let grw = grw_neg_matches_with(
                    Morphism::Homo, &edge_pairs, &neg, &target, &pattern_ids,
                );
                let oracle = oracle_neg_mappings(
                    Morphism::Homo, &pattern_ids, &edge_pairs, &neg, &target,
                );

                assert_eq!(
                    grw, oracle,
                    "oracle_neg_homo: seed={seed} nodes={nodes} k={pattern_k} \
                     neg={neg:?} grw={} oracle={}",
                    grw.len(),
                    oracle.len()
                );
            }
        }
    }
}

#[test]
fn oracle_neg_iso_sweep() {
    for seed in 0..10 {
        for nodes in 6..=8 {
            let target = random_graph(seed * 1000 + nodes as u64, nodes, 3);
            let edge_pairs = extract_edge_pairs(&target);
            let pattern_ids = extract_node_ids(&target);

            let edge_set: BTreeSet<(Id, Id)> = edge_pairs
                .iter()
                .map(|&(a, b)| (a.min(b), a.max(b)))
                .collect();

            let mut rng = SmallRng::seed_from_u64(seed * 13 + nodes as u64);
            let neg = pick_non_edges(&mut rng, &pattern_ids, &edge_set, 2);

            let grw = grw_neg_matches_with(
                Morphism::Iso, &edge_pairs, &neg, &target, &pattern_ids,
            );
            let oracle = oracle_neg_mappings(
                Morphism::Iso, &pattern_ids, &edge_pairs, &neg, &target,
            );

            assert_eq!(
                grw, oracle,
                "oracle_neg_iso: seed={seed} nodes={nodes} \
                 neg={neg:?} grw={} oracle={}",
                grw.len(),
                oracle.len()
            );
        }
    }
}

#[test]
fn oracle_neg_subiso_sweep() {
    for seed in 0..10 {
        for nodes in 8..=10 {
            for pattern_k in 3..=5 {
                let target = random_graph(seed * 1000 + nodes as u64, nodes, 3);
                let subgraph = extract_connected_subgraph(seed, &target, pattern_k);

                let edge_pairs = extract_edge_pairs(&subgraph);
                if edge_pairs.is_empty() {
                    continue;
                }
                let pattern_ids = extract_node_ids(&subgraph);

                let edge_set: BTreeSet<(Id, Id)> = edge_pairs
                    .iter()
                    .map(|&(a, b)| (a.min(b), a.max(b)))
                    .collect();

                let mut rng = SmallRng::seed_from_u64(seed * 17 + nodes as u64);
                let neg = pick_non_edges(&mut rng, &pattern_ids, &edge_set, 2);

                let grw = grw_neg_matches_with(
                    Morphism::SubIso, &edge_pairs, &neg, &target, &pattern_ids,
                );
                let oracle = oracle_neg_mappings(
                    Morphism::SubIso, &pattern_ids, &edge_pairs, &neg, &target,
                );

                assert_eq!(
                    grw, oracle,
                    "oracle_neg_subiso: seed={seed} nodes={nodes} k={pattern_k} \
                     neg={neg:?} grw={} oracle={}",
                    grw.len(),
                    oracle.len()
                );
            }
        }
    }
}

// ═════════════════════════════════════════════════════════════════════
// Tier 4 — Negated nodes
// ═════════════════════════════════════════════════════════════════════

// ── Oracle helpers ──────────────────────────────────────────────────

struct NegNodeSpec {
    positive_neighbors: Vec<Id>,
}

fn oracle_neg_node_filter(
    positive_matches: &BTreeSet<BTreeMap<Id, Id>>,
    neg_specs: &[NegNodeSpec],
    target: &ShagraGraph,
) -> BTreeSet<BTreeMap<Id, Id>> {
    let target_nodes = extract_node_ids(target);
    positive_matches
        .iter()
        .filter(|m| {
            let mapped_targets: BTreeSet<Id> = m.values().copied().collect();
            for spec in neg_specs {
                let has_violation = target_nodes.iter().any(|&tn| {
                    if mapped_targets.contains(&tn) {
                        return false;
                    }
                    spec.positive_neighbors.iter().all(|&pn| {
                        let mapped_pn = m[&pn];
                        target.is_adjacent(tn, mapped_pn)
                    })
                });
                if has_violation {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

fn build_neg_node_pattern(
    morphism: Morphism,
    positive_edges: &[(Id, Id)],
    positive_node_ids: &[Id],
    neg_nodes: &[(Id, Vec<Id>)],
) -> Vec<dsl::ClusterOps<(), ER>> {
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<(), ER>> = Vec::new();

    for &(a, b) in positive_edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);
        let op: dsl::Op<(), ER> = match (a_new, b_new) {
            (true, true) => (dsl::N::<(), ER>(a) ^ dsl::N::<(), ER>(b)).into(),
            (true, false) => (dsl::N::<(), ER>(a) ^ dsl::n::<(), ER>(b)).into(),
            (false, true) => (dsl::n::<(), ER>(a) ^ dsl::N::<(), ER>(b)).into(),
            (false, false) => (dsl::n::<(), ER>(a) ^ dsl::n::<(), ER>(b)).into(),
        };
        ops.push(op);
    }

    for &id in positive_node_ids {
        if seen.insert(id) {
            ops.push(dsl::N::<(), ER>(id).into());
        }
    }

    for (neg_id, neighbors) in neg_nodes {
        if let Some((&first, rest)) = neighbors.split_first() {
            seen.insert(*neg_id);
            ops.push((!dsl::N::<(), ER>(*neg_id) ^ dsl::n::<(), ER>(first)).into());
            for &neighbor in rest {
                ops.push((dsl::n::<(), ER>(*neg_id) ^ dsl::n::<(), ER>(neighbor)).into());
            }
        }
    }

    vec![dsl::get(morphism, ops)]
}

fn grw_neg_node_matches_with(
    morphism: Morphism,
    positive_edges: &[(Id, Id)],
    positive_node_ids: &[Id],
    neg_nodes: &[(Id, Vec<Id>)],
    target: &ShagraGraph,
) -> BTreeSet<BTreeMap<Id, Id>> {
    let clusters = build_neg_node_pattern(morphism, positive_edges, positive_node_ids, neg_nodes);
    let Search::Resolved(r) = search::compile::<(), ER>(clusters).unwrap()
    else { panic!("unexpected context nodes") };
    let query = r.into_query();
    let indexed = target.index(search::RevCsr);
    search::Seq::search(&query, &indexed)
        .map(|m| normalize_grw(&m, positive_node_ids))
        .collect()
}

// ── Tier 4a — Smoke tests (fixed graphs) ────────────────────────────

#[test]
fn neg_node_mono_connected_reject() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(0) ^ n(2)
    ].unwrap();

    let pos_nodes: Vec<Id> = vec![10, 11];
    let pos_edges = vec![(10, 11)];
    let neg_nodes = vec![(12, vec![11])];

    let grw = grw_neg_node_matches_with(
        Morphism::Mono, &pos_edges, &pos_nodes, &neg_nodes, &target,
    );
    let positive = oracle_mappings(Morphism::Mono, &pos_nodes, &pos_edges, &target);
    let oracle = oracle_neg_node_filter(
        &positive,
        &[NegNodeSpec { positive_neighbors: vec![11] }],
        &target,
    );

    assert_eq!(grw, oracle);
    assert_eq!(grw.len(), 0);
}

#[test]
fn neg_node_mono_connected_allow() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2)
    ].unwrap();

    let pos_nodes: Vec<Id> = vec![10, 11];
    let pos_edges = vec![(10, 11)];
    let neg_nodes = vec![(12, vec![11])];

    let grw = grw_neg_node_matches_with(
        Morphism::Mono, &pos_edges, &pos_nodes, &neg_nodes, &target,
    );
    let positive = oracle_mappings(Morphism::Mono, &pos_nodes, &pos_edges, &target);
    let oracle = oracle_neg_node_filter(
        &positive,
        &[NegNodeSpec { positive_neighbors: vec![11] }],
        &target,
    );

    assert_eq!(grw, oracle);
    assert!(grw.len() > 0);
}

#[test]
fn neg_node_subiso_connected() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3),
        n(0) ^ n(2), n(1) ^ n(3)
    ].unwrap();

    let pos_nodes: Vec<Id> = vec![10, 11];
    let pos_edges = vec![(10, 11)];
    let neg_nodes = vec![(12, vec![10])];

    let grw = grw_neg_node_matches_with(
        Morphism::SubIso, &pos_edges, &pos_nodes, &neg_nodes, &target,
    );
    let positive = oracle_mappings(Morphism::SubIso, &pos_nodes, &pos_edges, &target);
    let oracle = oracle_neg_node_filter(
        &positive,
        &[NegNodeSpec { positive_neighbors: vec![10] }],
        &target,
    );

    assert_eq!(grw, oracle);
}

#[test]
fn neg_node_iso_reject() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2)
    ].unwrap();

    let pos_nodes: Vec<Id> = vec![10, 11, 12];
    let pos_edges = vec![(10, 11), (11, 12)];
    let neg_nodes = vec![(13, vec![10, 12])];

    let grw = grw_neg_node_matches_with(
        Morphism::Iso, &pos_edges, &pos_nodes, &neg_nodes, &target,
    );
    let positive = oracle_mappings(Morphism::Iso, &pos_nodes, &pos_edges, &target);
    let oracle = oracle_neg_node_filter(
        &positive,
        &[NegNodeSpec { positive_neighbors: vec![10, 12] }],
        &target,
    );

    assert_eq!(grw, oracle);
    assert_eq!(grw.len(), 2);
}

// ── Tier 4b — Oracle sweep (negated nodes) ──────────────────────────

fn pick_negated_node(
    rng: &mut SmallRng,
    pattern_nodes: &[Id],
    pattern_edges: &[(Id, Id)],
) -> Option<(Vec<Id>, Vec<(Id, Id)>, Id, Vec<Id>)> {
    if pattern_nodes.len() < 3 {
        return None;
    }

    let adj: BTreeMap<Id, Vec<Id>> = {
        let mut a: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
        for &(x, y) in pattern_edges {
            a.entry(x).or_default().push(y);
            a.entry(y).or_default().push(x);
        }
        a
    };

    let candidates: Vec<Id> = pattern_nodes
        .iter()
        .copied()
        .filter(|n| adj.get(n).map_or(false, |ns| !ns.is_empty()))
        .collect();
    if candidates.is_empty() {
        return None;
    }

    let neg_node = candidates[rng.random_range(0..candidates.len())];
    let neighbors: Vec<Id> = adj[&neg_node].clone();
    let pos_nodes: Vec<Id> = pattern_nodes.iter().copied().filter(|&n| n != neg_node).collect();
    let pos_edges: Vec<(Id, Id)> = pattern_edges
        .iter()
        .copied()
        .filter(|&(a, b)| a != neg_node && b != neg_node)
        .collect();

    let remaining_adj: BTreeMap<Id, Vec<Id>> = {
        let mut a: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
        for &(x, y) in &pos_edges {
            a.entry(x).or_default().push(y);
            a.entry(y).or_default().push(x);
        }
        a
    };

    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();
    if let Some(&first) = pos_nodes.first() {
        visited.insert(first);
        queue.push_back(first);
    }
    while let Some(cur) = queue.pop_front() {
        if let Some(ns) = remaining_adj.get(&cur) {
            for &n in ns {
                if visited.insert(n) {
                    queue.push_back(n);
                }
            }
        }
    }
    if visited.len() != pos_nodes.len() {
        return None;
    }

    if pos_edges.is_empty() {
        return None;
    }

    Some((pos_nodes, pos_edges, neg_node, neighbors))
}

#[test]
fn oracle_neg_node_mono_sweep() {
    for seed in 0..10 {
        for nodes in 8..=10 {
            for pattern_k in 3..=4 {
                let target = random_graph(seed * 1000 + nodes as u64, nodes, 3);
                let subgraph = extract_connected_subgraph(seed, &target, pattern_k);

                let edge_pairs = extract_edge_pairs(&subgraph);
                if edge_pairs.is_empty() {
                    continue;
                }
                let pattern_ids = extract_node_ids(&subgraph);

                let mut rng = SmallRng::seed_from_u64(seed * 19 + nodes as u64);
                let Some((pos_nodes, pos_edges, _neg_node, neighbors)) =
                    pick_negated_node(&mut rng, &pattern_ids, &edge_pairs)
                else {
                    continue;
                };

                let neg_id = Id::MAX - 1;
                let neg_nodes = vec![(neg_id, neighbors.clone())];

                let grw = grw_neg_node_matches_with(
                    Morphism::Mono, &pos_edges, &pos_nodes, &neg_nodes, &target,
                );
                let positive = oracle_mappings(Morphism::Mono, &pos_nodes, &pos_edges, &target);
                let oracle = oracle_neg_node_filter(
                    &positive,
                    &[NegNodeSpec { positive_neighbors: neighbors }],
                    &target,
                );

                assert_eq!(
                    grw, oracle,
                    "oracle_neg_node_mono: seed={seed} nodes={nodes} k={pattern_k} \
                     grw={} oracle={}",
                    grw.len(),
                    oracle.len()
                );
            }
        }
    }
}

#[test]
fn oracle_neg_node_subiso_sweep() {
    for seed in 0..10 {
        for nodes in 8..=10 {
            for pattern_k in 3..=4 {
                let target = random_graph(seed * 1000 + nodes as u64, nodes, 3);
                let subgraph = extract_connected_subgraph(seed, &target, pattern_k);

                let edge_pairs = extract_edge_pairs(&subgraph);
                if edge_pairs.is_empty() {
                    continue;
                }
                let pattern_ids = extract_node_ids(&subgraph);

                let mut rng = SmallRng::seed_from_u64(seed * 23 + nodes as u64);
                let Some((pos_nodes, pos_edges, _neg_node, neighbors)) =
                    pick_negated_node(&mut rng, &pattern_ids, &edge_pairs)
                else {
                    continue;
                };

                let neg_id = Id::MAX - 1;
                let neg_nodes = vec![(neg_id, neighbors.clone())];

                let grw = grw_neg_node_matches_with(
                    Morphism::SubIso, &pos_edges, &pos_nodes, &neg_nodes, &target,
                );
                let positive = oracle_mappings(Morphism::SubIso, &pos_nodes, &pos_edges, &target);
                let oracle = oracle_neg_node_filter(
                    &positive,
                    &[NegNodeSpec { positive_neighbors: neighbors }],
                    &target,
                );

                assert_eq!(
                    grw, oracle,
                    "oracle_neg_node_subiso: seed={seed} nodes={nodes} k={pattern_k} \
                     grw={} oracle={}",
                    grw.len(),
                    oracle.len()
                );
            }
        }
    }
}

#[test]
fn oracle_neg_node_iso_sweep() {
    for seed in 0..10 {
        for nodes in 6..=8 {
            let target = random_graph(seed * 1000 + nodes as u64, nodes, 3);
            let edge_pairs = extract_edge_pairs(&target);
            let pattern_ids = extract_node_ids(&target);

            let mut rng = SmallRng::seed_from_u64(seed * 29 + nodes as u64);
            let Some((pos_nodes, pos_edges, _neg_node, neighbors)) =
                pick_negated_node(&mut rng, &pattern_ids, &edge_pairs)
            else {
                continue;
            };

            let neg_id = Id::MAX - 1;
            let neg_nodes = vec![(neg_id, neighbors.clone())];

            let grw = grw_neg_node_matches_with(
                Morphism::Iso, &pos_edges, &pos_nodes, &neg_nodes, &target,
            );
            let positive = oracle_mappings(Morphism::Iso, &pos_nodes, &pos_edges, &target);
            let oracle = oracle_neg_node_filter(
                &positive,
                &[NegNodeSpec { positive_neighbors: neighbors }],
                &target,
            );

            assert_eq!(
                grw, oracle,
                "oracle_neg_node_iso: seed={seed} nodes={nodes} \
                 grw={} oracle={}",
                grw.len(),
                oracle.len()
            );
        }
    }
}

#[test]
fn oracle_neg_node_homo_sweep() {
    for seed in 0..10 {
        for nodes in 8..=10 {
            for pattern_k in 2..=3 {
                let target = random_graph(seed * 1000 + nodes as u64, nodes, 3);
                let subgraph = extract_connected_subgraph(seed, &target, pattern_k);

                let edge_pairs = extract_edge_pairs(&subgraph);
                if edge_pairs.is_empty() {
                    continue;
                }
                let pattern_ids = extract_node_ids(&subgraph);

                let mut rng = SmallRng::seed_from_u64(seed * 31 + nodes as u64);
                let Some((pos_nodes, pos_edges, _neg_node, neighbors)) =
                    pick_negated_node(&mut rng, &pattern_ids, &edge_pairs)
                else {
                    continue;
                };

                let neg_id = Id::MAX - 1;
                let neg_nodes = vec![(neg_id, neighbors.clone())];

                let grw = grw_neg_node_matches_with(
                    Morphism::Homo, &pos_edges, &pos_nodes, &neg_nodes, &target,
                );
                let positive = oracle_mappings(Morphism::Homo, &pos_nodes, &pos_edges, &target);
                let oracle = oracle_neg_node_filter(
                    &positive,
                    &[NegNodeSpec { positive_neighbors: neighbors }],
                    &target,
                );

                assert_eq!(
                    grw, oracle,
                    "oracle_neg_node_homo: seed={seed} nodes={nodes} k={pattern_k} \
                     grw={} oracle={}",
                    grw.len(),
                    oracle.len()
                );
            }
        }
    }
}

// ── Tier 4c — Freestanding negated nodes ────────────────────────────

#[test]
fn neg_node_freestanding_bare_bail() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2)
    ].unwrap();
    let Search::Resolved(r) = grw::search![<(), ER>;
        get(Morphism::SubIso) { !N_() }
    ].unwrap()
    else { panic!("unexpected context nodes") };
    let query = r.into_query();
    let indexed = target.index(search::RevCsr);
    let matches: Vec<_> = search::Seq::search(&query, &indexed).collect();
    assert_eq!(matches.len(), 0);
}

#[test]
fn neg_node_freestanding_with_positive() {
    let target = grw::graph![<(), ER>;
        N(0) ^ N(1), n(1) ^ N(2)
    ].unwrap();
    let Search::Resolved(r) = grw::search![<(), ER>;
        get(Morphism::Mono) {
            N(0) ^ N(1),
            !N_()
        }
    ].unwrap()
    else { panic!("unexpected context nodes") };
    let query = r.into_query();
    let indexed = target.index(search::RevCsr);
    let matches: Vec<_> = search::Seq::search(&query, &indexed).collect();
    assert_eq!(matches.len(), 0);
}

// ═════════════════════════════════════════════════════════════════════
// Tier 5 — Directed graph parity
// ═════════════════════════════════════════════════════════════════════

use grw::graph::edge::dir;

type DirER = edge::Dir<()>;
type DirGraph = grw::graph::Dir0;

// ── Dir graph construction helpers ──────────────────────────────────

fn random_dir_graph(seed: u64, nodes: Id, avg_degree: Id) -> DirGraph {
    let target_edges = (nodes as u64 * avg_degree as u64) / 2;
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut seen = BTreeSet::new();

    while (seen.len() as u64) < target_edges {
        let a: Id = rng.random_range(0..nodes);
        let b: Id = rng.random_range(0..nodes);
        if a == b {
            continue;
        }
        seen.insert((a, b));
    }

    let edges: Vec<dir::E<Id>> = seen.into_iter().map(|(a, b)| dir::E::D(a, b)).collect();
    let node_ids: Vec<Id> = (0..nodes).collect();
    DirGraph::try_from((node_ids, edges)).unwrap()
}

fn extract_connected_dir_subgraph(seed: u64, graph: &DirGraph, k: usize) -> DirGraph {
    let (all_nodes, all_edges) = graph.to_vecs();

    let mut rng = SmallRng::seed_from_u64(seed);
    let start_idx = rng.random_range(0..all_nodes.len());
    let start = all_nodes[start_idx].0;

    let mut adj: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
    for (edef, _) in &all_edges {
        let dir::E::D(a, b) = edef;
        adj.entry(*a).or_default().push(*b);
        adj.entry(*b).or_default().push(*a);
    }

    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();
    visited.insert(start);
    queue.push_back(start);

    while visited.len() < k {
        let Some(current) = queue.pop_front() else {
            break;
        };
        if let Some(neighbors) = adj.get(&current) {
            for &n in neighbors {
                if visited.len() >= k {
                    break;
                }
                if visited.insert(n) {
                    queue.push_back(n);
                }
            }
        }
    }

    let sub_nodes: Vec<Id> = visited.iter().copied().collect();
    let sub_edges: Vec<dir::E<Id>> = all_edges
        .iter()
        .filter_map(|(edef, _)| {
            let dir::E::D(a, b) = edef;
            if visited.contains(a) && visited.contains(b) {
                Some(dir::E::D(*a, *b))
            } else {
                None
            }
        })
        .collect();

    DirGraph::try_from((sub_nodes, sub_edges)).unwrap()
}

// ── Dir extraction helpers ──────────────────────────────────────────

fn extract_dir_edge_pairs(g: &DirGraph) -> Vec<(Id, Id)> {
    let (_, edges) = g.to_vecs();
    edges
        .iter()
        .map(|(edef, _)| {
            let dir::E::D(a, b) = edef;
            (*a, *b)
        })
        .collect()
}

fn extract_dir_node_ids(g: &DirGraph) -> Vec<Id> {
    let (nodes, _) = g.to_vecs();
    nodes.iter().map(|(id, _)| *id).collect()
}

// ── Dir grw query helpers ────────────────────────────────────────

fn build_dir_pattern(
    morphism: Morphism,
    edges: &[(Id, Id)],
    all_node_ids: &[Id],
) -> Vec<dsl::ClusterOps<(), DirER>> {
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<(), DirER>> = Vec::new();

    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);

        let op: dsl::Op<(), DirER> = match (a_new, b_new) {
            (true, true) => (dsl::N::<(), DirER>(a) >> dsl::N::<(), DirER>(b)).into(),
            (true, false) => (dsl::N::<(), DirER>(a) >> dsl::n::<(), DirER>(b)).into(),
            (false, true) => (dsl::n::<(), DirER>(a) >> dsl::N::<(), DirER>(b)).into(),
            (false, false) => (dsl::n::<(), DirER>(a) >> dsl::n::<(), DirER>(b)).into(),
        };
        ops.push(op);
    }

    for &id in all_node_ids {
        if seen.insert(id) {
            ops.push(dsl::N::<(), DirER>(id).into());
        }
    }

    vec![dsl::get(morphism, ops)]
}

fn grw_dir_matches_with(
    morphism: Morphism,
    edges: &[(Id, Id)],
    target: &DirGraph,
    pattern_local_ids: &[Id],
) -> BTreeSet<BTreeMap<Id, Id>> {
    let clusters = build_dir_pattern(morphism, edges, pattern_local_ids);
    let Search::Resolved(r) = search::compile::<(), DirER>(clusters).unwrap()
    else { panic!("unexpected context nodes") };
    let query = r.into_query();
    let indexed = target.index(search::RevCsr);
    search::Seq::search(&query, &indexed)
        .map(|m| normalize_grw(&m, pattern_local_ids))
        .collect()
}

// ── Dir brute-force oracle ──────────────────────────────────────────

fn oracle_dir_mappings(
    morphism: Morphism,
    pattern_nodes: &[Id],
    pattern_edges: &[(Id, Id)],
    target: &DirGraph,
) -> BTreeSet<BTreeMap<Id, Id>> {
    let target_nodes = extract_dir_node_ids(target);
    let is_injective = matches!(morphism, Morphism::Iso | Morphism::SubIso | Morphism::Mono);
    let check_non_edges = matches!(morphism, Morphism::Iso | Morphism::SubIso);

    if morphism == Morphism::Iso && pattern_nodes.len() != target_nodes.len() {
        return BTreeSet::new();
    }

    let pattern_edge_set: BTreeSet<(Id, Id)> = pattern_edges.iter().copied().collect();

    let mut results = BTreeSet::new();
    let mut mapping = BTreeMap::new();
    let mut used = BTreeSet::new();

    oracle_dir_backtrack(
        pattern_nodes,
        &pattern_edge_set,
        &target_nodes,
        target,
        is_injective,
        check_non_edges,
        0,
        &mut mapping,
        &mut used,
        &mut results,
    );

    results
}

fn oracle_dir_backtrack(
    pattern_nodes: &[Id],
    pattern_edges: &BTreeSet<(Id, Id)>,
    target_nodes: &[Id],
    target: &DirGraph,
    is_injective: bool,
    check_non_edges: bool,
    depth: usize,
    mapping: &mut BTreeMap<Id, Id>,
    used: &mut BTreeSet<Id>,
    results: &mut BTreeSet<BTreeMap<Id, Id>>,
) {
    if depth == pattern_nodes.len() {
        if oracle_dir_validate(mapping, pattern_nodes, pattern_edges, target, check_non_edges) {
            results.insert(mapping.clone());
        }
        return;
    }

    let pnode = pattern_nodes[depth];

    for &tnode in target_nodes {
        if is_injective && used.contains(&tnode) {
            continue;
        }

        mapping.insert(pnode, tnode);
        if is_injective {
            used.insert(tnode);
        }

        oracle_dir_backtrack(
            pattern_nodes,
            pattern_edges,
            target_nodes,
            target,
            is_injective,
            check_non_edges,
            depth + 1,
            mapping,
            used,
            results,
        );

        mapping.remove(&pnode);
        if is_injective {
            used.remove(&tnode);
        }
    }
}

fn oracle_dir_validate(
    mapping: &BTreeMap<Id, Id>,
    pattern_nodes: &[Id],
    pattern_edges: &BTreeSet<(Id, Id)>,
    target: &DirGraph,
    check_non_edges: bool,
) -> bool {
    for &(pa, pb) in pattern_edges {
        let ta = mapping[&pa];
        let tb = mapping[&pb];
        if !target.has(dir::E::D(ta, tb)) {
            return false;
        }
    }

    if check_non_edges {
        for i in 0..pattern_nodes.len() {
            for j in 0..pattern_nodes.len() {
                if i == j {
                    continue;
                }
                let pa = pattern_nodes[i];
                let pb = pattern_nodes[j];
                if !pattern_edges.contains(&(pa, pb)) {
                    let ta = mapping[&pa];
                    let tb = mapping[&pb];
                    if target.has(dir::E::D(ta, tb)) {
                        return false;
                    }
                }
            }
        }
    }

    true
}

fn validate_dir_match_edges(
    mapping: &BTreeMap<Id, Id>,
    pattern_edges: &[(Id, Id)],
    target: &DirGraph,
) -> bool {
    for &(pa, pb) in pattern_edges {
        let ta = mapping[&pa];
        let tb = mapping[&pb];
        if !target.has(dir::E::D(ta, tb)) {
            return false;
        }
    }
    true
}

// ── Dir Tier 5a — Smoke tests ───────────────────────────────────────

#[test]
fn dir_iso_single_edge() {
    let target = DirGraph::try_from((
        vec![0u32, 1],
        vec![dir::E::D(0, 1)],
    )).unwrap();

    let edge_pairs = extract_dir_edge_pairs(&target);
    let pattern_ids = extract_dir_node_ids(&target);

    let grw = grw_dir_matches_with(Morphism::Iso, &edge_pairs, &target, &pattern_ids);
    let oracle = oracle_dir_mappings(Morphism::Iso, &pattern_ids, &edge_pairs, &target);

    assert_eq!(grw.len(), 1);
    assert_eq!(grw, oracle);
}

#[test]
fn dir_parity_iso_bidirectional_two_autos() {
    let target = DirGraph::try_from((
        vec![0u32, 1],
        vec![dir::E::D(0, 1), dir::E::D(1, 0)],
    )).unwrap();

    let edge_pairs = extract_dir_edge_pairs(&target);
    let pattern_ids = extract_dir_node_ids(&target);

    let grw = grw_dir_matches_with(Morphism::Iso, &edge_pairs, &target, &pattern_ids);
    let oracle = oracle_dir_mappings(Morphism::Iso, &pattern_ids, &edge_pairs, &target);

    assert_eq!(grw.len(), 2);
    assert_eq!(grw, oracle);
}

#[test]
fn dir_mono_chain_in_cycle() {
    let target = DirGraph::try_from((
        vec![0u32, 1, 2],
        vec![dir::E::D(0, 1), dir::E::D(1, 2), dir::E::D(2, 0)],
    )).unwrap();

    let pattern_edges = vec![(10, 11), (11, 12)];
    let pattern_ids: Vec<Id> = vec![10, 11, 12];

    let grw = grw_dir_matches_with(Morphism::Mono, &pattern_edges, &target, &pattern_ids);
    let oracle = oracle_dir_mappings(Morphism::Mono, &pattern_ids, &pattern_edges, &target);

    assert_eq!(grw.len(), 3);
    assert_eq!(grw, oracle);
}

#[test]
fn dir_reversed_pattern_swaps_mapping() {
    let target = DirGraph::try_from((
        vec![0u32, 1],
        vec![dir::E::D(0, 1)],
    )).unwrap();

    let pattern_edges = vec![(10, 11)];
    let pattern_ids: Vec<Id> = vec![10, 11];

    let forward = grw_dir_matches_with(Morphism::Mono, &pattern_edges, &target, &pattern_ids);
    assert_eq!(forward.len(), 1);
    assert_eq!(forward.iter().next().unwrap()[&10], 0);
    assert_eq!(forward.iter().next().unwrap()[&11], 1);

    let reversed_edges = vec![(11, 10)];
    let reversed = grw_dir_matches_with(Morphism::Mono, &reversed_edges, &target, &pattern_ids);
    assert_eq!(reversed.len(), 1);
    assert_eq!(reversed.iter().next().unwrap()[&10], 1);
    assert_eq!(reversed.iter().next().unwrap()[&11], 0);
}

#[test]
fn dir_homo_allows_non_injective() {
    let target = DirGraph::try_from((
        vec![0u32, 1],
        vec![dir::E::D(0, 1)],
    )).unwrap();

    let pattern_edges = vec![(10, 11)];
    let pattern_ids: Vec<Id> = vec![10, 11];

    let mono = grw_dir_matches_with(Morphism::Mono, &pattern_edges, &target, &pattern_ids);
    let homo = grw_dir_matches_with(Morphism::Homo, &pattern_edges, &target, &pattern_ids);

    assert_eq!(mono.len(), 1);
    assert!(homo.len() >= 1);
    assert!(mono.is_subset(&homo));

    for m in &homo {
        assert!(
            validate_dir_match_edges(m, &pattern_edges, &target),
            "homo: invalid match {m:?}"
        );
    }
}

// ── Dir Tier 5b — Oracle sweep ──────────────────────────────────────

#[test]
fn oracle_dir_iso_sweep() {
    for seed in 0..10 {
        for nodes in 6..=9 {
            let target = random_dir_graph(seed * 1000 + nodes as u64, nodes, 3);
            let edge_pairs = extract_dir_edge_pairs(&target);
            let pattern_ids = extract_dir_node_ids(&target);

            let grw = grw_dir_matches_with(Morphism::Iso, &edge_pairs, &target, &pattern_ids);
            let oracle = oracle_dir_mappings(Morphism::Iso, &pattern_ids, &edge_pairs, &target);

            assert_eq!(
                grw, oracle,
                "oracle_dir_iso: seed={seed} nodes={nodes} grw={} oracle={}",
                grw.len(),
                oracle.len()
            );
        }
    }
}

#[test]
fn oracle_dir_subiso_sweep() {
    for seed in 0..10 {
        for nodes in 8..=12 {
            for pattern_k in 3..=5 {
                let target = random_dir_graph(seed * 1000 + nodes as u64, nodes, 3);
                let subgraph = extract_connected_dir_subgraph(seed, &target, pattern_k);

                let edge_pairs = extract_dir_edge_pairs(&subgraph);
                if edge_pairs.is_empty() {
                    continue;
                }
                let pattern_ids = extract_dir_node_ids(&subgraph);

                let grw =
                    grw_dir_matches_with(Morphism::SubIso, &edge_pairs, &target, &pattern_ids);
                let oracle =
                    oracle_dir_mappings(Morphism::SubIso, &pattern_ids, &edge_pairs, &target);

                assert_eq!(
                    grw, oracle,
                    "oracle_dir_subiso: seed={seed} nodes={nodes} pattern_k={pattern_k} \
                     grw={} oracle={}",
                    grw.len(),
                    oracle.len()
                );
            }
        }
    }
}

#[test]
fn oracle_dir_mono_sweep() {
    for seed in 0..10 {
        for nodes in 8..=12 {
            for pattern_k in 3..=5 {
                let target = random_dir_graph(seed * 1000 + nodes as u64, nodes, 3);
                let subgraph = extract_connected_dir_subgraph(seed, &target, pattern_k);

                let edge_pairs = extract_dir_edge_pairs(&subgraph);
                if edge_pairs.is_empty() {
                    continue;
                }
                let pattern_ids = extract_dir_node_ids(&subgraph);

                let grw =
                    grw_dir_matches_with(Morphism::Mono, &edge_pairs, &target, &pattern_ids);
                let oracle =
                    oracle_dir_mappings(Morphism::Mono, &pattern_ids, &edge_pairs, &target);

                assert_eq!(
                    grw, oracle,
                    "oracle_dir_mono: seed={seed} nodes={nodes} pattern_k={pattern_k} \
                     grw={} oracle={}",
                    grw.len(),
                    oracle.len()
                );
            }
        }
    }
}

#[test]
fn oracle_dir_homo_sweep() {
    for seed in 0..10 {
        for nodes in 8..=10 {
            for pattern_k in 2..=4 {
                let target = random_dir_graph(seed * 1000 + nodes as u64, nodes, 3);
                let subgraph = extract_connected_dir_subgraph(seed, &target, pattern_k);

                let edge_pairs = extract_dir_edge_pairs(&subgraph);
                if edge_pairs.is_empty() {
                    continue;
                }
                let pattern_ids = extract_dir_node_ids(&subgraph);

                let grw =
                    grw_dir_matches_with(Morphism::Homo, &edge_pairs, &target, &pattern_ids);
                let oracle =
                    oracle_dir_mappings(Morphism::Homo, &pattern_ids, &edge_pairs, &target);

                assert_eq!(
                    grw, oracle,
                    "oracle_dir_homo: seed={seed} nodes={nodes} pattern_k={pattern_k} \
                     grw={} oracle={}",
                    grw.len(),
                    oracle.len()
                );
            }
        }
    }
}

#[test]
fn oracle_dir_containment() {
    for seed in 0..10 {
        let target = random_dir_graph(seed * 1000 + 8, 8, 3);
        let subgraph = extract_connected_dir_subgraph(seed, &target, 4);

        let edge_pairs = extract_dir_edge_pairs(&subgraph);
        if edge_pairs.is_empty() {
            continue;
        }
        let pattern_ids = extract_dir_node_ids(&subgraph);

        let subiso = oracle_dir_mappings(Morphism::SubIso, &pattern_ids, &edge_pairs, &target);
        let mono = oracle_dir_mappings(Morphism::Mono, &pattern_ids, &edge_pairs, &target);
        let homo = oracle_dir_mappings(Morphism::Homo, &pattern_ids, &edge_pairs, &target);

        let grw_subiso =
            grw_dir_matches_with(Morphism::SubIso, &edge_pairs, &target, &pattern_ids);
        let grw_mono =
            grw_dir_matches_with(Morphism::Mono, &edge_pairs, &target, &pattern_ids);
        let grw_homo =
            grw_dir_matches_with(Morphism::Homo, &edge_pairs, &target, &pattern_ids);

        assert_eq!(grw_subiso, subiso, "seed={seed}: dir subiso mismatch");
        assert_eq!(grw_mono, mono, "seed={seed}: dir mono mismatch");
        assert_eq!(grw_homo, homo, "seed={seed}: dir homo mismatch");

        assert!(
            subiso.is_subset(&mono),
            "seed={seed}: dir oracle subiso ⊄ mono (subiso={} mono={})",
            subiso.len(),
            mono.len()
        );
        assert!(
            mono.is_subset(&homo),
            "seed={seed}: dir oracle mono ⊄ homo (mono={} homo={})",
            mono.len(),
            homo.len()
        );
    }
}
