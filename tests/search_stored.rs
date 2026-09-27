use grw::graph::{edge, Graph, MGraph};
use grw::search::{error, BindError, Pattern, PinnedNode, RevCsr, Seq};
use grw::{mgraph, pattern, search};
use std::marker::PhantomData;

fn triangle() -> MGraph<u8, edge::Undir<()>> {
    mgraph![N(0).val(10u8) ^ (N(1).val(20u8) ^ (N(2).val(30u8) ^ n(0)))].unwrap()
}

fn edge_pat() -> Pattern<u8, edge::Undir<()>> {
    pattern![get(Mono) { N(a) ^ N(b) }].unwrap()
}

#[test]
fn engine_honours_prebound_free_node() {
    let g = triangle();
    let p = edge_pat();
    let indexed = g.index(RevCsr);
    let idx_a = p.query().node_local_id(0);
    assert_eq!(p.lid("a").unwrap(), idx_a);
    let bindings = vec![Some(grw::id::N(2)), None];
    let got: Vec<_> = Seq::search_bound(p.query(), &indexed, bindings).unwrap()
        .map(|m| (m[p.lid("a").unwrap()], m[p.lid("b").unwrap()]))
        .collect();
    assert_eq!(got.len(), 2);
    assert!(got.iter().all(|(a, _)| *a == grw::id::N(2)));
}

#[test]
fn stored_pattern_unpinned() {
    let g = triangle();
    let s = search![&g, edge_pat()].unwrap();
    assert_eq!(s.iter().count(), 6);
}

#[test]
fn stored_pattern_pinned_by_name() {
    let g = triangle();
    let two = grw::id::N(2);
    let s = search![&g, edge_pat() with X(a = two)].unwrap();
    let mut others: Vec<_> = s.iter().map(|m| m[s.query().node_local_id(1)]).collect();
    others.sort();
    assert_eq!(others, vec![grw::id::N(0), grw::id::N(1)]);
}

#[test]
fn stored_pattern_two_pins_and_errors() {
    let g = triangle();
    let (zero, one, ghost) = (grw::id::N(0), grw::id::N(1), grw::id::N(9));
    assert_eq!(search![&g, edge_pat() with X(a = zero), X(b = one)].unwrap().iter().count(), 1);
    assert!(matches!(search![&g, edge_pat() with X(zz = zero)].err().unwrap(), error::Search::UnknownName(_)));
    assert!(matches!(search![&g, edge_pat() with X(a = ghost)].err().unwrap(), error::Search::TargetMissing(_)));
    assert!(matches!(search![&g, edge_pat() with X(a = zero), X(b = zero)].err().unwrap(), error::Search::Bind(_)));
}

struct Holder(Pattern<u8, edge::Undir<()>>);

impl Holder {
    fn with(self, _keep: u8) -> Pattern<u8, edge::Undir<()>> {
        self.0
    }
}

#[test]
fn method_named_with_is_not_the_clause_keyword() {
    let g = triangle();
    let h = Holder(edge_pat());
    let s = search![&g, h.with(7u8)].unwrap();
    assert_eq!(s.iter().count(), 6);
}

fn make_pat() -> Result<Pattern<u8, edge::Undir<()>>, error::Search> {
    pattern![get(Mono) { N(a) ^ N(b) }]
}

#[test]
fn stored_pattern_from_try_expression() -> Result<(), error::Search> {
    let g = triangle();
    let s = search![&g, make_pat()?]?;
    assert_eq!(s.iter().count(), 6);
    Ok(())
}

#[test]
fn duplicate_pin_from_a_dynamic_pins_slice() {
    let g = triangle();
    let (zero, one) = (grw::id::N(0), grw::id::N(1));
    let err = grw::search::Session::from_pattern(&edge_pat(), &g, &[("a", zero), ("a", one)])
        .err()
        .unwrap();
    assert!(matches!(
        err,
        error::Search::Bind(BindError::Duplicate(PinnedNode::Named { ref name, .. })) if name == "a"
    ));
}

#[test]
fn a_repeated_pin_is_the_duplicate_even_when_its_target_is_missing() {
    let g = triangle();
    let (zero, ghost) = (grw::id::N(0), grw::id::N(9));
    let err = grw::search::Session::from_pattern(&edge_pat(), &g, &[("a", zero), ("a", ghost)])
        .err()
        .unwrap();
    assert!(matches!(
        err,
        error::Search::Bind(BindError::Duplicate(PinnedNode::Named { ref name, .. })) if name == "a"
    ));
}

#[test]
fn one_pattern_runs_over_two_graphs_and_twice_over_one() {
    let g1 = triangle();
    let g2 = triangle();
    let p = edge_pat();
    assert_eq!(search![&g1, p].unwrap().iter().count(), 6);
    assert_eq!(search![&g2, p].unwrap().iter().count(), 6);
    assert_eq!(search![&g1, p].unwrap().iter().count(), 6);
}

#[test]
fn pinned_node_still_obeys_value_pattern() {
    let g = triangle();
    let p: Pattern<u8, edge::Undir<()>> = pattern![get(Mono) { N(big: 30u8) ^ N(o) }].unwrap();
    let zero = grw::id::N(0);
    assert_eq!(search![&g, p with X(big = zero)].unwrap().iter().count(), 0);
}

#[test]
fn pinned_session_par_iter_matches_seq() {
    use rayon::iter::ParallelIterator;
    let g = triangle();
    let two = grw::id::N(2);
    let s = search![&g, edge_pat() with X(a = two)].unwrap();
    let (la, lb) = (s.query().node_local_id(0), s.query().node_local_id(1));
    let mut seq: Vec<(grw::id::N, grw::id::N)> = s.iter().map(|m| (m[la], m[lb])).collect();
    let mut par: Vec<(grw::id::N, grw::id::N)> = s.par_iter().map(|m| (m[la], m[lb])).collect();
    seq.sort();
    par.sort();
    assert_eq!(seq, vec![(two, grw::id::N(0)), (two, grw::id::N(1))]);
    assert_eq!(par, seq);
    assert_eq!(s.par_iter().count(), seq.len());
}

fn pick<A, B>(g: &MGraph<u8, edge::Undir<()>>, _a: PhantomData<A>, _b: PhantomData<B>) -> &MGraph<u8, edge::Undir<()>> {
    g
}

#[test]
fn stored_pattern_head_may_carry_a_generic_comma() {
    let g = triangle();
    let p = edge_pat();
    let s = search![pick::<u8, ()>(&g, PhantomData, PhantomData), p].unwrap();
    assert_eq!(s.iter().count(), 6);
}

#[test]
fn stored_pattern_head_with_a_generic_comma_takes_pins() {
    let g = triangle();
    let two = grw::id::N(2);
    let s = search![pick::<u8, ()>(&g, PhantomData, PhantomData), edge_pat() with X(a = two)].unwrap();
    assert_eq!(s.iter().count(), 2);
}

#[test]
fn graph_free_search_with_an_unpinned_context_node_stays_unresolved() {
    let typed: grw::Search<u8, edge::Undir<()>> = search![<u8, edge::Undir<()>>; get(Mono) { X(0) ^ N(1) }].unwrap();
    assert!(matches!(typed, grw::Search::Unresolved(_)));
    let bare: grw::Search<u8, edge::Undir<()>> = search![get(Mono) { X(a) ^ N(b) }].unwrap();
    assert!(matches!(bare, grw::Search::Unresolved(_)));
}
