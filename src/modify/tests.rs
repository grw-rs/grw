use super::node;
use super::*;
use crate::id;
use crate::modify;

fn undir0() -> crate::graph::Undir0 {
    crate::graph::Undir0::default()
}

fn dir0() -> crate::graph::Dir0 {
    crate::graph::Dir0::default()
}

fn anydir0() -> crate::graph::Anydir0 {
    crate::graph::Anydir0::default()
}

fn undir0_one() -> crate::graph::Undir0 {
    vec![crate::graph::edge::undir::E::U(0, 1)].try_into().unwrap()
}

fn dir0_one() -> crate::graph::Dir0 {
    vec![crate::graph::edge::dir::E::D(0, 1)].try_into().unwrap()
}

fn anydir0_one() -> crate::graph::Anydir0 {
    vec![crate::graph::edge::anydir::E::U(0, 1)].try_into().unwrap()
}

fn undir_n_empty<T: Sync>() -> crate::graph::UndirN<T> {
    (
        vec![] as Vec<(crate::Id, T)>,
        vec![] as Vec<crate::graph::edge::undir::E<crate::Id>>,
    )
        .try_into()
        .unwrap()
}

fn dir_n_empty<T: Sync>() -> crate::graph::DirN<T> {
    (
        vec![] as Vec<(crate::Id, T)>,
        vec![] as Vec<crate::graph::edge::dir::E<crate::Id>>,
    )
        .try_into()
        .unwrap()
}

fn undir_e_empty<T: Sync>() -> crate::graph::UndirE<T> {
    (vec![] as Vec<(crate::graph::edge::undir::E<crate::Id>, T)>)
        .try_into()
        .unwrap()
}

fn dir_e_empty<T: Sync>() -> crate::graph::DirE<T> {
    (vec![] as Vec<(crate::graph::edge::dir::E<crate::Id>, T)>)
        .try_into()
        .unwrap()
}

fn anydir_e_empty<T: Sync>() -> crate::graph::AnydirE<T> {
    (vec![] as Vec<(crate::graph::edge::anydir::E<crate::Id>, T)>)
        .try_into()
        .unwrap()
}

#[test]
fn local_id_equality() {
    assert_eq!(LocalId(1), LocalId(1));
    assert_ne!(LocalId(1), LocalId(2));
}

#[test]
fn node_constructors() {
    use crate::graph::edge as ge;

    let _: node::new::Node<(), (), ge::Undir<()>> = N(1);
    let _: node::exist::Node<(), (), ge::Undir<()>> = X(10);
    let _: node::new::Ref<(), ge::Undir<()>> = n(1);
    let _: node::exist::Ref<(), ge::Undir<()>> = x(10);
}

#[test]
fn node_val_typestate() {
    use crate::graph::edge as ge;

    let _: node::new::Node<&str, HasVal<&str>, ge::Undir<()>> = N(2).val("hello");
    let _: node::exist::Node<&str, HasVal<&str>, ge::Undir<()>> = X(10).val("hi");
}

#[test]
fn edge_constructors() {
    use crate::graph::edge as ge;

    let _: edge::new::Edge<(), (), ge::Undir<()>> = E();
    let _: edge::exist::Edge<(), (), ge::Undir<()>> = e();
}

#[test]
fn edge_val_typestate() {
    use crate::graph::edge as ge;

    let _: edge::new::Edge<HasVal<u32>, (), ge::Undir<u32>> = E().val(42u32);
    let _: edge::exist::Edge<HasVal<u32>, (), ge::Undir<u32>> = e().val(42u32);
}

#[test]
fn not_operators() {
    use crate::graph::edge as ge;

    let _: edge::exist::Rem<(), (), ge::Undir<()>> = !e::<(), ge::Undir<()>>();
    let _: node::exist::Rem<(), ge::Undir<()>> = !X::<(), ge::Undir<()>>(10);
}

#[test]
fn anon_edge_undir() {
    use crate::graph::edge;

    let _: Node<(), edge::Undir<()>> = (N(1) ^ N(2)).into();
}

#[test]
fn anon_edge_dir() {
    use crate::graph::edge;

    let _: Node<(), edge::Dir<()>> = (N(1) >> N(2)).into();
}

#[test]
fn anon_edge_chain() {
    use crate::graph::edge;

    let _: Node<(), edge::Undir<()>> = (N(1) ^ N(2) ^ n(1)).into();
}

#[test]
fn anon_edge_exist_to_new() {
    use crate::graph::edge;

    let _: Node<(), edge::Undir<()>> = (X(5) ^ N(3)).into();
}

#[test]
fn explicit_new_edge_with_val() {
    use crate::graph::edge;

    let _: Node<(), edge::Undir<u32>> = (N(1) & E().val(42u32) ^ N(2)).into();
}

