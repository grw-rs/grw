use crate::graph::*;
use crate::graph::dsl::LocalId;
use crate::graph;
use crate::id;

#[test]
fn t_graph_noval_noval() {
    use crate::edge::anydir::E::{D, U};

    let g = Anydir0::try_from((10, vec![U(0, 1), D(1, 2), U(2, 3)])).unwrap();

    assert_eq!(g.node_count(), 10);
    assert_eq!(g.edge_count(), 3);
}

#[test]
fn t_graph_noval_ev() {
    use crate::edge::dir::E::D;

    let g = DirE::<String>::try_from((
        10,
        vec![
            (D(0, 1), String::from("D 0-1 val")),
            (D(1, 2), String::from("D 1-2 val")),
            (D(3, 2), String::from("D 2-3 val")),
        ],
    ))
    .unwrap();

    assert_eq!(g.node_count(), 10);
    assert_eq!(g.edge_count(), 3);
}

#[test]
fn t_graph_nv_noval() {
    use crate::edge::undir::E::U;

    let g = UndirN::<String>::try_from((
        vec![
            (0, String::from("N(0) val")),
            (1, String::from("N(1) val")),
            (2, String::from("N(2) val")),
            (3, String::from("N(3) val")),
        ],
        vec![U(0, 1), U(1, 2), U(2, 3)],
    ))
    .unwrap();

    assert_eq!(g.node_count(), 4);
    assert_eq!(g.edge_count(), 3);
}

#[test]
fn t_graph_nv_ev() {
    use crate::edge::anydir::E::{D, U};

    let g = Anydir::<String, String>::try_from((
        vec![
            (0, String::from("N(0) val")),
            (1, String::from("N(1) val")),
            (2, String::from("N(2) val")),
            (3, String::from("N(3) val")),
            (100, String::from("N(100) val")),
        ],
        vec![
            (U(0, 1), String::from("U 0-1 val")),
            (U(1, 2), String::from("U 1-2 val")),
            (D(3, 2), String::from("D 2-3 val")),
        ],
    ))
    .unwrap();

    assert_eq!(g.node_count(), 5);
    assert_eq!(g.edge_count(), 3);
}

#[test]
fn t_graph_nv_ev_not_found() {
    use crate::edge::anydir::E::{D, U};
    use crate::{Id, id};

    let g = Anydir::<String, String>::try_from((
        vec![(0, String::from("N(0) val")), (1, String::from("N(1) val"))],
        vec![
            (U(0, 1), String::from("U 0-1 val")),
            (D(3, 2), String::from("D 2-3 val")),
        ],
    ));

    assert_eq!(
        g.err(),
        Some(error::Build::Edge(error::Edge::NodeNotFound(id::N(2 as Id))))
    );
}

#[test]
fn t_graph_nv_ev_duplicate() {
    use crate::edge::anydir::E::{D, U};
    use crate::{Id, id};

    let g = Anydir::<String, String>::try_from((
        vec![
            (0, String::from("N(0) val")),
            (1, String::from("N(1) val")),
            (2, String::from("N(2) val")),
            (3, String::from("N(3) val")),
            (0, String::from("another N(0) val")),
        ],
        vec![
            (U(0, 1), String::from("U 0-1 val")),
            (U(1, 2), String::from("U 1-2 val")),
            (D(3, 2), String::from("D 2-3 val")),
        ],
    ));

    assert_eq!(g.err(), Some(error::Build::Node(error::Node::Duplicate(id::N(0 as Id)))));
}

#[test]
fn t_graph_edges_only() {
    use crate::edge::undir::E::U;

    let _ = Undir0::try_from(vec![U(0, 1), U(4, 8)]).unwrap();
}

#[test]
fn t_graph_count_and_edges() {
    use crate::edge::dir::E::D;

    let _ = Dir0::try_from((10, vec![D(0, 1), D(4, 8)])).unwrap();
}

