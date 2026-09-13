//! The worked example behind grw.rs — every snippet on the landing page is
//! extracted verbatim from this file, so the site can never drift from the API.
//!
//! Story: a microservice call graph. A deploy introduces a circular
//! dependency; patterns find it, `modify!` removes it, navigation shows the
//! cheapest call chain improved.

use grw::edge::{dir, End};
use grw::search::path::{Config, Dijkstra, PathConstraint};
use grw::graph::dsl::LocalId;

type ER = grw::edge::Dir<u32>;
type Fleet = grw::MGraph<&'static str, ER>;

fn outgoing(slot: dir::Slot, _latency: &u32) -> bool {
    matches!(slot, dir::Slot(End::Src))
}

/// Act 1 — build the fleet: nodes are services, directed edges are calls,
/// edge values are latencies in ms.
fn build_fleet() -> Fleet {
    grw::mgraph![<&'static str, ER>;
        N(0).val("gateway") & E().val(2) >> N(1).val("auth"),
        n(0) & E().val(3) >> N(2).val("orders"),
        n(1) & E().val(9) >> N(3).val("db"),
        n(2) & E().val(4) >> N(4).val("billing"),
        n(2) & E().val(12) >> n(3),
        n(4) & E().val(1) >> n(3)
    ]
    .unwrap()
}

/// Act 2 — the deploy: add a cache between orders and db, plus an
/// accidental callback edge billing → orders. One atomic transaction.
fn deploy(g: &mut Fleet) {
    grw::modify!(g, [
        X(2) & E().val(1) >> N(0).val("cache"),
        n(0) & E().val(2) >> x(3),
        x(4) & E().val(5) >> x(2)
    ])
    .unwrap();
}

#[test]
fn act1_build() {
    let g = build_fleet();
    assert_eq!(g.node_count(), 5);
    assert_eq!(g.edge_count(), 6);
}

#[test]
fn act2_deploy() {
    let mut g = build_fleet();
    deploy(&mut g);
    assert_eq!(g.node_count(), 6); // + cache
    assert_eq!(g.edge_count(), 9); // + orders→cache, cache→db, billing→orders
}

/// Act 3a — fan-out: services that call at least two others.
#[test]
fn act3_fanout() {
    let mut g = build_fleet();
    deploy(&mut g);

    let session = grw::search![&g,
        get(Mono) {
            N(0) >> N(1),
            n(0) >> N(2)
        }
    ]
    .unwrap();

    let mut fanout: Vec<grw::id::N> = session.iter().map(|m| m[0]).collect();
    fanout.sort();
    fanout.dedup();
    // gateway, orders — and billing, whose callback edge made it a fan-out too
    assert_eq!(fanout.len(), 3);
}

/// Act 3b — entry points: services nobody calls. `!N` negates the caller.
#[test]
fn act3_roots() {
    let mut g = build_fleet();
    deploy(&mut g);

    let session = grw::search![&g,
        get(Mono) {
            !N(1) >> N(0)
        }
    ]
    .unwrap();

    let roots: Vec<_> = session.iter().map(|m| m[0]).collect();
    assert_eq!(roots.len(), 1); // only the gateway has no caller
}

/// Act 3c — redundant shortcut: a call chain a → b → c where a ALSO calls c
/// directly. The direct edge is the get; the ban would reject it — so here we
/// ask the opposite way: find chains where the shortcut exists.
#[test]
fn act3_shortcut() {
    let mut g = build_fleet();
    deploy(&mut g);

    let session = grw::search![&g,
        get(Mono) {
            N(0) >> N(1),
            n(1) >> N(2),
            n(0) >> n(2)
        }
    ]
    .unwrap();

    // orders → billing → db, with the direct orders → db shortcut
    let hits: Vec<_> = session.iter().collect();
    assert!(!hits.is_empty());
}

