use std::collections::{BTreeSet, BTreeMap, VecDeque};

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rayon::iter::ParallelIterator;

use grw::graph::{self, edge};
use grw::search::{self, dsl, Morphism, Session};
use grw::Id;

const SIZE: Id = 100_000;
const AVG_DEGREE: Id = 6;
const NUM_LABELS: i32 = 5;
const PATTERN_SIZES: [usize; 3] = [3, 4, 5];
const PATTERNS_PER_SIZE: usize = 5;
const SEED: u64 = 42;

fn extract_subgraph(
    rng: &mut SmallRng,
    node_count: Id,
    edges: &[(Id, Id)],
    k: usize,
) -> Vec<(Id, Id)> {
    let start: Id = rng.random_range(0..node_count);
    let mut adj: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
    for &(a, b) in edges {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    }
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();
    visited.insert(start);
    queue.push_back(start);
    while visited.len() < k {
        let Some(current) = queue.pop_front() else { break };
        if let Some(neighbors) = adj.get(&current) {
            for &n in neighbors {
                if visited.len() >= k { break; }
                if visited.insert(n) { queue.push_back(n); }
            }
        }
    }
    edges
        .iter()
        .filter(|(a, b)| visited.contains(a) && visited.contains(b))
        .copied()
        .collect()
}

fn check<NV, ER>(label: &str, session: &Session<'_, NV, ER>)
where
    NV: Clone + Sync + Send + 'static,
    ER: graph::Edge + 'static,
    ER::Val: Send + Sync + Clone,
    ER::Slot: Send + Sync,
    ER::CsrStore: Send + Sync,
{
    let seq_count = session.iter().count();
    let par_count = session.par_iter().count();
    assert!(
        seq_count == par_count,
        "{label}: MISMATCH seq={seq_count} par={par_count}"
    );
    eprintln!("  {label}: {seq_count} matches OK");
}

fn build_undir_pattern(
    morphism: Morphism,
    edges: &[(Id, Id)],
) -> Vec<dsl::ClusterOps<(), edge::Undir<()>>> {
    type ER = edge::Undir<()>;
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
    vec![dsl::get(morphism, ops)]
}

fn build_undir_pattern_nv(
    morphism: Morphism,
    edges: &[(Id, Id)],
    node_labels: &BTreeMap<Id, i32>,
) -> Vec<dsl::ClusterOps<i32, edge::Undir<()>>> {
    type ER = edge::Undir<()>;
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<i32, ER>> = Vec::new();
    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);
        let na = node_labels[&a];
        let nb = node_labels[&b];
        let op: dsl::Op<i32, ER> = match (a_new, b_new) {
            (true, true) => (dsl::N::<i32, ER>(a).val(na) ^ dsl::N::<i32, ER>(b).val(nb)).into(),
            (true, false) => (dsl::N::<i32, ER>(a).val(na) ^ dsl::n::<i32, ER>(b)).into(),
            (false, true) => (dsl::n::<i32, ER>(a) ^ dsl::N::<i32, ER>(b).val(nb)).into(),
            (false, false) => (dsl::n::<i32, ER>(a) ^ dsl::n::<i32, ER>(b)).into(),
        };
        ops.push(op);
    }
    vec![dsl::get(morphism, ops)]
}

fn build_undir_pattern_ev(
    morphism: Morphism,
    edges: &[(Id, Id)],
    edge_labels: &BTreeMap<(Id, Id), i32>,
) -> Vec<dsl::ClusterOps<(), edge::Undir<i32>>> {
    type ER = edge::Undir<i32>;
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<(), ER>> = Vec::new();
    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);
        let el = edge_labels[&(a, b)];
        let op: dsl::Op<(), ER> = match (a_new, b_new) {
            (true, true) => (dsl::N::<(), ER>(a) & dsl::E::<(), ER>().val(el) ^ dsl::N::<(), ER>(b)).into(),
            (true, false) => (dsl::N::<(), ER>(a) & dsl::E::<(), ER>().val(el) ^ dsl::n::<(), ER>(b)).into(),
            (false, true) => (dsl::n::<(), ER>(a) & dsl::E::<(), ER>().val(el) ^ dsl::N::<(), ER>(b)).into(),
            (false, false) => (dsl::n::<(), ER>(a) & dsl::E::<(), ER>().val(el) ^ dsl::n::<(), ER>(b)).into(),
        };
        ops.push(op);
    }
    vec![dsl::get(morphism, ops)]
}