#[test]
fn t_graph_valued_nodes_and_edges() {
    use crate::edge::anydir::E::U;

    let _ = Anydir::<String, String>::try_from((
        vec![(0, String::from("a")), (1, String::from("b"))],
        vec![(U(0, 1), String::from("edge"))],
    ))
    .unwrap();
}

#[test]
fn t_edge_id_indexing() {
    use crate::edge::undir::E::U;
    use crate::id;

    let g = Undir0::try_from(vec![U(0, 1), U(1, 2), U(2, 3)]).unwrap();

    assert_eq!(g.edge_count(), 3);
    let rec0 = g.edges.get_by_id(id::E(0)).unwrap();
    assert_eq!(rec0.n1, id::N(0));
    assert_eq!(rec0.n2, id::N(1));
}

#[test]
fn t_has_get_edge() {
    use crate::edge::dir::E::D;

    let g = Dir0::try_from(vec![D(0, 1), D(1, 2)]).unwrap();

    assert!(g.has(D(0, 1)));
    assert!(g.has(D(1, 2)));
    assert!(!g.has(D(2, 0)));
}

#[test]
fn t_adjacency_multi_edge_dir() {
    use crate::edge::dir::E::D;

    let g = Dir0::try_from(vec![D(0, 1), D(1, 0)]).unwrap();

    assert_eq!(g.edge_count(), 2);
    assert!(g.is_adjacent(0, 1));
    assert!(g.is_adjacent(1, 0));

    let edges: Vec<_> = g.edges_between(crate::id::N(0), crate::id::N(1)).collect();
    assert_eq!(edges.len(), 2);
}

#[test]
fn t_adjacency_multi_edge_anydir() {
    use crate::edge::anydir::E::{D, U};

    let g = Anydir0::try_from(vec![U(0, 1), D(0, 1)]).unwrap();

    assert_eq!(g.edge_count(), 2);
    assert!(g.is_adjacent(0, 1));

    let edges: Vec<_> = g.edges_between(crate::id::N(0), crate::id::N(1)).collect();
    assert_eq!(edges.len(), 2);
}

#[test]
fn t_self_loop_undir() {
    use crate::edge::undir::E::U;

    let g = Undir0::try_from((3, vec![U(0, 1), U(1, 1)])).unwrap();

    assert_eq!(g.edge_count(), 2);
    assert!(g.is_adjacent(1, 1));
    assert!(g.is_adjacent(0, 1));

    let self_edges: Vec<_> = g.edges_between(crate::id::N(1), crate::id::N(1)).collect();
    assert_eq!(self_edges.len(), 1);
}

#[test]
fn t_self_loop_dir() {
    use crate::edge::dir::E::D;

    let g = Dir0::try_from((3, vec![D(0, 1), D(1, 1)])).unwrap();

    assert_eq!(g.edge_count(), 2);
    assert!(g.is_adjacent(1, 1));

    let self_edges: Vec<_> = g.edges_between(crate::id::N(1), crate::id::N(1)).collect();
    assert_eq!(self_edges.len(), 1);
}

#[test]
fn t_to_vecs_roundtrip() {
    use crate::edge::undir::E::U;

    let g = Undir0::try_from(vec![U(0, 1), U(1, 2)]).unwrap();
    let (nodes, edges) = g.to_vecs();

    assert_eq!(nodes.len(), 3);
    assert_eq!(edges.len(), 2);
}

#[test]
fn t_rel_api() {
    use crate::edge::dir::E::D;
    use crate::id;

    let g = Dir::<(), u32>::try_from((
        3,
        vec![(D(0, 1), 10u32), (D(1, 0), 20u32)],
    ))
    .unwrap();

    let rel = g.rel([id::N(0), id::N(1)]);
    assert_eq!(rel.out, Some(&10));
    assert_eq!(rel.inc, Some(&20));
}

// ---- TryFrom edges-only ----

// Undir

