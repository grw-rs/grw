use grw::graph::{self, Graph as _, MAnydir0, VAnydir0, edge};
use grw::{id, mgraph, modify, vgraph};

fn sorted_edges<G: graph::Graph<(), edge::Anydir<()>>>(
    g: &G,
) -> Vec<(id::N, id::N, edge::anydir::Slot)> {
    let mut v: Vec<_> = g.iter_edges().map(|(a, b, s, _)| (a, b, s)).collect();
    v.sort();
    v
}

fn sorted_neighbors<G: graph::Graph<(), edge::Anydir<()>>>(
    g: &G,
    n: id::N,
) -> Vec<(id::N, edge::anydir::Slot)> {
    let mut v: Vec<_> = g.neighbors(n).unwrap().map(|(nb, s, _)| (nb, s)).collect();
    v.sort();
    v
}

fn sorted_between<G: graph::Graph<(), edge::Anydir<()>>>(
    g: &G,
    a: id::N,
    b: id::N,
) -> Vec<edge::anydir::Slot> {
    let mut v: Vec<_> = g.edges_between(a, b).map(|(s, _)| s).collect();
    v.sort();
    v
}

fn assert_anydir_view_parity(m: &MAnydir0, v: &VAnydir0, nodes: &[id::N]) {
    assert_eq!(m.node_count(), v.node_count());
    assert_eq!(m.edge_count(), v.edge_count());
    assert_eq!(sorted_edges(m), sorted_edges(v));

    for &n in nodes {
        assert_eq!(m.degree(n), v.degree(n), "degree parity at {n:?}");
        assert_eq!(sorted_neighbors(m, n), sorted_neighbors(v, n), "neighbors parity at {n:?}");
    }

    for &a in nodes {
        for &b in nodes {
            assert_eq!(m.is_adjacent(*a, *b), v.is_adjacent(*a, *b), "is_adjacent parity ({a:?},{b:?})");
            assert_eq!(sorted_between(m, a, b), sorted_between(v, a, b), "edges_between parity ({a:?},{b:?})");
        }
    }
}

#[test]
fn vgraph_anydir_all_three_kinds_trait_view_parity() {
    let m: MAnydir0 = mgraph![
        N(0) >> N(1), n(1) >> n(0), n(0) ^ n(1),
        N(2) ^ N(3)
    ]
    .unwrap();
    let v: VAnydir0 = vgraph![
        N(0) >> N(1), n(1) >> n(0), n(0) ^ n(1),
        N(2) ^ N(3)
    ]
    .unwrap();

    assert_eq!(
        sorted_edges(&m),
        vec![
            (id::N(0), id::N(1), edge::anydir::TGT),
            (id::N(0), id::N(1), edge::anydir::SRC),
            (id::N(0), id::N(1), edge::anydir::UND),
            (id::N(2), id::N(3), edge::anydir::UND),
        ]
    );

    assert_anydir_view_parity(&m, &v, &[id::N(0), id::N(1), id::N(2), id::N(3)]);
}

#[test]
fn vgraph_anydir_modify_parity_add_and_remove_per_slot_kind() {
    let m0: MAnydir0 = mgraph![N(0), N(1)].unwrap();
    let v0: VAnydir0 = vgraph![N(0), N(1)].unwrap();
    let nodes = [id::N(0), id::N(1)];
    assert_anydir_view_parity(&m0, &v0, &nodes);

    // add Und
    let mut m = m0;
    m.modify(modify![x(0) ^ x(1)]).unwrap();
    let (v, _) = v0.modify(modify![x(0) ^ x(1)]).unwrap();
    assert_anydir_view_parity(&m, &v, &nodes);

    // add Src (0 -> 1)
    m.modify(modify![x(0) >> x(1)]).unwrap();
    let (v, _) = v.modify(modify![x(0) >> x(1)]).unwrap();
    assert_anydir_view_parity(&m, &v, &nodes);

    // add reverse (1 -> 0, via <<)
    m.modify(modify![x(0) << x(1)]).unwrap();
    let (v, _) = v.modify(modify![x(0) << x(1)]).unwrap();
    assert_anydir_view_parity(&m, &v, &nodes);
    assert_eq!(m.edge_count(), 3);
    assert_eq!(v.edge_count(), 3);

    // remove Und
    m.modify(modify![x(0) & !e() ^ x(1)]).unwrap();
    let (v, _) = v.modify(modify![x(0) & !e() ^ x(1)]).unwrap();
    assert_anydir_view_parity(&m, &v, &nodes);

    // remove Src (0 -> 1)
    m.modify(modify![x(0) & !e() >> x(1)]).unwrap();
    let (v, _) = v.modify(modify![x(0) & !e() >> x(1)]).unwrap();
    assert_anydir_view_parity(&m, &v, &nodes);

    // remove remaining reverse edge (1 -> 0, via <<)
    m.modify(modify![x(0) & !e() << x(1)]).unwrap();
    let (v, _) = v.modify(modify![x(0) & !e() << x(1)]).unwrap();
    assert_anydir_view_parity(&m, &v, &nodes);
    assert_eq!(m.edge_count(), 0);
    assert_eq!(v.edge_count(), 0);
}