fn build_undir_pattern_nvev(
    morphism: Morphism,
    edges: &[(Id, Id)],
    node_labels: &BTreeMap<Id, i32>,
    edge_labels: &BTreeMap<(Id, Id), i32>,
) -> Vec<dsl::ClusterOps<i32, edge::Undir<i32>>> {
    type ER = edge::Undir<i32>;
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<i32, ER>> = Vec::new();
    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);
        let na = node_labels[&a];
        let nb = node_labels[&b];
        let el = edge_labels[&(a, b)];
        let op: dsl::Op<i32, ER> = match (a_new, b_new) {
            (true, true) => (dsl::N::<i32, ER>(a).val(na) & dsl::E::<i32, ER>().val(el) ^ dsl::N::<i32, ER>(b).val(nb)).into(),
            (true, false) => (dsl::N::<i32, ER>(a).val(na) & dsl::E::<i32, ER>().val(el) ^ dsl::n::<i32, ER>(b)).into(),
            (false, true) => (dsl::n::<i32, ER>(a) & dsl::E::<i32, ER>().val(el) ^ dsl::N::<i32, ER>(b).val(nb)).into(),
            (false, false) => (dsl::n::<i32, ER>(a) & dsl::E::<i32, ER>().val(el) ^ dsl::n::<i32, ER>(b)).into(),
        };
        ops.push(op);
    }
    vec![dsl::get(morphism, ops)]
}

fn build_dir_pattern(
    morphism: Morphism,
    edges: &[(Id, Id)],
) -> Vec<dsl::ClusterOps<(), edge::Dir<()>>> {
    type ER = edge::Dir<()>;
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<(), ER>> = Vec::new();
    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);
        let op: dsl::Op<(), ER> = match (a_new, b_new) {
            (true, true) => (dsl::N::<(), ER>(a) >> dsl::N::<(), ER>(b)).into(),
            (true, false) => (dsl::N::<(), ER>(a) >> dsl::n::<(), ER>(b)).into(),
            (false, true) => (dsl::n::<(), ER>(a) >> dsl::N::<(), ER>(b)).into(),
            (false, false) => (dsl::n::<(), ER>(a) >> dsl::n::<(), ER>(b)).into(),
        };
        ops.push(op);
    }
    vec![dsl::get(morphism, ops)]
}

fn build_dir_pattern_nv(
    morphism: Morphism,
    edges: &[(Id, Id)],
    node_labels: &BTreeMap<Id, i32>,
) -> Vec<dsl::ClusterOps<i32, edge::Dir<()>>> {
    type ER = edge::Dir<()>;
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<i32, ER>> = Vec::new();
    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);
        let na = node_labels[&a];
        let nb = node_labels[&b];
        let op: dsl::Op<i32, ER> = match (a_new, b_new) {
            (true, true) => (dsl::N::<i32, ER>(a).val(na) >> dsl::N::<i32, ER>(b).val(nb)).into(),
            (true, false) => (dsl::N::<i32, ER>(a).val(na) >> dsl::n::<i32, ER>(b)).into(),
            (false, true) => (dsl::n::<i32, ER>(a) >> dsl::N::<i32, ER>(b).val(nb)).into(),
            (false, false) => (dsl::n::<i32, ER>(a) >> dsl::n::<i32, ER>(b)).into(),
        };
        ops.push(op);
    }
    vec![dsl::get(morphism, ops)]
}

fn build_dir_pattern_ev(
    morphism: Morphism,
    edges: &[(Id, Id)],
    edge_labels: &BTreeMap<(Id, Id), i32>,
) -> Vec<dsl::ClusterOps<(), edge::Dir<i32>>> {
    type ER = edge::Dir<i32>;
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<(), ER>> = Vec::new();
    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);
        let el = edge_labels[&(a, b)];
        let op: dsl::Op<(), ER> = match (a_new, b_new) {
            (true, true) => (dsl::N::<(), ER>(a) & dsl::E::<(), ER>().val(el) >> dsl::N::<(), ER>(b)).into(),
            (true, false) => (dsl::N::<(), ER>(a) & dsl::E::<(), ER>().val(el) >> dsl::n::<(), ER>(b)).into(),
            (false, true) => (dsl::n::<(), ER>(a) & dsl::E::<(), ER>().val(el) >> dsl::N::<(), ER>(b)).into(),
            (false, false) => (dsl::n::<(), ER>(a) & dsl::E::<(), ER>().val(el) >> dsl::n::<(), ER>(b)).into(),
        };
        ops.push(op);
    }
    vec![dsl::get(morphism, ops)]
}