#[test]
fn test_from_undir_edges() {
    use edge::undir::E::U;
    let _ = Undir0::try_from(vec![U(0, 1), U(4, 8)]).unwrap();
}

#[test]
fn test_from_undir_valued_edges() {
    use edge::undir::E::U;
    let _ = UndirE::<String>::try_from(vec![
        (U(0, 1), String::from("01")),
        (U(4, 8), String::from("48")),
    ])
    .unwrap();
}

// Dir

#[test]
fn test_from_dir_edges() {
    use edge::dir::E::D;
    let _ = Dir0::try_from(vec![D(0, 1), D(4, 8)]).unwrap();
}

#[test]
fn test_from_dir_valued_edges() {
    use edge::dir::E::D;
    let _ = DirE::<String>::try_from(vec![
        (D(0, 1), String::from("01")),
        (D(4, 8), String::from("48")),
    ])
    .unwrap();
}

// Anydir

#[test]
fn test_from_anydir_edges() {
    use edge::anydir::E::{D, U};
    let _ = Anydir0::try_from(vec![U(0, 1), D(4, 8)]).unwrap();
}

#[test]
fn test_from_anydir_valued_edges() {
    use edge::anydir::E::{D, U};
    let _ = AnydirE::<String>::try_from(vec![
        (U(0, 1), String::from("01")),
        (D(4, 8), String::from("48")),
    ])
    .unwrap();
}

// ---- TryFrom with node count ----

// Undir

#[test]
fn test_from_count_undir_edges() {
    use edge::undir::E::U;
    let _ = Undir0::try_from((10 as Id, vec![U(0, 1), U(4, 8)])).unwrap();
}

#[test]
fn test_from_count_undir_valued_edges() {
    use edge::undir::E::U;
    let _ = UndirE::<String>::try_from((
        10 as Id,
        vec![(U(0, 1), String::from("01")), (U(4, 8), String::from("48"))],
    ))
    .unwrap();
}

// Dir

#[test]
fn test_from_count_dir_edges() {
    use edge::dir::E::D;
    let _ = Dir0::try_from((10 as Id, vec![D(0, 1), D(4, 8)])).unwrap();
}

#[test]
fn test_from_count_dir_valued_edges() {
    use edge::dir::E::D;
    let _ = DirE::<String>::try_from((
        10 as Id,
        vec![(D(0, 1), String::from("01")), (D(4, 8), String::from("48"))],
    ))
    .unwrap();
}

// Anydir

#[test]
fn test_from_count_anydir_edges() {
    use edge::anydir::E::{D, U};
    let _ = Anydir0::try_from((10 as Id, vec![U(0, 1), D(4, 8)])).unwrap();
}

#[test]
fn test_from_count_anydir_valued_edges() {
    use edge::anydir::E::{D, U};
    let _ = AnydirE::<String>::try_from((
        10 as Id,
        vec![(U(0, 1), String::from("01")), (D(4, 8), String::from("48"))],
    ))
    .unwrap();
}

// ---- TryFrom with node ids ----

// Undir

#[test]
fn test_from_ids_undir_edges() {
    use edge::undir::E::U;
    let _ = Undir0::try_from((vec![0, 1, 4, 8], vec![U(0, 1), U(4, 8)])).unwrap();
}

#[test]
fn test_from_ids_undir_valued_edges() {
    use edge::undir::E::U;
    let _ = UndirE::<String>::try_from((
        vec![0, 1, 4, 8],
        vec![(U(0, 1), String::from("01")), (U(4, 8), String::from("48"))],
    ))
    .unwrap();
}

// Dir

#[test]
fn test_from_ids_dir_edges() {
    use edge::dir::E::D;
    let _ = Dir0::try_from((vec![0, 1, 4, 8], vec![D(0, 1), D(4, 8)])).unwrap();
}

