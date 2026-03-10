use std::collections::BTreeSet;
use std::time::Instant;

use grw::graph::edge;
use grw::graph::edge::anydir;
use grw::search::{self, dsl, Morphism, Seq, Search, RevCsr};
use grw::Id;

type ER = edge::Anydir<i32>;
type G = grw::graph::Anydir<i32, i32>;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let size: Id = args.get(1).map(|s| s.parse().expect("invalid size")).unwrap_or(3000);
    let pattern_k: usize = args.get(2).map(|s| s.parse().expect("invalid pattern")).unwrap_or(6);
    let iters: usize = args.get(3).map(|s| s.parse().expect("invalid iters")).unwrap_or(3);
    let seed: u64 = args.get(4).map(|s| s.parse().expect("invalid seed")).unwrap_or(42);

    let mut rng = Lcg::new(seed);

    let avg_degree: Id = 4;
    let target_edges = (size as u64 * avg_degree as u64) / 2;
    let mut edge_set = BTreeSet::new();
    while (edge_set.len() as u64) < target_edges {
        let a = rng.next_bounded(size);
        let b = rng.next_bounded(size);
        if a == b { continue; }
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        edge_set.insert((lo, hi));
    }

    let node_labels: Vec<i32> = (0..size).map(|_| rng.next_bounded(5) as i32).collect();
    let nodes: Vec<(Id, i32)> = (0..size).map(|i| (i, node_labels[i as usize])).collect();

    let edges: Vec<(anydir::E<Id>, i32)> = edge_set.iter().map(|&(a, b)| {
        let ev = rng.next_bounded(10) as i32;
        let kind = rng.next_bounded(4);
        let edef = if kind == 0 {
            anydir::E::D(a, b)
        } else {
            anydir::E::U(a, b)
        };
        (edef, ev)
    }).collect();

    let graph: G = (nodes, edges).try_into().expect("graph construction failed");

    eprintln!("target: {}n/{}e  avg_degree: {}", graph.node_count(), graph.edge_count(), avg_degree);

    let subgraph = extract_subgraph(&graph, &mut rng, pattern_k);
    let pat_edges = extract_pattern_edges(&subgraph);
    let pat_nodes: Vec<Id> = {
        let mut s = BTreeSet::new();
        for &(a, b, _) in &pat_edges {
            s.insert(a);
            s.insert(b);
        }
        s.into_iter().collect()
    };

    eprintln!("pattern: {}n/{}e", pat_nodes.len(), pat_edges.len());
    eprintln!("pattern edges: {:?}", pat_edges);

    let clusters = build_pattern(&pat_edges, &node_labels, Morphism::SubIso);
    let compiled = search::compile::<i32, ER>(clusters).expect("compile failed");
    let query = match compiled {
        Search::Resolved(r) => r.into_query(),
        Search::Unresolved(_) => panic!("unexpected bound"),
    };
    let t = Instant::now();
    let search_graph = graph.index(RevCsr);
    let build_ms = t.elapsed().as_secs_f64() * 1000.0;
    eprintln!("build: {:.2}ms", build_ms);

    for i in 0..iters {
        let t = Instant::now();
        let matches: Vec<_> = Seq::search(&query, &search_graph).collect();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        eprintln!("iter {}: matches={} time={:.2}ms", i, matches.len(), ms);
    }
}

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self { Self(seed) }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn next_bounded(&mut self, bound: Id) -> Id {
        (self.next() % bound as u64) as Id
    }
}

fn extract_subgraph(graph: &G, rng: &mut Lcg, k: usize) -> Vec<(Id, Id, bool)> {
    let (all_nodes, all_edges) = graph.to_vecs();
    let mut adj: std::collections::BTreeMap<Id, Vec<(Id, bool)>> = std::collections::BTreeMap::new();
    for (edef, _) in &all_edges {
        match edef {
            anydir::E::U(a, b) => {
                adj.entry(*a).or_default().push((*b, false));
                adj.entry(*b).or_default().push((*a, false));
            }
            anydir::E::D(a, b) => {
                adj.entry(*a).or_default().push((*b, true));
                adj.entry(*b).or_default().push((*a, true));
            }
        }
    }
    let start_idx = rng.next_bounded(all_nodes.len() as Id) as usize;
    let start = all_nodes[start_idx].0;

    let mut visited = BTreeSet::new();
    let mut queue = std::collections::VecDeque::new();
    visited.insert(start);
    queue.push_back(start);

    while visited.len() < k {
        let Some(current) = queue.pop_front() else { break };
        if let Some(neighbors) = adj.get(&current) {
            for &(n, _) in neighbors {
                if visited.len() >= k { break; }
                if visited.insert(n) {
                    queue.push_back(n);
                }
            }
        }
    }

    let mut result = Vec::new();
    for (edef, _) in &all_edges {
        match edef {
            anydir::E::U(a, b) if visited.contains(a) && visited.contains(b) => {
                result.push((*a, *b, false));
            }
            anydir::E::D(a, b) if visited.contains(a) && visited.contains(b) => {
                result.push((*a, *b, true));
            }
            _ => {}
        }
    }
    result
}

fn extract_pattern_edges(edges: &[(Id, Id, bool)]) -> Vec<(Id, Id, bool)> {
    edges.to_vec()
}

fn build_pattern(
    edges: &[(Id, Id, bool)],
    node_labels: &[i32],
    morphism: Morphism,
) -> Vec<dsl::ClusterOps<i32, ER>> {
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<i32, ER>> = Vec::new();

    for &(a, b, is_dir) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);

        let op: dsl::Op<i32, ER> = match (a_new, b_new, is_dir) {
            (true, true, false) => (dsl::N::<i32, ER>(a).val(node_labels[a as usize]) ^ dsl::N::<i32, ER>(b).val(node_labels[b as usize])).into(),
            (true, false, false) => (dsl::N::<i32, ER>(a).val(node_labels[a as usize]) ^ dsl::n::<i32, ER>(b)).into(),
            (false, true, false) => (dsl::n::<i32, ER>(a) ^ dsl::N::<i32, ER>(b).val(node_labels[b as usize])).into(),
            (false, false, false) => (dsl::n::<i32, ER>(a) ^ dsl::n::<i32, ER>(b)).into(),
            (true, true, true) => (dsl::N::<i32, ER>(a).val(node_labels[a as usize]) >> dsl::N::<i32, ER>(b).val(node_labels[b as usize])).into(),
            (true, false, true) => (dsl::N::<i32, ER>(a).val(node_labels[a as usize]) >> dsl::n::<i32, ER>(b)).into(),
            (false, true, true) => (dsl::n::<i32, ER>(a) >> dsl::N::<i32, ER>(b).val(node_labels[b as usize])).into(),
            (false, false, true) => (dsl::n::<i32, ER>(a) >> dsl::n::<i32, ER>(b)).into(),
        };
        ops.push(op);
    }

    vec![dsl::get(morphism, ops)]
}