/// Act 3d — the accident: find circular dependencies. Homo lets the two
/// endpoints collapse onto the SAME node; the Mono path can't reuse it in
/// between. Consumers filter with the Match mapping itself.
#[test]
fn act3_cycles() {
    let mut g = build_fleet();
    let session = grw::search![&g,
        get(Homo) { N(0), N(1) },
        get(Mono) { n(0) >> ..n(1).dfs().len(1..) }
    ]
    .unwrap();
    let cycles_before = session
        .iter()
        .filter(|m| m[0] == m[1] && !m.path(0).is_empty())
        .count();
    assert_eq!(cycles_before, 0); // clean before the deploy

    deploy(&mut g);
    let session = grw::search![&g,
        get(Homo) { N(0), N(1) },
        get(Mono) { n(0) >> ..n(1).dfs().len(1..) }
    ]
    .unwrap();
    let cycles_after = session
        .iter()
        .filter(|m| m[0] == m[1] && !m.path(0).is_empty())
        .count();
    assert!(cycles_after > 0); // billing → orders closed a loop
}

/// Act 4 — the fix: remove the callback edge, atomically. Re-run the same
/// pattern: the cycle is gone.
#[test]
fn act4_fix() {
    let mut g = build_fleet();
    deploy(&mut g);

    grw::modify!(g, [x(4) & !e() >> x(2)]).unwrap();

    let session = grw::search![&g,
        get(Homo) { N(0), N(1) },
        get(Mono) { n(0) >> ..n(1).dfs().len(1..) }
    ]
    .unwrap();
    let cycles = session
        .iter()
        .filter(|m| m[0] == m[1] && !m.path(0).is_empty())
        .count();
    assert_eq!(cycles, 0);
}

/// Act 5 — navigation: cheapest call chain gateway → db, by summed latency.
/// Before the deploy: gw → orders → billing → db = 8 ms.
/// After: gw → orders → cache → db = 6 ms. The cache paid off.
#[test]
fn act5_cheapest_chain() {
    let mut g = build_fleet();
    let constraint = PathConstraint::default();

    let cfg = Config::new(()).navigate(Dijkstra::weighted(|ms: &u32| *ms as f64));
    let (cost, _path) = g
        .path_navigate(grw::id::N(0), grw::id::N(3), outgoing, cfg, &constraint)
        .next()
        .unwrap();
    assert_eq!(cost, 8.0);

    deploy(&mut g);
    let cfg = Config::new(()).navigate(Dijkstra::weighted(|ms: &u32| *ms as f64));
    let (cost, path) = g
        .path_navigate(grw::id::N(0), grw::id::N(3), outgoing, cfg, &constraint)
        .next()
        .unwrap();
    assert_eq!(cost, 6.0);
    assert_eq!(path.len(), 4); // gw, orders, cache, db
}

/// Consuming — the mapping is addressed by YOUR pattern numbering:
/// `m[0]` / `m.get(0)` give graph ids, `translate` adds values,
/// `m.path(edge)` gives the concrete node sequence behind a path edge.
#[test]
fn consume_matches() {
    let mut g = build_fleet();
    deploy(&mut g);

    let session = grw::search![&g,
        get(Mono) { N(0) >> N(1) }
    ]
    .unwrap();

    let mut names = Vec::new();
    for m in &session {
        let caller: grw::id::N = m[0]; // Index — absence would be a bug
        let callee = m.get(1).unwrap(); // Option — the tolerant form
        let tm = session.translate(&m);
        let (_, caller_name) = tm.node(LocalId(0)).unwrap();
        let (_, callee_name) = tm.node(LocalId(1)).unwrap();
        names.push((*caller_name, *callee_name));
        let _ = (caller, callee);
    }
    assert!(names.contains(&("gateway", "orders")));
    assert!(names.contains(&("billing", "db")));
}

#[test]
fn story_named_nodes_and_pattern() {
    use grw::graph::{edge, Graph, MGraph};
    use grw::search::{Pattern, RevCsr, Seq};
    use grw::{mgraph, pattern};

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Role { Signer, Provider }

    let g: MGraph<Role, edge::Dir<()>> = mgraph![N(0).val(Role::Signer) >> N(1).val(Role::Provider)].unwrap();
    let p: Pattern<Role, edge::Dir<()>> = pattern![get(Mono) { N(s: Role::Signer) >> N(p: Role::Provider) }].unwrap();
    let indexed = g.index(RevCsr);
    let matches: Vec<_> = Seq::search(p.query(), &indexed).unwrap().collect();
    assert_eq!(matches.len(), 1);
    for m in &matches {
        assert_eq!(m[p.lid("s").unwrap()], grw::id::N(0));
        assert_eq!(m[p.lid("p").unwrap()], grw::id::N(1));
    }
}

