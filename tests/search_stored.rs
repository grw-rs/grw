use grw::graph::{edge, Graph, MGraph};
use grw::search::{error, Pattern, RevCsr, Seq};
use grw::{mgraph, pattern, search};

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
    let err = grw::search::Session::from_pattern(edge_pat(), &g, &[("a", zero), ("a", one)])
        .err()
        .unwrap();
    assert!(matches!(err, error::Search::DuplicatePin(ref n) if n == "a"));
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
