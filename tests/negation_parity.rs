use grw::*;
use grw::search::{self, Session};

type UER = grw::graph::edge::Undir<()>;
type DER = grw::graph::edge::Dir<()>;
type AER = grw::graph::edge::Anydir<()>;

fn count<NV: Clone, ER: graph::Edge + 'static>(
    g: &graph::Graph<NV, ER>,
    compiled: search::Search<NV, ER>,
) -> usize
where
    ER::Val: Clone,
{
    let session = Session::from_search(compiled, g).unwrap();
    session.iter().count()
}

fn count_result<NV: Clone, ER: graph::Edge + 'static>(
    g: &graph::Graph<NV, ER>,
    compiled: Result<search::Search<NV, ER>, search::error::Search>,
) -> Result<usize, String>
where
    ER::Val: Clone,
{
    match compiled {
        Err(e) => Err(format!("compile: {e}")),
        Ok(s) => match Session::from_search(s, g) {
            Err(e) => Err(format!("session: {e}")),
            Ok(session) => Ok(session.iter().count()),
        }
    }
}

fn parity(label: &str, neg_get: usize, ban_equiv: usize) {
    let status = if neg_get == ban_equiv { "PARITY" } else { "DIVERGE" };
    println!("{status}: {label}: neg_get={neg_get} ban_equiv={ban_equiv}");
}

fn parity_result(label: &str, neg_get: Result<usize, String>, ban_equiv: Result<usize, String>) {
    match (&neg_get, &ban_equiv) {
        (Ok(a), Ok(b)) => {
            let status = if a == b { "PARITY" } else { "DIVERGE" };
            println!("{status}: {label}: neg_get={a} ban_equiv={b}");
        }
        _ => {
            println!("RESULT: {label}: neg_get={neg_get:?} ban_equiv={ban_equiv:?}");
        }
    }
}

// ============================================================================
// § All-Negated Patterns
// ============================================================================

#[test]
fn all_negated_nonempty_graph() {
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        ban(Mono) { N_() }
    ].unwrap());

    parity("all_negated_nonempty", neg_get, ban_equiv);
}

#[test]
fn all_negated_empty_graph() {
    let g: graph::Undir0 = graph![<(), UER>;].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        ban(Mono) { N_() }
    ].unwrap());

    parity("all_negated_empty", neg_get, ban_equiv);
}

#[test]
fn all_negated_with_test_no_match() {
    let g: graph::Undir<i32, ()> = graph![N(0).val(1) ^ N(1).val(2)].unwrap();

    let neg_get = count(&g, search![<i32, UER>;
        get(Mono) { !N_().test(|v: &i32| *v > 100) }
    ].unwrap());

    let ban_equiv = count(&g, search![<i32, UER>;
        ban(Mono) { N_().test(|v: &i32| *v > 100) }
    ].unwrap());

    parity("all_negated_test_no_match", neg_get, ban_equiv);
}

#[test]
fn all_negated_with_test_has_match() {
    let g: graph::Undir<i32, ()> = graph![N(0).val(1) ^ N(1).val(200)].unwrap();

    let neg_get = count(&g, search![<i32, UER>;
        get(Mono) { !N_().test(|v: &i32| *v > 100) }
    ].unwrap());

    let ban_equiv = count(&g, search![<i32, UER>;
        ban(Mono) { N_().test(|v: &i32| *v > 100) }
    ].unwrap());

    parity("all_negated_test_has_match", neg_get, ban_equiv);
}

// ============================================================================
// § Negated Freestanding Nodes (with positive edges)
// ============================================================================

#[test]
fn negated_freestanding_isolated_exists() {
    let g: graph::Undir0 = graph![N(0) ^ N(1), N(2)].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_() }
    ].unwrap());

    parity("negated_freestanding_isolated", neg_get, ban_equiv);
}