fn build_dir_pattern_nvev(
    morphism: Morphism,
    edges: &[(Id, Id)],
    node_labels: &BTreeMap<Id, i32>,
    edge_labels: &BTreeMap<(Id, Id), i32>,
) -> Vec<dsl::ClusterOps<i32, edge::Dir<i32>>> {
    type ER = edge::Dir<i32>;
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<i32, ER>> = Vec::new();
    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);
        let na = node_labels[&a];
        let nb = node_labels[&b];
        let el = edge_labels[&(a, b)];
        let op: dsl::Op<i32, ER> = match (a_new, b_new) {
            (true, true) => (dsl::N::<i32, ER>(a).val(na) & dsl::E::<i32, ER>().val(el) >> dsl::N::<i32, ER>(b).val(nb)).into(),
            (true, false) => (dsl::N::<i32, ER>(a).val(na) & dsl::E::<i32, ER>().val(el) >> dsl::n::<i32, ER>(b)).into(),
            (false, true) => (dsl::n::<i32, ER>(a) & dsl::E::<i32, ER>().val(el) >> dsl::N::<i32, ER>(b).val(nb)).into(),
            (false, false) => (dsl::n::<i32, ER>(a) & dsl::E::<i32, ER>().val(el) >> dsl::n::<i32, ER>(b)).into(),
        };
        ops.push(op);
    }
    vec![dsl::get(morphism, ops)]
}

fn build_anydir_pattern(
    morphism: Morphism,
    edges: &[(Id, Id)],
    is_directed: &BTreeMap<(Id, Id), bool>,
) -> Vec<dsl::ClusterOps<(), edge::Anydir<()>>> {
    type ER = edge::Anydir<()>;
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<(), ER>> = Vec::new();
    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);
        let directed = is_directed[&(a, b)];
        let op: dsl::Op<(), ER> = match (a_new, b_new, directed) {
            (true, true, true) => (dsl::N::<(), ER>(a) >> dsl::N::<(), ER>(b)).into(),
            (true, true, false) => (dsl::N::<(), ER>(a) ^ dsl::N::<(), ER>(b)).into(),
            (true, false, true) => (dsl::N::<(), ER>(a) >> dsl::n::<(), ER>(b)).into(),
            (true, false, false) => (dsl::N::<(), ER>(a) ^ dsl::n::<(), ER>(b)).into(),
            (false, true, true) => (dsl::n::<(), ER>(a) >> dsl::N::<(), ER>(b)).into(),
            (false, true, false) => (dsl::n::<(), ER>(a) ^ dsl::N::<(), ER>(b)).into(),
            (false, false, true) => (dsl::n::<(), ER>(a) >> dsl::n::<(), ER>(b)).into(),
            (false, false, false) => (dsl::n::<(), ER>(a) ^ dsl::n::<(), ER>(b)).into(),
        };
        ops.push(op);
    }
    vec![dsl::get(morphism, ops)]
}

fn build_anydir_pattern_nv(
    morphism: Morphism,
    edges: &[(Id, Id)],
    is_directed: &BTreeMap<(Id, Id), bool>,
    node_labels: &BTreeMap<Id, i32>,
) -> Vec<dsl::ClusterOps<i32, edge::Anydir<()>>> {
    type ER = edge::Anydir<()>;
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<i32, ER>> = Vec::new();
    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);
        let directed = is_directed[&(a, b)];
        let na = node_labels[&a];
        let nb = node_labels[&b];
        let op: dsl::Op<i32, ER> = match (a_new, b_new, directed) {
            (true, true, true) => (dsl::N::<i32, ER>(a).val(na) >> dsl::N::<i32, ER>(b).val(nb)).into(),
            (true, true, false) => (dsl::N::<i32, ER>(a).val(na) ^ dsl::N::<i32, ER>(b).val(nb)).into(),
            (true, false, true) => (dsl::N::<i32, ER>(a).val(na) >> dsl::n::<i32, ER>(b)).into(),
            (true, false, false) => (dsl::N::<i32, ER>(a).val(na) ^ dsl::n::<i32, ER>(b)).into(),
            (false, true, true) => (dsl::n::<i32, ER>(a) >> dsl::N::<i32, ER>(b).val(nb)).into(),
            (false, true, false) => (dsl::n::<i32, ER>(a) ^ dsl::N::<i32, ER>(b).val(nb)).into(),
            (false, false, true) => (dsl::n::<i32, ER>(a) >> dsl::n::<i32, ER>(b)).into(),
            (false, false, false) => (dsl::n::<i32, ER>(a) ^ dsl::n::<i32, ER>(b)).into(),
        };
        ops.push(op);
    }
    vec![dsl::get(morphism, ops)]
}