#[test]
fn story_value_patterns() {
    use grw::graph::{edge, Graph, MGraph};
    use grw::search::{Pattern, RevCsr, Seq};
    use grw::{mgraph, pattern};

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Shape { Circle(u8), Square(u8) }

    let g: MGraph<Shape, edge::Undir<u8>> =
        mgraph![N(0).val(Shape::Circle(1)) & E().val(7u8) ^ N(1).val(Shape::Square(2))].unwrap();
    let nodes: Pattern<Shape, edge::Undir<u8>> =
        pattern![get(Mono) { N(c: Shape::Circle(_)) ^ N(s: Shape::Square(2)) }].unwrap();
    let edges: Pattern<Shape, edge::Undir<u8>> = pattern![get(Mono) { N(a) & E(7) ^ N(b) }].unwrap();
    let indexed = g.index(RevCsr);
    assert_eq!(Seq::search(nodes.query(), &indexed).unwrap().count(), 1);
    assert_eq!(Seq::search(edges.query(), &indexed).unwrap().count(), 2);
}

#[test]
fn story_context_pin_in_session() {
    use grw::graph::{edge, MGraph};
    use grw::{mgraph, search};

    let g: MGraph<(), edge::Undir<()>> = mgraph![N(0) ^ (N(1) ^ (N(2) ^ n(0)))].unwrap();
    let hub = grw::id::N(1);
    let session = search![&g, get(Mono) { X(h = hub) ^ N(o) }].unwrap();
    let mut neighbours: Vec<_> = session.iter().map(|m| m.get(1).unwrap()).collect();
    neighbours.sort();
    assert_eq!(neighbours, vec![grw::id::N(0), grw::id::N(2)]);
}

#[test]
fn story_pin_stored_pattern() {
    use grw::graph::{edge, MGraph};
    use grw::search::Pattern;
    use grw::{mgraph, pattern, search};

    let g: MGraph<u8, edge::Undir<()>> = mgraph![N(0).val(1u8) ^ (N(1).val(2u8) ^ N(2).val(3u8))].unwrap();
    let p: Pattern<u8, edge::Undir<()>> = pattern![get(Mono) { N(a) ^ N(b) }].unwrap();
    let middle = grw::id::N(1);
    let s = search![&g, p with X(a = middle)].unwrap();
    assert_eq!(s.iter().count(), 2);
}

#[test]
fn story_negation_bans_and_pins_and_form_vs_or_form() {
    use grw::graph::{edge, MGraph};
    use grw::search::{Pattern, Session};
    use grw::{mgraph, pattern};

    let g: MGraph<(), edge::Undir<()>> = mgraph![N(0), N(1), N(2), n(0) ^ n(2)].unwrap();
    let (p_id, q_id, five) = (grw::id::N(0), grw::id::N(1), grw::id::N(2));

    // "not all of these together": one ban_only node shared by both edges.
    let and_form: Pattern<(), edge::Undir<()>> = pattern![
        get(Mono) { N(p), N(q) },
        ban(Mono) { n(p) ^ N(c), n(q) ^ n(c) }
    ]
    .unwrap();
    let and_session =
        Session::from_pattern(and_form, &g, &[("p", p_id), ("q", q_id), ("c", five)]).unwrap();
    assert_eq!(and_session.iter().count(), 1); // only p—5 holds, so the ban can't fire

    // "none of these": two separate bans, each pinned to the same node 5.
    let or_form: Pattern<(), edge::Undir<()>> = pattern![
        get(Mono) { N(p), N(q) },
        ban(Mono) { n(p) ^ N(c1) },
        ban(Mono) { n(q) ^ N(c2) }
    ]
    .unwrap();
    let or_session =
        Session::from_pattern(or_form, &g, &[("p", p_id), ("q", q_id), ("c1", five), ("c2", five)])
            .unwrap();
    assert_eq!(or_session.iter().count(), 0); // p—5 alone is enough to fire the first ban
}

