use std::collections::{BTreeSet, HashSet};
use std::hint::black_box;
use std::time::Instant;

use grw::Id;
use grw::graph::edge::{self, Undir};
use grw::graph::{Graph, MGraph, VGraph};
use grw::modify::{self, E, N, e, n, x};
use grw::search::{self, Morphism, RevCsr, Search, Seq, dsl};

type ER = Undir<i32>;
type M = MGraph<i32, ER>;
type V = VGraph<i32, ER>;

const AVG_DEGREE: Id = 4;
const INCREMENTAL_STEPS: usize = 1000;
const CLONE_BATCH: usize = 1000;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let size: Id = args.get(1).map(|s| s.parse().expect("invalid size")).unwrap_or(10000);
    let iters: usize = args.get(2).map(|s| s.parse().expect("invalid iters")).unwrap_or(3);
    let seed: u64 = args.get(3).map(|s| s.parse().expect("invalid seed")).unwrap_or(42);

    let (node_vals, edges) = gen_shape(size, seed);
    eprintln!("shape: {}n/{}e avg_degree={}", node_vals.len(), edges.len(), AVG_DEGREE);

    let mgraph = build_mgraph(&node_vals, &edges);
    let vgraph = build_vgraph(&node_vals, &edges);

    assert_eq!(mgraph.node_count(), vgraph.node_count(), "node count mismatch");
    assert_eq!(mgraph.edge_count(), vgraph.edge_count(), "edge count mismatch");

    let clusters = triangle_pattern();
    let Search::Resolved(resolved) =
        search::compile::<i32, ER>(clusters).expect("triangle pattern compile failed")
    else {
        panic!("triangle pattern must resolve, not bind against an outer context")
    };
    let query = resolved.into_query();

    let m_count = Seq::search(&query, &mgraph.index(RevCsr)).count();
    let v_count = Seq::search(&query, &vgraph.index(RevCsr)).count();
    assert_eq!(m_count, v_count, "triangle search count mismatch: mgraph={m_count} vgraph={v_count}");

    let plan = gen_incremental_plan(&edges, size, seed.wrapping_add(1), INCREMENTAL_STEPS);

    let build_m = timed_median(iters, 1, || (), |_| build_mgraph(&node_vals, &edges));
    let build_v = timed_median(iters, 1, || (), |_| build_vgraph(&node_vals, &edges));

    let incr_m = timed_median(iters, 1, || mgraph.clone(), |g: &mut M| {
        for &(aa, ab, av, ra, rb) in &plan {
            g.modify(incr_ops(aa, ab, av, ra, rb)).expect("valid incremental batch");
        }
    });

    let incr_v = timed_median(iters, 1, || vgraph.clone(), |g: &mut V| {
        for &(aa, ab, av, ra, rb) in &plan {
            let (next, _) = g.modify(incr_ops(aa, ab, av, ra, rb)).expect("valid incremental batch");
            *g = next;
        }
    });

    let scan_m = timed_median(iters, 1, || (), |_| scan_edges(&mgraph));
    let scan_v = timed_median(iters, 1, || (), |_| scan_edges(&vgraph));

    let index_m = timed_median(iters, 1, || (), |_| mgraph.index(RevCsr));
    let index_v = timed_median(iters, 1, || (), |_| vgraph.index(RevCsr));

    let m_idx = mgraph.index(RevCsr);
    let v_idx = vgraph.index(RevCsr);

    let search_m = timed_median(iters, 1, || (), |_| Seq::search(&query, &m_idx).count());
    let search_v = timed_median(iters, 1, || (), |_| Seq::search(&query, &v_idx).count());

    // Clone row is batched (CLONE_BATCH clones per timed sample, elapsed / CLONE_BATCH):
    // a single-shot Instant pair around one O(1) VGraph clone is dominated by
    // Instant::now()/elapsed() overhead, not the clone itself. Each clone is
    // black_box'd (and, per `timed_median`'s batch>1 path, dropped) *inside*
    // the timed loop rather than collected into a Vec of CLONE_BATCH held
    // graphs — holding that many live MGraph clones simultaneously at
    // measured sizes would be a real memory cost the VGraph side wouldn't
    // pay, which would itself bias the comparison.
    let clone_m = timed_median(iters, CLONE_BATCH, || (), |_| mgraph.clone());
    let clone_v = timed_median(iters, CLONE_BATCH, || (), |_| vgraph.clone());

    println!(
        "size={size} iters={iters} seed={seed} nodes={} edges={} triangle_matches={m_count}",
        mgraph.node_count(),
        mgraph.edge_count()
    );
    println!();
    println!("| measurement | MGraph | VGraph | ratio (V/M) |");
    println!("|---|---|---|---|");
    print_row("build", build_m, build_v);
    print_row("incremental modify (1000 batches)", incr_m, incr_v);
    print_row("full scan (iter_edges)", scan_m, scan_v);
    print_row("index build (RevCsr)", index_m, index_v);
    print_row("search (triangle, Mono)", search_m, search_v);
    print_row("snapshot/clone", clone_m, clone_v);
}

fn print_row(name: &str, m_ms: f64, v_ms: f64) {
    println!("| {name} | {} | {} | {:.2} |", fmt_ms(m_ms), fmt_ms(v_ms), v_ms / m_ms);
}

