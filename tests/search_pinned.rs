use grw::graph::{edge, MGraph};
use grw::search::error;
use grw::search::Search;
use grw::{mgraph, search};

type UER = edge::Undir<()>;

fn triangle() -> MGraph<u8, edge::Undir<()>> {
    mgraph![N(0).val(10u8) ^ (N(1).val(20u8) ^ (N(2).val(30u8) ^ n(0)))].unwrap()
}

#[test]
fn pinned_context_node_in_session() {
    let g = triangle();
    let one = grw::id::N(1);
    let s = search![&g, get(Mono) { X(c = one) ^ N(o) }].unwrap();
    let mut seen = Vec::new();
    for m in &s {
        assert_eq!(m.get(0).unwrap(), one);
        seen.push(m.get(1).unwrap());
    }
    seen.sort();
    assert_eq!(seen, vec![grw::id::N(0), grw::id::N(2)]);
}

#[test]
fn pinned_node_with_value_check_fails_match() {
    let g = triangle();
    let one = grw::id::N(1);
    let s = search![&g, get(Mono) { X(c = one : 99u8) ^ N(o) }].unwrap();
    assert_eq!(s.iter().count(), 0);
    let s = search![&g, get(Mono) { X(c = one : 20u8) ^ N(o) }].unwrap();
    assert_eq!(s.iter().count(), 2);
}

#[test]
fn pin_to_missing_node_is_error() {
    let g = triangle();
    let ghost = grw::id::N(77);
    let err = search![&g, get(Mono) { X(c = ghost) ^ N(o) }].err().unwrap();
    assert!(matches!(err, error::Search::TargetMissing(_)));
}

#[test]
fn unpinned_context_in_session_still_rejected() {
    let g = triangle();
    let err = search![&g, get(Mono) { X(c) ^ N(o) }].err().unwrap();
    assert!(matches!(err, error::Search::BoundPatternInSession));
}

#[test]
fn explicit_types_head_does_not_shadow_caller_edge_module() {
    let Search::Resolved(r): Search<u8, edge::Undir<()>> =
        search![<u8, edge::Undir<()>>; get(Mono) { N(a) ^ N(b) }].unwrap()
    else {
        panic!("unexpected context nodes")
    };
    assert_eq!(r.query().node_count(), 2);
}

#[test]
fn graph_head_is_an_arbitrary_expression() {
    let gs = vec![triangle()];
    let one = grw::id::N(1);
    let s = search![&gs[0], get(Mono) { X(c = one) ^ N(o) }].unwrap();
    assert_eq!(s.iter().count(), 2);
}

fn pick<A, B>(g: &MGraph<u8, UER>) -> &MGraph<u8, UER> {
    g
}

#[test]
fn graph_head_may_contain_a_top_level_comma() {
    let g = triangle();
    let one = grw::id::N(1);
    let s = search![pick::<u8, ()>(&g), get(Mono) { X(c = one) ^ N(o) }].unwrap();
    assert_eq!(s.iter().count(), 2);
}

#[test]
fn inline_mgraph_head_still_works() {
    let count = search![mgraph![<u8, UER>; N(0).val(10u8) ^ N(1).val(20u8)],
        get(Mono) { N(0) ^ N(1) }
    ]
    .unwrap()
    .count();
    assert_eq!(count, 2);
}

#[test]
fn bare_cluster_head_still_compiles() {
    let Search::Resolved(r): Search<u8, edge::Undir<()>> = search![get(Mono) { N(a) ^ N(b) }].unwrap() else {
        panic!("unexpected context nodes")
    };
    assert_eq!(r.query().node_count(), 2);
}

#[test]
fn pinned_session_par_iter_matches_seq() {
    use rayon::iter::ParallelIterator;
    let g = triangle();
    let one = grw::id::N(1);
    let s = search![&g, get(Mono) { X(c = one) ^ N(o) }].unwrap();
    let mut seq: Vec<(grw::id::N, grw::id::N)> = s.iter().map(|m| (m.get(0).unwrap(), m.get(1).unwrap())).collect();
    let mut par: Vec<(grw::id::N, grw::id::N)> = s.par_iter().map(|m| (m.get(0).unwrap(), m.get(1).unwrap())).collect();
    seq.sort();
    par.sort();
    assert_eq!(seq, vec![(one, grw::id::N(0)), (one, grw::id::N(2))]);
    assert_eq!(par, seq);
    assert_eq!(s.par_iter().count(), seq.len());
}