#[test]
fn negated_freestanding_no_isolated() {
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_() }
    ].unwrap());

    parity("negated_freestanding_no_isolated", neg_get, ban_equiv);
}

#[test]
fn negated_freestanding_val() {
    let g: graph::Undir<i32, ()> = graph![
        N(0).val(10) ^ N(1).val(20),
        N(2).val(30)
    ].unwrap();

    let neg_get = count(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1), !N_().val(30) }
    ].unwrap());

    let ban_equiv = count(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_().val(30) }
    ].unwrap());

    parity("negated_freestanding_val", neg_get, ban_equiv);
}

#[test]
fn negated_freestanding_test() {
    let g: graph::Undir<i32, ()> = graph![
        N(0).val(10) ^ N(1).val(20),
        N(2).val(30)
    ].unwrap();

    let neg_get = count(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1), !N_().test(|v: &i32| *v > 25) }
    ].unwrap());

    let ban_equiv = count(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_().test(|v: &i32| *v > 25) }
    ].unwrap());

    parity("negated_freestanding_test", neg_get, ban_equiv);
}

// ============================================================================
// § Negated Connected Nodes (node with edges)
// ============================================================================

#[test]
fn negated_connected_neighbor() {
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        n(1) ^ N(2)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) ^ !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(1) ^ N_() }
    ].unwrap());

    parity("negated_connected_neighbor", neg_get, ban_equiv);
}

#[test]
fn negated_connected_neighbor_triangle() {
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        n(1) ^ N(2),
        n(0) ^ n(2)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Homo) { N(0) ^ N(1) ^ !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Homo) { N(0) ^ N(1) },
        ban(Homo) { n(1) ^ N_() }
    ].unwrap());

    parity("negated_connected_neighbor_triangle", neg_get, ban_equiv);
}

// ============================================================================
// § Negated Edges
// ============================================================================

#[test]
fn negated_edge_different_slot_anydir() {
    let g: graph::Anydir0 = graph![
        N(0) >> N(1),
        N(2)
    ].unwrap();

    let neg_get = count(&g, search![<(), AER>;
        get(Mono) { N(0) >> N(1), n(0) & !E() << n(1) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), AER>;
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(0) << n(1) }
    ].unwrap());

    parity("negated_edge_different_slot_anydir", neg_get, ban_equiv);
}

#[test]
fn negated_edge_undir_no_edge() {
    let g: graph::Undir0 = graph![N(0), N(1)].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0), N(1), n(0) & !E() ^ n(1) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0), N(1) },
        ban(Mono) { n(0) ^ n(1) }
    ].unwrap());

    parity("negated_edge_undir_no_edge", neg_get, ban_equiv);
}

#[test]
fn negated_edge_undir_edge_exists() {
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0), N(1), n(0) & !E() ^ n(1) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0), N(1) },
        ban(Mono) { n(0) ^ n(1) }
    ].unwrap());

    parity("negated_edge_undir_edge_exists", neg_get, ban_equiv);
}

// ============================================================================
// § Negated Edge with Negated Node
// ============================================================================

#[test]
fn negated_node_and_negated_edge() {
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        n(1) ^ N(2)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), !N(2) & !E() ^ n(1) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N(2) ^ n(1) }
    ].unwrap());

    parity("negated_node_and_negated_edge", neg_get, ban_equiv);
}

// ============================================================================
// § Negated Context Nodes (X = Exist, pinned to graph node)
// ============================================================================

#[test]
fn negated_context_val() {
    let g: graph::Undir<i32, ()> = graph![
        N(0).val(10) ^ N(1).val(20),
        n(1) ^ N(2).val(30),
        n(0) ^ n(2)
    ].unwrap();

    let neg_get = count_result(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1), !X(2).val(30) }
    ]);

    let ban_equiv = count_result(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { X(2).val(30) }
    ]);

    parity_result("negated_context_val", neg_get, ban_equiv);
}