fn elapsed_ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn fmt_ms(ms: f64) -> String {
    if ms < 1.0 { format!("{:.2} \u{b5}s", ms * 1000.0) } else { format!("{ms:.2} ms") }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

// batch == 1: untimed `setup()`, then `work()` timed alone, `out`/`s` black_box'd
// only after `elapsed_ms` is captured — drop excluded from every such row, same
// as before this helper existed. batch > 1 (the clone row): `work()` runs
// `batch` times inside the timed span, each result black_box'd (and dropped)
// *inside* the loop so none are elided and none pile up in memory; elapsed is
// divided by `batch`.
fn timed_median<S, T>(
    iters: usize,
    batch: usize,
    mut setup: impl FnMut() -> S,
    mut work: impl FnMut(&mut S) -> T,
) -> f64 {
    median(
        (0..iters)
            .map(|_| {
                let mut s = setup();
                if batch == 1 {
                    let t = Instant::now();
                    let out = work(&mut s);
                    let ms = elapsed_ms(t);
                    black_box(out);
                    black_box(s);
                    ms
                } else {
                    let t = Instant::now();
                    for _ in 0..batch {
                        black_box(work(&mut s));
                    }
                    let ms = elapsed_ms(t);
                    black_box(s);
                    ms / batch as f64
                }
            })
            .collect(),
    )
}

fn scan_edges<G: Graph<i32, ER>>(g: &G) -> i64 {
    let mut acc: i64 = 0;
    for (_, _, _, v) in g.iter_edges() {
        acc = acc.wrapping_add(black_box(*v) as i64);
    }
    acc
}

fn incr_ops(aa: Id, ab: Id, av: i32, ra: Id, rb: Id) -> Vec<modify::Node<i32, ER>> {
    vec![
        (x::<i32, ER>(aa) & E::<i32, ER>().val(av) ^ x::<i32, ER>(ab)).into(),
        (x::<i32, ER>(ra) & !e::<i32, ER>() ^ x::<i32, ER>(rb)).into(),
    ]
}

fn build_mgraph(node_vals: &[i32], edges: &[(Id, Id, i32)]) -> M {
    let nodes: Vec<(Id, i32)> = node_vals.iter().enumerate().map(|(i, &v)| (i as Id, v)).collect();
    let evs: Vec<(edge::undir::E<Id>, i32)> =
        edges.iter().map(|&(a, b, v)| (edge::undir::E::U(a, b), v)).collect();
    (nodes, evs).try_into().expect("mgraph build failed")
}

fn build_vgraph(node_vals: &[i32], edges: &[(Id, Id, i32)]) -> V {
    // Single-Fragment bulk load: one `modify()` call carrying every node and
    // edge op. VGraph exposes no direct bulk constructor (no `TryFrom` like
    // MGraph's), so this is the fastest path the modify/batch API offers —
    // one version bump instead of N+2N sequential ones. It still walks the
    // trie once per node/edge internally (no bulk-specific fast path), so it
    // does not match MGraph's flat Vec-based constructor.
    let mut ops: Vec<modify::Node<i32, ER>> = Vec::with_capacity(node_vals.len() + edges.len());
    for (i, &v) in node_vals.iter().enumerate() {
        ops.push(N::<i32, ER>(i as Id).val(v).into());
    }
    for &(a, b, v) in edges {
        ops.push((n::<i32, ER>(a) & E::<i32, ER>().val(v) ^ n::<i32, ER>(b)).into());
    }
    let g0 = V::new();
    let (g, _) = g0.modify(ops).expect("vgraph build failed");
    g
}

fn triangle_pattern() -> Vec<dsl::ClusterOps<i32, ER>> {
    vec![dsl::get(
        Morphism::Mono,
        vec![
            (dsl::N::<i32, ER>(0) ^ dsl::N::<i32, ER>(1)).into(),
            (dsl::n::<i32, ER>(1) ^ dsl::N::<i32, ER>(2)).into(),
            (dsl::n::<i32, ER>(0) ^ dsl::n::<i32, ER>(2)).into(),
        ],
    )]
}

fn gen_shape(size: Id, seed: u64) -> (Vec<i32>, Vec<(Id, Id, i32)>) {
    let mut rng = Lcg::new(seed);
    let target_edges = (size as u64 * AVG_DEGREE as u64) / 2;
    let mut edge_set = BTreeSet::new();
    while (edge_set.len() as u64) < target_edges {
        let a = rng.next_bounded(size);
        let b = rng.next_bounded(size);
        if a == b {
            continue;
        }
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        edge_set.insert((lo, hi));
    }

    let node_vals: Vec<i32> = (0..size).map(|_| rng.next_bounded(1000) as i32).collect();
    let edges: Vec<(Id, Id, i32)> = edge_set
        .into_iter()
        .map(|(a, b)| {
            let v = rng.next_bounded(1000) as i32;
            (a, b, v)
        })
        .collect();
    (node_vals, edges)
}

fn gen_incremental_plan(
    edges: &[(Id, Id, i32)],
    size: Id,
    seed: u64,
    steps: usize,
) -> Vec<(Id, Id, i32, Id, Id)> {
    let mut member: HashSet<(Id, Id)> = edges.iter().map(|&(a, b, _)| (a, b)).collect();
    let mut list: Vec<(Id, Id)> = edges.iter().map(|&(a, b, _)| (a, b)).collect();
    let mut rng = Lcg::new(seed);
    let mut plan = Vec::with_capacity(steps);

    for _ in 0..steps {
        let idx = rng.next_bounded(list.len() as Id) as usize;
        let (ra, rb) = list[idx];
        list.swap_remove(idx);
        member.remove(&(ra, rb));

        loop {
            let a = rng.next_bounded(size);
            let b = rng.next_bounded(size);
            if a == b {
                continue;
            }
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            if (lo, hi) == (ra, rb) {
                continue;
            }
            if member.contains(&(lo, hi)) {
                continue;
            }
            member.insert((lo, hi));
            list.push((lo, hi));
            let val = rng.next_bounded(1000) as i32;
            plan.push((lo, hi, val, ra, rb));
            break;
        }
    }

    plan
}

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn next_bounded(&mut self, bound: Id) -> Id {
        (self.next() % bound as u64) as Id
    }
}
