use grw::graph::index::{Cardinality, IndexDecl, IndexName};
use grw::graph::{edge, MGraph};
use grw::search::{error, Pattern, RevCsr, Seq};
use grw::search::dsl::LocalId;
use grw::{mgraph, pattern};
use grw::Graph as _;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    Circle(u32),
    Square(u32),
}

fn shapes() -> MGraph<Shape, edge::Undir<()>> {
    mgraph![N(0).val(Shape::Circle(3)) ^ (N(1).val(Shape::Square(2)) ^ (N(2).val(Shape::Circle(9)) ^ n(0)))].unwrap()
}

const BY_RADIUS: IndexName = IndexName("by_radius");

fn by_radius() -> IndexDecl<Shape> {
    IndexDecl::new(BY_RADIUS, Cardinality::Unique, |v: &Shape| match v {
        Shape::Circle(r) | Shape::Square(r) => Some(*r),
    })
}

fn indexed_shapes() -> MGraph<Shape, edge::Undir<()>> {
    shapes().with_indices(vec![by_radius()]).unwrap()
}

#[test]
fn named_nodes_and_lookup() {
    let p: Pattern<Shape, edge::Undir<()>> = pattern![get(Mono) { N(a) ^ N(b) }].unwrap();
    assert_eq!(p.lid("a").unwrap(), LocalId(0));
    assert_eq!(p.lid("b").unwrap(), LocalId(1));
    let g = shapes();
    let indexed = g.index(RevCsr);
    let count = Seq::search(p.query(), &indexed).unwrap().count();
    assert_eq!(count, 6);
}

#[test]
fn value_patterns_on_nodes() {
    let p: Pattern<Shape, edge::Undir<()>> = pattern![get(Mono) { N(c: Shape::Circle(_)) ^ N(s: Shape::Square(2)) }].unwrap();
    let g = shapes();
    let indexed = g.index(RevCsr);
    let matches: Vec<_> = Seq::search(p.query(), &indexed).unwrap().collect();
    assert_eq!(matches.len(), 2);
    for m in &matches {
        assert_eq!(m[p.lid("s").unwrap()], grw::id::N(1));
    }
}

#[test]
fn mixed_ints_and_names() {
    let p: Pattern<Shape, edge::Undir<()>> = pattern![get(Mono) { N(5) ^ N(k) }].unwrap();
    assert_eq!(p.lid("k").unwrap(), LocalId(6));
    assert_eq!(p.query().node_count(), 2);
}

#[test]
fn edge_value_pattern() {
    let g: MGraph<(), edge::Undir<u8>> = mgraph![N(0) & E().val(1u8) ^ (N(1) & E().val(2u8) ^ N(2))].unwrap();
    let p: Pattern<(), edge::Undir<u8>> = pattern![get(Mono) { N(a) & E(2) ^ N(b) }].unwrap();
    let indexed = g.index(RevCsr);
    assert_eq!(Seq::search(p.query(), &indexed).unwrap().count(), 2);
}

#[test]
fn explicit_types_form() {
    let p = pattern![<Shape, edge::Undir<()>>; get(Mono) { N(a) ^ N(b) }, ban(Mono) { n(a) ^ N(z: Shape::Circle(9)) }].unwrap();
    assert_eq!(p.names().iter().count(), 3);
    let g = shapes();
    let indexed = g.index(RevCsr);
    assert_eq!(Seq::search(p.query(), &indexed).unwrap().count(), 4);
}

#[test]
fn unknown_name_is_runtime_error() {
    let p: Pattern<Shape, edge::Undir<()>> = pattern![get(Mono) { N(a) ^ N(b) }].unwrap();
    assert!(matches!(p.lid("nope"), Err(error::Search::UnknownName(_))));
}

#[test]
fn key_predicate_macro_form_matches_method_form() {
    let macro_form: Pattern<Shape, edge::Undir<()>> =
        pattern![get(Mono) { N(a: key(BY_RADIUS, 3u32)) ^ N(b) }].unwrap();
    let method_form: Pattern<Shape, edge::Undir<()>> =
        pattern![get(Mono) { N(a).key(BY_RADIUS, 3u32) ^ N(b) }].unwrap();
    let g = indexed_shapes();
    let indexed = g.index(RevCsr);
    let bindings = |p: &Pattern<Shape, edge::Undir<()>>| -> std::collections::BTreeSet<(grw::id::N, grw::id::N)> {
        Seq::search(p.query(), &indexed)
            .unwrap()
            .map(|m| (m.get(p.lid("a").unwrap()).unwrap(), m.get(p.lid("b").unwrap()).unwrap()))
            .collect()
    };
    let macro_bindings = bindings(&macro_form);
    let method_bindings = bindings(&method_form);
    assert_eq!(macro_bindings, method_bindings);
    assert!(!macro_bindings.is_empty());
    assert!(macro_bindings.iter().all(|(a, _)| *a == grw::id::N(0)));
}
