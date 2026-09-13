//! One test per named invariant of `graph::index`, `modify_atomicity`-style:
//! direct assertions on the exact behaviour, no randomness.

use grw::Graph as _;
use grw::graph::edge::Dir;
use grw::graph::error::Index as IndexError;
use grw::graph::index::{Cardinality, IndexDecl, IndexHit, IndexName, KeyBytes, KeyTag};
use grw::graph::{self};
use grw::modify::error::{Apply, Modify, apply};
use grw::modify::{N, X};

type ER = Dir<()>;
type MG = graph::MDir<u32, ()>;

const BY_VAL: IndexName = IndexName("by_val");
const BY_BUCKET: IndexName = IndexName("by_bucket");
const BY_EVEN: IndexName = IndexName("by_even");

fn by_val() -> IndexDecl<u32> {
    IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v))
}

fn by_bucket() -> IndexDecl<u32> {
    IndexDecl::new(BY_BUCKET, Cardinality::Multi, |v: &u32| Some(*v % 3))
}

fn by_even() -> IndexDecl<u32> {
    IndexDecl::new(BY_EVEN, Cardinality::Unique, |v: &u32| (*v % 2 == 0).then_some(*v))
}

const ZETA: IndexName = IndexName("zeta");
const ALPHA: IndexName = IndexName("alpha");

fn by_zeta() -> IndexDecl<u32> {
    IndexDecl::new(ZETA, Cardinality::Unique, |v: &u32| Some(*v))
}

fn by_alpha() -> IndexDecl<u32> {
    IndexDecl::new(ALPHA, Cardinality::Unique, |v: &u32| Some(*v))
}

/// Nodes 0..=3 with distinct values 10, 11, 12, 13 — 10 and 13 share bucket 1
/// (`v % 3`), every value is a distinct unique key.
fn base() -> MG {
    let g: MG =
        grw::mgraph![<u32, ER>; N(0).val(10u32), N(1).val(11u32), N(2).val(12u32), N(3).val(13u32)]
            .unwrap();
    g.with_indices(vec![by_val(), by_bucket()]).unwrap()
}

fn view(g: &MG) -> Vec<(u32, u32)> {
    let mut v: Vec<(u32, u32)> = g.iter_node_ids().map(|n| (*n as u32, *g.node_val(n).unwrap())).collect();
    v.sort_unstable();
    v
}

fn tag() -> KeyTag {
    KeyTag::of::<u32>()
}

#[test]
fn unique_hit_returns_one() {
    let g = base();
    match g.index_hit(BY_VAL, &KeyBytes::of(&11u32), tag()).unwrap() {
        IndexHit::One(n) => assert_eq!(*n, 1),
        _ => panic!("expected One"),
    }
}

#[test]
fn multi_hit_returns_many() {
    let g = base();
    match g.index_hit(BY_BUCKET, &KeyBytes::of(&1u32), tag()).unwrap() {
        IndexHit::Many(set) => {
            let mut ids: Vec<u32> = set.iter().map(|n| *n).collect();
            ids.sort_unstable();
            assert_eq!(ids, vec![0, 3]);
        }
        _ => panic!("expected Many"),
    }
}

#[test]
fn hit_none_for_absent_key() {
    let g = base();
    assert!(matches!(g.index_hit(BY_VAL, &KeyBytes::of(&999u32), tag()).unwrap(), IndexHit::None));
    assert!(matches!(g.index_hit(BY_BUCKET, &KeyBytes::of(&999u32), tag()).unwrap(), IndexHit::None));
}

#[test]
fn unknown_index_errors() {
    let g = base();
    let err = match g.index_hit(IndexName("nope"), &KeyBytes::of(&1u32), tag()) {
        Err(e) => e,
        Ok(_) => panic!("expected Err"),
    };
    assert_eq!(err, IndexError::NoSuchIndex(IndexName("nope")));
}

#[test]
fn wrong_key_type_errors() {
    let g = base();
    let wrong_tag = KeyTag::of::<u8>();
    let err = match g.index_hit(BY_VAL, &KeyBytes::of(&11u8), wrong_tag) {
        Err(e) => e,
        Ok(_) => panic!("expected Err"),
    };
    match err {
        IndexError::KeyTagMismatch { index, expected, got } => {
            assert_eq!(index, BY_VAL);
            assert_eq!(got, wrong_tag);
            assert_eq!(expected, tag());
        }
        other => panic!("expected KeyTagMismatch, got {other:?}"),
    }
}

#[test]
fn duplicate_key_in_batch_refused_leaves_graph_and_ids_untouched() {
    let mut g = base();
    let before = view(&g);
    let r = g.modify(vec![N::<u32, ER>(10).val(50u32).into(), N::<u32, ER>(11).val(50u32).into()]);
    match r {
        Err(Modify::Apply(Apply::Index(apply::Index::DuplicateKey { index, existing }))) => {
            assert_eq!(index, BY_VAL);
            assert_eq!(*existing, 4);
        }
        Err(other) => panic!("expected DuplicateKey, got {other:?}"),
        Ok(_) => panic!("expected Err"),
    }
    assert_eq!(view(&g), before);

    let m = g.modify(vec![N::<u32, ER>(20).val(200u32).into()]).unwrap();
    assert_eq!(*m.new_node_ids[&grw::graph::dsl::LocalId(20)] as u32, 4, "id space must not have leaked");
}

