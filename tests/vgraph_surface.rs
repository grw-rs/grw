use grw::graph::{self, Graph as _, MGraph, VUndir0, VUndir};
use grw::{Id, modify, vgraph};

#[test]
fn vgraph_macro_and_aliases() {
    let g: VUndir0 = vgraph![N(0) ^ (N(1) ^ (N(2) ^ n(0)))].unwrap();
    assert_eq!(g.node_count(), 3);
    assert_eq!(g.edge_count(), 3);
    assert_eq!(g.version(), 0);
}

#[test]
fn vgraph_load_roundtrip() {
    let m: graph::MUndir0 = grw::mgraph![N(0) ^ N(1)].unwrap();
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("grw_vload_test_{}.grw", pid));
    m.save(&dir).unwrap();
    let v = VUndir0::load(&dir).unwrap();
    assert_eq!(v.node_count(), 2);
    assert_eq!(v.edge_count(), 1);
    std::fs::remove_file(&dir).unwrap();
}

fn tombstoned_vgraph() -> VUndir<u32, u32> {
    let v: VUndir<u32, u32> = vgraph![
        N(0).val(10u32) & E().val(100u32) ^ N(1).val(20u32),
        n(1) & E().val(200u32) ^ N(2).val(30u32),
        n(2) & E().val(300u32) ^ N(3).val(40u32)
    ]
    .unwrap();
    let (v, _) = v.modify(modify![x(1) & !e() ^ x(2)]).unwrap();
    let (v, _) = v.modify(modify![!X(3)]).unwrap();
    let (v, _) = v.modify(modify![N(9).val(90u32) & E().val(500u32) ^ x(0)]).unwrap();
    let (v, _) = v.modify(modify![N(11).val(110u32)]).unwrap();
    let (v, _) = v.modify(modify![!X(1)]).unwrap();
    v
}

fn nodes_view<NV: Clone + PartialEq, E: graph::Edge, G: graph::Graph<NV, E>>(
    g: &G,
) -> Vec<(Id, NV, Vec<Id>)> {
    let mut v: Vec<(Id, NV, Vec<Id>)> = g
        .iter_nodes()
        .map(|(n, val, adj)| {
            let mut nbs: Vec<Id> = adj.map(|nb| *nb).collect();
            nbs.sort_unstable();
            (*n, val.clone(), nbs)
        })
        .collect();
    v.sort_by_key(|(n, _, _)| *n);
    v
}

fn edges_view<NV, E: graph::Edge, G: graph::Graph<NV, E>>(g: &G) -> Vec<(Id, Id, E::Slot, E::Val)>
where
    E::Val: Clone + Ord,
{
    let mut v: Vec<(Id, Id, E::Slot, E::Val)> =
        g.iter_edges().map(|(a, b, slot, val)| (*a, *b, slot, val.clone())).collect();
    v.sort();
    v
}

fn assert_same_view<E: graph::Edge, A: graph::Graph<u32, E>, B: graph::Graph<u32, E>>(
    a: &A,
    b: &B,
) where
    E::Val: Clone + Ord + std::fmt::Debug,
    E::Slot: std::fmt::Debug,
{
    assert_eq!(a.node_count(), b.node_count());
    assert_eq!(a.edge_count(), b.edge_count());
    assert_eq!(a.id_space_len(), b.id_space_len());
    assert_eq!(nodes_view(a), nodes_view(b));
    assert_eq!(edges_view(a), edges_view(b));
}

#[test]
fn tombstoned_vgraph_shape() {
    let v = tombstoned_vgraph();
    assert_eq!(v.node_count(), 4);
    assert_eq!(v.id_space_len(), 5);
    assert_eq!(v.edge_count(), 1);
    let ids: Vec<Id> = v.iter_node_ids().map(|n| *n).collect();
    assert_eq!(ids, vec![0, 2, 3, 4]);
}