#[test]
fn negated_context_test() {
    let g: graph::Undir<i32, ()> = graph![
        N(0).val(10) ^ N(1).val(20),
        n(1) ^ N(2).val(30),
        n(0) ^ n(2)
    ].unwrap();

    let neg_get = count_result(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1), !X(2).test(|v: &i32| *v > 25) }
    ]);

    let ban_equiv = count_result(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { X(2).test(|v: &i32| *v > 25) }
    ]);

    parity_result("negated_context_test", neg_get, ban_equiv);
}

// ============================================================================
// § Ban Cluster with Negated Elements
// ============================================================================

#[test]
fn ban_negated_edge_shared_contradicts() {
    let result = search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(0) & !E() ^ n(1) }
    ];
    assert!(result.is_err(), "ban negated edge on same-slot shared pair should be contradictory");
    println!("ban_negated_edge_shared: correctly rejected: {}", result.err().unwrap());
}

#[test]
fn ban_negated_freestanding_node() {
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        N(2)
    ].unwrap();

    let ban_neg_node = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(0) ^ !N_() }
    ].unwrap());

    println!("RESULT: ban_negated_freestanding_node: count={ban_neg_node}");
}

#[test]
fn ban_negated_edge_to_new_node() {
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        N(2)
    ].unwrap();

    let ban_neg_edge_new = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(0) & !E() ^ N(2) }
    ].unwrap());

    println!("RESULT: ban_negated_edge_to_new_node: count={ban_neg_edge_new}");
}

// ============================================================================
// § Morphism variations
// ============================================================================

#[test]
fn all_negated_iso() {
    let g: graph::Undir0 = graph![<(), UER>;].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Iso) { !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        ban(Iso) { N_() }
    ].unwrap());

    parity("all_negated_iso_empty", neg_get, ban_equiv);
}

#[test]
fn all_negated_homo() {
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Homo) { !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        ban(Homo) { N_() }
    ].unwrap());

    parity("all_negated_homo_nonempty", neg_get, ban_equiv);
}

#[test]
fn negated_freestanding_subiso() {
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        n(1) ^ N(2),
        n(0) ^ n(2)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(SubIso) { N(0) ^ N(1), !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(SubIso) { N(0) ^ N(1) },
        ban(SubIso) { N_() }
    ].unwrap());

    parity("negated_freestanding_subiso", neg_get, ban_equiv);
}

// ============================================================================
// § Directed graphs
// ============================================================================

#[test]
fn negated_freestanding_dir() {
    let g: graph::Dir0 = graph![N(0) >> N(1), N(2)].unwrap();

    let neg_get = count(&g, search![<(), DER>;
        get(Mono) { N(0) >> N(1), !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), DER>;
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { N_() }
    ].unwrap());

    parity("negated_freestanding_dir", neg_get, ban_equiv);
}

#[test]
fn negated_edge_dir_reverse() {
    let g: graph::Dir0 = graph![N(0) >> N(1)].unwrap();

    let neg_get = count(&g, search![<(), DER>;
        get(Mono) { N(0) >> N(1), n(0) & !E() << n(1) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), DER>;
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(0) << n(1) }
    ].unwrap());

    parity("negated_edge_dir_reverse", neg_get, ban_equiv);
}

#[test]
fn negated_edge_dir_bidirectional() {
    let g: graph::Dir0 = graph![N(0) >> (N(1) >> n(0))].unwrap();

    let neg_get = count(&g, search![<(), DER>;
        get(Mono) { N(0) >> N(1), n(0) & !E() << n(1) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), DER>;
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(0) << n(1) }
    ].unwrap());

    parity("negated_edge_dir_bidirectional", neg_get, ban_equiv);
}

// ============================================================================
// § All-negated with val (standalone, not mixed with positive pattern)
// ============================================================================

