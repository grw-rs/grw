use super::node;
use super::*;
use crate::id;
use crate::modify;

fn undir0() -> crate::graph::MUndir0 {
    crate::graph::MUndir0::default()
}

fn dir0() -> crate::graph::MDir0 {
    crate::graph::MDir0::default()
}

fn anydir0() -> crate::graph::MAnydir0 {
    crate::graph::MAnydir0::default()
}

fn undir0_one() -> crate::graph::MUndir0 {
    vec![crate::graph::edge::undir::E::U(0, 1)].try_into().unwrap()
}

fn dir0_one() -> crate::graph::MDir0 {
    vec![crate::graph::edge::dir::E::D(0, 1)].try_into().unwrap()
}

fn anydir0_one() -> crate::graph::MAnydir0 {
    vec![crate::graph::edge::anydir::E::U(0, 1)].try_into().unwrap()
}

fn undir_n_empty<T: Sync>() -> crate::graph::MUndirN<T> {
    (
        vec![] as Vec<(crate::Id, T)>,
        vec![] as Vec<crate::graph::edge::undir::E<crate::Id>>,
    )
        .try_into()
        .unwrap()
}

fn dir_n_empty<T: Sync>() -> crate::graph::MDirN<T> {
    (
        vec![] as Vec<(crate::Id, T)>,
        vec![] as Vec<crate::graph::edge::dir::E<crate::Id>>,
    )
        .try_into()
        .unwrap()
}

fn undir_e_empty<T: Sync>() -> crate::graph::MUndirE<T> {
    (vec![] as Vec<(crate::graph::edge::undir::E<crate::Id>, T)>)
        .try_into()
        .unwrap()
}

fn dir_e_empty<T: Sync>() -> crate::graph::MDirE<T> {
    (vec![] as Vec<(crate::graph::edge::dir::E<crate::Id>, T)>)
        .try_into()
        .unwrap()
}

