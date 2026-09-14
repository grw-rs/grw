//! Regression: `CsrAdj::build` sized `node_vals` by the target's id space but
//! only wrote the slots of live nodes, then transmuted the `Vec<MaybeUninit<NV>>`
//! to `Vec<NV>` — so dropping an index over a graph with deleted nodes dropped
//! uninitialised values. Every case here builds and drops an index whose target
//! has gaps, with a heap-owning `NV`, enough times that a bad free is loud.

use grw::graph::{self, edge, Graph as _, MGraph, VGraph};
use grw::search::{Par, RevCsr, RevCsrVal, Search, Seq};
use grw::{modify, search, Id};
use rayon::iter::ParallelIterator;
use std::alloc::{GlobalAlloc, Layout, System};

/// Fresh pages from the OS arrive zeroed, and a zeroed `String` header drops
/// without freeing anything — so an uninitialised-drop bug hides unless the
/// memory is dirty. Poisoning every allocation makes the regression
/// deterministic instead of allocator-state dependent.
struct Poison;

/// SAFETY: every method forwards to `System` with the same `ptr`/`layout` it was
/// given and only writes inside `[ptr, ptr + layout.size())`, which `System`
/// guarantees it owns for the returned (non-null) pointer, so the `GlobalAlloc`
/// contract is exactly `System`'s.
unsafe impl GlobalAlloc for Poison {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            unsafe { std::ptr::write_bytes(ptr, 0xAB, layout.size()) };
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { std::ptr::write_bytes(ptr, 0xCD, layout.size()) };
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static POISON: Poison = Poison;

type DirEdge = edge::Dir<()>;
type MG = MGraph<String, DirEdge>;
type VG = VGraph<String, DirEdge>;

const NODES: Id = 24;
const DELETED: [Id; 5] = [1, 4, 9, 16, 21];
const ROUNDS: usize = 100;

fn label(i: Id) -> String {
    format!("node-{i}-{}", "payload".repeat(4))
}

fn ops_with_gaps() -> Vec<modify::Node<String, DirEdge>> {
    let mut ops: Vec<modify::Node<String, DirEdge>> = Vec::new();
    for i in 0..NODES {
        ops.push(modify::N::<String, DirEdge>(i).val(label(i)).into());
    }
    for i in 0..NODES - 1 {
        ops.push((modify::n::<String, DirEdge>(i) >> modify::n::<String, DirEdge>(i + 1)).into());
    }
    for i in 0..NODES - 2 {
        ops.push((modify::n::<String, DirEdge>(i) >> modify::n::<String, DirEdge>(i + 2)).into());
    }
    ops
}

fn delete_ops() -> Vec<modify::Node<String, DirEdge>> {
    DELETED
        .iter()
        .map(|&i| (!modify::X::<String, DirEdge>(i)).into())
        .collect()
}

fn mgraph_with_gaps() -> MG {
    let mut g = MG::default();
    g.modify(ops_with_gaps()).unwrap();
    g.modify(delete_ops()).unwrap();
    g
}

fn vgraph_with_gaps() -> VG {
    let g0 = VG::new();
    let (g1, _) = g0.modify(ops_with_gaps()).unwrap();
    let (g2, _) = g1.modify(delete_ops()).unwrap();
    g2
}

fn assert_has_gaps<G: graph::Graph<String, DirEdge>>(g: &G, what: &str) {
    assert_eq!(g.node_count(), (NODES as usize) - DELETED.len(), "{what}: node count");
    assert!(
        g.id_space_len() > g.node_count(),
        "{what}: deletions must leave the id space sparse, else this test proves nothing"
    );
}

/// `Seq::search` / `Par::search` take `search::Graph`, i.e. the `RevCsr` tier
/// specifically; `RevCsrVal` has no search entry point of its own. It wraps the
/// same `CsrAdj`, so building and dropping one covers the same hazard.
fn two_path_matches<G: graph::Graph<String, DirEdge> + Sync>(g: &G) -> (usize, usize) {
    let Search::Resolved(r): Search<String, DirEdge> = search![
        get(Morphism::Mono) {
            N(0) >> (N(1) >> N(2))
        }
    ]
    .unwrap() else {
        panic!("a two-hop path is a resolved query")
    };
    let query = r.query();

    let csr = g.index(RevCsr);
    let seq_csr = Seq::search(query, &csr).unwrap().count();
    let par_csr = Par::search(query, &csr).unwrap().count();
    drop(csr);

    drop(g.index(RevCsrVal));

    (seq_csr, par_csr)
}

fn expected_two_paths() -> usize {
    let live: Vec<Id> = (0..NODES).filter(|i| !DELETED.contains(i)).collect();
    let edge = |a: Id, b: Id| {
        (b == a + 1 || b == a + 2) && live.contains(&a) && live.contains(&b)
    };
    let mut n = 0;
    for &a in &live {
        for &b in &live {
            for &c in &live {
                if a != b && b != c && a != c && edge(a, b) && edge(b, c) {
                    n += 1;
                }
            }
        }
    }
    n
}

#[test]
fn csr_index_over_a_sparse_id_space_survives_repeated_build_and_drop() {
    let m = mgraph_with_gaps();
    let v = vgraph_with_gaps();
    assert_has_gaps(&m, "mgraph");
    assert_has_gaps(&v, "vgraph");

    let expected = expected_two_paths();
    assert!(expected > 0, "fixture must actually match something");

    for round in 0..ROUNDS {
        let got_m = two_path_matches(&m);
        let got_v = two_path_matches(&v);
        assert_eq!(
            got_m,
            (expected, expected),
            "mgraph round {round}: (seq, par)"
        );
        assert_eq!(got_v, got_m, "vgraph round {round} must agree with mgraph");
    }
}

#[test]
fn node_values_of_live_nodes_are_intact_after_gaps() {
    let m = mgraph_with_gaps();
    let v = vgraph_with_gaps();

    for i in 0..NODES {
        let want = if DELETED.contains(&i) { None } else { Some(label(i)) };
        assert_eq!(m.node_val(grw::id::N(i)).cloned(), want, "mgraph node {i}");
        assert_eq!(v.node_val(grw::id::N(i)).cloned(), want, "vgraph node {i}");
    }
}