#[test]
fn all_negated_val_nonempty_match() {
    let g: graph::Undir<i32, ()> = graph![N(0).val(10) ^ N(1).val(20)].unwrap();

    let neg_get = count(&g, search![<i32, UER>;
        get(Mono) { !N_().val(10) }
    ].unwrap());

    let ban_equiv = count(&g, search![<i32, UER>;
        ban(Mono) { N_().val(10) }
    ].unwrap());

    parity("all_negated_val_nonempty_match", neg_get, ban_equiv);
}

#[test]
fn all_negated_val_nonempty_no_match() {
    let g: graph::Undir<i32, ()> = graph![N(0).val(10) ^ N(1).val(20)].unwrap();

    let neg_get = count(&g, search![<i32, UER>;
        get(Mono) { !N_().val(99) }
    ].unwrap());

    let ban_equiv = count(&g, search![<i32, UER>;
        ban(Mono) { N_().val(99) }
    ].unwrap());

    parity("all_negated_val_nonempty_no_match", neg_get, ban_equiv);
}

// ============================================================================
// § Ban negation proxy correctness
// ============================================================================

#[test]
fn ban_negated_freestanding_node_proxy() {
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        N(2)
    ].unwrap();

    let ban_neg_node = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(0) ^ !N_() }
    ].unwrap());

    let get_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) ^ N_() }
    ].unwrap());

    parity("ban_neg_freestanding_proxy (ban{n^!N} vs get{N^N^N})", ban_neg_node, get_equiv);
}

#[test]
fn ban_negated_edge_to_new_node_proxy() {
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        N(2)
    ].unwrap();

    let ban_neg_edge = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(0) & !E() ^ N(2) }
    ].unwrap());

    let get_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        get(Mono) { n(0) ^ N(2) }
    ].unwrap());

    parity("ban_neg_edge_to_new_proxy (ban{n&!E^N} vs get+get{n^N})", ban_neg_edge, get_equiv);
}

// ============================================================================
// § Type-system rejections (verified via compile_fail tests, not here)
// ============================================================================
// !N(0) & E() ^ N(1) — negated node + positive edge: rejected by BitAnd trait bounds
// !X(0) bare — rejected by HasConstraint trait bound

// ============================================================================
// § Compilation errors (should-reject)
// ============================================================================

#[test]
fn same_slot_positive_and_negated_contradicts() {
    let result = search![<(), UER>;
        get(Mono) { N(0) ^ N(1), n(0) & !E() ^ n(1) }
    ];
    assert!(result.is_err(), "same-slot positive + negated edge should be contradictory");
    println!("same_slot_contradicts: correctly rejected: {}", result.err().unwrap());
}

// ============================================================================
// § Homo with negated freestanding (different from Mono - allows reuse)
// ============================================================================

#[test]
fn negated_freestanding_homo_3node() {
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        n(1) ^ N(2),
        n(0) ^ n(2)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Homo) { N(0) ^ N(1), !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Homo) { N(0) ^ N(1) },
        ban(Homo) { N_() }
    ].unwrap());

    parity("negated_freestanding_homo_3node", neg_get, ban_equiv);
}

#[test]
fn negated_freestanding_homo_2node() {
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Homo) { N(0) ^ N(1), !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Homo) { N(0) ^ N(1) },
        ban(Homo) { N_() }
    ].unwrap());

    parity("negated_freestanding_homo_2node", neg_get, ban_equiv);
}

// ============================================================================
// § Connected negative subgraph (shadow ban = AND within get)
//
// All negated elements in one get cluster form a single shadow ban.
// ALL negated elements must be simultaneously satisfiable for rejection.
// This is AND semantics within one get cluster.
// ============================================================================

#[test]
fn connected_neg_pair_found() {
    // graph: path 0-1-2-3 (4 nodes)
    // pattern: get edge 0^1, reject if negated pair !N(2)^!N(3) exists
    // graph HAS a pair of connected unmapped nodes (2-3) → should reject
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        n(1) ^ N(2),
        n(2) ^ N(3)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), !N(2) ^ !N(3) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N(2) ^ N(3) }
    ].unwrap());

    parity("connected_neg_pair_found", neg_get, ban_equiv);
}