fn anydir_e_empty<T: Sync>() -> crate::graph::MAnydirE<T> {
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

    let _: edge::new::Edge<HasRawVal<u32>, (), ge::Undir<u32>> = E().val(42u32);
    let _: edge::exist::Edge<HasRawVal<u32>, (), ge::Undir<u32>> = e().val(42u32);
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

    let mut g: crate::graph::MUndir0 =
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

    let mut g: crate::graph::MUndir<&str, u32> = (
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

    let mut g: crate::graph::MUndirN<&str> =
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

    let mut g: crate::graph::MDir0 =
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

    let mut g: crate::graph::MUndir0 = vec![
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

    let mut g: crate::graph::MUndir<(), u32> =
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

    let mut g: crate::graph::MDirN<&str> =
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

    let mut g: crate::graph::MDirE<u32> =
        vec![(dir::E::D(0, 1), 10u32)].try_into().unwrap();

    let result = modify!(g, [N(1) & E().val(99u32) >> N(2),]).unwrap();

    assert_eq!(g.edges.len(), 2);
    assert_eq!(result.new_node_ids.len(), 2);
}

#[test]
fn apply_dir_both_valued() {
    use crate::graph::edge::dir;

    let mut g: crate::graph::MDir<&str, u32> = (
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

    let mut g: crate::graph::MAnydirN<&str> =
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

    let mut g: crate::graph::MAnydirE<u32> =
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

    let mut g: crate::graph::MAnydir<&str, u32> = (
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

    let mut g: crate::graph::MDir<&str, u32> =
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

    let mut g: crate::graph::MDir<(), u32> =
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

    let mut g: crate::graph::MUndir<(), u32> =
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

    let mut g: crate::graph::MAnydir<(), u32> =
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
    assert_eq!(*g.get(crate::graph::edge::anydir::E::D(*id_1, *id_2)).unwrap(), crate::graph::edge::AnyVal::Undir(0u32));
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

    let g: crate::graph::MUndir0 = vec![
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
    deg_map.sort_by_key(|b| std::cmp::Reverse(b.0));
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

    let mut g: crate::graph::MUndir0 =
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

mod parts {
    use crate::composite::{Composite, KindPresent, NotHeld, Part as Parted, TypeSet};
    use crate::graph::edge::{self, dir};
    use crate::graph::{self, Graph};
    use crate::modify::error::{self, Apply, apply};
    use crate::modify::{self, ExcludePart, IncludePart, Modification, Node, Part, node};
    use crate::search::engine::Index;
    use crate::{Id, RevCsr, id};

    #[derive(Debug, Clone, PartialEq)]
    struct Signs {
        since: u32,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Uses {
        service: &'static str,
    }

    #[derive(Debug, Clone, PartialEq, Parted)]
    enum Rel {
        Signs(Signs),
        Uses(Uses),
    }

    type Link = TypeSet<Rel>;
    type ER = edge::Dir<Link>;
    type Ops = Vec<Node<(), ER>>;
    type Applied<G> = Result<(G, Modification<(), ER>), error::Modify>;

    trait Subject: Graph<(), ER> + Sized {
        fn built(links: Vec<(dir::E<Id>, Link)>) -> Self;
        fn applied(self, ops: Ops) -> Applied<Self>;
        fn attempted(&mut self, ops: Ops) -> Result<Modification<(), ER>, error::Modify>;
    }

    impl Subject for graph::MGraph<(), ER> {
        fn built(links: Vec<(dir::E<Id>, Link)>) -> Self {
            (3, links).try_into().unwrap()
        }

        fn applied(mut self, ops: Ops) -> Applied<Self> {
            let modification = self.modify(ops)?;
            Ok((self, modification))
        }

        fn attempted(&mut self, ops: Ops) -> Result<Modification<(), ER>, error::Modify> {
            self.modify(ops)
        }
    }

    impl Subject for graph::VGraph<(), ER> {
        fn built(links: Vec<(dir::E<Id>, Link)>) -> Self {
            graph::VGraph::from_mgraph(&<graph::MGraph<(), ER> as Subject>::built(links))
        }

        fn applied(self, ops: Ops) -> Applied<Self> {
            self.modify(ops)
        }

        fn attempted(&mut self, ops: Ops) -> Result<Modification<(), ER>, error::Modify> {
            let (next, modification) = self.modify(ops)?;
            *self = next;
            Ok(modification)
        }
    }

    fn signs(since: u32) -> Rel {
        Rel::Signs(Signs { since })
    }

    fn uses(service: &'static str) -> Rel {
        Rel::Uses(Uses { service })
    }

    fn link(parts: Vec<Rel>) -> Link {
        let mut parts = parts.into_iter();
        let first = Link::from_part(parts.next().expect("a link holds at least one part"));
        parts.fold(first, |held, part| match held.with_part(part).unwrap() {
            crate::composite::Included::Added(next) => next,
            crate::composite::Included::Held => held,
        })
    }

    fn exist(n: Id) -> Node<(), ER> {
        Node::Exist(node::Exist::Bind { id: id::N(n), op: node::Bind::Ref, edges: vec![] })
    }

    fn on(a: Id, b: Id, op: Part<Link>) -> Node<(), ER> {
        Node::Exist(node::Exist::Bind {
            id: id::N(a),
            op: node::Bind::Ref,
            edges: vec![modify::edge::Edge::Part { slot: dir::SRC, op, target: exist(b) }],
        })
    }

    fn include(a: Id, b: Id, part: Rel) -> Node<(), ER> {
        on(a, b, IncludePart::new(part).into())
    }

    fn exclude(a: Id, b: Id, kind: RelKind) -> Node<(), ER> {
        on(a, b, ExcludePart::new(kind).into())
    }

    fn held<G: Subject>(g: &G, a: Id, b: Id) -> Option<&Link> {
        g.get_edge_val(dir::E::D(a, b))
    }

    fn untouched(m: &Modification<(), ER>) -> bool {
        m.added_edges.is_empty()
            && m.removed_edges.is_empty()
            && m.swapped_edge_vals.is_empty()
            && m.removed_nodes.is_empty()
            && m.swapped_node_vals.is_empty()
    }

    fn include_on_an_empty_pair_creates_the_link<G: Subject>() {
        let (g, m) = G::built(vec![]).applied(vec![include(0, 1, signs(2020))]).unwrap();

        assert_eq!(m.added_edges.len(), 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(held(&g, 0, 1), Some(&link(vec![signs(2020)])));
    }

    fn include_a_second_kind_joins_the_link<G: Subject>() {
        let g = G::built(vec![(dir::E::D(0, 1), link(vec![signs(2020)]))]);

        let (g, m) = g.applied(vec![include(0, 1, uses("custody"))]).unwrap();

        assert_eq!(m.swapped_edge_vals.len(), 1);
        assert_eq!(m.swapped_edge_vals[0].3, link(vec![signs(2020)]));
        assert!(m.added_edges.is_empty());
        assert_eq!(g.edge_count(), 1);
        assert_eq!(held(&g, 0, 1), Some(&link(vec![signs(2020), uses("custody")])));
    }

    fn include_an_equal_part_changes_nothing<G: Subject>() {
        let g = G::built(vec![(dir::E::D(0, 1), link(vec![signs(2020)]))]);

        let (g, m) = g.applied(vec![include(0, 1, signs(2020))]).unwrap();

        assert!(untouched(&m));
        assert_eq!(held(&g, 0, 1), Some(&link(vec![signs(2020)])));
    }

    fn include_a_different_part_of_a_held_kind_is_rejected<G: Subject>() {
        let g = G::built(vec![(dir::E::D(0, 1), link(vec![signs(2020)]))]);

        let Err(error::Modify::Apply(Apply::Edge(apply::Edge::PartRejected { a, b, refused }))) =
            g.applied(vec![include(0, 1, signs(2024))])
        else {
            panic!("a second Signs on one link must be refused by the combinator")
        };

        assert_eq!((a, b), (id::N(0), id::N(1)));
        assert_eq!(refused.of::<Link>(), Some(&KindPresent { kind: RelKind::Signs }));
    }

    fn exclude_to_remains_replaces_the_value<G: Subject>() {
        let g = G::built(vec![(dir::E::D(0, 1), link(vec![signs(2020), uses("custody")]))]);

        let (g, m) = g.applied(vec![exclude(0, 1, RelKind::Signs)]).unwrap();

        assert_eq!(m.swapped_edge_vals.len(), 1);
        assert!(m.removed_edges.is_empty());
        assert_eq!(g.edge_count(), 1);
        assert_eq!(held(&g, 0, 1), Some(&link(vec![uses("custody")])));
    }

    fn exclude_to_vacant_deletes_the_link<G: Subject>() {
        let g = G::built(vec![
            (dir::E::D(0, 1), link(vec![signs(2020)])),
            (dir::E::D(1, 2), link(vec![uses("custody")])),
        ]);

        let (g, m) = g.applied(vec![exclude(0, 1, RelKind::Signs)]).unwrap();

        assert_eq!(m.removed_edges.len(), 1);
        assert_eq!(m.removed_edges[0].3, link(vec![signs(2020)]));
        assert_eq!(g.edge_count(), 1);
        assert_eq!(held(&g, 0, 1), None);
        assert_eq!(g.edges_between(id::N(0), id::N(1)).count(), 0);
        assert_eq!(g.neighbors(id::N(0)).unwrap().count(), 0);
        assert_eq!(g.degree(id::N(0)), Some(0));
        assert_eq!(g.degree(id::N(1)), Some(1));
        assert!(!g.is_adjacent(0, 1));

        let csr = g.index(RevCsr);
        assert!(!csr.is_adjacent(0, 1));
        assert!(!csr.has_edge_in_slot(0, 1, dir::SRC, false));
        assert_eq!(csr.degree(0), 0);
        assert!(csr.has_edge_in_slot(1, 2, dir::SRC, false));
    }

    fn exclude_a_kind_the_link_does_not_hold_is_refused<G: Subject>() {
        let g = G::built(vec![(dir::E::D(0, 1), link(vec![signs(2020)]))]);

        let Err(error::Modify::Apply(Apply::Edge(apply::Edge::PartNotHeld { a, b, refused }))) =
            g.applied(vec![exclude(0, 1, RelKind::Uses)])
        else {
            panic!("excluding an absent kind must be refused by the combinator")
        };

        assert_eq!((a, b), (id::N(0), id::N(1)));
        assert_eq!(refused.of::<Link>(), Some(&NotHeld { kind: RelKind::Uses }));
    }

    fn exclude_on_a_pair_with_no_link_is_the_missing_edge<G: Subject>() {
        let result = G::built(vec![]).applied(vec![exclude(0, 1, RelKind::Signs)]);

        assert!(matches!(
            result,
            Err(error::Modify::Apply(Apply::Edge(apply::Edge::NotFound(a, b)))) if (a, b) == (id::N(0), id::N(1))
        ));
    }

    fn whole_link_add_keeps_its_duplicate_refusal<G: Subject>() {
        let g = G::built(vec![(dir::E::D(0, 1), link(vec![signs(2020)]))]);
        let add = Node::Exist(node::Exist::Bind {
            id: id::N(0),
            op: node::Bind::Ref,
            edges: vec![modify::edge::Edge::New { slot: dir::SRC, val: link(vec![uses("custody")]), target: exist(1) }],
        });

        let result = g.applied(vec![add]);

        assert!(matches!(result, Err(error::Modify::Apply(Apply::Edge(apply::Edge::Duplicate(_, _))))));
    }

    fn whole_and_part_ops_on_one_pair_conflict<G: Subject>() {
        let g = G::built(vec![(dir::E::D(0, 1), link(vec![signs(2020)]))]);
        let remove = Node::Exist(node::Exist::Bind {
            id: id::N(0),
            op: node::Bind::Ref,
            edges: vec![modify::edge::Edge::Exist { slot: dir::SRC, op: modify::edge::Exist::Rem, target: exist(1) }],
        });

        let result = g.applied(vec![remove, include(0, 1, uses("custody"))]);

        assert!(matches!(result, Err(error::Modify::Apply(Apply::Edge(apply::Edge::PartConflict(_, _))))));
    }

    fn parts_on_one_pair_fold_in_batch_order<G: Subject>() {
        let (g, m) = G::built(vec![])
            .applied(vec![include(0, 1, signs(2020)), include(0, 1, signs(2020)), include(0, 1, uses("custody"))])
            .unwrap();

        assert_eq!(m.added_edges.len(), 1);
        assert_eq!(held(&g, 0, 1), Some(&link(vec![signs(2020), uses("custody")])));
    }

    fn include_then_exclude_all_in_one_batch_leaves_the_pair_empty<G: Subject>() {
        let (g, m) = G::built(vec![])
            .applied(vec![include(0, 1, signs(2020)), exclude(0, 1, RelKind::Signs)])
            .unwrap();

        assert!(untouched(&m));
        assert_eq!(g.edge_count(), 0);
    }

    fn include_towards_a_new_node_creates_the_link<G: Subject>() {
        let ops: Ops = vec![
            Node::New(node::New::Add { id: Some(modify::LocalId(7)), val: () }, vec![]),
            Node::Exist(node::Exist::Bind {
                id: id::N(0),
                op: node::Bind::Ref,
                edges: vec![modify::edge::Edge::Part {
                    slot: dir::SRC,
                    op: IncludePart::new(signs(2020)).into(),
                    target: Node::New(node::New::Ref { id: modify::LocalId(7) }, vec![]),
                }],
            }),
        ];

        let (g, m) = G::built(vec![]).applied(ops).unwrap();

        let fresh = m.new_node_ids[&modify::LocalId(7)];
        assert_eq!(held(&g, 0, *fresh), Some(&link(vec![signs(2020)])));
    }

    #[test]
    fn a_refused_part_leaves_the_whole_batch_unapplied() {
        let mut g = <graph::MGraph<(), ER> as Subject>::built(vec![(dir::E::D(0, 1), link(vec![signs(2020)]))]);

        let result = g.modify(vec![include(1, 2, uses("custody")), include(0, 1, signs(2024))]);

        assert!(matches!(result, Err(error::Modify::Apply(Apply::Edge(apply::Edge::PartRejected { .. })))));
        assert_eq!(g.edge_count(), 1);
        assert_eq!(held(&g, 1, 2), None);
        assert_eq!(held(&g, 0, 1), Some(&link(vec![signs(2020)])));
    }

    fn whole(a: Id, b: Id, op: modify::edge::Exist<Link>) -> Node<(), ER> {
        Node::Exist(node::Exist::Bind {
            id: id::N(a),
            op: node::Bind::Ref,
            edges: vec![modify::edge::Edge::Exist { slot: dir::SRC, op, target: exist(b) }],
        })
    }

    fn added(a: Id, b: Id, val: Link) -> Node<(), ER> {
        Node::Exist(node::Exist::Bind {
            id: id::N(a),
            op: node::Bind::Ref,
            edges: vec![modify::edge::Edge::New { slot: dir::SRC, val, target: exist(b) }],
        })
    }

    fn shape<G: Subject>(g: &G) -> (usize, usize, Vec<(id::N, Option<Link>)>) {
        let links = [(0, 1), (1, 2)].into_iter().map(|(a, b)| (id::N(a), held(g, a, b).cloned())).collect();
        (g.node_count(), g.edge_count(), links)
    }

    fn refused_toward_a_missing_node<G: Subject>(toward_99: Node<(), ER>) {
        let mut g = G::built(vec![
            (dir::E::D(0, 1), link(vec![signs(2020)])),
            (dir::E::D(1, 2), link(vec![uses("custody")])),
        ]);
        let before = shape(&g);

        let result = g.attempted(vec![whole(1, 2, modify::edge::Exist::Rem), toward_99]);

        assert!(matches!(
            result,
            Err(error::Modify::Apply(Apply::Node(apply::Node::NotFound(n)))) if n == id::N(99)
        ));
        assert_eq!(shape(&g), before);
        assert_eq!(g.neighbors(id::N(2)).unwrap().count(), 1);
    }

    fn include_toward_a_missing_node_is_refused<G: Subject>() {
        refused_toward_a_missing_node::<G>(include(0, 99, signs(2020)));
    }

    fn whole_link_add_toward_a_missing_node_is_refused<G: Subject>() {
        refused_toward_a_missing_node::<G>(added(0, 99, link(vec![signs(2020)])));
    }

    fn part_and_whole_add_on_one_pair_conflict<G: Subject>() {
        let result = G::built(vec![]).applied(vec![added(0, 1, link(vec![signs(2020)])), include(0, 1, uses("custody"))]);

        assert!(matches!(result, Err(error::Modify::Apply(Apply::Edge(apply::Edge::PartConflict(_, _))))));
    }

    fn part_and_whole_swap_on_one_pair_conflict<G: Subject>() {
        let g = G::built(vec![(dir::E::D(0, 1), link(vec![signs(2020)]))]);
        let swap = modify::edge::Exist::Bind(modify::edge::Bind::Swap(link(vec![signs(1999)])));

        let result = g.applied(vec![whole(0, 1, swap), exclude(0, 1, RelKind::Signs)]);

        assert!(matches!(result, Err(error::Modify::Apply(Apply::Edge(apply::Edge::PartConflict(_, _))))));
    }

    fn include_exclude_include_on_a_vacant_pair_adds_only_the_last<G: Subject>() {
        let (g, m) = G::built(vec![])
            .applied(vec![include(0, 1, signs(2020)), exclude(0, 1, RelKind::Signs), include(0, 1, signs(2024))])
            .unwrap();

        assert_eq!(m.added_edges.len(), 1);
        assert!(m.swapped_edge_vals.is_empty() && m.removed_edges.is_empty());
        assert_eq!(held(&g, 0, 1), Some(&link(vec![signs(2024)])));
    }

    fn include_exclude_include_on_a_held_pair_swaps<G: Subject>() {
        let g = G::built(vec![(dir::E::D(0, 1), link(vec![uses("custody")]))]);

        let (g, m) = g
            .applied(vec![include(0, 1, signs(2020)), exclude(0, 1, RelKind::Signs), include(0, 1, signs(2024))])
            .unwrap();

        assert_eq!(m.swapped_edge_vals.len(), 1);
        assert!(m.added_edges.is_empty() && m.removed_edges.is_empty());
        assert_eq!(held(&g, 0, 1), Some(&link(vec![signs(2024), uses("custody")])));
    }

    fn a_batch_that_returns_to_the_held_value_writes_nothing<G: Subject>() {
        let g = G::built(vec![(dir::E::D(0, 1), link(vec![signs(2020)]))]);

        let (g, m) = g.applied(vec![include(0, 1, uses("custody")), exclude(0, 1, RelKind::Uses)]).unwrap();

        assert!(untouched(&m));
        assert_eq!(held(&g, 0, 1), Some(&link(vec![signs(2020)])));
    }

    fn include_on_a_descending_held_pair_joins_the_link<G: Subject>() {
        let g = G::built(vec![(dir::E::D(2, 1), link(vec![signs(2020)]))]);

        let (g, m) = g.applied(vec![include(2, 1, uses("custody"))]).unwrap();

        assert_eq!(m.swapped_edge_vals.len(), 1);
        assert!(m.added_edges.is_empty());
        assert_eq!(g.edge_count(), 1);
        assert_eq!(held(&g, 2, 1), Some(&link(vec![signs(2020), uses("custody")])));
    }

    fn exclude_on_a_descending_held_pair_leaves_the_rest<G: Subject>() {
        let g = G::built(vec![(dir::E::D(2, 1), link(vec![signs(2020), uses("custody")]))]);

        let (g, m) = g.applied(vec![exclude(2, 1, RelKind::Signs)]).unwrap();

        assert_eq!(m.swapped_edge_vals.len(), 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(held(&g, 2, 1), Some(&link(vec![uses("custody")])));
    }

    macro_rules! on_both_graphs {
        ($($case:ident),* $(,)?) => {
            mod mgraph {
                $(#[test] fn $case() { super::$case::<crate::graph::MGraph<(), super::ER>>() })*
            }
            mod vgraph {
                $(#[test] fn $case() { super::$case::<crate::graph::VGraph<(), super::ER>>() })*
            }
        };
    }

    on_both_graphs!(
        include_on_an_empty_pair_creates_the_link,
        include_a_second_kind_joins_the_link,
        include_an_equal_part_changes_nothing,
        include_a_different_part_of_a_held_kind_is_rejected,
        exclude_to_remains_replaces_the_value,
        exclude_to_vacant_deletes_the_link,
        exclude_a_kind_the_link_does_not_hold_is_refused,
        exclude_on_a_pair_with_no_link_is_the_missing_edge,
        whole_link_add_keeps_its_duplicate_refusal,
        whole_and_part_ops_on_one_pair_conflict,
        parts_on_one_pair_fold_in_batch_order,
        include_then_exclude_all_in_one_batch_leaves_the_pair_empty,
        include_towards_a_new_node_creates_the_link,
        include_toward_a_missing_node_is_refused,
        whole_link_add_toward_a_missing_node_is_refused,
        part_and_whole_add_on_one_pair_conflict,
        part_and_whole_swap_on_one_pair_conflict,
        include_exclude_include_on_a_vacant_pair_adds_only_the_last,
        include_exclude_include_on_a_held_pair_swaps,
        a_batch_that_returns_to_the_held_value_writes_nothing,
        include_on_a_descending_held_pair_joins_the_link,
        exclude_on_a_descending_held_pair_leaves_the_rest,
    );
}
