use grw::modify::error::{Apply, Modify, apply};

fn view<ER: grw::graph::Edge, G: grw::graph::Graph<(), ER>>(g: &G) -> Vec<(u32, Vec<u32>)> {
    let mut v: Vec<(u32, Vec<u32>)> = g.iter_node_ids().map(|n| {
        let mut adj: Vec<u32> = g.neighbor_ids(n).unwrap().map(|m| *m as u32).collect();
        adj.sort_unstable();
        (*n as u32, adj)
    }).collect();
    v.sort_unstable(); v
}

fn edge_err<T>(r: Result<T, Modify>) -> apply::Edge {
    match r {
        Err(Modify::Apply(Apply::Edge(e))) => e,
        Err(other) => panic!("expected Apply::Edge, got {other:?}"),
        Ok(_) => panic!("expected Err, got Ok"),
    }
}

fn edge_vals(g: &grw::graph::MUndirE<u32>) -> Vec<u32> {
    let mut v: Vec<u32> = g.edge_iter().map(|(_, val)| *val).collect();
    v.sort_unstable();
    v
}

fn valued_pair() -> grw::graph::MUndirE<u32> {
    let mut g = grw::graph::MUndirE::<u32>::default();
    grw::modify!(g, [N(0) & E().val(1u32) ^ N(1), N(2)]).unwrap();
    g
}

#[test]
fn mgraph_err_leaves_graph_untouched_edge_notfound() {
    let mut g: grw::graph::MUndir0 = grw::mgraph![N(0) ^ N(1), n(1) ^ N(2)].unwrap();
    let before = view(&g);
    let r = grw::modify!(g, [N(9) ^ x(0), x(0) & !e() ^ x(2)]);
    assert!(r.is_err());
    assert_eq!(view(&g), before, "partial apply: hole A");
}

#[test]
fn mgraph_err_leaves_graph_untouched_duplicate() {
    let mut g: grw::graph::MUndir0 = grw::mgraph![N(0) ^ N(1)].unwrap();
    let before = view(&g);
    let r = grw::modify!(g, [N(9) ^ x(0), x(0) ^ x(1)]);
    assert!(r.is_err());
    assert_eq!(view(&g), before, "partial apply: hole B");
}

#[test]
fn mgraph_err_does_not_leak_id_space() {
    let mut g: grw::graph::MUndir0 = grw::mgraph![N(0) ^ N(1), n(1) ^ N(2)].unwrap();
    let r = grw::modify!(g, [N(9) ^ x(0), x(0) & !e() ^ x(2)]);
    assert!(r.is_err());
    let m = grw::modify!(g, [N(7)]).unwrap();
    assert_eq!(*m.new_node_ids[&grw::graph::dsl::LocalId(7)] as u32, 3,
        "hole C: failed modify consumed ids; expected next fresh id 3");
}

#[test]
fn mgraph_validation_error_restores_anon_ids() {
    let mut g: grw::graph::MUndir0 = grw::mgraph![N(0) ^ N(1)].unwrap();
    let before = view(&g);
    let r = grw::modify!(g, [N_(), N_(), X(99)]);
    assert!(r.is_err());
    assert_eq!(view(&g), before, "validation error mutated the graph");
    let m = grw::modify!(g, [N(7)]).unwrap();
    assert_eq!(*m.new_node_ids[&grw::graph::dsl::LocalId(7)] as u32, 2,
        "hole C: anonymous ids popped during flattening were not returned to the free list");
}

#[test]
fn overlay_remove_then_readd_same_edge_in_one_batch() {
    let mut g: grw::graph::MUndir0 = grw::mgraph![N(0) ^ N(1)].unwrap();
    let before = view(&g);

    grw::modify!(g, [x(0) & !e() ^ x(1), x(0) ^ x(1)]).unwrap();
    assert_eq!(view(&g), before, "remove-then-re-add must leave the edge in place");
    assert_eq!(g.edge_count(), 1);

    grw::modify!(g, [x(0) ^ x(1), x(0) & !e() ^ x(1)]).unwrap();
    assert_eq!(view(&g), before,
        "phase order is fixed (removes before adds), so DSL order must not change the verdict");
    assert_eq!(g.edge_count(), 1);
}