#[test]
fn test_from_ids_dir_valued_edges() {
    use edge::dir::E::D;
    let _ = DirE::<String>::try_from((
        vec![0, 1, 4, 8],
        vec![(D(0, 1), String::from("01")), (D(4, 8), String::from("48"))],
    ))
    .unwrap();
}

// Anydir

#[test]
fn test_from_ids_anydir_edges() {
    use edge::anydir::E::{D, U};
    let _ = Anydir0::try_from((vec![0, 1, 4, 8], vec![U(0, 1), D(4, 8)])).unwrap();
}

#[test]
fn test_from_ids_anydir_valued_edges() {
    use edge::anydir::E::{D, U};
    let _ = AnydirE::<String>::try_from((
        vec![0, 1, 4, 8],
        vec![(U(0, 1), String::from("01")), (D(4, 8), String::from("48"))],
    ))
    .unwrap();
}

// ---- TryFrom with valued nodes ----

// Undir

#[test]
fn test_from_valued_nodes_undir_edges() {
    use edge::undir::E::U;
    let _ = UndirN::<String>::try_from((
        vec![
            (0, String::from("0")),
            (1, String::from("1")),
            (4, String::from("4")),
            (8, String::from("8")),
        ],
        vec![U(0, 1), U(4, 8)],
    ))
    .unwrap();
}

#[test]
fn test_from_valued_nodes_undir_valued_edges() {
    use edge::undir::E::U;
    let _ = Undir::<String, String>::try_from((
        vec![
            (0, String::from("0")),
            (1, String::from("1")),
            (4, String::from("4")),
            (8, String::from("8")),
        ],
        vec![(U(0, 1), String::from("01")), (U(4, 8), String::from("48"))],
    ))
    .unwrap();
}

// Dir

#[test]
fn test_from_valued_nodes_dir_edges() {
    use edge::dir::E::D;
    let _ = DirN::<String>::try_from((
        vec![
            (0, String::from("0")),
            (1, String::from("1")),
            (4, String::from("4")),
            (8, String::from("8")),
        ],
        vec![D(0, 1), D(4, 8)],
    ))
    .unwrap();
}

#[test]
fn test_from_valued_nodes_dir_valued_edges() {
    use edge::dir::E::D;
    let _ = Dir::<String, String>::try_from((
        vec![
            (0, String::from("0")),
            (1, String::from("1")),
            (4, String::from("4")),
            (8, String::from("8")),
        ],
        vec![(D(0, 1), String::from("01")), (D(4, 8), String::from("48"))],
    ))
    .unwrap();
}

// Anydir

#[test]
fn test_from_valued_nodes_anydir_edges() {
    use edge::anydir::E::{D, U};
    let _ = AnydirN::<String>::try_from((
        vec![
            (0, String::from("0")),
            (1, String::from("1")),
            (4, String::from("4")),
            (8, String::from("8")),
        ],
        vec![U(0, 1), D(4, 8)],
    ))
    .unwrap();
}

#[test]
fn test_from_valued_nodes_anydir_valued_edges() {
    use edge::anydir::E::{D, U};
    let _ = Anydir::<String, String>::try_from((
        vec![
            (0, String::from("0")),
            (1, String::from("1")),
            (4, String::from("4")),
            (8, String::from("8")),
        ],
        vec![(U(0, 1), String::from("01")), (D(4, 8), String::from("48"))],
    ))
    .unwrap();
}

// ---- Directed D(0,1) and D(1,0) are distinct (opposite directions) ----

#[test]
fn test_dir_opposite_directions_ok() {
    use edge::dir::E::D;
    let _ = Dir0::try_from(vec![D(0, 1), D(1, 0)]).unwrap();
}

// ---- Undirected U(0,1) and U(1,0) are duplicates (same edge) ----

#[test]
fn test_undir_reversed_duplicate() {
    use super::error;
    use edge::undir::E::U;
    let g = Undir0::try_from(vec![U(0, 1), U(1, 0)]);
    assert_eq!(
        g.err(),
        Some(error::Edge::Duplicate(
            NR([id::N(0), id::N(1)]),
            edge::undir::UND,
        ))
    );
}

