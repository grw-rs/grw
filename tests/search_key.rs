//! One test per named invariant of index-backed node predicates, in the
//! `modify_atomicity` style: each test asserts exactly one behaviour of
//! `.key` / `.key_in` and the candidate pools they resolve to.

use std::collections::BTreeSet;

use rayon::iter::ParallelIterator;

use grw::Graph as _;
use grw::graph::edge;
use grw::graph::index::{Cardinality, IndexDecl, IndexName};
use grw::graph::MGraph;
use grw::modify::{self, N as MN, n as mn};
use grw::search::engine::seq::OwnedIter;
use grw::search::{error, Par, Pattern, RevCsr, Search, Seq, Session};
use grw::{mgraph, pattern, search, Silent};

type ER = edge::Undir<()>;
type MG = MGraph<u32, ER>;

const BY_VAL: IndexName = IndexName("by_val");
const BY_BUCKET: IndexName = IndexName("by_bucket");

fn by_val() -> IndexDecl<u32> {
    IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v))
}

fn by_bucket() -> IndexDecl<u32> {
    IndexDecl::new(BY_BUCKET, Cardinality::Multi, |v: &u32| Some(*v % 100))
}

/// A path of `count` nodes whose values are their own ids, so `by_val` is a
/// unique key and `by_bucket` (`v % 100`) puts every hundredth node together.
fn chain(count: u32) -> MG {
    let mut ops: Vec<modify::Node<u32, ER>> =
        (0..count).map(|i| MN::<u32, ER>(i).val(i).into()).collect();
    for i in 0..count.saturating_sub(1) {
        ops.push((mn::<u32, ER>(i) ^ mn::<u32, ER>(i + 1)).into());
    }
    let mut g: MG = mgraph![].unwrap();
    g.modify(ops).unwrap();
    g
}

fn indexed_chain(count: u32) -> MG {
    chain(count).with_indices(vec![by_val(), by_bucket()]).unwrap()
}

fn pairs<I: IntoIterator<Item = grw::search::Match>>(matches: I) -> BTreeSet<(u32, u32)> {
    matches
        .into_iter()
        .map(|m| (*m.get(0u32).unwrap() as u32, *m.get(1u32).unwrap() as u32))
        .collect()
}

#[test]
fn key_hit_seeds_the_match() {
    let g = indexed_chain(10_000);
    let by_key = search![&g, get(Mono) { N(a).key(BY_VAL, 4242u32) ^ N(b) }].unwrap();
    let by_test = search![&g, get(Mono) { N(a).test(|v: &u32| *v == 4242) ^ N(b) }].unwrap();

    assert_eq!(
        pairs(by_key.iter()),
        pairs(by_test.iter()),
        "a key predicate must select exactly what the equivalent closure selects"
    );
    assert_eq!(
        by_key.candidate_pool(0).unwrap(),
        &[grw::id::N(4242)],
        "a unique-index hit resolves to a one-element pool"
    );
    assert!(
        by_key.candidate_pool(1).is_none(),
        "an unpredicated node has no pool"
    );
}

#[test]
fn key_in_unions_the_hits() {
    let g = indexed_chain(300);
    let s = search![&g, get(Mono) { N(a).key_in(BY_BUCKET, [7u32, 8u32]) ^ N(b) }].unwrap();
    let t = search![&g, get(Mono) { N(a).test(|v: &u32| *v % 100 == 7 || *v % 100 == 8) ^ N(b) }]
        .unwrap();

    assert_eq!(
        s.candidate_pool(0).unwrap(),
        &[
            grw::id::N(7),
            grw::id::N(8),
            grw::id::N(107),
            grw::id::N(108),
            grw::id::N(207),
            grw::id::N(208)
        ],
        "key_in resolves to the sorted union of its per-key hits"
    );
    assert_eq!(pairs(s.iter()), pairs(t.iter()));
}