fn build_anydir_pattern_ev(
    morphism: Morphism,
    edges: &[(Id, Id)],
    is_directed: &BTreeMap<(Id, Id), bool>,
    edge_labels: &BTreeMap<(Id, Id), i32>,
) -> Vec<dsl::ClusterOps<(), edge::Anydir<i32>>> {
    type ER = edge::Anydir<i32>;
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<(), ER>> = Vec::new();
    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);
        let directed = is_directed[&(a, b)];
        let el = edge_labels[&(a, b)];
        let op: dsl::Op<(), ER> = match (a_new, b_new, directed) {
            (true, true, true) => (dsl::N::<(), ER>(a) & dsl::E::<(), ER>().val(el) >> dsl::N::<(), ER>(b)).into(),
            (true, true, false) => (dsl::N::<(), ER>(a) & dsl::E::<(), ER>().val(el) ^ dsl::N::<(), ER>(b)).into(),
            (true, false, true) => (dsl::N::<(), ER>(a) & dsl::E::<(), ER>().val(el) >> dsl::n::<(), ER>(b)).into(),
            (true, false, false) => (dsl::N::<(), ER>(a) & dsl::E::<(), ER>().val(el) ^ dsl::n::<(), ER>(b)).into(),
            (false, true, true) => (dsl::n::<(), ER>(a) & dsl::E::<(), ER>().val(el) >> dsl::N::<(), ER>(b)).into(),
            (false, true, false) => (dsl::n::<(), ER>(a) & dsl::E::<(), ER>().val(el) ^ dsl::N::<(), ER>(b)).into(),
            (false, false, true) => (dsl::n::<(), ER>(a) & dsl::E::<(), ER>().val(el) >> dsl::n::<(), ER>(b)).into(),
            (false, false, false) => (dsl::n::<(), ER>(a) & dsl::E::<(), ER>().val(el) ^ dsl::n::<(), ER>(b)).into(),
        };
        ops.push(op);
    }
    vec![dsl::get(morphism, ops)]
}

fn build_anydir_pattern_nvev(
    morphism: Morphism,
    edges: &[(Id, Id)],
    is_directed: &BTreeMap<(Id, Id), bool>,
    node_labels: &BTreeMap<Id, i32>,
    edge_labels: &BTreeMap<(Id, Id), i32>,
) -> Vec<dsl::ClusterOps<i32, edge::Anydir<i32>>> {
    type ER = edge::Anydir<i32>;
    let mut seen = BTreeSet::new();
    let mut ops: Vec<dsl::Op<i32, ER>> = Vec::new();
    for &(a, b) in edges {
        let a_new = seen.insert(a);
        let b_new = seen.insert(b);
        let directed = is_directed[&(a, b)];
        let na = node_labels[&a];
        let nb = node_labels[&b];
        let el = edge_labels[&(a, b)];
        let op: dsl::Op<i32, ER> = match (a_new, b_new, directed) {
            (true, true, true) => (dsl::N::<i32, ER>(a).val(na) & dsl::E::<i32, ER>().val(el) >> dsl::N::<i32, ER>(b).val(nb)).into(),
            (true, true, false) => (dsl::N::<i32, ER>(a).val(na) & dsl::E::<i32, ER>().val(el) ^ dsl::N::<i32, ER>(b).val(nb)).into(),
            (true, false, true) => (dsl::N::<i32, ER>(a).val(na) & dsl::E::<i32, ER>().val(el) >> dsl::n::<i32, ER>(b)).into(),
            (true, false, false) => (dsl::N::<i32, ER>(a).val(na) & dsl::E::<i32, ER>().val(el) ^ dsl::n::<i32, ER>(b)).into(),
            (false, true, true) => (dsl::n::<i32, ER>(a) & dsl::E::<i32, ER>().val(el) >> dsl::N::<i32, ER>(b).val(nb)).into(),
            (false, true, false) => (dsl::n::<i32, ER>(a) & dsl::E::<i32, ER>().val(el) ^ dsl::N::<i32, ER>(b).val(nb)).into(),
            (false, false, true) => (dsl::n::<i32, ER>(a) & dsl::E::<i32, ER>().val(el) >> dsl::n::<i32, ER>(b)).into(),
            (false, false, false) => (dsl::n::<i32, ER>(a) & dsl::E::<i32, ER>().val(el) ^ dsl::n::<i32, ER>(b)).into(),
        };
        ops.push(op);
    }
    vec![dsl::get(morphism, ops)]
}

struct UndirData {
    edges: Vec<(Id, Id)>,
    node_labels: BTreeMap<Id, i32>,
    edge_labels: BTreeMap<(Id, Id), i32>,
}

fn gen_undir_data(rng: &mut SmallRng) -> UndirData {
    let target_edges = (SIZE as u64 * AVG_DEGREE as u64) / 2;
    let mut seen = BTreeSet::new();
    while (seen.len() as u64) < target_edges {
        let a: Id = rng.random_range(0..SIZE);
        let b: Id = rng.random_range(0..SIZE);
        if a == b { continue; }
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        seen.insert((lo, hi));
    }
    let edges: Vec<(Id, Id)> = seen.into_iter().collect();
    let node_labels: BTreeMap<Id, i32> = (0..SIZE).map(|i| (i, rng.random_range(0..NUM_LABELS))).collect();
    let edge_labels: BTreeMap<(Id, Id), i32> = edges.iter().map(|&e| (e, rng.random_range(0..NUM_LABELS))).collect();
    UndirData { edges, node_labels, edge_labels }
}