#[test]
fn exist_edge_ref() {
    use crate::graph::edge;

    let _: Node<(), edge::Dir<()>> = (X(1) & e() >> X(2)).into();
}

#[test]
fn remove_edge() {
    use crate::graph::edge;

    let _: Node<(), edge::Dir<()>> = (X(1) & !e() >> X(2)).into();
}

#[test]
fn vec_of_top_nodes() {
    use crate::graph::edge;

    let v: Vec<Node<(), edge::Undir<()>>> = modify![N(1) ^ N(2) ^ n(1), X(5) ^ N(3), !X(99),];

    assert_eq!(v.len(), 3);
}

#[test]
fn valued_nodes_with_edges() {
    use crate::graph::edge;

    let _: Node<&str, edge::Dir<u32>> = (N(1).val("a") & E().val(10u32) >> N(2).val("b")).into();
}

#[test]
fn exist_node_with_val_and_edge() {
    use crate::graph::edge;

    let _: Node<&str, edge::Dir<u32>> =
        (X(1).val("updated") & E().val(20u32) >> N(1).val("new")).into();
}

#[test]
fn validate_rejects_duplicate_new_node() {
    use crate::graph::edge;

    let v: Vec<Node<(), edge::Undir<()>>> = modify![N(1) ^ N(2), N(1) ^ N(3),];
    let result = Fragment::new(v).validate();

    assert!(matches!(
        result,
        Err(error::Fragment::Node(error::fragment::Node::DuplicateNew(
            LocalId(1)
        )))
    ));
}

#[test]
fn validate_rejects_undefined_ref() {
    use crate::graph::edge;

    let v: Vec<Node<(), edge::Undir<()>>> = modify![N(1) ^ n(2),];
    let result = Fragment::new(v).validate();

    assert!(matches!(
        result,
        Err(error::Fragment::Node(error::fragment::Node::UndefinedRef(
            LocalId(2)
        )))
    ));
}

#[test]
fn validate_accepts_valid_fragment() {
    use crate::graph::edge;

    let v: Vec<Node<(), edge::Undir<()>>> = modify![N(1) ^ N(2) ^ n(1),];
    let result = Fragment::new(v).validate();

    assert!(result.is_ok());
}

#[test]
fn validate_rejects_exist_remove_conflict() {
    use crate::graph::edge;

    let v: Vec<Node<(), edge::Undir<()>>> = modify![X(id::N(5)), !X(id::N(5)),];
    let result = Fragment::new(v).validate();

    assert!(matches!(
        result,
        Err(error::Fragment::Node(
            error::fragment::Node::RemoveConflict(id::N(5))
        ))
    ));
}

#[test]
fn validate_rejects_duplicate_exist_node() {
    use crate::graph::edge;

    let v: Vec<Node<(), edge::Undir<()>>> = modify![X(id::N(1)), X(id::N(1)),];
    let result = Fragment::new(v).validate();

    assert!(matches!(
        result,
        Err(error::Fragment::Node(
            error::fragment::Node::DuplicateExist(id::N(1))
        ))
    ));
}

#[test]
fn apply_add_nodes_undir() {
    let mut g = undir0_one();
    assert_eq!(g.nodes.len(), 2);

    let result = modify!(g, [N(1) ^ N(2),]).unwrap();

    assert_eq!(g.nodes.len(), 4);
    assert_eq!(result.new_node_ids.len(), 2);
}

#[test]
fn apply_exist_node_add_edge() {
    let mut g = undir0_one();

    let _result = modify!(g, [X(id::N(0)) ^ N(1),]).unwrap();

    assert_eq!(g.nodes.len(), 3);
}

#[test]
fn apply_remove_node_cascade() {
    use crate::graph::edge::undir;

    let mut g: crate::graph::Undir0 =
        vec![undir::E::U(0, 1), undir::E::U(1, 2)].try_into().unwrap();
    assert_eq!(g.nodes.len(), 3);
    assert_eq!(g.edges.len(), 2);

    let _result = modify!(g, [!X(id::N(1)),]).unwrap();

    assert_eq!(g.nodes.len(), 2);
    assert_eq!(g.edges.len(), 0);
}

#[test]
fn apply_rejects_nonexistent_node() {
    let mut g = undir0_one();

    let result = modify!(g, [X(id::N(99)),]);

    assert!(matches!(
        result,
        Err(error::Modify::Apply(error::Apply::Node(
            error::apply::Node::NotFound(id::N(99))
        )))
    ));
}