#[test]
fn key_and_test_intersect() {
    let g = indexed_chain(300);
    let s = search![&g, get(Mono) {
        N(a).key_in(BY_BUCKET, [7u32, 8u32]).test(|v: &u32| *v > 100) ^ N(b)
    }]
    .unwrap();

    assert_eq!(
        s.candidate_pool(0).unwrap(),
        &[grw::id::N(107), grw::id::N(108), grw::id::N(207), grw::id::N(208)],
        "the closure half of an And is evaluated on the pooled ids only"
    );
    let t = search![&g, get(Mono) {
        N(a).test(|v: &u32| (*v % 100 == 7 || *v % 100 == 8) && *v > 100) ^ N(b)
    }]
    .unwrap();
    assert_eq!(pairs(s.iter()), pairs(t.iter()));
}

#[test]
fn key_on_context_node() {
    let g = indexed_chain(50);
    let five = grw::id::N(5);
    let s = search![&g, get(Mono) { X(c = five).key(BY_VAL, 5u32) ^ N(o) }].unwrap();
    let mut seen: Vec<u32> = s.iter().map(|m| *m.get(1u32).unwrap() as u32).collect();
    seen.sort_unstable();
    assert_eq!(seen, vec![4, 6], "a key on a context node narrows it like any other node");

    let miss = search![&g, get(Mono) { X(c = five).key(BY_VAL, 6u32) ^ N(o) }].unwrap();
    assert_eq!(
        miss.iter().count(),
        0,
        "a pin outside its node's key pool yields no match"
    );
}

#[test]
fn key_on_negated_context() {
    let g = indexed_chain(50);
    let five = grw::id::N(5);
    let blocked =
        search![&g, get(Mono) { N(a) ^ N(b), n(a) ^ (!X(c = five)).key(BY_VAL, 5u32) }].unwrap();
    let blocked_test = search![&g, get(Mono) {
        N(a) ^ N(b), n(a) ^ (!X(c = five)).test(|v: &u32| *v == 5)
    }]
    .unwrap();
    assert_eq!(
        pairs(blocked.iter()),
        pairs(blocked_test.iter()),
        "a key on a negated context node behaves as the equivalent closure"
    );

    let open =
        search![&g, get(Mono) { N(a) ^ N(b), n(a) ^ (!X(c = five)).key(BY_VAL, 999_999u32) }].unwrap();
    let open_test = search![&g, get(Mono) {
        N(a) ^ N(b), n(a) ^ (!X(c = five)).test(|v: &u32| *v == 999_999)
    }]
    .unwrap();
    assert_eq!(pairs(open.iter()), pairs(open_test.iter()));
    assert!(
        pairs(open.iter()).len() > pairs(blocked.iter()).len(),
        "guard: the negated key actually discriminates"
    );
}

#[test]
fn key_inside_ban_cluster() {
    let g = indexed_chain(50);
    let banned = search![&g,
        get(Mono) { N(a) ^ N(b) },
        ban(Mono) { n(a) ^ N(z).key(BY_VAL, 5u32) }
    ]
    .unwrap();
    let plain = search![&g,
        get(Mono) { N(a) ^ N(b) },
        ban(Mono) { n(a) ^ N(z).test(|v: &u32| *v == 5) }
    ]
    .unwrap();
    assert_eq!(
        pairs(banned.iter()),
        pairs(plain.iter()),
        "a ban-only node's key pool drives its backtracking candidates"
    );
    let unbanned = search![&g, get(Mono) { N(a) ^ N(b) }].unwrap();
    assert!(
        pairs(banned.iter()).len() < pairs(unbanned.iter()).len(),
        "guard: the ban cluster with a key predicate actually fires"
    );
}

#[test]
fn key_with_pin_member_and_non_member() {
    let g = indexed_chain(50);
    let indexed = g.index(RevCsr);
    let Search::Resolved(r): Search<u32, ER> =
        search![get(Mono) { N(0).key(BY_VAL, 5u32) ^ N(1) }].unwrap()
    else {
        panic!("unexpected context nodes")
    };

    let member = Seq::search_bound(r.query(), &indexed, vec![Some(grw::id::N(5)), None])
        .unwrap()
        .count();
    assert_eq!(member, 2, "a pin inside the key pool keeps its matches");

    let non_member = Seq::search_bound(r.query(), &indexed, vec![Some(grw::id::N(6)), None])
        .unwrap()
        .count();
    assert_eq!(non_member, 0, "a pin outside the key pool matches nothing");
}