// ---- Error cases ----

#[test]
fn test_err_node_not_found_undir() {
    use super::error;
    use edge::undir::E::U;
    let g = Undir0::try_from((vec![0, 1], vec![U(0, 1), U(2, 3)]));
    assert_eq!(
        g.err(),
        Some(error::Build::Edge(error::Edge::NodeNotFound(id::N(2 as Id))))
    );
}

#[test]
fn test_err_node_not_found_dir() {
    use super::error;
    use edge::dir::E::D;
    let g = Dir0::try_from((vec![0, 1], vec![D(0, 1), D(2, 3)]));
    assert_eq!(
        g.err(),
        Some(error::Build::Edge(error::Edge::NodeNotFound(id::N(2 as Id))))
    );
}

#[test]
fn test_err_node_not_found_anydir() {
    use super::error;
    use edge::anydir::E::{D, U};
    let g = Anydir0::try_from((vec![0, 1], vec![U(0, 1), D(2, 3)]));
    assert_eq!(
        g.err(),
        Some(error::Build::Edge(error::Edge::NodeNotFound(id::N(2 as Id))))
    );
}

#[test]
fn test_err_node_duplicate_undir() {
    use super::error;
    use edge::undir::E::U;
    let g = UndirN::<String>::try_from((
        vec![
            (0, String::from("a")),
            (1, String::from("b")),
            (0, String::from("c")),
        ],
        vec![U(0, 1)],
    ));
    assert_eq!(g.err(), Some(error::Build::Node(error::Node::Duplicate(id::N(0 as Id)))));
}

#[test]
fn test_err_node_duplicate_dir() {
    use super::error;
    use edge::dir::E::D;
    let g = DirN::<String>::try_from((
        vec![
            (0, String::from("a")),
            (1, String::from("b")),
            (0, String::from("c")),
        ],
        vec![D(0, 1)],
    ));
    assert_eq!(g.err(), Some(error::Build::Node(error::Node::Duplicate(id::N(0 as Id)))));
}

#[test]
fn test_err_node_duplicate_anydir() {
    use super::error;
    use edge::anydir::E::U;
    let g = AnydirN::<String>::try_from((
        vec![
            (0, String::from("a")),
            (1, String::from("b")),
            (0, String::from("c")),
        ],
        vec![U(0, 1)],
    ));
    assert_eq!(g.err(), Some(error::Build::Node(error::Node::Duplicate(id::N(0 as Id)))));
}

// ---- graph! macro ----

#[test]
fn test_macro_undir_chain() {
    let g: Graph<(), edge::Undir<()>> = graph![N(0) ^ N(1) ^ N(2)].unwrap();
    assert_eq!(g.node_count(), 3);
    assert_eq!(g.edge_count(), 2);
}

#[test]
fn test_macro_undir_with_node_vals() {
    let g: Graph<&str, edge::Undir<()>> = graph![N(0).val("a") ^ N(1).val("b")].unwrap();
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
}

#[test]
fn test_macro_undir_with_edge_vals() {
    let g: Graph<(), edge::Undir<u32>> = graph![N(0) & E().val(42u32) ^ N(1)].unwrap();
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
}

#[test]
fn test_macro_dir_chain() {
    let g: Graph<(), edge::Dir<()>> = graph![N(0) >> N(1) >> N(2)].unwrap();
    assert_eq!(g.node_count(), 3);
    assert_eq!(g.edge_count(), 2);
}

#[test]
fn test_macro_back_reference() {
    let g: Graph<(), edge::Undir<()>> = graph![N(0) ^ N(1) ^ N(2) ^ n(0)].unwrap();
    assert_eq!(g.node_count(), 3);
    assert_eq!(g.edge_count(), 3);
}