#[test]
fn apply_dir_graph() {
    let mut g = dir0_one();
    assert_eq!(g.nodes.len(), 2);
    assert_eq!(g.edges.len(), 1);

    let result = modify!(g, [N(1) >> N(2),]).unwrap();

    assert_eq!(g.nodes.len(), 4);
    assert_eq!(g.edges.len(), 2);
    assert_eq!(result.new_node_ids.len(), 2);
}

#[test]
fn apply_anydir_mixed_edges() {
    let mut g = anydir0_one();
    assert_eq!(g.nodes.len(), 2);
    assert_eq!(g.edges.len(), 1);

    let result = modify!(g, [N(1) ^ N(2), N(3) >> N(4),]).unwrap();

    assert_eq!(g.nodes.len(), 6);
    assert_eq!(g.edges.len(), 3);
    assert_eq!(result.new_node_ids.len(), 4);
}

#[test]
fn apply_valued_nodes_and_edges() {
    use crate::graph::edge::undir;

    let mut g: crate::graph::Undir<&str, u32> = (
        vec![(0, "zero"), (1, "one")],
        vec![(undir::E::U(0, 1), 100u32)],
    )
        .try_into()
        .unwrap();
    assert_eq!(g.nodes.len(), 2);

    let result = modify!(g, [N(1).val("a") & E().val(42u32) ^ N(2).val("b"),]).unwrap();

    assert_eq!(g.nodes.len(), 4);
    assert_eq!(g.edges.len(), 2);
    let id_a = result.new_node_ids[&LocalId(1)];
    let id_b = result.new_node_ids[&LocalId(2)];
    assert_eq!(g.nodes.get(id_a), Some(&"a"));
    assert_eq!(g.nodes.get(id_b), Some(&"b"));
}

#[test]
fn apply_node_value_swap() {
    use crate::graph::edge::undir;

    let mut g: crate::graph::UndirN<&str> =
        (vec![(0, "old")], vec![] as Vec<undir::E<crate::Id>>)
            .try_into()
            .unwrap();
    assert_eq!(g.nodes.get(id::N(0)), Some(&"old"));

    let result = modify!(g, [X(id::N(0)).val("new"),]).unwrap();

    assert_eq!(g.nodes.get(id::N(0)), Some(&"new"));
    assert_eq!(result.swapped_node_vals.len(), 1);
    assert_eq!(result.swapped_node_vals[0], (id::N(0), "old"));
}

#[test]
fn apply_edge_removal() {
    use crate::graph::edge::dir;

    let mut g: crate::graph::Dir0 =
        vec![dir::E::D(0, 1), dir::E::D(1, 2)].try_into().unwrap();
    assert_eq!(g.edges.len(), 2);

    let result = modify!(g, [X(id::N(0)) & !e() >> X(id::N(1)),]).unwrap();

    assert_eq!(g.edges.len(), 1);
    assert_eq!(result.removed_edges.len(), 1);
}

#[test]
fn apply_chain_of_new_nodes() {
    let mut g = undir0_one();
    assert_eq!(g.nodes.len(), 2);

    let result = modify!(g, [N(1) ^ N(2) ^ n(1),]).unwrap();

    assert_eq!(g.nodes.len(), 4);
    assert_eq!(g.edges.len(), 3);
    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert!(g.has((id_1, id_2)));
    assert!(g.has((id_2, id_1)));
}

#[test]
fn apply_standalone_new_node() {
    let mut g = undir0_one();
    assert_eq!(g.nodes.len(), 2);

    let result = modify!(g, [N(1),]).unwrap();

    assert_eq!(g.nodes.len(), 3);
    assert_eq!(result.new_node_ids.len(), 1);
}

#[test]
fn apply_remove_preserves_unrelated_edges() {
    use crate::graph::edge::undir;

    let mut g: crate::graph::Undir0 = vec![
        undir::E::U(0, 1),
        undir::E::U(2, 3),
        undir::E::U(1, 2),
    ]
    .try_into()
    .unwrap();
    assert_eq!(g.nodes.len(), 4);
    assert_eq!(g.edges.len(), 3);

    let _result = modify!(g, [!X(id::N(1)),]).unwrap();

    assert_eq!(g.nodes.len(), 3);
    assert_eq!(g.edges.len(), 1);
    assert!(g.has((id::N(2), id::N(3))));
    assert!(!g.has((id::N(0), id::N(1))));
}

#[test]
fn apply_exist_to_exist_new_edge() {
    let mut g = undir0_one();
    assert_eq!(g.edges.len(), 1);
    assert!(!g.has((id::N(0), id::N(0))));

    let _result = modify!(g, [X(id::N(0)) ^ X(id::N(1)),]);
}

