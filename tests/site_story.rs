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