struct DirData {
    edges: Vec<(Id, Id)>,
    node_labels: BTreeMap<Id, i32>,
    edge_labels: BTreeMap<(Id, Id), i32>,
}

fn gen_dir_data(rng: &mut SmallRng) -> DirData {
    let target_edges = (SIZE as u64 * AVG_DEGREE as u64) / 2;
    let mut seen = BTreeSet::new();
    while (seen.len() as u64) < target_edges {
        let a: Id = rng.random_range(0..SIZE);
        let b: Id = rng.random_range(0..SIZE);
        if a == b { continue; }
        seen.insert((a, b));
    }
    let edges: Vec<(Id, Id)> = seen.into_iter().collect();
    let node_labels: BTreeMap<Id, i32> = (0..SIZE).map(|i| (i, rng.random_range(0..NUM_LABELS))).collect();
    let edge_labels: BTreeMap<(Id, Id), i32> = edges.iter().map(|&e| (e, rng.random_range(0..NUM_LABELS))).collect();
    DirData { edges, node_labels, edge_labels }
}

struct AnydirData {
    edges: Vec<(Id, Id)>,
    is_directed: BTreeMap<(Id, Id), bool>,
    node_labels: BTreeMap<Id, i32>,
    edge_labels: BTreeMap<(Id, Id), i32>,
}

fn gen_anydir_data(rng: &mut SmallRng) -> AnydirData {
    let target_edges = (SIZE as u64 * AVG_DEGREE as u64) / 2;
    let mut seen = BTreeSet::new();
    while (seen.len() as u64) < target_edges {
        let a: Id = rng.random_range(0..SIZE);
        let b: Id = rng.random_range(0..SIZE);
        if a == b { continue; }
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        seen.insert((lo, hi));
    }
    let edges: Vec<(Id, Id)> = seen.iter().copied().collect();
    let is_directed: BTreeMap<(Id, Id), bool> = edges.iter().map(|&e| (e, rng.random_range(0..4u32) == 0)).collect();
    let node_labels: BTreeMap<Id, i32> = (0..SIZE).map(|i| (i, rng.random_range(0..NUM_LABELS))).collect();
    let edge_labels: BTreeMap<(Id, Id), i32> = edges.iter().map(|&e| (e, rng.random_range(0..NUM_LABELS))).collect();
    AnydirData { edges, is_directed, node_labels, edge_labels }
}

fn run_patterns<F>(rng: &mut SmallRng, edges: &[(Id, Id)], mut run_one: F)
where
    F: FnMut(&[(Id, Id)], usize),
{
    let mut idx = 0usize;
    for &k in &PATTERN_SIZES {
        for _ in 0..PATTERNS_PER_SIZE {
            let pat = extract_subgraph(rng, SIZE, edges, k);
            if pat.is_empty() { continue; }
            run_one(&pat, idx);
            idx += 1;
        }
    }
}

#[test]
fn par_vs_seq_undir_unvalued() {
    let mut rng = SmallRng::seed_from_u64(SEED);
    let data = gen_undir_data(&mut rng);
    let undir_edges: Vec<edge::undir::E<Id>> = data.edges.iter().map(|&(a, b)| edge::undir::E::U(a, b)).collect();
    let node_ids: Vec<Id> = (0..SIZE).collect();
    let graph: graph::Undir0 = graph::Undir0::try_from((node_ids, undir_edges)).unwrap();
    eprintln!("undir unvalued: {}n/{}e", graph.node_count(), graph.edge_count());
    run_patterns(&mut rng, &data.edges, |pat, idx| {
        let clusters = build_undir_pattern(Morphism::SubIso, pat);
        let compiled = search::compile::<(), edge::Undir<()>>(clusters).unwrap();
        let session = Session::from_search(compiled, &graph).unwrap();
        check(&format!("undir/unval/{idx}"), &session);
    });
}

#[test]
fn par_vs_seq_undir_nv() {
    let mut rng = SmallRng::seed_from_u64(SEED + 1);
    let data = gen_undir_data(&mut rng);
    let undir_edges: Vec<edge::undir::E<Id>> = data.edges.iter().map(|&(a, b)| edge::undir::E::U(a, b)).collect();
    let nodes: Vec<(Id, i32)> = (0..SIZE).map(|i| (i, data.node_labels[&i])).collect();
    let graph: graph::UndirN<i32> = (nodes, undir_edges).try_into().unwrap();
    eprintln!("undir nv: {}n/{}e", graph.node_count(), graph.edge_count());
    run_patterns(&mut rng, &data.edges, |pat, idx| {
        let pat_nodes: BTreeSet<Id> = pat.iter().flat_map(|(a, b)| [*a, *b]).collect();
        let nl: BTreeMap<Id, i32> = pat_nodes.iter().map(|&n| (n, data.node_labels[&n])).collect();
        let clusters = build_undir_pattern_nv(Morphism::SubIso, pat, &nl);
        let compiled = search::compile::<i32, edge::Undir<()>>(clusters).unwrap();
        let session = Session::from_search(compiled, &graph).unwrap();
        check(&format!("undir/nv/{idx}"), &session);
    });
}