#[test]
fn test_macro_anonymous_nodes() {
    let g: Graph<(), edge::Undir<()>> = graph![N_() ^ N_()].unwrap();
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
}

#[test]
fn test_macro_turbofish_syntax() {
    let g = graph![<(), crate::graph::edge::Undir<()>>; N(0) ^ N(1)].unwrap();
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
}

#[test]
fn test_macro_multi_fragment() {
    let g: Graph<(), edge::Undir<()>> = graph![N(0) ^ N(1), N(2) ^ N(3), n(0) ^ n(2)].unwrap();
    assert_eq!(g.node_count(), 4);
    assert_eq!(g.edge_count(), 3);
}

#[test]
fn test_macro_single_node() {
    let g: Graph<(), edge::Undir<()>> = graph![N(0)].unwrap();
    assert_eq!(g.node_count(), 1);
    assert_eq!(g.edge_count(), 0);
}

#[test]
fn test_macro_err_duplicate_local_id() {
    use super::error;
    let g = graph![<(), crate::graph::edge::Undir<()>>; N(0) ^ N(0)];
    assert!(matches!(g, Err(error::Build::Node(error::Node::DuplicateLocalId(0)))));
}

#[test]
fn test_macro_err_undefined_ref() {
    use super::error;
    let g = graph![<(), crate::graph::edge::Undir<()>>; N(0) ^ n(99)];
    assert!(matches!(g, Err(error::Build::Node(error::Node::UndefinedRef(99)))));
}

#[test]
fn test_macro_err_duplicate_edge() {
    use super::error;
    let g = graph![<(), crate::graph::edge::Undir<()>>; N(0) ^ N(1), n(0) ^ n(1)];
    assert!(matches!(g, Err(error::Build::Edge(..))));
}

#[test]
fn test_macro_anon_mixed_with_explicit() {
    let g: Graph<(), edge::Undir<()>> = graph![N(5) ^ N_() ^ N_()].unwrap();
    assert_eq!(g.node_count(), 3);
    assert_eq!(g.edge_count(), 2);
}

#[test]
fn test_macro_dir_with_edge_val() {
    let g: Graph<(), edge::Dir<i32>> = graph![N(0) & E().val(10) >> N(1)].unwrap();
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
}

#[test]
fn test_macro_node_and_edge_vals() {
    let g: Graph<String, edge::Undir<String>> =
        graph![N(0).val(String::from("x")) & E().val(String::from("e")) ^ N(1).val(String::from("y"))].unwrap();
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
}

// ---- Watcher tests ----

use crate::graph::watcher::{Watcher, Control, BanVerdict, Silent};

struct Recorder {
    binds: Vec<(usize, LocalId, id::N)>,
    unbinds: Vec<(usize, LocalId)>,
    matches: Vec<Vec<(LocalId, id::N)>>,
    edge_tests: Vec<(usize, id::N, id::N, bool)>,
    stop_after: Option<usize>,
}

impl Recorder {
    fn new() -> Self {
        Recorder {
            binds: Vec::new(),
            unbinds: Vec::new(),
            matches: Vec::new(),
            edge_tests: Vec::new(),
            stop_after: None,
        }
    }
}

impl Watcher<(), edge::Undir<()>> for Recorder {
    const ACTIVE: bool = true;

    fn on_bind(&mut self, step: usize, pn: LocalId, gn: id::N) -> Control {
        self.binds.push((step, pn, gn));
        Control::Continue
    }
    fn on_unbind(&mut self, step: usize, pn: LocalId) {
        self.unbinds.push((step, pn));
    }
    fn on_edge_test(&mut self, pe: usize, src: id::N, tgt: id::N, exists: bool, _pp: bool, _neg: bool) -> Control {
        self.edge_tests.push((pe, src, tgt, exists));
        Control::Continue
    }
    fn on_ban_verdict(&mut self, _c: usize, _v: BanVerdict) -> Control { Control::Continue }
    fn on_match(&mut self, mapping: &[(LocalId, id::N)]) -> Control {
        self.matches.push(mapping.to_vec());
        if let Some(limit) = self.stop_after {
            if self.matches.len() >= limit {
                return Control::Stop;
            }
        }
        Control::Continue
    }