#[test]
fn connected_neg_pair_not_found() {
    // graph: just 0-1 (2 nodes)
    // pattern: get edge 0^1, reject if negated pair !N(2)^!N(3) exists
    // graph has NO extra pair of connected nodes → should survive
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), !N(2) ^ !N(3) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N(2) ^ N(3) }
    ].unwrap());

    parity("connected_neg_pair_not_found", neg_get, ban_equiv);
}

#[test]
fn connected_neg_pair_partial_no_edge() {
    // graph: 0-1, isolated 2, isolated 3 (no edge between 2 and 3)
    // pattern: reject if !N(2)^!N(3) found (connected pair)
    // two unmapped nodes exist but NOT connected → shadow ban unsatisfied → survive
    let g: graph::Undir0 = graph![N(0) ^ N(1), N(2), N(3)].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), !N(2) ^ !N(3) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N(2) ^ N(3) }
    ].unwrap());

    parity("connected_neg_pair_partial_no_edge", neg_get, ban_equiv);
}

#[test]
fn connected_neg_triangle() {
    // graph: complete K5
    // pattern: get edge 0^1, reject if negated triangle !N(2)^!N(3)^!N(4) found
    // K5 has plenty of triangles among unmapped nodes → reject
    let g: graph::Undir0 = graph![
        N(0) ^ N(1), n(0) ^ N(2), n(0) ^ N(3), n(0) ^ N(4),
        n(1) ^ n(2), n(1) ^ n(3), n(1) ^ n(4),
        n(2) ^ n(3), n(2) ^ n(4),
        n(3) ^ n(4)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), !N(2) ^ (!N(3) ^ !N(4)), n(2) ^ n(4) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N(2) ^ (N(3) ^ N(4)), n(2) ^ n(4) }
    ].unwrap());

    parity("connected_neg_triangle", neg_get, ban_equiv);
}

#[test]
fn connected_neg_triangle_not_found() {
    // graph: path 0-1-2-3-4 (no triangles among 2,3,4)
    // pattern: reject if negated triangle found → should survive
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        n(1) ^ N(2),
        n(2) ^ N(3),
        n(3) ^ N(4)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), !N(2) ^ (!N(3) ^ !N(4)), n(2) ^ n(4) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N(2) ^ (N(3) ^ N(4)), n(2) ^ n(4) }
    ].unwrap());

    parity("connected_neg_triangle_not_found", neg_get, ban_equiv);
}

// ============================================================================
// § Negative islands within one get cluster (AND semantics)
//
// Two disconnected negative islands in one get cluster = one shadow ban.
// BOTH islands must be satisfiable simultaneously for rejection.
// ============================================================================

#[test]
fn neg_two_islands_both_found() {
    // graph: 0-1, 2, 3-4 (two unmapped components: isolated 2, edge 3-4)
    // pattern: get 0^1, island1=!N_() (any unmapped), island2=!N(3)^!N(4)
    // both islands found → reject
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        N(2),
        N(3) ^ N(4)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), !N_(), !N(3) ^ !N(4) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_(), N(3) ^ N(4) }
    ].unwrap());

    parity("neg_two_islands_both_found", neg_get, ban_equiv);
}

#[test]
fn neg_two_islands_one_missing() {
    // graph: 0-1, 2 (isolated 2, but no edge pair for second island)
    // pattern: get 0^1, island1=!N_(), island2=!N(3)^!N(4)
    // island1 found (node 2), island2 NOT found (no connected pair) → survive
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        N(2)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), !N_(), !N(3) ^ !N(4) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_(), N(3) ^ N(4) }
    ].unwrap());

    parity("neg_two_islands_one_missing", neg_get, ban_equiv);
}