#[test]
fn story_negation_bans_and_pins_narrows_to_one_node() {
    use grw::graph::{edge, MGraph};
    use grw::{mgraph, search};

    let g: MGraph<bool, edge::Dir<()>> = mgraph![
        N(0).val(true), N(1).val(true), N(2).val(true),
        N(3).val(false), N(4).val(false),
        n(0) >> n(3), // p1 -> alice
        n(1) >> n(4)  // p2 -> bob
    ]
    .unwrap();
    let alice = grw::id::N(3);

    let session = search![&g,
        get(Mono) { N(p).val(true) >> (!X(a = alice)).test(|_: &bool| true) }
    ]
    .unwrap();
    let mut survivors: Vec<_> = session.iter().map(|m| m.get(0).unwrap()).collect();
    survivors.sort();
    assert_eq!(survivors, vec![grw::id::N(1), grw::id::N(2)]); // p2 and p3 — only p1 blocks Alice
}

// README.md § Indices

#[test]
fn readme_indices_declare() {
    use grw::graph::index::{Cardinality, IndexDecl, IndexName};
    use grw::graph::{edge, MGraph};

    const BY_VAL: IndexName = IndexName("by_val");

    let g: MGraph<u32, edge::Undir<()>> = grw::mgraph![
        N(0).val(10u32) ^ N(1).val(20u32)
    ]
    .unwrap();

    let g = g
        .with_indices(vec![IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v))])
        .unwrap();
    assert_eq!(g.catalogue().len(), 1);
}

#[test]
fn readme_indices_maintain() {
    use grw::graph::index::{Cardinality, IndexDecl, IndexName};
    use grw::graph::{edge, MGraph};
    use grw::modify;

    const BY_VAL: IndexName = IndexName("by_val");
    const BY_MOD: IndexName = IndexName("by_mod");

    let g: MGraph<u32, edge::Undir<()>> = grw::mgraph![
        N(0).val(10u32) ^ N(1).val(20u32)
    ]
    .unwrap();
    let mut g = g
        .with_indices(vec![IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v))])
        .unwrap();

    g.add_index(IndexDecl::new(BY_MOD, Cardinality::Multi, |v: &u32| Some(*v % 5))).unwrap();
    let _ = g.drop_index(BY_MOD).unwrap();

    let Err(err) = modify!(g, [N(2).val(10u32)]) else { panic!("expected Err") };
    assert!(matches!(
        err,
        grw::modify::error::Modify::Apply(grw::modify::error::Apply::Index(
            grw::modify::error::apply::Index::DuplicateKey { .. }
        ))
    ));
}

#[test]
fn readme_indices_hit() {
    use grw::graph::index::{Cardinality, IndexDecl, IndexHit, IndexName, KeyBytes, KeyTag};
    use grw::graph::{edge, MGraph};
    use grw::Graph as _;

    const BY_VAL: IndexName = IndexName("by_val");

    let g: MGraph<u32, edge::Undir<()>> = grw::mgraph![
        N(0).val(10u32) ^ N(1).val(20u32)
    ]
    .unwrap();
    let g = g
        .with_indices(vec![IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v))])
        .unwrap();

    let hit = g.index_hit(BY_VAL, &KeyBytes::of(&10u32), KeyTag::of::<u32>()).unwrap();
    assert!(matches!(hit, IndexHit::One(n) if n == grw::id::N(0)));
}

#[test]
fn readme_indices_persistence() {
    use grw::graph::index::{Cardinality, IndexDecl, IndexName};
    use grw::graph::{edge, MGraph};

    const BY_VAL: IndexName = IndexName("by_val");

    let g: MGraph<u32, edge::Undir<()>> = grw::mgraph![
        N(0).val(10u32) ^ N(1).val(20u32)
    ]
    .unwrap();
    let g = g
        .with_indices(vec![IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v))])
        .unwrap();

    let path = std::env::temp_dir().join(format!("grw_readme_indices_{}.grw", std::process::id()));
    g.save(&path).unwrap();
    let g2: MGraph<u32, edge::Undir<()>> = MGraph::load_with(
        &path,
        vec![IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v))],
    )
    .unwrap();
    assert_eq!(g2.catalogue().len(), 1);

    std::fs::remove_file(&path).unwrap();
}