    fn on_node_added(&mut self, _id: id::N, _val: &()) {}
    fn on_node_removed(&mut self, _id: id::N) {}
    fn on_node_changed(&mut self, _id: id::N, _val: &()) {}
    fn on_edge_added(&mut self, _id: id::E, _n1: id::N, _n2: id::N, _s: &edge::undir::Slot, _v: &()) {}
    fn on_edge_removed(&mut self, _id: id::E) {}
    fn on_edge_changed(&mut self, _id: id::E, _val: &()) {}
}

#[test]
fn silent_matches_normal_count() {
    type ER = edge::Undir<()>;
    let g: Undir0 = Graph::try_from(
        vec![edge::undir::E::U(0, 1), edge::undir::E::U(1, 2), edge::undir::E::U(2, 0)]
    ).unwrap();

    use crate::search::{Search, Seq, RevCsr, dsl};
    let Search::Resolved(r) = crate::search![<(), ER>;
        get(Morphism::Mono) {
            dsl::N(0) ^ dsl::N(1),
        }
    ].expect("valid pattern")
    else { panic!("unexpected context nodes") };

    let query = r.query;
    let sg = g.index(RevCsr);

    let normal_count = Seq::search(&query, &sg).count();
    let watched_count = Seq::search_watched(&query, &sg, Silent).count();
    assert_eq!(normal_count, watched_count);
}

#[test]
fn watcher_receives_bind_unbind_events() {
    type ER = edge::Undir<()>;
    let g: Undir0 = Graph::try_from(
        vec![edge::undir::E::U(0, 1), edge::undir::E::U(1, 2)]
    ).unwrap();

    use crate::search::{Search, Seq, RevCsr, dsl};
    let Search::Resolved(r) = crate::search![<(), ER>;
        get(Morphism::Mono) {
            dsl::N(0) ^ dsl::N(1),
        }
    ].expect("valid pattern")
    else { panic!("unexpected context nodes") };

    let query = r.query;
    let sg = g.index(RevCsr);

    let recorder = Recorder::new();
    let watched = Seq::search_watched(&query, &sg, recorder);
    let matches: Vec<_> = watched.collect();
    assert_eq!(matches.len(), 4);
}

#[test]
fn watcher_receives_events_and_returns_via_into_watcher() {
    type ER = edge::Undir<()>;
    let g: Undir0 = Graph::try_from(
        vec![edge::undir::E::U(0, 1), edge::undir::E::U(1, 2)]
    ).unwrap();

    use crate::search::{Search, Seq, RevCsr, dsl};
    let Search::Resolved(r) = crate::search![<(), ER>;
        get(Morphism::Mono) {
            dsl::N(0) ^ dsl::N(1),
        }
    ].expect("valid pattern")
    else { panic!("unexpected context nodes") };

    let query = r.query;
    let sg = g.index(RevCsr);

    let recorder = Recorder::new();
    let mut watched = Seq::search_watched(&query, &sg, recorder);
    while watched.next().is_some() {}
    let recorder = watched.into_watcher();

    assert!(!recorder.binds.is_empty());
    assert!(!recorder.unbinds.is_empty());
    assert!(!recorder.matches.is_empty());
}