#[test]
fn neg_two_freestanding_islands_both_found() {
    // graph: 0-1, 2, 3  (two unmapped isolated nodes)
    // pattern: get 0^1, !N_(), !N_()  (two separate freestanding negated nodes)
    // under Mono both must map to different unmapped nodes: 2 and 3 → reject
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        N(2),
        N(3)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), !N_(), !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_(), N_() }
    ].unwrap());

    parity("neg_two_freestanding_both_found", neg_get, ban_equiv);
}

#[test]
fn neg_two_freestanding_islands_one_short() {
    // graph: 0-1, 2  (only one unmapped node)
    // pattern: get 0^1, !N_(), !N_()  (need two different unmapped nodes under Mono)
    // only 1 unmapped node, can't satisfy both → survive
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        N(2)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), !N_(), !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_(), N_() }
    ].unwrap());

    parity("neg_two_freestanding_one_short", neg_get, ban_equiv);
}

// ============================================================================
// § OR semantics: separate ban clusters
// ============================================================================

#[test]
fn separate_bans_or_first_fires() {
    // graph: 0-1, 2
    // ban1: N_() (any unmapped node) → fires (finds 2)
    // ban2: N(3)^N(4) (connected pair) → doesn't fire
    // OR: ban1 fires → reject
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        N(2)
    ].unwrap();

    let result = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_() },
        ban(Mono) { N(3) ^ N(4) }
    ].unwrap());

    println!("separate_bans_or_first_fires: count={result}");
    assert_eq!(result, 0);
}

#[test]
fn separate_bans_or_neither_fires() {
    // graph: 0-1  (no extra nodes, no connected pair)
    // ban1: N_() → doesn't fire under Mono (all nodes mapped)
    // ban2: N(3)^N(4) → doesn't fire
    // OR: neither fires → survive
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let result = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_() },
        ban(Mono) { N(3) ^ N(4) }
    ].unwrap());

    println!("separate_bans_or_neither_fires: count={result}");
    assert_eq!(result, 2);
}

// ============================================================================
// § AND vs OR: same negated elements, different grouping
// ============================================================================

#[test]
fn and_vs_or_single_ban_and() {
    // graph: 0-1, 2 (one unmapped node, no connected pair)
    // single ban with TWO elements: N_(), N(3)^N(4)
    // AND: N_() found, but N(3)^N(4) not found → ban unsatisfied → survive
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        N(2)
    ].unwrap();

    let single_ban = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_(), N(3) ^ N(4) }
    ].unwrap());

    println!("and_vs_or_single_ban_and: count={single_ban}");
    assert!(single_ban > 0, "single ban AND: one island missing → should survive");
}

#[test]
fn and_vs_or_separate_bans_or() {
    // same graph, but two separate bans (OR)
    // ban1: N_() → fires
    // ban2: N(3)^N(4) → doesn't fire
    // OR: ban1 fires → reject
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        N(2)
    ].unwrap();

    let separate_bans = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_() },
        ban(Mono) { N(3) ^ N(4) }
    ].unwrap());

    println!("and_vs_or_separate_bans_or: count={separate_bans}");
    assert_eq!(separate_bans, 0, "separate bans OR: first fires → should reject");
}

// ============================================================================
// § Connected neg anchored to positive nodes
// ============================================================================

#[test]
fn neg_connected_to_positive_found() {
    // graph: 0-1-2-3
    // pattern: get 0^1, negated: n(1)^!N_() (neighbor of mapped node 1)
    // node 2 is unmapped neighbor of 1 → violation → reject
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        n(1) ^ N(2),
        n(2) ^ N(3)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) ^ !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(1) ^ N_() }
    ].unwrap());

    parity("neg_connected_to_positive_found", neg_get, ban_equiv);
}