#[test]
fn par_vs_seq_undir_ev() {
    let mut rng = SmallRng::seed_from_u64(SEED + 2);
    let data = gen_undir_data(&mut rng);
    let edges_with_val: Vec<(edge::undir::E<Id>, i32)> = data.edges.iter().map(|&(a, b)| {
        (edge::undir::E::U(a, b), data.edge_labels[&(a, b)])
    }).collect();
    let node_ids: Vec<Id> = (0..SIZE).collect();
    let graph: graph::UndirE<i32> = (node_ids, edges_with_val).try_into().unwrap();
    eprintln!("undir ev: {}n/{}e", graph.node_count(), graph.edge_count());
    run_patterns(&mut rng, &data.edges, |pat, idx| {
        let el: BTreeMap<(Id, Id), i32> = pat.iter().map(|&e| (e, data.edge_labels[&e])).collect();
        let clusters = build_undir_pattern_ev(Morphism::SubIso, pat, &el);
        let compiled = search::compile::<(), edge::Undir<i32>>(clusters).unwrap();
        let session = Session::from_search(compiled, &graph).unwrap();
        check(&format!("undir/ev/{idx}"), &session);
    });
}

#[test]
fn par_vs_seq_undir_nvev() {
    let mut rng = SmallRng::seed_from_u64(SEED + 3);
    let data = gen_undir_data(&mut rng);
    let nodes: Vec<(Id, i32)> = (0..SIZE).map(|i| (i, data.node_labels[&i])).collect();
    let edges_with_val: Vec<(edge::undir::E<Id>, i32)> = data.edges.iter().map(|&(a, b)| {
        (edge::undir::E::U(a, b), data.edge_labels[&(a, b)])
    }).collect();
    let graph: graph::Undir<i32, i32> = (nodes, edges_with_val).try_into().unwrap();
    eprintln!("undir nvev: {}n/{}e", graph.node_count(), graph.edge_count());
    run_patterns(&mut rng, &data.edges, |pat, idx| {
        let pat_nodes: BTreeSet<Id> = pat.iter().flat_map(|(a, b)| [*a, *b]).collect();
        let nl: BTreeMap<Id, i32> = pat_nodes.iter().map(|&n| (n, data.node_labels[&n])).collect();
        let el: BTreeMap<(Id, Id), i32> = pat.iter().map(|&e| (e, data.edge_labels[&e])).collect();
        let clusters = build_undir_pattern_nvev(Morphism::SubIso, pat, &nl, &el);
        let compiled = search::compile::<i32, edge::Undir<i32>>(clusters).unwrap();
        let session = Session::from_search(compiled, &graph).unwrap();
        check(&format!("undir/nvev/{idx}"), &session);
    });
}

#[test]
fn par_vs_seq_dir_unvalued() {
    let mut rng = SmallRng::seed_from_u64(SEED + 10);
    let data = gen_dir_data(&mut rng);
    let dir_edges: Vec<edge::dir::E<Id>> = data.edges.iter().map(|&(a, b)| edge::dir::E::D(a, b)).collect();
    let node_ids: Vec<Id> = (0..SIZE).collect();
    let graph: graph::Dir0 = graph::Dir0::try_from((node_ids, dir_edges)).unwrap();
    eprintln!("dir unvalued: {}n/{}e", graph.node_count(), graph.edge_count());
    run_patterns(&mut rng, &data.edges, |pat, idx| {
        let clusters = build_dir_pattern(Morphism::SubIso, pat);
        let compiled = search::compile::<(), edge::Dir<()>>(clusters).unwrap();
        let session = Session::from_search(compiled, &graph).unwrap();
        check(&format!("dir/unval/{idx}"), &session);
    });
}