#[test]
fn apply_edge_value_swap() {
    use crate::graph::edge::undir;

    let mut g: crate::graph::Undir<(), u32> =
        vec![(undir::E::U(0, 1), 100u32)].try_into().unwrap();
    assert_eq!(g.edges.len(), 1);

    let result = modify!(g, [X(id::N(0)) & e().val(200u32) ^ X(id::N(1)),]).unwrap();

    assert_eq!(result.swapped_edge_vals.len(), 1);
    assert_eq!(*g.get(undir::E::U(0, 1)).unwrap(), 200u32);
}

#[test]
fn apply_cascade_conflict() {
    let mut g = dir0_one();

    let result = modify!(g, [!X(id::N(0)), X(id::N(1)) & !e() >> x(0),]);

    assert!(matches!(
        result,
        Err(error::Modify::Apply(error::Apply::Node(
            error::apply::Node::CascadeConflict(id::N(0))
        )))
    ));
}

// ============================================================
// Direction coverage: << (Tgt/incoming)
// ============================================================

#[test]
fn apply_dir_incoming_edge() {
    let mut g = dir0();

    let result = modify!(g, [N(1) << N(2),]).unwrap();

    assert_eq!(g.nodes.len(), 2);
    assert_eq!(g.edges.len(), 1);
    assert_eq!(result.new_node_ids.len(), 2);
}

#[test]
fn apply_anydir_incoming_edge() {
    let mut g = anydir0();

    let result = modify!(g, [N(1) << N(2),]).unwrap();

    assert_eq!(g.nodes.len(), 2);
    assert_eq!(g.edges.len(), 1);
    assert_eq!(result.new_node_ids.len(), 2);
}

// ============================================================
// Valueness × Directedness matrix (missing cells)
// ============================================================

#[test]
fn apply_dir_valued_nodes() {
    use crate::graph::edge::dir;

    let mut g: crate::graph::DirN<&str> =
        (vec![(0, "a")], vec![] as Vec<dir::E<crate::Id>>).try_into().unwrap();

    let result = modify!(g, [N(1).val("b") >> N(2).val("c"),]).unwrap();

    assert_eq!(g.nodes.len(), 3);
    assert_eq!(g.edges.len(), 1);
    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(g.nodes.get(id_1), Some(&"b"));
    assert_eq!(g.nodes.get(id_2), Some(&"c"));
}

#[test]
fn apply_dir_valued_edges() {
    use crate::graph::edge::dir;

    let mut g: crate::graph::DirE<u32> =
        vec![(dir::E::D(0, 1), 10u32)].try_into().unwrap();

    let result = modify!(g, [N(1) & E().val(99u32) >> N(2),]).unwrap();

    assert_eq!(g.edges.len(), 2);
    assert_eq!(result.new_node_ids.len(), 2);
}

#[test]
fn apply_dir_both_valued() {
    use crate::graph::edge::dir;

    let mut g: crate::graph::Dir<&str, u32> = (
        vec![(0, "a"), (1, "b")],
        vec![(dir::E::D(0, 1), 100u32)],
    )
        .try_into()
        .unwrap();

    let result = modify!(g, [N(1).val("x") & E().val(55u32) >> N(2).val("y"),]).unwrap();

    assert_eq!(g.nodes.len(), 4);
    assert_eq!(g.edges.len(), 2);
    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(g.nodes.get(id_1), Some(&"x"));
    assert_eq!(g.nodes.get(id_2), Some(&"y"));
}

#[test]
fn apply_anydir_valued_nodes() {
    use crate::graph::edge::anydir;

    let mut g: crate::graph::AnydirN<&str> =
        (vec![(0, "a")], vec![] as Vec<anydir::E<crate::Id>>).try_into().unwrap();

    let result = modify!(g, [
        N(1).val("b") >> N(2).val("c"),
        N(3).val("d") ^ N(4).val("e"),
    ])
    .unwrap();

    assert_eq!(g.nodes.len(), 5);
    assert_eq!(g.edges.len(), 2);
    assert_eq!(result.new_node_ids.len(), 4);
    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_3 = result.new_node_ids[&LocalId(3)];
    assert_eq!(g.nodes.get(id_1), Some(&"b"));
    assert_eq!(g.nodes.get(id_3), Some(&"d"));
}

#[test]
fn apply_anydir_valued_edges() {
    use crate::graph::edge::anydir;

    let mut g: crate::graph::AnydirE<u32> =
        vec![(anydir::E::U(0, 1), 10u32)].try_into().unwrap();

    let result = modify!(g, [
        N(1) & E().val(20u32) >> N(2),
        N(3) & E().val(30u32) ^ N(4),
    ])
    .unwrap();

    assert_eq!(g.edges.len(), 3);
    assert_eq!(result.new_node_ids.len(), 4);
}