#[test]
fn neg_connected_to_positive_not_found() {
    // graph: 0-1 (node 1 has no other neighbors besides 0)
    // pattern: get 0^1, negated: n(1)^!N_()
    // under Mono, only candidate for !N_() neighbor of 1 is node 0, but it's mapped → no violation
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) ^ !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(1) ^ N_() }
    ].unwrap());

    parity("neg_connected_to_positive_not_found", neg_get, ban_equiv);
}

#[test]
fn neg_chain_from_positive() {
    // graph: 0-1-2-3-4
    // pattern: get 0^1, negated chain: n(1)^!N(2)^!N(3)
    // nodes 2,3 are unmapped and connected via 1-2-3 → violation
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        n(1) ^ N(2),
        n(2) ^ N(3),
        n(3) ^ N(4)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), n(1) ^ !N(2) ^ !N(3) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(1) ^ N(2) ^ N(3) }
    ].unwrap());

    parity("neg_chain_from_positive", neg_get, ban_equiv);
}

#[test]
fn neg_chain_from_positive_too_short() {
    // graph: 0-1-2 (only one extra node after 1)
    // pattern: get 0^1, negated chain: n(1)^!N(2)^!N(3)
    // need 2 unmapped connected nodes from 1, only have 1 (node 2) → survive
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        n(1) ^ N(2)
    ].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), n(1) ^ !N(2) ^ !N(3) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(1) ^ N(2) ^ N(3) }
    ].unwrap());

    parity("neg_chain_from_positive_too_short", neg_get, ban_equiv);
}

// ============================================================================
// § Multiple explicit ban clusters — OR semantics (one fires → reject)
// ============================================================================

#[test]
fn multi_ban_first_fires_second_doesnt() {
    // ban1: N_() → fires (node 2 exists)
    // ban2: N(3)^N(4) → doesn't fire (no connected pair)
    // OR → reject
    let g: graph::Undir0 = graph![N(0) ^ N(1), N(2)].unwrap();

    let result = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_() },
        ban(Mono) { N(3) ^ N(4) }
    ].unwrap());

    println!("multi_ban_first_fires: count={result}");
    assert_eq!(result, 0, "OR: first ban fires → all rejected");
}

#[test]
fn multi_ban_second_fires_first_doesnt() {
    // ban1: N_().val(99) → doesn't fire (no node with val 99)
    // ban2: N_() → fires (node 2 exists)
    // OR → reject
    let g: graph::Undir<i32, ()> = graph![
        N(0).val(10) ^ N(1).val(20),
        N(2).val(30)
    ].unwrap();

    let result = count(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_().val(99) },
        ban(Mono) { N_() }
    ].unwrap());

    println!("multi_ban_second_fires: count={result}");
    assert_eq!(result, 0, "OR: second ban fires → all rejected");
}

#[test]
fn multi_ban_none_fires() {
    // ban1: N(3)^N(4) → no connected unmapped pair
    // ban2: N_().val(99) → no node with val 99
    // OR: neither fires → survive
    let g: graph::Undir<i32, ()> = graph![
        N(0).val(10) ^ N(1).val(20),
        N(2).val(30)
    ].unwrap();

    let result = count(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N(3) ^ N(4) },
        ban(Mono) { N_().val(99) }
    ].unwrap());

    println!("multi_ban_none_fires: count={result}");
    assert!(result > 0, "OR: no ban fires → matches survive");
}

// ============================================================================
// § Connected neg with val predicates on negated nodes
// ============================================================================

#[test]
fn connected_neg_with_val_found() {
    // graph: 0(10)-1(20)-2(30)
    // pattern: get 0^1, reject if negated neighbor of 1 has val 30
    // node 2 is neighbor of 1 with val 30 → reject
    let g: graph::Undir<i32, ()> = graph![
        N(0).val(10) ^ N(1).val(20),
        n(1) ^ N(2).val(30)
    ].unwrap();

    let neg_get = count(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1) ^ !N_().val(30) }
    ].unwrap());

    let ban_equiv = count(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(1) ^ N_().val(30) }
    ].unwrap());

    parity("connected_neg_with_val_found", neg_get, ban_equiv);
}