// doc/site/src/indices.md

#[test]
fn indices_declaring_an_index() {
    use grw::graph::index::{Cardinality, IndexDecl, IndexName};
    use grw::graph::{edge, MGraph};

    const BY_VAL: IndexName = IndexName("by_val");
    const BY_BUCKET: IndexName = IndexName("by_bucket");

    let g: MGraph<u32, edge::Undir<()>> = grw::mgraph![
        N(0).val(10u32) ^ N(1).val(20u32) ^ N(2).val(30u32)
    ]
    .unwrap();

    let g = g
        .with_indices(vec![
            IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v)),
            IndexDecl::new(BY_BUCKET, Cardinality::Multi, |v: &u32| Some(*v % 20)),
        ])
        .unwrap();
    assert_eq!(g.catalogue().len(), 2);
}

#[test]
fn indices_maintaining_an_index_add_and_drop() {
    use grw::graph::index::{Cardinality, IndexDecl, IndexName};
    use grw::graph::{edge, MGraph};

    const BY_PARITY: IndexName = IndexName("by_parity");

    let mut g: MGraph<u32, edge::Undir<()>> = grw::mgraph![
        N(0).val(10u32) ^ N(1).val(20u32)
    ]
    .unwrap();

    g.add_index(IndexDecl::new(BY_PARITY, Cardinality::Multi, |v: &u32| Some(*v % 2))).unwrap();
    let dropped = g.drop_index(BY_PARITY).unwrap();
    assert_eq!(dropped.name(), BY_PARITY);
}

#[test]
fn indices_maintaining_an_index_duplicate_key_refused() {
    use grw::graph::index::{Cardinality, IndexDecl, IndexName};
    use grw::graph::{edge, MGraph};
    use grw::modify;

    const BY_VAL: IndexName = IndexName("by_val");

    let g: MGraph<u32, edge::Undir<()>> = grw::mgraph![N(0).val(10u32)].unwrap();
    let mut g = g
        .with_indices(vec![IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v))])
        .unwrap();

    let Err(err) = modify!(g, [N(1).val(10u32), N(2).val(20u32)]) else { panic!("expected Err") };
    assert!(matches!(
        err,
        grw::modify::error::Modify::Apply(grw::modify::error::Apply::Index(
            grw::modify::error::apply::Index::DuplicateKey { index, .. }
        )) if index == BY_VAL
    ));
    assert_eq!(g.node_count(), 1);
}

#[test]
fn indices_hitting_an_index() {
    use grw::graph::index::{Cardinality, IndexDecl, IndexHit, IndexName, KeyBytes, KeyTag};
    use grw::graph::{edge, MGraph};
    use grw::Graph as _;

    const BY_BUCKET: IndexName = IndexName("by_bucket");

    let g: MGraph<u32, edge::Undir<()>> = grw::mgraph![
        N(0).val(1u32) ^ N(1).val(21u32) ^ N(2).val(2u32)
    ]
    .unwrap();
    let g = g
        .with_indices(vec![IndexDecl::new(BY_BUCKET, Cardinality::Multi, |v: &u32| Some(*v % 20))])
        .unwrap();

    match g.index_hit(BY_BUCKET, &KeyBytes::of(&1u32), KeyTag::of::<u32>()).unwrap() {
        IndexHit::Many(set) => assert_eq!(set.len(), 2), // nodes 0 and 1 both key to 1
        _ => panic!("expected a Many hit"),
    }
}

// doc/site/src/persistence.md