#[test]
fn par_vs_seq_dir_nv() {
    let mut rng = SmallRng::seed_from_u64(SEED + 11);
    let data = gen_dir_data(&mut rng);
    let dir_edges: Vec<edge::dir::E<Id>> = data.edges.iter().map(|&(a, b)| edge::dir::E::D(a, b)).collect();
    let nodes: Vec<(Id, i32)> = (0..SIZE).map(|i| (i, data.node_labels[&i])).collect();
    let graph: graph::DirN<i32> = (nodes, dir_edges).try_into().unwrap();
    eprintln!("dir nv: {}n/{}e", graph.node_count(), graph.edge_count());
    run_patterns(&mut rng, &data.edges, |pat, idx| {
        let pat_nodes: BTreeSet<Id> = pat.iter().flat_map(|(a, b)| [*a, *b]).collect();
        let nl: BTreeMap<Id, i32> = pat_nodes.iter().map(|&n| (n, data.node_labels[&n])).collect();
        let clusters = build_dir_pattern_nv(Morphism::SubIso, pat, &nl);
        let compiled = search::compile::<i32, edge::Dir<()>>(clusters).unwrap();
        let session = Session::from_search(compiled, &graph).unwrap();
        check(&format!("dir/nv/{idx}"), &session);
    });
}

#[test]
fn par_vs_seq_dir_ev() {
    let mut rng = SmallRng::seed_from_u64(SEED + 12);
    let data = gen_dir_data(&mut rng);
    let edges_with_val: Vec<(edge::dir::E<Id>, i32)> = data.edges.iter().map(|&(a, b)| {
        (edge::dir::E::D(a, b), data.edge_labels[&(a, b)])
    }).collect();
    let node_ids: Vec<Id> = (0..SIZE).collect();
    let graph: graph::DirE<i32> = (node_ids, edges_with_val).try_into().unwrap();
    eprintln!("dir ev: {}n/{}e", graph.node_count(), graph.edge_count());
    run_patterns(&mut rng, &data.edges, |pat, idx| {
        let el: BTreeMap<(Id, Id), i32> = pat.iter().map(|&e| (e, data.edge_labels[&e])).collect();
        let clusters = build_dir_pattern_ev(Morphism::SubIso, pat, &el);
        let compiled = search::compile::<(), edge::Dir<i32>>(clusters).unwrap();
        let session = Session::from_search(compiled, &graph).unwrap();
        check(&format!("dir/ev/{idx}"), &session);
    });
}

#[test]
fn par_vs_seq_dir_nvev() {
    let mut rng = SmallRng::seed_from_u64(SEED + 13);
    let data = gen_dir_data(&mut rng);
    let nodes: Vec<(Id, i32)> = (0..SIZE).map(|i| (i, data.node_labels[&i])).collect();
    let edges_with_val: Vec<(edge::dir::E<Id>, i32)> = data.edges.iter().map(|&(a, b)| {
        (edge::dir::E::D(a, b), data.edge_labels[&(a, b)])
    }).collect();
    let graph: graph::Dir<i32, i32> = (nodes, edges_with_val).try_into().unwrap();
    eprintln!("dir nvev: {}n/{}e", graph.node_count(), graph.edge_count());
    run_patterns(&mut rng, &data.edges, |pat, idx| {
        let pat_nodes: BTreeSet<Id> = pat.iter().flat_map(|(a, b)| [*a, *b]).collect();
        let nl: BTreeMap<Id, i32> = pat_nodes.iter().map(|&n| (n, data.node_labels[&n])).collect();
        let el: BTreeMap<(Id, Id), i32> = pat.iter().map(|&e| (e, data.edge_labels[&e])).collect();
        let clusters = build_dir_pattern_nvev(Morphism::SubIso, pat, &nl, &el);
        let compiled = search::compile::<i32, edge::Dir<i32>>(clusters).unwrap();
        let session = Session::from_search(compiled, &graph).unwrap();
        check(&format!("dir/nvev/{idx}"), &session);
    });
}

#[test]
fn par_vs_seq_anydir_unvalued() {
    let mut rng = SmallRng::seed_from_u64(SEED + 20);
    let data = gen_anydir_data(&mut rng);
    let anydir_edges: Vec<edge::anydir::E<Id>> = data.edges.iter().map(|&(a, b)| {
        if data.is_directed[&(a, b)] { edge::anydir::E::D(a, b) } else { edge::anydir::E::U(a, b) }
    }).collect();
    let node_ids: Vec<Id> = (0..SIZE).collect();
    let graph: graph::Anydir0 = graph::Anydir0::try_from((node_ids, anydir_edges)).unwrap();
    eprintln!("anydir unvalued: {}n/{}e", graph.node_count(), graph.edge_count());
    run_patterns(&mut rng, &data.edges, |pat, idx| {
        let dir_map: BTreeMap<(Id, Id), bool> = pat.iter().map(|&e| (e, data.is_directed[&e])).collect();
        let clusters = build_anydir_pattern(Morphism::SubIso, pat, &dir_map);
        let compiled = search::compile::<(), edge::Anydir<()>>(clusters).unwrap();
        let session = Session::from_search(compiled, &graph).unwrap();
        check(&format!("anydir/unval/{idx}"), &session);
    });
}

