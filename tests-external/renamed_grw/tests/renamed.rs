use grw2::composite::{Kinded, PartOf};
use grw2::layout::{FieldType, Val};
use renamed_grw::{edge_pattern, pinned_edge_count, triangle, Er, Rel, RelKind, Signs, Tier, Uses, Weight};
use std::marker::PhantomData;

fn pick<A, B>(g: &grw2::MGraph<u8, Er>, _a: PhantomData<A>, _b: PhantomData<B>) -> &grw2::MGraph<u8, Er> {
    g
}

#[test]
fn search_runs_through_the_renamed_crate() {
    let g = triangle();
    let count = grw2::search![&g, get(Mono) { N(a) ^ N(b) }].unwrap().iter().count();
    assert_eq!(count, 6);
    assert_eq!(pinned_edge_count(&g, grw2::id::N(2)), 2);
}

#[test]
fn stored_pattern_runs_through_the_renamed_crate() {
    let g = triangle();
    let two = grw2::id::N(2);
    assert_eq!(grw2::search![pick::<u8, ()>(&g, PhantomData, PhantomData), edge_pattern()].unwrap().iter().count(), 6);
    assert_eq!(grw2::search![&g, edge_pattern() with X(a = two)].unwrap().iter().count(), 2);
}

#[test]
fn inline_graph_search_runs_through_the_renamed_crate() {
    let count = grw2::search![mgraph![<u8, Er>; N(0).val(1u8) ^ N(1).val(2u8)], get(Mono) { N(0) ^ N(1) }]
        .unwrap()
        .count();
    assert_eq!(count, 2);
}

#[test]
fn graph_free_search_runs_through_the_renamed_crate() {
    let s: grw2::Search<u8, Er> = grw2::search![<u8, Er>; get(Mono) { X(0) ^ N(1) }].unwrap();
    assert!(matches!(s, grw2::Search::Unresolved(_)));
    let bare: grw2::Search<u8, Er> = grw2::search![get(Mono) { N(a) ^ N(b) }].unwrap();
    assert!(matches!(bare, grw2::Search::Resolved(_)));
}

#[test]
fn vgraph_runs_through_the_renamed_crate() {
    let g = grw2::vgraph![<u8, Er>; N(0).val(1u8) ^ N(1).val(2u8), n(1) ^ N(2).val(3u8)].unwrap();
    assert_eq!(grw2::Graph::node_count(&g), 3);
}

#[test]
fn val_derive_runs_through_the_renamed_crate() {
    let fields = Weight::fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "w");
    assert_eq!(fields[0].ty, FieldType::F64);
    assert_eq!(fields[1].ty, FieldType::Bool);
    assert!(matches!(Tier::field_type(), FieldType::Enum(_)));
    assert_ne!(Weight::layout_hash(), Tier::layout_hash());
}

#[test]
fn repl_runs_through_the_renamed_crate() {
    let methods = Weight::methods();
    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0].name, "weight");
    assert!(!methods[0].is_static);
    assert_eq!(methods[1].name, "heavy");
    assert!(methods[1].is_static);
    assert_eq!(Weight::heavy(3.0).weight(), 3.0);
}

#[test]
fn part_derive_runs_through_the_renamed_crate() {
    let signs = Rel::from(Signs { since: 7 });
    assert_eq!(signs.kind(), RelKind::Signs);
    assert_eq!(<Rel as PartOf<Uses>>::kind(), RelKind::Uses);
    assert_eq!(<Rel as PartOf<Signs>>::project(&signs), Some(&Signs { since: 7 }));
}
