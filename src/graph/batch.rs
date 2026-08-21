use super::error;
use crate::*;
use crate::graph::EdgeRec;
use std::collections::BTreeSet;

pub fn collect_nodes<NV>(
    nvs: &mut impl Iterator<Item = (Id, NV)>,
) -> Result<Nodes<NV>, error::Node> {
    let mut free_ids = IdSpace::default();
    let mut store: Vec<Option<Node<NV>>> = Vec::new();
    let mut count = 0usize;

    for (id, val) in nvs {
        let idx = id as usize;
        if idx >= store.len() {
            store.resize_with(idx + 1, || None);
        }
        if store[idx].is_some() {
            return Err(error::Node::Duplicate(id::N(id)));
        }
        store[idx] = Some(Node::new(val));
        free_ids.remove_id(id);
        count += 1;
    }

    Ok(Nodes { store, free_ids, count })
}

pub fn collect_nspace_nodes(ns_len: Id) -> Nodes<()> {
    let free_ids = IdSpace::new_starting_at(ns_len);
    let store: Vec<Option<Node<()>>> = (0..ns_len)
        .map(|_| Some(Node::new(())))
        .collect();
    Nodes { store, free_ids, count: ns_len as usize }
}

pub(crate) fn collect_edges<NV, E: Edge>(
    nodes: &mut Nodes<NV>,
    evs: &mut impl Iterator<Item = (E::Def, E::Val)>,
) -> Result<Edges<E>, error::Build<E::Slot>> {
    let mut store: Vec<Option<EdgeRec<E::Slot, E::Val>>> = Vec::new();
    let mut edge_free_ids = IdSpace::default();
    let mut count: usize = 0;
    let mut seen = BTreeSet::new();

    for (e, val) in evs {
        let (rel, slot) = e.into();

        let key = (rel, slot);
        if !seen.insert(key) {
            return Err(error::Edge::Duplicate(rel, slot).into());
        }

        let edge_id = id::E(edge_free_ids.pop_id().unwrap());
        let n1 = *rel.n1();
        let n2 = *rel.n2();

        if !rel.is_cycle() {
            let ns = rel.ns_refs();
            let [ref_a, ref_b] = nodes.get_two_mut(*ns[0], *ns[1]);
            let node_a = ref_a.ok_or(error::Edge::NodeNotFound(*ns[0]))?;
            let node_b = ref_b.ok_or(error::Edge::NodeNotFound(*ns[1]))?;
            node_a.adj.insert(*ns[1], edge_id);
            node_b.adj.insert(*ns[0], edge_id);
        } else {
            let node = nodes.get_node_mut(*rel.n1())
                .ok_or(error::Edge::NodeNotFound(*rel.n1()))?;
            node.adj.insert(n1, edge_id);
        }

        store.push(Some(EdgeRec { n1, n2, slot, val }));
        count += 1;
    }

    Ok(Edges { store, free_ids: edge_free_ids, count })
}

pub(crate) fn collect_derived_nodes_from_edges<E: Edge>(
    evs: &mut impl Iterator<Item = (E::Def, E::Val)>,
) -> Result<(Nodes<()>, Edges<E>), error::Edge<E::Slot>> {
    let mut nodes = Nodes::default();
    let mut store: Vec<Option<EdgeRec<E::Slot, E::Val>>> = Vec::new();
    let mut edge_free_ids = IdSpace::default();
    let mut count: usize = 0;
    let mut seen = BTreeSet::new();

    let insert_node = |nodes: &mut Nodes<()>, n: id::N| {
        if !nodes.has(n) {
            nodes.free_ids.remove_id(*n);
            nodes.insert(n, Node::new(()));
        }
    };

    for (e, val) in evs {
        let (rel, slot) = e.into();

        let key = (rel, slot);
        if !seen.insert(key) {
            return Err(error::Edge::Duplicate(rel, slot));
        }

        let edge_id = id::E(edge_free_ids.pop_id().unwrap());
        let n1 = *rel.n1();
        let n2 = *rel.n2();

        if !rel.is_cycle() {
            let ns = rel.ns_refs();
            ns.iter().for_each(|n| insert_node(&mut nodes, **n));
            let [ref_a, ref_b] = nodes.get_two_mut(*ns[0], *ns[1]);
            let node_a = ref_a.unwrap();
            let node_b = ref_b.unwrap();
            node_a.adj.insert(*ns[1], edge_id);
            node_b.adj.insert(*ns[0], edge_id);
        } else {
            insert_node(&mut nodes, *rel.n1());
            let node = nodes.get_node_mut(*rel.n1()).unwrap();
            node.adj.insert(n1, edge_id);
        }

        store.push(Some(EdgeRec { n1, n2, slot, val }));
        count += 1;
    }

    Ok((nodes, Edges { store, free_ids: edge_free_ids, count }))
}