#[test]
fn vgraph_to_mgraph_and_back() {
    let v = tombstoned_vgraph();
    let m = v.to_mgraph();
    assert_same_view(&v, &m);

    let m_ids: Vec<Id> = m.iter_node_ids().map(|n| *n).collect();
    assert_eq!(m_ids, vec![0, 2, 3, 4]);

    let v2 = graph::VGraph::from_mgraph(&m);
    assert_same_view(&v, &v2);
    assert_eq!(v2.version(), 0);
}

#[test]
fn vgraph_save_loads_into_both_representations() {
    let v = tombstoned_vgraph();
    let path = std::env::temp_dir().join(format!("grw_vsave_{}.grw", std::process::id()));
    v.save(&path).unwrap();

    let m: graph::MUndir<u32, u32> = MGraph::load(&path).unwrap();
    assert_same_view(&v, &m);

    let v2: VUndir<u32, u32> = VUndir::load(&path).unwrap();
    assert_same_view(&v, &v2);
    assert_eq!(v2.version(), 0);

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn from_graph_free_list_reuses_vacant_slot() {
    let mut m: graph::MUndir0 = grw::mgraph![N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3)].unwrap();
    m.modify(modify![!X(1)]).unwrap();
    assert_eq!(m.iter_node_ids().map(|n| *n).collect::<Vec<Id>>(), vec![0, 2, 3]);
    assert_eq!(m.id_space_len(), 4);

    let mut rebuilt: graph::MUndir0 = MGraph::from_graph(&m);
    assert_eq!(rebuilt.iter_node_ids().map(|n| *n).collect::<Vec<Id>>(), vec![0, 2, 3]);
    assert_eq!(rebuilt.id_space_len(), 4);

    rebuilt.modify(modify![N(9)]).unwrap();
    assert_eq!(rebuilt.iter_node_ids().map(|n| *n).collect::<Vec<Id>>(), vec![0, 1, 2, 3]);
    assert_eq!(rebuilt.id_space_len(), 4);

    let mut organic = m;
    organic.modify(modify![N(9)]).unwrap();
    assert_eq!(organic.iter_node_ids().map(|n| *n).collect::<Vec<Id>>(), vec![0, 1, 2, 3]);
    assert_eq!(organic.id_space_len(), 4);
}

#[test]
fn to_mgraph_free_list_reuses_vacant_slot() {
    let v: VUndir0 = vgraph![N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3)].unwrap();
    let (v, _) = v.modify(modify![!X(1)]).unwrap();
    assert_eq!(v.iter_node_ids().map(|n| *n).collect::<Vec<Id>>(), vec![0, 2, 3]);
    assert_eq!(v.id_space_len(), 4);

    let mut m = v.to_mgraph();
    assert_eq!(m.iter_node_ids().map(|n| *n).collect::<Vec<Id>>(), vec![0, 2, 3]);
    assert_eq!(m.id_space_len(), 4);

    m.modify(modify![N(9)]).unwrap();
    assert_eq!(m.iter_node_ids().map(|n| *n).collect::<Vec<Id>>(), vec![0, 1, 2, 3]);
    assert_eq!(m.id_space_len(), 4);

    let (v2, _) = v.modify(modify![N(9)]).unwrap();
    assert_eq!(v2.iter_node_ids().map(|n| *n).collect::<Vec<Id>>(), vec![0, 1, 2, 3]);
    assert_eq!(v2.id_space_len(), 4);
}

#[test]
fn vgraph_save_matches_mgraph_save_of_conversion() {
    let v = tombstoned_vgraph();
    let dir = std::env::temp_dir().join(format!("grw_vsave_cmp_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let from_v = dir.join("from_v.grw");
    let from_m = dir.join("from_m.grw");
    v.save(&from_v).unwrap();
    v.to_mgraph().save(&from_m).unwrap();

    // Real byte-level proof (gap-free gap-free behavior) verified in persist_golden.rs::generic_write_byte_identical_when_gap_free.
    assert_eq!(std::fs::read(&from_v).unwrap(), std::fs::read(&from_m).unwrap());
    std::fs::remove_dir_all(&dir).unwrap();
}