#[test]
fn overlay_swap_cannot_see_edge_added_in_same_batch() {
    let mut g = valued_pair();
    let before = view(&g);
    let vals_before = edge_vals(&g);

    let e = edge_err(grw::modify!(g, [
        x(0) & E().val(5u32) ^ x(2),
        x(0) & e().val(9u32) ^ x(2)
    ]));
    assert!(matches!(e, apply::Edge::NotFound(s, t) if *s as u32 == 0 && *t as u32 == 2),
        "swaps run before adds, so an edge added in the same batch is not visible: {e:?}");
    assert_eq!(view(&g), before);
    assert_eq!(edge_vals(&g), vals_before);
}

#[test]
fn overlay_double_remove_of_one_edge_is_notfound() {
    let mut g: grw::graph::MUndir0 = grw::mgraph![N(0) ^ N(1), n(1) ^ N(2)].unwrap();
    let before = view(&g);

    let e = edge_err(grw::modify!(g, [x(0) & !e() ^ x(1), x(0) & !e() ^ x(1)]));
    assert!(matches!(e, apply::Edge::NotFound(s, t) if *s as u32 == 0 && *t as u32 == 1),
        "second removal of a singly-present edge must be NotFound: {e:?}");
    assert_eq!(view(&g), before);
    assert_eq!(g.edge_count(), 2);
}

#[test]
fn overlay_add_on_batch_removed_node_sees_pre_batch_edges() {
    let mut g: grw::graph::MUndir0 = grw::mgraph![N(0) ^ N(1), n(1) ^ N(2)].unwrap();
    let before = view(&g);
    let e = edge_err(grw::modify!(g, [!X(1), x(0) ^ x(1)]));
    assert!(matches!(e, apply::Edge::Duplicate(s, t) if *s as u32 == 0 && *t as u32 == 1),
        "node removal is the last mutation step, so the edge is still present for the add: {e:?}");
    assert_eq!(view(&g), before);

    let mut g2: grw::graph::MUndir0 = grw::mgraph![N(0) ^ N(1), n(1) ^ N(2), N(3)].unwrap();
    grw::modify!(g2, [!X(1), x(1) ^ x(3)]).unwrap();
    assert_eq!(view(&g2), vec![(0, vec![]), (2, vec![]), (3, vec![])],
        "a non-duplicate add onto a batch-removed node is accepted, then cascaded away");
    assert_eq!(g2.edge_count(), 0);
}

#[test]
fn overlay_swap_branch_accepts_and_rejects() {
    let mut g = valued_pair();
    grw::modify!(g, [x(0) & e().val(9u32) ^ x(1)]).unwrap();
    assert_eq!(edge_vals(&g), vec![9], "swap of an existing edge must land");

    let before = view(&g);
    let e = edge_err(grw::modify!(g, [x(0) & e().val(7u32) ^ x(2)]));
    assert!(matches!(e, apply::Edge::NotFound(s, t) if *s as u32 == 0 && *t as u32 == 2),
        "swap of an absent edge must be rejected by validation: {e:?}");
    assert_eq!(view(&g), before);
    assert_eq!(edge_vals(&g), vec![9], "rejected batch must not have swapped anything");
}

#[test]
fn edge_notfound_payload_keeps_dsl_endpoint_order() {
    let mut g: grw::graph::MUndir0 = grw::mgraph![N(0) ^ N(1), n(1) ^ N(2)].unwrap();
    let e = edge_err(grw::modify!(g, [x(2) & !e() ^ x(0)]));
    assert!(matches!(e, apply::Edge::NotFound(s, t) if *s as u32 == 2 && *t as u32 == 0),
        "payload carries the DSL (source, target), not the canonical (n1, n2) = (0, 2): {e:?}");
}