#[test]
fn apply_anydir_both_valued() {
    use crate::graph::edge::anydir;

    let mut g: crate::graph::Anydir<&str, u32> = (
        vec![(0, "a"), (1, "b")],
        vec![(anydir::E::U(0, 1), 10u32)],
    )
        .try_into()
        .unwrap();

    let result = modify!(g, [N(1).val("x") & E().val(77u32) >> N(2).val("y"),]).unwrap();

    assert_eq!(g.nodes.len(), 4);
    assert_eq!(g.edges.len(), 2);
    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(g.nodes.get(id_1), Some(&"x"));
    assert_eq!(g.nodes.get(id_2), Some(&"y"));
}

// ============================================================
// DSL construction type-checks (operator × node-type coverage)
// ============================================================

#[test]
fn chain_all_node_types_undir() {
    use crate::graph::edge;

    let _: Node<(), edge::Undir<()>> = (N(1) ^ N(2) ^ n(1) ^ N_()).into();
    let _: Node<(), edge::Undir<()>> = (X(id::N(0)) ^ N(1) ^ x(id::N(0)) ^ n(1)).into();
}

#[test]
fn chain_all_node_types_dir_shr() {
    use crate::graph::edge;

    let _: Node<(), edge::Dir<()>> = (N(1) >> N(2) >> n(1) >> N_()).into();
    let _: Node<(), edge::Dir<()>> = (X(id::N(0)) >> N(1) >> x(id::N(0)) >> n(1)).into();
}

#[test]
fn chain_all_node_types_dir_shl() {
    use crate::graph::edge;

    let _: Node<(), edge::Dir<()>> = (N(1) << N(2) << n(1) << N_()).into();
    let _: Node<(), edge::Dir<()>> = (X(id::N(0)) << N(1) << x(id::N(0)) << n(1)).into();
}

#[test]
fn chain_anydir_mixed_directions() {
    use crate::graph::edge;

    let _: Node<(), edge::Anydir<()>> = (N(1) >> N(2) << N(3) ^ N(4)).into();
    let _: Node<(), edge::Anydir<()>> = (X(id::N(0)) >> N(1) ^ N(2) << x(id::N(0))).into();
}

#[test]
fn explicit_edge_all_directions() {
    use crate::graph::edge;

    let _: Node<(), edge::Dir<()>> = (N(1) & E() >> N(2)).into();
    let _: Node<(), edge::Dir<()>> = (N(1) & E() << N(2)).into();
    let _: Node<(), edge::Undir<()>> = (N(1) & E() ^ N(2)).into();
    let _: Node<(), edge::Anydir<()>> = (N(1) & E() >> N(2)).into();
    let _: Node<(), edge::Anydir<()>> = (N(1) & E() << N(2)).into();
    let _: Node<(), edge::Anydir<()>> = (N(1) & E() ^ N(2)).into();
}

#[test]
fn explicit_edge_with_val_all_directions() {
    use crate::graph::edge;

    let _: Node<(), edge::Dir<u32>> = (N(1) & E().val(1u32) >> N(2)).into();
    let _: Node<(), edge::Dir<u32>> = (N(1) & E().val(2u32) << N(2)).into();
    let _: Node<(), edge::Undir<u32>> = (N(1) & E().val(3u32) ^ N(2)).into();
    let _: Node<(), edge::Anydir<u32>> = (N(1) & E().val(4u32) >> N(2)).into();
    let _: Node<(), edge::Anydir<u32>> = (N(1) & E().val(5u32) << N(2)).into();
    let _: Node<(), edge::Anydir<u32>> = (N(1) & E().val(6u32) ^ N(2)).into();
}

// ============================================================
// Recursive tree structure: depth-2 and depth-3
// ============================================================

#[test]
fn apply_tree_depth2_dir() {
    use crate::graph::edge::dir;

    let mut g = dir0_one();

    let result = modify!(g, [N(1) >> (N(2) >> N(3)),]).unwrap();

    assert_eq!(g.nodes.len(), 5);
    assert_eq!(g.edges.len(), 3);
    assert_eq!(result.new_node_ids.len(), 3);
    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    let id_3 = result.new_node_ids[&LocalId(3)];
    assert!(g.has(dir::E::D(*id_1, *id_2)));
    assert!(g.has(dir::E::D(*id_2, *id_3)));
    assert!(!g.has(dir::E::D(*id_1, *id_3)));
}

#[test]
fn apply_tree_depth3_dir() {
    use crate::graph::edge::dir;

    let mut g = dir0_one();

    let result = modify!(g, [N(1) >> (N(2) >> (N(3) >> N(4))),]).unwrap();

    assert_eq!(g.nodes.len(), 6);
    assert_eq!(g.edges.len(), 4);
    assert_eq!(result.new_node_ids.len(), 4);
    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    let id_3 = result.new_node_ids[&LocalId(3)];
    let id_4 = result.new_node_ids[&LocalId(4)];
    assert!(g.has(dir::E::D(*id_1, *id_2)));
    assert!(g.has(dir::E::D(*id_2, *id_3)));
    assert!(g.has(dir::E::D(*id_3, *id_4)));
    assert!(!g.has(dir::E::D(*id_1, *id_3)));
    assert!(!g.has(dir::E::D(*id_2, *id_4)));
}