#[test]
fn duplicate_key_against_existing_refused() {
    let mut g = base();
    let before = view(&g);
    let r = g.modify(vec![N::<u32, ER>(10).val(11u32).into()]);
    match r {
        Err(Modify::Apply(Apply::Index(apply::Index::DuplicateKey { index, existing }))) => {
            assert_eq!(index, BY_VAL);
            assert_eq!(*existing, 1);
        }
        Err(other) => panic!("expected DuplicateKey, got {other:?}"),
        Ok(_) => panic!("expected Err"),
    }
    assert_eq!(view(&g), before);
}

#[test]
fn swap_releases_old_key_and_claims_new() {
    let mut g = base();
    g.modify(vec![X::<u32, ER>(1).val(999u32).into()]).unwrap();
    assert!(matches!(g.index_hit(BY_VAL, &KeyBytes::of(&11u32), tag()).unwrap(), IndexHit::None));
    match g.index_hit(BY_VAL, &KeyBytes::of(&999u32), tag()).unwrap() {
        IndexHit::One(n) => assert_eq!(*n, 1),
        _ => panic!("expected One"),
    }
}

#[test]
fn remove_releases_key() {
    let mut g = base();
    g.modify(vec![(!X::<u32, ER>(1)).into()]).unwrap();
    assert!(matches!(g.index_hit(BY_VAL, &KeyBytes::of(&11u32), tag()).unwrap(), IndexHit::None));
}

#[test]
fn swap_away_in_batch_allows_reuse() {
    let mut g = base();
    let m = g
        .modify(vec![X::<u32, ER>(1).val(999u32).into(), N::<u32, ER>(10).val(11u32).into()])
        .unwrap();
    let new_id = m.new_node_ids[&grw::graph::dsl::LocalId(10)];
    match g.index_hit(BY_VAL, &KeyBytes::of(&11u32), tag()).unwrap() {
        IndexHit::One(n) => assert_eq!(n, new_id),
        _ => panic!("expected One"),
    }
}

#[test]
fn with_indices_on_populated_graph_detects_collision() {
    let g: MG = grw::mgraph![<u32, ER>; N(0).val(5u32), N(1).val(5u32), N(2).val(6u32)].unwrap();
    match g.with_indices(vec![by_val()]) {
        Err(IndexError::NotUnique { index, nodes }) => {
            assert_eq!(index, BY_VAL);
            assert_eq!(nodes, vec![grw::id::N(0), grw::id::N(1)]);
        }
        Err(other) => panic!("expected NotUnique, got {other:?}"),
        Ok(_) => panic!("expected Err"),
    }
}

#[test]
fn drop_index_then_hit_errors() {
    let mut g = base();
    let decl = g.drop_index(BY_VAL).unwrap();
    assert_eq!(decl.name(), BY_VAL);
    let err = match g.index_hit(BY_VAL, &KeyBytes::of(&11u32), tag()) {
        Err(e) => e,
        Ok(_) => panic!("expected Err"),
    };
    assert_eq!(err, IndexError::NoSuchIndex(BY_VAL));
    // the other index is untouched
    assert!(matches!(g.index_hit(BY_BUCKET, &KeyBytes::of(&1u32), tag()).unwrap(), IndexHit::Many(_)));
}

#[test]
fn from_graph_preserves_catalogue_and_hits() {
    let g = base();
    let g2 = MG::from_graph(&g);
    assert_eq!(g2.catalogue().len(), g.catalogue().len());
    match g2.index_hit(BY_VAL, &KeyBytes::of(&11u32), tag()).unwrap() {
        IndexHit::One(n) => assert_eq!(*n, 1),
        _ => panic!("expected One"),
    }
    match g2.index_hit(BY_BUCKET, &KeyBytes::of(&1u32), tag()).unwrap() {
        IndexHit::Many(set) => {
            let mut ids: Vec<u32> = set.iter().map(|n| *n).collect();
            ids.sort_unstable();
            assert_eq!(ids, vec![0, 3]);
        }
        _ => panic!("expected Many"),
    }
}

#[test]
fn optional_key_none_excludes_node_from_index() {
    let g: MG = grw::mgraph![<u32, ER>; N(0).val(5u32), N(1).val(6u32)].unwrap();
    let g = g.with_indices(vec![by_even()]).unwrap();
    assert!(matches!(g.index_hit(BY_EVEN, &KeyBytes::of(&5u32), tag()).unwrap(), IndexHit::None));
    match g.index_hit(BY_EVEN, &KeyBytes::of(&6u32), tag()).unwrap() {
        IndexHit::One(n) => assert_eq!(*n, 1),
        _ => panic!("expected One"),
    }
}

#[test]
fn duplicate_key_reports_alphabetically_first_index_regardless_of_declaration_order() {
    // Declared zeta-then-alpha; both key off the same value, so a batch that
    // collides on either collides on both. The reported index must be
    // catalogue order (sorted by name: alpha, zeta), not declaration order.
    let g: MG = grw::mgraph![<u32, ER>; N(0).val(1u32)].unwrap();
    let mut g = g.with_indices(vec![by_zeta(), by_alpha()]).unwrap();
    let before = view(&g);

    let r = g.modify(vec![N::<u32, ER>(10).val(99u32).into(), N::<u32, ER>(11).val(99u32).into()]);
    match r {
        Err(Modify::Apply(Apply::Index(apply::Index::DuplicateKey { index, existing }))) => {
            assert_eq!(index, ALPHA);
            assert_eq!(*existing, 1);
        }
        Err(other) => panic!("expected DuplicateKey, got {other:?}"),
        Ok(_) => panic!("expected Err"),
    }
    assert_eq!(view(&g), before);
}