#[test]
fn edge_duplicate_payload_keeps_dsl_endpoint_order() {
    let mut g: grw::graph::MUndir0 = grw::mgraph![N(0) ^ N(1)].unwrap();
    let e = edge_err(grw::modify!(g, [x(1) ^ x(0)]));
    assert!(matches!(e, apply::Edge::Duplicate(s, t) if *s as u32 == 1 && *t as u32 == 0),
        "payload carries the DSL (source, target), not the canonical (n1, n2) = (0, 1): {e:?}");
}

#[test]
fn dir_edge_notfound_payload_keeps_dsl_endpoint_order() {
    let mut g: grw::graph::MDir0 = grw::mgraph![N(0) >> N(1), n(1) >> N(2)].unwrap();
    let e = edge_err(grw::modify!(g, [x(2) & !e() >> x(0)]));
    assert!(matches!(e, apply::Edge::NotFound(s, t) if *s as u32 == 2 && *t as u32 == 0),
        "payload carries the DSL (source, target): {e:?}");
}

#[test]
fn dir_edge_duplicate_payload_keeps_dsl_endpoint_order() {
    let mut g: grw::graph::MDir0 = grw::mgraph![N(0), N(1)].unwrap();
    grw::modify!(g, [x(1) >> x(0)]).unwrap();
    let e = edge_err(grw::modify!(g, [x(1) >> x(0)]));
    assert!(matches!(e, apply::Edge::Duplicate(s, t) if *s as u32 == 1 && *t as u32 == 0),
        "payload carries the DSL (source, target), not the canonical (n1, n2) = (0, 1): {e:?}");
}

#[test]
fn anydir_edge_notfound_payload_keeps_dsl_endpoint_order() {
    let mut g: grw::graph::MAnydir0 = grw::mgraph![N(0) ^ N(1), n(1) ^ N(2)].unwrap();
    let e = edge_err(grw::modify!(g, [x(2) & !e() ^ x(0)]));
    assert!(matches!(e, apply::Edge::NotFound(s, t) if *s as u32 == 2 && *t as u32 == 0),
        "payload carries the DSL (source, target) for the Und slot: {e:?}");
}

#[test]
fn anydir_edge_duplicate_payload_keeps_dsl_endpoint_order() {
    let mut g: grw::graph::MAnydir0 = grw::mgraph![N(0), N(1)].unwrap();
    grw::modify!(g, [x(1) >> x(0)]).unwrap();
    let e = edge_err(grw::modify!(g, [x(1) >> x(0)]));
    assert!(matches!(e, apply::Edge::Duplicate(s, t) if *s as u32 == 1 && *t as u32 == 0),
        "payload carries the DSL (source, target) for the Src slot: {e:?}");
}

#[test]
fn mgraph_err_leaves_graph_untouched_dir() {
    let mut g: grw::graph::MDir0 = grw::mgraph![N(0) >> N(1), n(1) >> N(2)].unwrap();
    let before = view(&g);
    let r = grw::modify!(g, [N(9) >> x(0), x(0) & !e() >> x(2)]);
    assert!(r.is_err());
    assert_eq!(view(&g), before, "partial apply must not mutate a directed graph on Err");
}

#[test]
fn vgraph_err_atomicity_parity() {
    let v: grw::graph::VUndir0 = grw::vgraph![N(0) ^ N(1)].unwrap();
    let r = v.modify(grw::modify![N(9) ^ x(0), x(0) ^ x(1)]);
    assert!(r.is_err());
    let m2 = v.modify(grw::modify![N(7)]).unwrap();
    assert_eq!(m2.1.new_node_ids.len(), 1);
    assert_eq!(*m2.1.new_node_ids[&grw::graph::dsl::LocalId(7)] as u32, 2,
        "rejected modify consumed a slot from the versioned free set");
}

#[test]
fn vgraph_validation_error_leaves_free_set_intact() {
    let v: grw::graph::VUndir0 = grw::vgraph![N(0) ^ N(1)].unwrap();
    let r = v.modify(grw::modify![N_(), N_(), X(99)]);
    assert!(r.is_err());
    let m2 = v.modify(grw::modify![N(7)]).unwrap();
    assert_eq!(*m2.1.new_node_ids[&grw::graph::dsl::LocalId(7)] as u32, 2,
        "rejected modify consumed a slot from the versioned free set");
}