#[test]
fn par_vs_seq_anydir_nv() {
    let mut rng = SmallRng::seed_from_u64(SEED + 21);
    let data = gen_anydir_data(&mut rng);
    let anydir_edges: Vec<edge::anydir::E<Id>> = data.edges.iter().map(|&(a, b)| {
        if data.is_directed[&(a, b)] { edge::anydir::E::D(a, b) } else { edge::anydir::E::U(a, b) }
    }).collect();
    let nodes: Vec<(Id, i32)> = (0..SIZE).map(|i| (i, data.node_labels[&i])).collect();
    let graph: graph::AnydirN<i32> = (nodes, anydir_edges).try_into().unwrap();
    eprintln!("anydir nv: {}n/{}e", graph.node_count(), graph.edge_count());
    run_patterns(&mut rng, &data.edges, |pat, idx| {
        let pat_nodes: BTreeSet<Id> = pat.iter().flat_map(|(a, b)| [*a, *b]).collect();
        let nl: BTreeMap<Id, i32> = pat_nodes.iter().map(|&n| (n, data.node_labels[&n])).collect();
        let dir_map: BTreeMap<(Id, Id), bool> = pat.iter().map(|&e| (e, data.is_directed[&e])).collect();
        let clusters = build_anydir_pattern_nv(Morphism::SubIso, pat, &dir_map, &nl);
        let compiled = search::compile::<i32, edge::Anydir<()>>(clusters).unwrap();
        let session = Session::from_search(compiled, &graph).unwrap();
        check(&format!("anydir/nv/{idx}"), &session);
    });
}

#[test]
fn par_vs_seq_anydir_ev() {
    let mut rng = SmallRng::seed_from_u64(SEED + 22);
    let data = gen_anydir_data(&mut rng);
    let edges_with_val: Vec<(edge::anydir::E<Id>, i32)> = data.edges.iter().map(|&(a, b)| {
        let edef = if data.is_directed[&(a, b)] { edge::anydir::E::D(a, b) } else { edge::anydir::E::U(a, b) };
        (edef, data.edge_labels[&(a, b)])
    }).collect();
    let node_ids: Vec<Id> = (0..SIZE).collect();
    let graph: graph::AnydirE<i32> = (node_ids, edges_with_val).try_into().unwrap();
    eprintln!("anydir ev: {}n/{}e", graph.node_count(), graph.edge_count());
    run_patterns(&mut rng, &data.edges, |pat, idx| {
        let el: BTreeMap<(Id, Id), i32> = pat.iter().map(|&e| (e, data.edge_labels[&e])).collect();
        let dir_map: BTreeMap<(Id, Id), bool> = pat.iter().map(|&e| (e, data.is_directed[&e])).collect();
        let clusters = build_anydir_pattern_ev(Morphism::SubIso, pat, &dir_map, &el);
        let compiled = search::compile::<(), edge::Anydir<i32>>(clusters).unwrap();
        let session = Session::from_search(compiled, &graph).unwrap();
        check(&format!("anydir/ev/{idx}"), &session);
    });
}

#[test]
fn par_vs_seq_anydir_nvev() {
    let mut rng = SmallRng::seed_from_u64(SEED + 23);
    let data = gen_anydir_data(&mut rng);
    let nodes: Vec<(Id, i32)> = (0..SIZE).map(|i| (i, data.node_labels[&i])).collect();
    let edges_with_val: Vec<(edge::anydir::E<Id>, i32)> = data.edges.iter().map(|&(a, b)| {
        let edef = if data.is_directed[&(a, b)] { edge::anydir::E::D(a, b) } else { edge::anydir::E::U(a, b) };
        (edef, data.edge_labels[&(a, b)])
    }).collect();
    let graph: graph::Anydir<i32, i32> = (nodes, edges_with_val).try_into().unwrap();
    eprintln!("anydir nvev: {}n/{}e", graph.node_count(), graph.edge_count());
    run_patterns(&mut rng, &data.edges, |pat, idx| {
        let pat_nodes: BTreeSet<Id> = pat.iter().flat_map(|(a, b)| [*a, *b]).collect();
        let nl: BTreeMap<Id, i32> = pat_nodes.iter().map(|&n| (n, data.node_labels[&n])).collect();
        let el: BTreeMap<(Id, Id), i32> = pat.iter().map(|&e| (e, data.edge_labels[&e])).collect();
        let dir_map: BTreeMap<(Id, Id), bool> = pat.iter().map(|&e| (e, data.is_directed[&e])).collect();
        let clusters = build_anydir_pattern_nvev(Morphism::SubIso, pat, &dir_map, &nl, &el);
        let compiled = search::compile::<i32, edge::Anydir<i32>>(clusters).unwrap();
        let session = Session::from_search(compiled, &graph).unwrap();
        check(&format!("anydir/nvev/{idx}"), &session);
    });
}