#[test]
fn connected_neg_with_val_not_found() {
    // graph: 0(10)-1(20)-2(30)
    // pattern: get 0^1, reject if negated neighbor of 1 has val 99
    // no node with val 99 → survive
    let g: graph::Undir<i32, ()> = graph![
        N(0).val(10) ^ N(1).val(20),
        n(1) ^ N(2).val(30)
    ].unwrap();

    let neg_get = count(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1) ^ !N_().val(99) }
    ].unwrap());

    let ban_equiv = count(&g, search![<i32, UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(1) ^ N_().val(99) }
    ].unwrap());

    parity("connected_neg_with_val_not_found", neg_get, ban_equiv);
}

// ============================================================================
// § Directed connected negation
// ============================================================================

#[test]
fn connected_neg_dir_outgoing_found() {
    // graph: 0→1→2
    // pattern: get 0>>1, reject if 1 has outgoing negated neighbor
    // node 2 is outgoing neighbor of 1 → reject
    let g: graph::Dir0 = graph![
        N(0) >> N(1),
        n(1) >> N(2)
    ].unwrap();

    let neg_get = count(&g, search![<(), DER>;
        get(Mono) { N(0) >> (N(1) >> !N_()) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), DER>;
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(1) >> N_() }
    ].unwrap());

    parity("connected_neg_dir_outgoing_found", neg_get, ban_equiv);
}

#[test]
fn connected_neg_dir_outgoing_not_found() {
    // graph: 0→1 (no outgoing from 1)
    // pattern: get 0>>1, reject if 1 has outgoing negated neighbor
    // no outgoing from 1 → survive
    let g: graph::Dir0 = graph![N(0) >> N(1)].unwrap();

    let neg_get = count(&g, search![<(), DER>;
        get(Mono) { N(0) >> (N(1) >> !N_()) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), DER>;
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(1) >> N_() }
    ].unwrap());

    parity("connected_neg_dir_outgoing_not_found", neg_get, ban_equiv);
}

#[test]
fn connected_neg_dir_incoming_only() {
    // graph: 0→1←2
    // pattern: get 0>>1, reject if 1 has outgoing negated neighbor (>>)
    // node 2 points TO 1 (incoming), not from 1 → survive
    let g: graph::Dir0 = graph![
        N(0) >> N(1),
        N(2) >> n(1)
    ].unwrap();

    let neg_get = count(&g, search![<(), DER>;
        get(Mono) { N(0) >> (N(1) >> !N_()) }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), DER>;
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(1) >> N_() }
    ].unwrap());

    parity("connected_neg_dir_incoming_only", neg_get, ban_equiv);
}

// ============================================================================
// § Homo connected negation (no injectivity — ban can reuse mapped nodes)
// ============================================================================

#[test]
fn connected_neg_homo_reuses_mapped() {
    // graph: 0-1 (triangle would be 0-1, 1-0 under homo)
    // pattern: get(Homo) 0^1, reject if negated neighbor of 1
    // under Homo, node 0 is unmapped-equivalent (can be reused) and IS neighbor of 1
    // ban fires → reject
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Homo) { N(0) ^ N(1) ^ !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Homo) { N(0) ^ N(1) },
        ban(Homo) { n(1) ^ N_() }
    ].unwrap());

    parity("connected_neg_homo_reuses_mapped", neg_get, ban_equiv);
}

#[test]
fn connected_neg_mono_doesnt_reuse_mapped() {
    // same graph and pattern but Mono — !N_() must be unmapped
    // only neighbor of 1 is 0, which is already mapped → survive
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let neg_get = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) ^ !N_() }
    ].unwrap());

    let ban_equiv = count(&g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(1) ^ N_() }
    ].unwrap());

    parity("connected_neg_mono_doesnt_reuse_mapped", neg_get, ban_equiv);
}