#[test]
fn apply_tree_depth2_undir() {
    let mut g = undir0_one();

    let result = modify!(g, [N(1) ^ (N(2) ^ N(3)),]).unwrap();

    assert_eq!(g.nodes.len(), 5);
    assert_eq!(g.edges.len(), 3);
    assert_eq!(result.new_node_ids.len(), 3);
    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    let id_3 = result.new_node_ids[&LocalId(3)];
    assert!(g.has((id_1, id_2)));
    assert!(g.has((id_2, id_3)));
    assert!(!g.has((id_1, id_3)));
}

#[test]
fn apply_tree_depth3_undir() {
    let mut g = undir0_one();

    let result = modify!(g, [N(1) ^ (N(2) ^ (N(3) ^ N(4))),]).unwrap();

    assert_eq!(g.nodes.len(), 6);
    assert_eq!(g.edges.len(), 4);
    assert_eq!(result.new_node_ids.len(), 4);
    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    let id_3 = result.new_node_ids[&LocalId(3)];
    let id_4 = result.new_node_ids[&LocalId(4)];
    assert!(g.has((id_1, id_2)));
    assert!(g.has((id_2, id_3)));
    assert!(g.has((id_3, id_4)));
    assert!(!g.has((id_1, id_3)));
    assert!(!g.has((id_2, id_4)));
}

#[test]
fn apply_tree_depth2_anydir() {
    let mut g = anydir0();

    let result = modify!(g, [N(1) >> (N(2) ^ N(3)),]).unwrap();

    assert_eq!(g.nodes.len(), 3);
    assert_eq!(g.edges.len(), 2);
    assert_eq!(result.new_node_ids.len(), 3);
    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    let id_3 = result.new_node_ids[&LocalId(3)];
    assert!(g.has((id_1, id_2)));
    assert!(g.has((id_2, id_3)));
    assert!(!g.has((id_1, id_3)));
}

#[test]
fn apply_tree_depth2_valued() {
    use crate::graph::edge::dir;

    let mut g: crate::graph::Dir<&str, u32> =
        (vec![(0, "root")], vec![] as Vec<(dir::E<crate::Id>, u32)>)
            .try_into()
            .unwrap();

    let result = modify!(g, [
        N(1).val("a") & E().val(10u32) >> (N(2).val("b") & E().val(20u32) >> N(3).val("c")),
    ])
    .unwrap();

    assert_eq!(g.nodes.len(), 4);
    assert_eq!(g.edges.len(), 2);
    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    let id_3 = result.new_node_ids[&LocalId(3)];
    assert_eq!(g.nodes.get(id_1), Some(&"a"));
    assert_eq!(g.nodes.get(id_2), Some(&"b"));
    assert_eq!(g.nodes.get(id_3), Some(&"c"));
    assert!(g.has(dir::E::D(*id_1, *id_2)));
    assert!(g.has(dir::E::D(*id_2, *id_3)));
    assert!(!g.has(dir::E::D(*id_1, *id_3)));
}

// ============================================================
// Fan/star: left-associative chaining creates a star not a chain
// ============================================================

#[test]
fn apply_fan_dir() {
    use crate::graph::edge::dir;

    let mut g = dir0();

    let result = modify!(g, [N(1) >> N(2) >> N(3),]).unwrap();

    assert_eq!(g.nodes.len(), 3);
    assert_eq!(g.edges.len(), 2);
    assert_eq!(result.new_node_ids.len(), 3);
    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    let id_3 = result.new_node_ids[&LocalId(3)];
    assert!(g.has(dir::E::D(*id_1, *id_2)));
    assert!(g.has(dir::E::D(*id_1, *id_3)));
    assert!(!g.has(dir::E::D(*id_2, *id_3)));
}

#[test]
fn apply_fan_undir() {
    let mut g = undir0();

    let result = modify!(g, [N(1) ^ N(2) ^ N(3),]).unwrap();

    assert_eq!(g.nodes.len(), 3);
    assert_eq!(g.edges.len(), 2);
    assert_eq!(result.new_node_ids.len(), 3);
    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    let id_3 = result.new_node_ids[&LocalId(3)];
    assert!(g.has((id_1, id_2)));
    assert!(g.has((id_1, id_3)));
    assert!(!g.has((id_2, id_3)));
}

// ============================================================
// ConflictingEdgeSwap: same edge targeted by two swap ops
// ============================================================