#[test]
fn persistence_binary_format_read_header() {
    use grw::graph::persist;
    use grw::graph::{edge, MGraph};

    let g: MGraph<(), edge::Undir<()>> = grw::mgraph![N(0) ^ N(1)].unwrap();
    let path = std::env::temp_dir().join(format!("grw_site_story_header_{}.grw", std::process::id()));
    g.save(&path).unwrap();

    let header = persist::read_header(&path).unwrap();
    assert_eq!(header.version, 3);
    assert_eq!(header.sections.len(), 7);

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn persistence_indices_round_trip() {
    use grw::graph::index::{Cardinality, IndexDecl, IndexName};
    use grw::graph::{edge, MGraph};

    const BY_VAL: IndexName = IndexName("by_val");

    let g: MGraph<u32, edge::Undir<()>> = grw::mgraph![N(0).val(10u32)].unwrap();
    let g = g
        .with_indices(vec![IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v))])
        .unwrap();
    let path = std::env::temp_dir().join(format!("grw_site_story_indexed_{}.grw", std::process::id()));
    g.save(&path).unwrap();

    let g2: MGraph<u32, edge::Undir<()>> = MGraph::load_with(
        &path,
        vec![IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v))],
    )
    .unwrap();
    assert_eq!(g2.catalogue().len(), 1);

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn persistence_loading_an_old_file_convert() {
    use grw::graph::persist;
    use grw::graph::{self, edge, MGraph};

    let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/golden_v2_dir.grw");
    let dir = std::env::temp_dir().join(format!("grw_site_story_convert_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let to = dir.join("upgraded_v3.grw");

    let report = persist::convert::<(), edge::Dir<()>>(&from, &to).unwrap();
    let g: graph::MDir0 = MGraph::load(&to).unwrap();
    assert_eq!(report.node_count, g.node_count() as u64);

    std::fs::remove_dir_all(&dir).unwrap();
}

// doc/site/src/search.md § Key Predicates

#[test]
fn search_key_predicates() {
    use grw::graph::index::{Cardinality, IndexDecl, IndexName};
    use grw::graph::{edge, MGraph};
    use grw::{mgraph, search};

    const BY_VAL: IndexName = IndexName("by_val");

    let g: MGraph<u32, edge::Undir<()>> = mgraph![
        N(0).val(10u32) ^ (N(1).val(20u32) ^ N(2).val(30u32))
    ]
    .unwrap();
    let g = g
        .with_indices(vec![IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v))])
        .unwrap();

    // method form and macro form bind identically
    let by_key = search![&g, get(Mono) { N(a).key(BY_VAL, 20u32) ^ N(b) }].unwrap();
    let macro_form = search![&g, get(Mono) { N(a: key(BY_VAL, 20u32)) ^ N(b) }].unwrap();
    assert_eq!(by_key.iter().count(), 2);
    assert_eq!(macro_form.iter().count(), 2);

    // key_in unions its hits
    let in_set = search![&g, get(Mono) { N(a).key_in(BY_VAL, [10u32, 30u32]) ^ N(b) }].unwrap();
    assert_eq!(in_set.iter().count(), 2);

    // a pinned context node can carry a key check too
    let ten = grw::id::N(0);
    let pinned = search![&g, get(Mono) { X(c = ten : key(BY_VAL, 10u32)) ^ N(o) }].unwrap();
    assert_eq!(pinned.iter().count(), 1);
}

#[test]
fn search_key_predicates_result_change() {
    use grw::graph::index::IndexName;
    use grw::graph::{edge, MGraph};
    use grw::search::{error, RevCsr, Search, Seq};
    use grw::{mgraph, search, Graph as _};

    const BY_VAL: IndexName = IndexName("by_val");

    let plain: MGraph<u32, edge::Undir<()>> = mgraph![N(0).val(10u32)].unwrap();
    let Search::Resolved(r) = search![<u32, edge::Undir<()>>;
        get(Mono) { N(a).key(BY_VAL, 10u32) }
    ]
    .unwrap() else {
        panic!()
    };

    let indexed = plain.index(RevCsr);
    let Err(error::Search::IndexMissing { index }) = Seq::search(r.query(), &indexed) else {
        panic!("expected IndexMissing")
    };
    assert_eq!(index, BY_VAL);
}