#[test]
fn index_missing_from_session() {
    let g = chain(20);
    let err = search![&g, get(Mono) { N(a).key(BY_VAL, 5u32) ^ N(b) }].err().unwrap();
    assert!(
        matches!(err, error::Search::IndexMissing { index } if index == BY_VAL),
        "Session must refuse a key predicate the graph has no index for"
    );
}

#[test]
fn index_missing_from_seq() {
    let g = chain(20);
    let indexed = g.index(RevCsr);
    let Search::Resolved(r): Search<u32, ER> =
        search![get(Mono) { N(0).key(BY_VAL, 5u32) ^ N(1) }].unwrap()
    else {
        panic!("unexpected context nodes")
    };
    let err = Seq::search(r.query(), &indexed).err().unwrap();
    assert!(matches!(err, error::Search::IndexMissing { index } if index == BY_VAL));
}

#[test]
fn index_missing_from_par() {
    let g = chain(20);
    let indexed = g.index(RevCsr);
    let Search::Resolved(r): Search<u32, ER> =
        search![get(Mono) { N(0).key(BY_VAL, 5u32) ^ N(1) }].unwrap()
    else {
        panic!("unexpected context nodes")
    };
    let err = Par::search(r.query(), &indexed).err().unwrap();
    assert!(matches!(err, error::Search::IndexMissing { index } if index == BY_VAL));
}

#[test]
fn index_missing_from_seq_watched() {
    let g = chain(20);
    let indexed = g.index(RevCsr);
    let Search::Resolved(r): Search<u32, ER> =
        search![get(Mono) { N(0).key(BY_VAL, 5u32) ^ N(1) }].unwrap()
    else {
        panic!("unexpected context nodes")
    };
    let err = Seq::search_watched(r.query(), &indexed, Silent).err().unwrap();
    assert!(
        matches!(err, error::Search::IndexMissing { index } if index == BY_VAL),
        "the watched entry point refuses a key predicate the graph has no index for"
    );
}

#[test]
fn key_tag_mismatch_from_seq_watched() {
    let g = indexed_chain(20);
    let indexed = g.index(RevCsr);
    let Search::Resolved(r): Search<u32, ER> =
        search![get(Mono) { N(0).key(BY_VAL, 5u64) ^ N(1) }].unwrap()
    else {
        panic!("unexpected context nodes")
    };
    let err = Seq::search_watched(r.query(), &indexed, Silent).err().unwrap();
    assert!(matches!(err, error::Search::KeyTagMismatch { index } if index == BY_VAL));
}

#[test]
fn index_missing_from_owned_iter() {
    let compiled: Search<u32, ER> =
        search![get(Mono) { N(0).key(BY_VAL, 5u32) ^ N(1) }].unwrap();
    let err = OwnedIter::from_graph_and_search(chain(20), compiled).err().unwrap();
    assert!(matches!(err, error::Search::IndexMissing { index } if index == BY_VAL));
}

#[test]
fn key_tag_mismatch_is_rejected() {
    let g = indexed_chain(20);
    let err = search![&g, get(Mono) { N(a).key(BY_VAL, 5u64) ^ N(b) }].err().unwrap();
    assert!(
        matches!(err, error::Search::KeyTagMismatch { index } if index == BY_VAL),
        "a key of a different type than the index declares is an error, not a miss"
    );
}

#[test]
fn empty_pool_exhausts_immediately() {
    let g = indexed_chain(50);
    let s = search![&g, get(Mono) { N(a).key(BY_VAL, 999_999u32) ^ N(b) }].unwrap();
    assert_eq!(
        s.candidate_pool(0).unwrap().len(),
        0,
        "a key with no hit resolves to an empty pool"
    );
    assert_eq!(s.iter().count(), 0, "an empty pool yields zero matches without panicking");
    assert_eq!(s.par_iter().count(), 0);
}