#[test]
fn apply_rejects_conflicting_edge_swap_dir() {
    use crate::graph::edge::dir;

    let mut g: crate::graph::Dir<(), u32> =
        vec![(dir::E::D(0, 1), 10u32)].try_into().unwrap();

    let result = modify!(g, [
        X(id::N(0)) & e().val(100u32) >> x(id::N(1)),
        X(id::N(1)) & e().val(200u32) << x(id::N(0)),
    ]);

    assert!(matches!(
        result,
        Err(error::Modify::Apply(error::Apply::Edge(
            error::apply::Edge::SwapConflict(_, _)
        )))
    ));
}

#[test]
fn apply_rejects_conflicting_edge_swap_undir() {
    use crate::graph::edge::undir;

    let mut g: crate::graph::Undir<(), u32> =
        vec![(undir::E::U(0, 1), 10u32)].try_into().unwrap();

    let result = modify!(g, [
        X(id::N(0)) & e().val(100u32) ^ x(id::N(1)),
        X(id::N(1)) & e().val(200u32) ^ x(id::N(0)),
    ]);

    assert!(matches!(
        result,
        Err(error::Modify::Apply(error::Apply::Edge(
            error::apply::Edge::SwapConflict(_, _)
        )))
    ));
}

#[test]
fn apply_rejects_conflicting_edge_swap_anydir() {
    use crate::graph::edge::anydir;

    let mut g: crate::graph::Anydir<(), u32> =
        vec![(anydir::E::D(0, 1), 10u32)].try_into().unwrap();

    let result = modify!(g, [
        X(id::N(0)) & e().val(100u32) >> x(id::N(1)),
        X(id::N(1)) & e().val(200u32) << x(id::N(0)),
    ]);

    assert!(matches!(
        result,
        Err(error::Modify::Apply(error::Apply::Edge(
            error::apply::Edge::SwapConflict(_, _)
        )))
    ));
}

// ============================================================
// IntoVal / Default: value materialization
// ============================================================

#[test]
fn default_node_val_anon_undir_chain() {
    let mut g = undir_n_empty::<u32>();

    let result = modify!(g, [N(1) ^ N(2)]).unwrap();

    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(g.nodes.get(id_1), Some(&0u32));
    assert_eq!(g.nodes.get(id_2), Some(&0u32));
}

#[test]
fn default_node_val_in_edge_chain() {
    let mut g = dir_n_empty::<u32>();

    let result = modify!(g, [N(1) >> N(2)]).unwrap();

    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(g.nodes.get(id_1), Some(&0u32));
    assert_eq!(g.nodes.get(id_2), Some(&0u32));
}

#[test]
fn partial_override_node_val_in_chain() {
    let mut g = dir_n_empty::<u32>();

    let result = modify!(g, [N(1).val(5u32) >> N(2)]).unwrap();

    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(g.nodes.get(id_1), Some(&5u32));
    assert_eq!(g.nodes.get(id_2), Some(&0u32));
}

#[test]
fn default_node_val_is_applied() {
    let mut g = undir_n_empty::<u32>();

    let result = modify!(g, [N(1)]).unwrap();

    let id = result.new_node_ids[&LocalId(1)];
    assert_eq!(g.nodes.get(id), Some(&0u32));
}

#[test]
fn explicit_val_overrides_default_node() {
    let mut g = undir_n_empty::<u32>();

    let result = modify!(g, [N(1).val(42u32)]).unwrap();

    let id = result.new_node_ids[&LocalId(1)];
    assert_eq!(g.nodes.get(id), Some(&42u32));
}

#[test]
fn default_edge_val_anon_undir() {
    let mut g = undir_e_empty::<u32>();

    let result = modify!(g, [N(1) ^ N(2)]).unwrap();

    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(*g.get(crate::graph::edge::undir::E::U(*id_1, *id_2)).unwrap(), 0u32);
}

#[test]
fn default_edge_val_anon_dir() {
    let mut g = dir_e_empty::<u32>();

    let result = modify!(g, [N(1) >> N(2)]).unwrap();

    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(*g.get(crate::graph::edge::dir::E::D(*id_1, *id_2)).unwrap(), 0u32);
}

#[test]
fn default_edge_val_explicit_edge() {
    let mut g = undir_e_empty::<u32>();

    let result = modify!(g, [N(1) & E() ^ N(2)]).unwrap();

    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(*g.get(crate::graph::edge::undir::E::U(*id_1, *id_2)).unwrap(), 0u32);
}

#[test]
fn explicit_val_overrides_default_edge() {
    let mut g = undir_e_empty::<u32>();

    let result = modify!(g, [N(1) & E().val(77u32) ^ N(2)]).unwrap();

    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(*g.get(crate::graph::edge::undir::E::U(*id_1, *id_2)).unwrap(), 77u32);
}

