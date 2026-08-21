use grw::graph::{self, Graph, edge};
use grw::{id, modify};
use grw::modify::*;

#[test]
fn add_two_nodes() {
    let mut g: Graph<(), edge::Undir<()>> = Graph::default();
    let _ = modify!(g, [N(1) ^ N(2)]).unwrap();
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
}

#[test]
fn connect_to_existing() {
    let mut g: Graph<(), edge::Undir<()>> = Graph::default();
    let _ = modify!(g, [N(1) ^ N(2)]).unwrap();
    let _ = modify!(g, [X(0) ^ N(3)]).unwrap();
    assert_eq!(g.node_count(), 3);
    assert_eq!(g.edge_count(), 2);
}

#[test]
fn dir_path() {
    let mut g: Graph<(), edge::Dir<()>> = Graph::default();
    let _ = modify!(g, [N(1) >> (N(2) >> N(3))]).unwrap();
    assert_eq!(g.node_count(), 3);
    assert_eq!(g.edge_count(), 2);
}

#[test]
fn triangle() {
    let mut g: Graph<(), edge::Undir<()>> = Graph::default();
    let _ = modify!(g, [N(1) ^ (N(2) ^ (N(3) ^ n(1)))]).unwrap();
    assert_eq!(g.node_count(), 3);
    assert_eq!(g.edge_count(), 3);
}

#[test]
fn remove_node() {
    let mut g: Graph<(), edge::Undir<()>> = Graph::default();
    let _ = modify!(g, [N(1) ^ N(2) ^ N(3)]).unwrap();
    assert_eq!(g.node_count(), 3);
    let _ = modify!(g, [!X(1)]).unwrap();
    assert_eq!(g.node_count(), 2);
}

#[test]
fn remove_edge() {
    let mut g: Graph<(), edge::Dir<()>> = Graph::default();
    let _ = modify!(g, [N(1) >> N(2)]).unwrap();
    assert_eq!(g.edge_count(), 1);
    let _ = modify!(g, [X(0) & !e() >> x(1)]).unwrap();
    assert_eq!(g.edge_count(), 0);
}

#[test]
fn swap_node_val() {
    let mut g: Graph<&str, edge::Undir<()>> = Graph::default();
    let _ = modify!(g, [N(1).val("old")]).unwrap();
    assert_eq!(g.get(0), Some(&"old"));
    let _ = modify!(g, [X(0).val("new")]).unwrap();
    assert_eq!(g.get(0), Some(&"new"));
}

#[test]
fn swap_edge_val() {
    let mut g: Graph<(), edge::Undir<u32>> = Graph::default();
    let _ = modify!(g, [N(1) & E().val(100u32) ^ N(2)]).unwrap();
    assert_eq!(g.edge_count(), 1);
    let _ = modify!(g, [X(0) & e().val(200u32) ^ X(1)]).unwrap();
    assert_eq!(g.edge_count(), 1);
}