#[test]
fn seq_and_par_agree_on_key_pools() {
    let g = indexed_chain(300);
    let s = search![&g, get(Mono) { N(a).key_in(BY_BUCKET, [7u32, 8u32]) ^ N(b) }].unwrap();
    let seq = pairs(s.iter());
    let par: BTreeSet<(u32, u32)> = {
        s.par_iter()
            .map(|m| (*m.get(0u32).unwrap() as u32, *m.get(1u32).unwrap() as u32))
            .collect()
    };
    assert_eq!(seq, par, "the parallel driver reads the same pool as the sequential one");
    assert_eq!(seq.len(), s.par_iter().count());
}

#[test]
fn stored_pattern_with_key_runs_via_from_pattern() {
    let g = indexed_chain(300);
    let p: Pattern<u32, ER> =
        pattern![get(Mono) { N(a).key_in(BY_BUCKET, [7u32]) ^ N(b) }].unwrap();
    let s = Session::from_pattern(p, &g, &[]).unwrap();
    let mut seen: Vec<u32> = s.iter().map(|m| *m.get(0u32).unwrap() as u32).collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen, vec![7, 107, 207], "a stored pattern's key predicate survives storage");

    let p2: Pattern<u32, ER> =
        pattern![get(Mono) { N(a).key(BY_VAL, 5u32) ^ N(b) }].unwrap();
    let err = Session::from_pattern(p2, &chain(300), &[]).err().unwrap();
    assert!(matches!(err, error::Search::IndexMissing { index } if index == BY_VAL));
}

#[test]
fn macro_key_form_matches_method_form() {
    let g = indexed_chain(10_000);
    let macro_form = search![&g, get(Mono) { N(a: key(BY_VAL, 4242u32)) ^ N(b) }].unwrap();
    let method_form = search![&g, get(Mono) { N(a).key(BY_VAL, 4242u32) ^ N(b) }].unwrap();
    assert_eq!(
        pairs(macro_form.iter()),
        pairs(method_form.iter()),
        "the `key(..)` macro grammar and the `.key(..)` method must bind identically"
    );
    assert_eq!(macro_form.candidate_pool(0).unwrap(), method_form.candidate_pool(0).unwrap());
}

#[test]
fn macro_key_in_form_matches_method_form() {
    let g = indexed_chain(300);
    let macro_form = search![&g, get(Mono) { N(a: key_in(BY_BUCKET, [7u32, 8u32])) ^ N(b) }].unwrap();
    let method_form = search![&g, get(Mono) { N(a).key_in(BY_BUCKET, [7u32, 8u32]) ^ N(b) }].unwrap();
    assert_eq!(
        pairs(macro_form.iter()),
        pairs(method_form.iter()),
        "the `key_in(..)` macro grammar and the `.key_in(..)` method must bind identically"
    );
    assert_eq!(macro_form.candidate_pool(0).unwrap(), method_form.candidate_pool(0).unwrap());
}

#[test]
fn macro_key_form_on_context_pin_member_and_non_member() {
    let g = indexed_chain(50);
    let five = grw::id::N(5);
    let member = search![&g, get(Mono) { X(c = five : key(BY_VAL, 5u32)) ^ N(o) }].unwrap();
    let mut seen: Vec<u32> = member.iter().map(|m| *m.get(1u32).unwrap() as u32).collect();
    seen.sort_unstable();
    assert_eq!(
        seen,
        vec![4, 6],
        "a macro-form key on a pinned context node narrows it like the method form"
    );

    let non_member = search![&g, get(Mono) { X(c = five : key(BY_VAL, 6u32)) ^ N(o) }].unwrap();
    assert_eq!(
        non_member.iter().count(),
        0,
        "a pin outside its macro-form key's pool yields no match"
    );
}

#[test]
fn macro_key_in_pattern_runs_via_from_pattern() {
    let g = indexed_chain(300);
    let p: Pattern<u32, ER> = pattern![get(Mono) { N(a: key_in(BY_BUCKET, [7u32])) ^ N(b) }].unwrap();
    let s = Session::from_pattern(p, &g, &[]).unwrap();
    let mut seen: Vec<u32> = s.iter().map(|m| *m.get(0u32).unwrap() as u32).collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(
        seen,
        vec![7, 107, 207],
        "a stored pattern built from the macro-form key grammar survives storage"
    );
}