#[test]
fn default_edge_val_anon_shl() {
    let mut g = dir_e_empty::<u32>();

    let result = modify!(g, [N(1) << N(2)]).unwrap();

    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(*g.get(crate::graph::edge::dir::E::D(*id_2, *id_1)).unwrap(), 0u32);
}

#[test]
fn default_edge_val_explicit_shr() {
    let mut g = dir_e_empty::<u32>();

    let result = modify!(g, [N(1) & E() >> N(2)]).unwrap();

    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(*g.get(crate::graph::edge::dir::E::D(*id_1, *id_2)).unwrap(), 0u32);
}

#[test]
fn default_edge_val_explicit_shl() {
    let mut g = dir_e_empty::<u32>();

    let result = modify!(g, [N(1) & E() << N(2)]).unwrap();

    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(*g.get(crate::graph::edge::dir::E::D(*id_2, *id_1)).unwrap(), 0u32);
}

#[test]
fn default_edge_val_anon_anydir_dir() {
    let mut g = anydir_e_empty::<u32>();

    let result = modify!(g, [N(1) >> N(2)]).unwrap();

    let id_1 = result.new_node_ids[&LocalId(1)];
    let id_2 = result.new_node_ids[&LocalId(2)];
    assert_eq!(*g.get(crate::graph::edge::anydir::E::D(*id_1, *id_2)).unwrap(), 0u32);
}

#[test]
fn default_edge_val_anon_anydir_undir() {
    let mut g = anydir_e_empty::<u32>();

    let result = modify!(g, [N(1) ^ N(2)]).unwrap();

    assert_eq!(g.edges.len(), 1);
    assert_eq!(result.new_node_ids.len(), 2);
}

#[test]
fn degrees_after_construction() {
    use crate::graph::edge::undir;

    let g: crate::graph::Undir0 = vec![
        undir::E::U(0, 1),
        undir::E::U(1, 2),
        undir::E::U(2, 0),
    ]
    .try_into()
    .unwrap();

    assert_eq!(g.node_count(), 3);
    for &(deg, ref set) in &g.degrees {
        assert_eq!(deg, 2);
        assert_eq!(set.len(), 3);
    }
}

#[test]
fn degrees_after_modify_add_edge() {
    let mut g = undir0_one();
    let initial_degrees: Vec<(crate::Id, u64)> = g.degrees.iter().map(|(d, s)| (*d, s.len())).collect();
    assert_eq!(initial_degrees, vec![(1, 2)]);

    let _result = modify!(g, [X(id::N(0)) ^ N(1),]).unwrap();

    assert_eq!(g.node_count(), 3);
    assert_eq!(g.edge_count(), 2);
    let mut deg_map: Vec<(crate::Id, u64)> = g.degrees.iter().map(|(d, s)| (*d, s.len())).collect();
    deg_map.sort_by(|a, b| b.0.cmp(&a.0));
    assert_eq!(deg_map[0].0, 2);
    assert_eq!(deg_map[0].1, 1);
}

#[test]
fn degrees_after_modify_add_isolated_nodes() {
    let mut g = undir0();

    let ops: Vec<modify::Node<(), crate::graph::edge::Undir<()>>> =
        (0..5).map(|_| N_().into()).collect();
    g.modify(ops).unwrap();

    assert_eq!(g.node_count(), 5);
    assert_eq!(g.degrees.len(), 1);
    assert_eq!(g.degrees[0].0, 0);
    assert_eq!(g.degrees[0].1.len(), 5);
}

#[test]
fn degrees_after_modify_remove_node() {
    use crate::graph::edge::undir;

    let mut g: crate::graph::Undir0 =
        vec![undir::E::U(0, 1), undir::E::U(1, 2)].try_into().unwrap();
    assert_eq!(g.node_count(), 3);

    let _result = modify!(g, [!X(id::N(1)),]).unwrap();

    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 0);
    assert_eq!(g.degrees.len(), 1);
    assert_eq!(g.degrees[0].0, 0);
    assert_eq!(g.degrees[0].1.len(), 2);
}

#[test]
fn degrees_after_modify_add_pairs() {
    use crate::graph::edge;

    let mut g = undir0();

    let ops: Vec<modify::Node<(), edge::Undir<()>>> =
        (0..3).map(|_| (N_() ^ N_()).into()).collect();
    g.modify(ops).unwrap();

    assert_eq!(g.node_count(), 6);
    assert_eq!(g.edge_count(), 3);
    assert_eq!(g.degrees.len(), 1);
    assert_eq!(g.degrees[0].0, 1);
    assert_eq!(g.degrees[0].1.len(), 6);
}