#[test]
fn stop_terminates_early() {
    type ER = edge::Undir<()>;
    let g: Undir0 = Graph::try_from(
        vec![
            edge::undir::E::U(0, 1),
            edge::undir::E::U(1, 2),
            edge::undir::E::U(2, 0),
            edge::undir::E::U(0, 3),
            edge::undir::E::U(1, 3),
            edge::undir::E::U(2, 3),
        ]
    ).unwrap();

    use crate::search::{Search, Seq, RevCsr, dsl};
    let Search::Resolved(r) = crate::search![<(), ER>;
        get(Morphism::Mono) {
            dsl::N(0) ^ dsl::N(1),
        }
    ].expect("valid pattern")
    else { panic!("unexpected context nodes") };

    let query = r.query;
    let sg = g.index(RevCsr);

    let normal_count = Seq::search(&query, &sg).count();
    assert!(normal_count > 2);

    let mut recorder = Recorder::new();
    recorder.stop_after = Some(2);
    let mut watched = Seq::search_watched(&query, &sg, recorder);
    let mut count = 0;
    while watched.next().is_some() {
        count += 1;
    }
    let recorder = watched.into_watcher();
    assert_eq!(recorder.matches.len(), 2);
    assert!(count <= 2);
}

struct MutRecorder {
    nodes_added: Vec<id::N>,
    nodes_removed: Vec<id::N>,
    edges_added: Vec<id::E>,
    edges_removed: Vec<id::E>,
}

impl MutRecorder {
    fn new() -> Self {
        MutRecorder {
            nodes_added: Vec::new(),
            nodes_removed: Vec::new(),
            edges_added: Vec::new(),
            edges_removed: Vec::new(),
        }
    }
}

impl Watcher<(), edge::Undir<()>> for MutRecorder {
    const ACTIVE: bool = true;

    fn on_bind(&mut self, _: usize, _: LocalId, _: id::N) -> Control { Control::Continue }
    fn on_unbind(&mut self, _: usize, _: LocalId) {}
    fn on_edge_test(&mut self, _: usize, _: id::N, _: id::N, _: bool, _: bool, _: bool) -> Control { Control::Continue }
    fn on_ban_verdict(&mut self, _: usize, _: BanVerdict) -> Control { Control::Continue }
    fn on_match(&mut self, _: &[(LocalId, id::N)]) -> Control { Control::Continue }

    fn on_node_added(&mut self, nid: id::N, _val: &()) { self.nodes_added.push(nid); }
    fn on_node_removed(&mut self, nid: id::N) { self.nodes_removed.push(nid); }
    fn on_node_changed(&mut self, _nid: id::N, _val: &()) {}
    fn on_edge_added(&mut self, eid: id::E, _: id::N, _: id::N, _: &edge::undir::Slot, _: &()) { self.edges_added.push(eid); }
    fn on_edge_removed(&mut self, eid: id::E) { self.edges_removed.push(eid); }
    fn on_edge_changed(&mut self, _: id::E, _: &()) {}
}

#[test]
fn watcher_fires_on_modify() {
    type ER = edge::Undir<()>;
    let mut g: crate::Graph<(), ER> = Graph::try_from(
        vec![edge::undir::E::U(0, 1)]
    ).unwrap();

    let mut rec = MutRecorder::new();
    {
        let mut wg = g.watched(&mut rec);
        let ops: Vec<crate::modify::Node<(), ER>> = crate::modify![
            crate::modify::N(10u32) ^ crate::modify::N(11u32)
        ];
        wg.modify(ops).unwrap();
    }
    assert_eq!(rec.nodes_added.len(), 2);
    assert_eq!(rec.edges_added.len(), 1);
}

#[test]
fn watcher_fires_on_node_remove() {
    type ER = edge::Undir<()>;
    let mut g: crate::Graph<(), ER> = Graph::try_from(
        vec![edge::undir::E::U(0, 1)]
    ).unwrap();

    let mut rec = MutRecorder::new();
    {
        let mut wg = g.watched(&mut rec);
        let ops: Vec<crate::modify::Node<(), ER>> = crate::modify![
            !crate::modify::X(id::N(0))
        ];
        wg.modify(ops).unwrap();
    }
    assert_eq!(rec.nodes_removed.len(), 1);
    assert_eq!(rec.edges_removed.len(), 1);
}
