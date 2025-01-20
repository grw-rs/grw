use crate::modify::dsl::LocalId;
use crate::modify::Node;
use crate::modify::edge::{self, Edge};
use crate::modify::error::{Apply, apply};
use crate::modify::node::{Bind, Exist, New};
use crate::graph::{self, Graph, EdgeRec};
use crate::{NR, id};
use std::collections::BTreeSet;
use rustc_hash::{FxHashMap, FxHashSet};

pub struct Modification<NV, ER: graph::Edge> {
    pub new_node_ids: FxHashMap<LocalId, id::N>,
    pub added_edges: Vec<(id::E, id::N, id::N, ER::Slot)>,
    pub removed_nodes: Vec<(id::N, NV)>,
    pub removed_edges: Vec<(id::E, NR<id::N>, ER::Slot, ER::Val)>,
    pub swapped_node_vals: Vec<(id::N, NV)>,
    pub swapped_edge_vals: Vec<(id::E, NR<id::N>, ER::Slot, ER::Val)>,
}

impl<NV, ER: graph::Edge> Default for Modification<NV, ER> {
    fn default() -> Self {
        Self {
            new_node_ids: FxHashMap::default(),
            added_edges: Vec::new(),
            removed_nodes: Vec::new(),
            removed_edges: Vec::new(),
            swapped_node_vals: Vec::new(),
            swapped_edge_vals: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
enum NewRef {
    Named(LocalId),
    Anon(id::N),
}

#[derive(Clone, Copy)]
enum Endpoint {
    New(NewRef),
    Exist(id::N),
}

struct AddEdge<ER: graph::Edge> {
    source: Endpoint,
    slot: ER::Slot,
    val: ER::Val,
    target: Endpoint,
}

struct SwapEdge<ER: graph::Edge> {
    source: Endpoint,
    slot: ER::Slot,
    val: ER::Val,
    target: Endpoint,
}

struct RemoveEdge<ER: graph::Edge> {
    source: Endpoint,
    slot: ER::Slot,
    target: Endpoint,
}

struct FlatOps<NV, ER: graph::Edge> {
    new_named: Vec<(LocalId, NV)>,
    new_anon: Vec<(id::N, NV)>,
    exist_nodes: Vec<(id::N, Option<NV>)>,
    add_edges: Vec<AddEdge<ER>>,
    swap_edges: Vec<SwapEdge<ER>>,
    remove_edges: Vec<RemoveEdge<ER>>,
    remove_nodes: Vec<id::N>,
}

impl<NV, ER: graph::Edge> Default for FlatOps<NV, ER> {
    fn default() -> Self {
        Self {
            new_named: Vec::new(),
            new_anon: Vec::new(),
            exist_nodes: Vec::new(),
            add_edges: Vec::new(),
            swap_edges: Vec::new(),
            remove_edges: Vec::new(),
            remove_nodes: Vec::new(),
        }
    }
}

fn flatten_node<NV, ER: graph::Edge>(
    node: Node<NV, ER>,
    flat: &mut FlatOps<NV, ER>,
    alloc: &mut impl FnMut() -> id::N,
) -> Endpoint {
    match node {
        Node::New(new, edges) => {
            let ep = match new {
                New::Add {
                    id: Some(local),
                    val,
                } => {
                    flat.new_named.push((local, val));
                    Endpoint::New(NewRef::Named(local))
                }
                New::Add { id: None, val } => {
                    let anon_id = alloc();
                    flat.new_anon.push((anon_id, val));
                    Endpoint::New(NewRef::Anon(anon_id))
                }
                New::Ref { id } => Endpoint::New(NewRef::Named(id)),
            };
            edges
                .into_iter()
                .for_each(|edge| flatten_edge(edge, ep, flat, alloc));
            ep
        }
        Node::Exist(exist) => match exist {
            Exist::Bind { id, op, edges } => {
                match op {
                    Bind::Pass => {
                        flat.exist_nodes.push((id, None));
                    }
                    Bind::Swap(v) => {
                        flat.exist_nodes.push((id, Some(v)));
                    }
                    Bind::Ref => {}
                }
                let ep = Endpoint::Exist(id);
                edges
                    .into_iter()
                    .for_each(|edge| flatten_edge(edge, ep, flat, alloc));
                ep
            }
            Exist::Rem { id } => {
                flat.remove_nodes.push(id);
                Endpoint::Exist(id)
            }
        },
        Node::Translated(_) => {
            unimplemented!("T nodes require .bind() before apply")
        }
    }
}

fn flatten_edge<NV, ER: graph::Edge>(
    edge: Edge<NV, ER>,
    source: Endpoint,
    flat: &mut FlatOps<NV, ER>,
    alloc: &mut impl FnMut() -> id::N,
) {
    match edge {
        Edge::New { slot, val, target } => {
            let target_ep = flatten_node(target, flat, alloc);
            flat.add_edges.push(AddEdge {
                source,
                slot,
                val,
                target: target_ep,
            });
        }
        Edge::Exist { slot, op, target } => {
            let target_ep = flatten_node(target, flat, alloc);
            match op {
                edge::Exist::Bind(edge::Bind::Swap(v)) => flat.swap_edges.push(SwapEdge {
                    source,
                    slot,
                    val: v,
                    target: target_ep,
                }),
                edge::Exist::Bind(edge::Bind::Pass) => {}
                edge::Exist::Rem => flat.remove_edges.push(RemoveEdge {
                    source,
                    slot,
                    target: target_ep,
                }),
            }
        }
    }
}

fn resolve_endpoint(ep: Endpoint, local_map: &FxHashMap<LocalId, id::N>) -> id::N {
    match ep {
        Endpoint::New(NewRef::Named(local)) => local_map[&local],
        Endpoint::New(NewRef::Anon(id)) => id,
        Endpoint::Exist(id) => id,
    }
}

impl<NV: Sync, ER: graph::Edge> Graph<NV, ER> {
    pub(crate) fn apply_ops(
        &mut self,
        ops: Vec<Node<NV, ER>>,
    ) -> Result<Modification<NV, ER>, Apply> {
        let mut flat = FlatOps::default();

        {
            let mut alloc = || {
                id::N(
                    self.nodes
                        .free_ids
                        .pop_id()
                        .expect("exhausted node id space"),
                )
            };
            ops.into_iter().for_each(|op| {
                flatten_node(op, &mut flat, &mut alloc);
            });
        }

        flat.exist_nodes
            .iter()
            .find(|(nid, _)| !self.nodes.has(*nid))
            .map_or(Ok(()), |(nid, _)| {
                Err(Apply::Node(apply::Node::NotFound(*nid)))
            })?;

        flat.remove_nodes
            .iter()
            .copied()
            .find(|nid| !self.nodes.has(*nid))
            .map_or(Ok(()), |nid| Err(Apply::Node(apply::Node::NotFound(nid))))?;

        let mut local_map = FxHashMap::default();
        flat.new_named.iter().for_each(|(local, _)| {
            local_map.entry(*local).or_insert_with(|| {
                id::N(
                    self.nodes
                        .free_ids
                        .pop_id()
                        .expect("exhausted node id space"),
                )
            });
        });

        flat.remove_nodes
            .iter()
            .copied()
            .find(|&remove_id| {
                flat.remove_edges
                    .iter()
                    .map(|e| {
                        (
                            resolve_endpoint(e.source, &local_map),
                            resolve_endpoint(e.target, &local_map),
                        )
                    })
                    .chain(flat.swap_edges.iter().map(|e| {
                        (
                            resolve_endpoint(e.source, &local_map),
                            resolve_endpoint(e.target, &local_map),
                        )
                    }))
                    .any(|(src, tgt)| src == remove_id || tgt == remove_id)
            })
            .map_or(Ok(()), |nid| {
                Err(Apply::Node(apply::Node::CascadeConflict(nid)))
            })?;

        let mut seen_swaps = BTreeSet::new();
        for swap_edge in &flat.swap_edges {
            let source_id = resolve_endpoint(swap_edge.source, &local_map);
            let target_id = resolve_endpoint(swap_edge.target, &local_map);
            let def = ER::edge(swap_edge.slot, (*source_id, *target_id));
            let (nr, stored_slot): (NR<id::N>, ER::Slot) = def.into();
            if !seen_swaps.insert((nr, stored_slot)) {
                return Err(Apply::Edge(apply::Edge::SwapConflict(source_id, target_id)));
            }
        }

        let mut result = Modification {
            new_node_ids: local_map.clone(),
            ..Default::default()
        };

        let mut inserted_locals = FxHashSet::default();
        flat.new_named
            .into_iter()
            .filter(|(local, _)| inserted_locals.insert(*local))
            .for_each(|(local, val)| {
                let real_id = local_map[&local];
                self.nodes.insert(real_id, graph::Node::new(val));
                self.degrees_insert(real_id, 0);
            });

        flat.new_anon.into_iter().for_each(|(anon_id, val)| {
            self.nodes.insert(anon_id, graph::Node::new(val));
            self.degrees_insert(anon_id, 0);
        });

        let mut swapped_exist = FxHashSet::default();
        for (nid, val) in flat.exist_nodes {
            if let Some(new_val) = val
                && swapped_exist.insert(nid)
            {
                let node = self
                    .nodes
                    .get_node_mut(nid)
                    .expect("exist node verified present");
                let old_val = std::mem::replace(&mut node.val, new_val);
                result.swapped_node_vals.push((nid, old_val));
            }
        }

        flat.remove_edges.iter().try_for_each(|remove_edge| {
            let source_id = resolve_endpoint(remove_edge.source, &local_map);
            let target_id = resolve_endpoint(remove_edge.target, &local_map);
            let def = ER::edge(remove_edge.slot, (*source_id, *target_id));
            let (nr, stored_slot): (NR<id::N>, ER::Slot) = def.into();
            let n1 = *nr.n1();
            let n2 = *nr.n2();

            let eid = self.nodes.get_node(n1)
                .into_iter()
                .flat_map(|node| node.adj.edges_to(n2))
                .find(|&eid| {
                    self.edges.store[*eid as usize]
                        .as_ref()
                        .map(|r| r.slot == stored_slot)
                        .unwrap_or(false)
                });

            match eid {
                Some(eid) => {
                    let rec = self.edges.store[*eid as usize].take().unwrap();
                    self.edges.count -= 1;
                    self.edges.free_ids.push_id(*eid);
                    result.removed_edges.push((eid, nr, rec.slot, rec.val));

                    let n1_deg_before = self.nodes.get_node(n1).unwrap().adj.len();
                    self.nodes.get_node_mut(n1).unwrap().adj.remove_entry(n2, eid);
                    let n1_deg_after = self.nodes.get_node(n1).unwrap().adj.len();
                    self.degrees_move(n1, n1_deg_before, n1_deg_after);

                    if !nr.is_cycle() {
                        let n2_deg_before = self.nodes.get_node(n2).unwrap().adj.len();
                        self.nodes.get_node_mut(n2).unwrap().adj.remove_entry(n1, eid);
                        let n2_deg_after = self.nodes.get_node(n2).unwrap().adj.len();
                        self.degrees_move(n2, n2_deg_before, n2_deg_after);
                    }

                    Ok(())
                }
                None => Err(Apply::Edge(apply::Edge::NotFound(source_id, target_id))),
            }
        })?;

        flat.swap_edges.into_iter().try_for_each(|swap_edge| {
            let source_id = resolve_endpoint(swap_edge.source, &local_map);
            let target_id = resolve_endpoint(swap_edge.target, &local_map);
            let def = ER::edge(swap_edge.slot, (*source_id, *target_id));
            let (nr, stored_slot): (NR<id::N>, ER::Slot) = def.into();
            let n1 = *nr.n1();
            let n2 = *nr.n2();

            let eid = self.nodes.get_node(n1)
                .into_iter()
                .flat_map(|node| node.adj.edges_to(n2))
                .find(|&eid| {
                    self.edges.store[*eid as usize]
                        .as_ref()
                        .map(|r| r.slot == stored_slot)
                        .unwrap_or(false)
                });

            match eid {
                Some(eid) => {
                    let rec = self.edges.store[*eid as usize].as_mut().unwrap();
                    let old_val = std::mem::replace(&mut rec.val, swap_edge.val);
                    result.swapped_edge_vals.push((eid, nr, stored_slot, old_val));
                    Ok(())
                }
                None => Err(Apply::Edge(apply::Edge::NotFound(source_id, target_id))),
            }
        })?;

        flat.add_edges.into_iter().try_for_each(|add_edge| {
            let source_id = resolve_endpoint(add_edge.source, &local_map);
            let target_id = resolve_endpoint(add_edge.target, &local_map);
            let def = ER::edge(add_edge.slot, (*source_id, *target_id));
            let (nr, stored_slot): (NR<id::N>, ER::Slot) = def.into();
            let n1 = *nr.n1();
            let n2 = *nr.n2();

            if self.edges_between(n1, n2).any(|(s, _)| s == stored_slot) {
                return Err(Apply::Edge(apply::Edge::Duplicate(source_id, target_id)));
            }

            let eid_raw = self.edges.free_ids.pop_id().expect("exhausted edge id space");
            let eid = id::E(eid_raw);
            let idx = eid_raw as usize;
            if idx >= self.edges.store.len() {
                self.edges.store.resize_with(idx + 1, || None);
            }
            self.edges.store[idx] = Some(EdgeRec { n1, n2, slot: stored_slot, val: add_edge.val });
            self.edges.count += 1;
            result.added_edges.push((eid, n1, n2, stored_slot));

            let n1_deg_before = self.nodes.get_node(n1).unwrap().adj.len();
            self.nodes.get_node_mut(n1).unwrap().adj.insert(n2, eid);
            let n1_deg_after = self.nodes.get_node(n1).unwrap().adj.len();
            self.degrees_move(n1, n1_deg_before, n1_deg_after);

            if !nr.is_cycle() {
                let n2_deg_before = self.nodes.get_node(n2).unwrap().adj.len();
                self.nodes.get_node_mut(n2).unwrap().adj.insert(n1, eid);
                let n2_deg_after = self.nodes.get_node(n2).unwrap().adj.len();
                self.degrees_move(n2, n2_deg_before, n2_deg_after);
            }

            Ok(())
        })?;

        for nid in flat.remove_nodes {
            let node = self.nodes.get_node(nid).unwrap();
            let node_deg = node.adj.len();
            let adj_entries: Vec<(id::N, id::E)> = node.adj.entries_iter().collect();

            for (neighbor, eid) in adj_entries {
                if let Some(rec) = self.edges.store[*eid as usize].take() {
                    self.edges.count -= 1;
                    self.edges.free_ids.push_id(*eid);
                    let nr: NR<id::N> = (rec.n1, rec.n2).into();
                    result.removed_edges.push((eid, nr, rec.slot, rec.val));
                }

                if neighbor != nid {
                    let nbr_deg_before = self.nodes.get_node(neighbor).unwrap().adj.len();
                    self.nodes.get_node_mut(neighbor).unwrap().adj.remove_entry(nid, eid);
                    let nbr_deg_after = self.nodes.get_node(neighbor).unwrap().adj.len();
                    self.degrees_move(neighbor, nbr_deg_before, nbr_deg_after);
                }
            }

            let removed_node = self.nodes.remove(nid).unwrap();
            self.nodes.free_ids.push_id(*nid);
            self.degrees_remove(nid, node_deg);
            result.removed_nodes.push((nid, removed_node.val));
        }

        Ok(result)
    }
}
