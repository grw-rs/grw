use std::sync::Arc;

use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use super::trie::PVec;
use crate::graph::node::Adjacents;
use crate::graph::{self, AdjIds, Edge, HasRel, MGraph, layout};
use crate::modify::apply::{EdgeOp, FlatOps, flatten_node, resolve_endpoint, validate_edge_ops};
use crate::modify::error::{self, Apply, Modify, apply};
use crate::modify::{Fragment, LocalId, Modification};
use crate::{Id, NR, id};

pub(crate) struct VNode<NV> {
    pub(crate) val: NV,
    pub(crate) r#gen: u64,
    pub(crate) adj: Adjacents,
}

impl<NV: Clone> Clone for VNode<NV> {
    fn clone(&self) -> Self {
        VNode { val: self.val.clone(), r#gen: self.r#gen, adj: self.adj.clone() }
    }
}

pub(crate) struct VEdge<E: Edge> {
    pub(crate) r#gen: u64,
    pub(crate) slot_kind: E::Slot,
    pub(crate) val: E::Val,
    pub(crate) lo: id::N,
    pub(crate) hi: id::N,
}

impl<E: Edge> Clone for VEdge<E>
where
    E::Val: Clone,
{
    fn clone(&self) -> Self {
        VEdge {
            r#gen: self.r#gen,
            slot_kind: self.slot_kind,
            val: self.val.clone(),
            lo: self.lo,
            hi: self.hi,
        }
    }
}

/// Free slots, ordered. Allocation takes the lowest free slot, matching
/// `IdSpace::pop_id` on the mutable side so both graphs hand out the same ids.
type FreeSet = PVec<()>;

fn nslot(n: id::N) -> u32 {
    *n as u32
}

fn eslot(e: id::E) -> u32 {
    *e as u32
}

fn nid(slot: u32) -> id::N {
    id::N(slot as Id)
}

fn eid(slot: u32) -> id::E {
    id::E(slot as Id)
}

fn alloc_slot(free: &mut FreeSet, next: &mut u32) -> u32 {
    match free.first() {
        Some(slot) => {
            *free = free.remove(slot);
            slot
        }
        None => {
            let slot = *next;
            *next += 1;
            slot
        }
    }
}

fn release_slot(free: &FreeSet, slot: u32) -> FreeSet {
    free.set(slot, ())
}

fn edges_between_in<'a, NV, E: Edge>(
    nodes: &'a PVec<Arc<VNode<NV>>>,
    edges: &'a PVec<Arc<VEdge<E>>>,
    n1: id::N,
    n2: id::N,
) -> impl Iterator<Item = (E::Slot, &'a E::Val)> + 'a {
    nodes.get(nslot(n1)).into_iter().flat_map(move |node| {
        node.adj.edges_to(n2).map(move |e| {
            let rec = edges.get(eslot(e)).expect("edge record for adjacency entry");
            (rec.slot_kind, &rec.val)
        })
    })
}

fn edge_id_with_slot<NV, E: Edge>(
    nodes: &PVec<Arc<VNode<NV>>>,
    edges: &PVec<Arc<VEdge<E>>>,
    n1: id::N,
    n2: id::N,
    slot: E::Slot,
) -> Option<id::E> {
    nodes
        .get(nslot(n1))
        .into_iter()
        .flat_map(|node| node.adj.edges_to(n2))
        .find(|&e| edges.get(eslot(e)).map(|r| r.slot_kind == slot).unwrap_or(false))
}

pub struct VGraph<NV, E: Edge> {
    nodes: PVec<Arc<VNode<NV>>>,
    edges: PVec<Arc<VEdge<E>>>,
    node_free: FreeSet,
    edge_free: FreeSet,
    next_node: u32,
    next_edge: u32,
    version: u64,
}

impl<NV, E: Edge> Clone for VGraph<NV, E> {
    fn clone(&self) -> Self {
        VGraph {
            nodes: self.nodes.clone(),
            edges: self.edges.clone(),
            node_free: self.node_free.clone(),
            edge_free: self.edge_free.clone(),
            next_node: self.next_node,
            next_edge: self.next_edge,
            version: self.version,
        }
    }
}

impl<NV, E: Edge> VGraph<NV, E> {
    pub fn new() -> Self {
        VGraph {
            nodes: PVec::new(),
            edges: PVec::new(),
            node_free: FreeSet::new(),
            edge_free: FreeSet::new(),
            next_node: 0,
            next_edge: 0,
            version: 0,
        }
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn node_gen(&self, n: id::N) -> Option<u64> {
        self.nodes.get(nslot(n)).map(|node| node.r#gen)
    }

    pub fn stable_id(&self, n: id::N) -> Option<(u32, u64)> {
        self.nodes.get(nslot(n)).map(|node| (nslot(n), node.r#gen))
    }

    pub fn edge_gen(&self, e: id::E) -> Option<u64> {
        self.edges.get(eslot(e)).map(|rec| rec.r#gen)
    }
}

impl<NV: Clone, E: Edge> VGraph<NV, E>
where
    E::Val: Clone,
{
    pub fn from_mgraph(g: &MGraph<NV, E>) -> Self {
        let mut nodes = PVec::new();
        for (n, node) in g.nodes.nodes_iter() {
            nodes = nodes.set(
                nslot(n),
                Arc::new(VNode { val: node.val.clone(), r#gen: 0, adj: node.adj.clone() }),
            );
        }

        let mut edges = PVec::new();
        for (i, rec) in g.edges.store.iter().enumerate() {
            let Some(rec) = rec else { continue };
            edges = edges.set(
                i as u32,
                Arc::new(VEdge {
                    r#gen: 0,
                    slot_kind: rec.slot,
                    val: rec.val.clone(),
                    lo: rec.n1,
                    hi: rec.n2,
                }),
            );
        }

        let next_node = g.nodes.store.len() as u32;
        let next_edge = g.edges.store.len() as u32;

        let mut node_free = FreeSet::new();
        for slot in 0..next_node {
            if nodes.get(slot).is_none() {
                node_free = release_slot(&node_free, slot);
            }
        }
        let mut edge_free = FreeSet::new();
        for slot in 0..next_edge {
            if edges.get(slot).is_none() {
                edge_free = release_slot(&edge_free, slot);
            }
        }

        VGraph { nodes, edges, node_free, edge_free, next_node, next_edge, version: 0 }
    }

    /// Node ids survive; edge ids are freshly assigned — see
    /// `MGraph::from_graph`.
    pub fn to_mgraph(&self) -> MGraph<NV, E> {
        MGraph::from_graph(self)
    }

    pub fn load(path: &Path) -> io::Result<Self>
    where
        NV: ::serde::de::DeserializeOwned + layout::Val,
        E::Slot: ::serde::de::DeserializeOwned,
        E::Val: ::serde::de::DeserializeOwned + layout::Val,
    {
        MGraph::load(path).map(|g| Self::from_mgraph(&g))
    }

    pub fn save(&self, path: &Path) -> io::Result<()>
    where
        NV: ::serde::Serialize + layout::Val,
        E::Slot: ::serde::Serialize,
        E::Val: ::serde::Serialize + layout::Val,
    {
        graph::persist::save_graph(self, path)
    }

    pub fn modify(
        &self,
        ops: Vec<crate::modify::Node<NV, E>>,
    ) -> Result<(Self, Modification<NV, E>), Modify> {
        let version = self.version + 1;
        self.modify_versioned(ops, version)
    }

    pub fn modify_versioned(
        &self,
        ops: Vec<crate::modify::Node<NV, E>>,
        version: u64,
    ) -> Result<(Self, Modification<NV, E>), Modify> {
        if version <= self.version {
            return Err(error::Version::NotMonotonic {
                current: self.version,
                requested: version,
            }
            .into());
        }
        let fragment = Fragment::new(ops).validate()?;
        Ok(self.clone().apply_ops(fragment.ops, version)?)
    }

    fn apply_ops(
        self,
        ops: Vec<crate::modify::Node<NV, E>>,
        version: u64,
    ) -> Result<(Self, Modification<NV, E>), Apply> {
        let VGraph {
            mut nodes,
            mut edges,
            mut node_free,
            mut edge_free,
            mut next_node,
            mut next_edge,
            version: _,
        } = self;

        let mut flat: FlatOps<NV, E> = FlatOps::default();

        {
            let mut alloc = || nid(alloc_slot(&mut node_free, &mut next_node));
            ops.into_iter().for_each(|op| {
                flatten_node(op, &mut flat, &mut alloc);
            });
        }

        flat.exist_nodes
            .iter()
            .find(|(nid, _)| nodes.get(nslot(*nid)).is_none())
            .map_or(Ok(()), |(nid, _)| Err(Apply::Node(apply::Node::NotFound(*nid))))?;

        flat.remove_nodes
            .iter()
            .copied()
            .find(|nid| nodes.get(nslot(*nid)).is_none())
            .map_or(Ok(()), |nid| Err(Apply::Node(apply::Node::NotFound(nid))))?;

        let mut local_map: FxHashMap<LocalId, id::N> = FxHashMap::default();
        flat.new_named.iter().for_each(|(local, _)| {
            local_map
                .entry(*local)
                .or_insert_with(|| nid(alloc_slot(&mut node_free, &mut next_node)));
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
            .map_or(Ok(()), |nid| Err(Apply::Node(apply::Node::CascadeConflict(nid))))?;

        let mut seen_swaps = BTreeSet::new();
        for swap_edge in &flat.swap_edges {
            let key = swap_edge.key(&local_map);
            if !seen_swaps.insert((key.nr, key.slot)) {
                return Err(Apply::Edge(apply::Edge::SwapConflict(key.source, key.target)));
            }
        }

        validate_edge_ops(&flat, &local_map, |n1, n2, slot| {
            edges_between_in(&nodes, &edges, n1, n2).filter(|(s, _)| *s == slot).count()
        })?;

        let mut result =
            Modification { new_node_ids: local_map.clone(), ..Default::default() };

        let mut inserted_locals = FxHashSet::default();
        flat.new_named
            .into_iter()
            .filter(|(local, _)| inserted_locals.insert(*local))
            .for_each(|(local, val)| {
                let real_id = local_map[&local];
                nodes = nodes.set(
                    nslot(real_id),
                    Arc::new(VNode { val, r#gen: version, adj: Adjacents::new() }),
                );
            });

        flat.new_anon.into_iter().for_each(|(anon_id, val)| {
            nodes = nodes.set(
                nslot(anon_id),
                Arc::new(VNode { val, r#gen: version, adj: Adjacents::new() }),
            );
        });

        let mut swapped_exist = FxHashSet::default();
        for (nid, val) in flat.exist_nodes {
            if let Some(new_val) = val
                && swapped_exist.insert(nid)
            {
                let mut node = clone_node(&nodes, nid);
                let old_val = std::mem::replace(&mut node.val, new_val);
                nodes = nodes.set(nslot(nid), Arc::new(node));
                result.swapped_node_vals.push((nid, old_val));
            }
        }

        for remove_edge in &flat.remove_edges {
            let key = remove_edge.key(&local_map);
            let (nr, stored_slot) = (key.nr, key.slot);
            let n1 = key.n1();
            let n2 = key.n2();

            let e = edge_id_with_slot(&nodes, &edges, n1, n2, stored_slot)
                .expect("edge presence decided by validate_edge_ops");

            let rec = edges.get(eslot(e)).expect("edge id just resolved").clone();
            edges = edges.remove(eslot(e));
            edge_free = release_slot(&edge_free, eslot(e));
            result.removed_edges.push((e, nr, rec.slot_kind, rec.val.clone()));

            let mut node = clone_node(&nodes, n1);
            node.adj.remove_entry(n2, e);
            nodes = nodes.set(nslot(n1), Arc::new(node));

            if !nr.is_cycle() {
                let mut node = clone_node(&nodes, n2);
                node.adj.remove_entry(n1, e);
                nodes = nodes.set(nslot(n2), Arc::new(node));
            }
        }

        for swap_edge in flat.swap_edges {
            let key = swap_edge.key(&local_map);
            let (nr, stored_slot) = (key.nr, key.slot);
            let n1 = key.n1();
            let n2 = key.n2();

            let e = edge_id_with_slot(&nodes, &edges, n1, n2, stored_slot)
                .expect("edge presence decided by validate_edge_ops");

            let mut rec = (**edges.get(eslot(e)).expect("edge id just resolved")).clone();
            let old_val = std::mem::replace(&mut rec.val, swap_edge.val);
            edges = edges.set(eslot(e), Arc::new(rec));
            result.swapped_edge_vals.push((e, nr, stored_slot, old_val));
        }

        for add_edge in flat.add_edges {
            let key = add_edge.key(&local_map);
            let (nr, stored_slot) = (key.nr, key.slot);
            let n1 = key.n1();
            let n2 = key.n2();

            let e = eid(alloc_slot(&mut edge_free, &mut next_edge));
            edges = edges.set(
                eslot(e),
                Arc::new(VEdge {
                    r#gen: version,
                    slot_kind: stored_slot,
                    val: add_edge.val,
                    lo: n1,
                    hi: n2,
                }),
            );
            result.added_edges.push((e, n1, n2, stored_slot));

            let mut node = clone_node(&nodes, n1);
            node.adj.insert(n2, e);
            nodes = nodes.set(nslot(n1), Arc::new(node));

            if !nr.is_cycle() {
                let mut node = clone_node(&nodes, n2);
                node.adj.insert(n1, e);
                nodes = nodes.set(nslot(n2), Arc::new(node));
            }
        }

        for nid in flat.remove_nodes {
            let node = nodes.get(nslot(nid)).expect("remove node verified present").clone();
            let adj_entries: Vec<(id::N, id::E)> = node.adj.entries_iter().collect();

            for (neighbor, e) in adj_entries {
                if let Some(rec) = edges.get(eslot(e)).cloned() {
                    edges = edges.remove(eslot(e));
                    edge_free = release_slot(&edge_free, eslot(e));
                    let nr: NR<id::N> = (rec.lo, rec.hi).into();
                    result.removed_edges.push((e, nr, rec.slot_kind, rec.val.clone()));
                }

                if neighbor != nid {
                    let mut nbr = clone_node(&nodes, neighbor);
                    nbr.adj.remove_entry(nid, e);
                    nodes = nodes.set(nslot(neighbor), Arc::new(nbr));
                }
            }

            nodes = nodes.remove(nslot(nid));
            node_free = release_slot(&node_free, nslot(nid));
            result.removed_nodes.push((nid, node.val.clone()));
        }

        let g = VGraph { nodes, edges, node_free, edge_free, next_node, next_edge, version };
        Ok((g, result))
    }
}

fn clone_node<NV: Clone>(nodes: &PVec<Arc<VNode<NV>>>, n: id::N) -> VNode<NV> {
    (**nodes.get(nslot(n)).expect("node present")).clone()
}

impl<NV, E: Edge> graph::Graph<NV, E> for VGraph<NV, E> {
    type NodeAdj<'a>
        = AdjIds<'a>
    where
        Self: 'a;

    #[inline(always)]
    fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[inline(always)]
    fn edge_count(&self) -> usize {
        self.edges.len()
    }

    #[inline(always)]
    fn id_space_len(&self) -> usize {
        self.next_node as usize
    }

    #[inline(always)]
    fn has_node(&self, n: id::N) -> bool {
        self.nodes.get(nslot(n)).is_some()
    }

    #[inline(always)]
    fn node_val(&self, n: id::N) -> Option<&NV> {
        self.nodes.get(nslot(n)).map(|node| &node.val)
    }

    #[inline(always)]
    fn degree(&self, n: id::N) -> Option<Id> {
        self.nodes.get(nslot(n)).map(|node| node.adj.len())
    }

    #[inline(always)]
    fn neighbors<'a>(
        &'a self,
        n: id::N,
    ) -> Option<impl Iterator<Item = (id::N, E::Slot, &'a E::Val)> + 'a>
    where
        E::Val: 'a,
    {
        let node = self.nodes.get(nslot(n))?;
        let edges = &self.edges;
        Some(node.adj.entries_iter().map(move |(nb, e)| {
            let rec = edges.get(eslot(e)).expect("edge record for adjacency entry");
            let slot =
                if n == rec.lo { rec.slot_kind } else { E::reverse_slot(rec.slot_kind) };
            (nb, slot, &rec.val)
        }))
    }

    #[inline(always)]
    fn neighbor_ids(&self, n: id::N) -> Option<AdjIds<'_>> {
        self.nodes.get(nslot(n)).map(|node| node.adj.iter())
    }

    #[inline(always)]
    fn is_adjacent(&self, n1: Id, n2: Id) -> bool {
        self.nodes
            .get(nslot(id::N(n1)))
            .map(|node| node.adj.contains(id::N(n2)))
            .unwrap_or(false)
    }

    #[inline(always)]
    fn get_edge_val(&self, e: E::Def) -> Option<&E::Val> {
        let (nr, slot): (NR<id::N>, E::Slot) = e.into();
        edges_between_in(&self.nodes, &self.edges, *nr.n1(), *nr.n2())
            .find(|(s, _)| *s == slot)
            .map(|(_, v)| v)
    }

    #[inline(always)]
    fn edges_between<'a>(
        &'a self,
        n1: id::N,
        n2: id::N,
    ) -> impl Iterator<Item = (E::Slot, &'a E::Val)> + 'a
    where
        E::Val: 'a,
    {
        edges_between_in(&self.nodes, &self.edges, n1, n2)
    }

    #[inline(always)]
    fn iter_edges<'a>(&'a self) -> impl Iterator<Item = (id::N, id::N, E::Slot, &'a E::Val)> + 'a
    where
        E::Val: 'a,
    {
        self.edges.iter().map(|(_, rec)| (rec.lo, rec.hi, rec.slot_kind, &rec.val))
    }

    #[inline(always)]
    fn iter_node_ids(&self) -> impl Iterator<Item = id::N> + '_ {
        self.nodes.iter().map(|(slot, _)| nid(slot))
    }

    #[inline(always)]
    fn iter_nodes<'a>(&'a self) -> impl Iterator<Item = (id::N, &'a NV, AdjIds<'a>)> + 'a
    where
        NV: 'a,
    {
        self.nodes.iter().map(|(slot, node)| (nid(slot), &node.val, node.adj.iter()))
    }

    #[inline(always)]
    fn rel(&self, ns: impl Into<NR<id::N>>) -> E::Rel<'_>
    where
        E: HasRel,
    {
        let nr = ns.into();
        E::rel(nr, edges_between_in(&self.nodes, &self.edges, *nr.n1(), *nr.n2()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph as _;
    use crate::graph::edge;

    impl<NV: Clone, E: Edge> VGraph<NV, E> {
        fn iter_view(&self) -> Vec<(u32, u64, NV, Vec<Id>)> {
            let mut view: Vec<(u32, u64, NV, Vec<Id>)> = self
                .nodes
                .iter()
                .map(|(slot, node)| {
                    let mut nbs: Vec<Id> = node.adj.iter().map(|n| *n).collect();
                    nbs.sort_unstable();
                    (slot, node.r#gen, node.val.clone(), nbs)
                })
                .collect();
            view.sort_by_key(|(slot, _, _, _)| *slot);
            view
        }
    }

    impl<NV, E: Edge> VGraph<NV, E> {
        fn node_arc(&self, n: id::N) -> Option<&Arc<VNode<NV>>> {
            self.nodes.get(nslot(n))
        }
    }

    #[test]
    fn build_and_read_via_trait() {
        let g0: VGraph<(), edge::Undir<()>> = VGraph::new();
        let (g1, _) = g0.modify(crate::modify![N(0) ^ N(1), n(1) ^ N(2)]).unwrap();
        assert_eq!(g1.node_count(), 3);
        assert_eq!(g1.edge_count(), 2);
        assert!(g1.is_adjacent(0, 1));
        assert!(!g1.is_adjacent(0, 2));
        assert_eq!(g1.version(), 1);
    }

    #[test]
    fn version_jump_stamps_gens_and_advances() {
        let g0: VGraph<(), edge::Undir<()>> = VGraph::new();
        let (g1, _) = g0.modify(crate::modify![N(0) ^ N(1)]).unwrap();
        assert_eq!(g1.version(), 1);
        let (g7, _) = g1.modify_versioned(crate::modify![N(2) ^ x(0)], 7).unwrap();
        assert_eq!(g7.version(), 7);
        assert_eq!(g7.node_gen(id::N(2)), Some(7));
        assert_eq!(g7.node_gen(id::N(0)), Some(1));
        assert_eq!(g7.edge_gen(id::E(1)), Some(7));
        assert_eq!(g7.edge_gen(id::E(0)), Some(1));
    }

    #[test]
    fn non_monotonic_version_rejected() {
        let g0: VGraph<(), edge::Undir<()>> = VGraph::new();
        let (g1, _) = g0.modify(crate::modify![N(0)]).unwrap();
        let (g5, _) = g1.modify_versioned(crate::modify![N(1)], 5).unwrap();

        let Err(equal) = g5.modify_versioned(crate::modify![N(2)], 5) else {
            panic!("equal version must be rejected")
        };
        assert!(matches!(
            equal,
            Modify::Version(error::Version::NotMonotonic { current: 5, requested: 5 })
        ));

        let Err(lower) = g5.modify_versioned(crate::modify![N(2)], 4) else {
            panic!("lower version must be rejected")
        };
        assert!(matches!(
            lower,
            Modify::Version(error::Version::NotMonotonic { current: 5, requested: 4 })
        ));

        assert_eq!(g5.node_count(), 2);
        assert_eq!(g5.version(), 5);
    }

    #[test]
    fn slot_reuse_bumps_generation() {
        let g0: VGraph<(), edge::Undir<()>> = VGraph::new();
        let (g1, _) = g0.modify(crate::modify![N(0), N(1)]).unwrap();
        let s = g1.stable_id(id::N(1)).unwrap();
        let (g2, _) = g1.modify(crate::modify![!X(1)]).unwrap();
        let (g3, _) = g2.modify(crate::modify![N(9)]).unwrap();
        let s2 = g3.stable_id(id::N(1)).unwrap();
        assert_eq!(s.0, s2.0);
        assert_ne!(s.1, s2.1);
        assert_eq!(s2.1, 3);
    }

    #[test]
    fn old_version_untouched() {
        let g0: VGraph<u32, edge::Undir<()>> = VGraph::new();
        let (g1, _) = g0
            .modify(crate::modify![N(0).val(1u32) ^ N(1).val(2u32), n(1) ^ N(2).val(3u32)])
            .unwrap();
        let snap = g1.iter_view();

        let (g2, _) = g1.modify(crate::modify![X(0).val(99u32)]).unwrap();
        let (g3, _) = g2.modify(crate::modify![x(0) & !e() ^ x(1)]).unwrap();
        let (g4, _) = g3.modify(crate::modify![x(0) ^ x(2)]).unwrap();
        let mut g = g4;
        for i in 3..53 as Id {
            let (next, _) = g.modify(crate::modify![N(i).val(i)]).unwrap();
            g = next;
        }

        assert_eq!(g.node_count(), 53);
        assert_eq!(g.node_val(id::N(0)), Some(&99));
        assert!(!g.is_adjacent(0, 1));
        assert!(g.is_adjacent(0, 2));

        assert_eq!(g1.iter_view(), snap);
        assert_eq!(g1.node_val(id::N(0)), Some(&1));
        assert!(g1.is_adjacent(0, 1));
        assert!(!g1.is_adjacent(0, 2));
        assert_eq!(g1.node_count(), 3);
        assert_eq!(g1.edge_count(), 2);
    }

    #[test]
    fn untouched_node_arc_is_shared() {
        let g0: VGraph<u32, edge::Undir<()>> = VGraph::new();
        let (g1, _) = g0
            .modify(crate::modify![N(0).val(1u32) ^ N(1).val(2u32), N(2).val(3u32)])
            .unwrap();
        let (g2, _) = g1.modify(crate::modify![X(2).val(9u32)]).unwrap();

        assert!(Arc::ptr_eq(
            g1.node_arc(id::N(0)).unwrap(),
            g2.node_arc(id::N(0)).unwrap()
        ));
        assert!(Arc::ptr_eq(
            g1.node_arc(id::N(1)).unwrap(),
            g2.node_arc(id::N(1)).unwrap()
        ));
        assert!(!Arc::ptr_eq(
            g1.node_arc(id::N(2)).unwrap(),
            g2.node_arc(id::N(2)).unwrap()
        ));
    }

    #[test]
    fn from_mgraph_matches() {
        let m: crate::graph::MUndir0 = crate::mgraph![N(0) ^ N(1), n(0) ^ N(2)].unwrap();
        let v = VGraph::from_mgraph(&m);
        assert_eq!(v.node_count(), m.node_count());
        assert_eq!(v.edge_count(), m.edge_count());
        for n in 0..3 as Id {
            assert_eq!(v.is_adjacent(0, n), m.is_adjacent(0, n));
        }
        assert_eq!(v.version(), 0);
    }

    #[test]
    fn dir_slot_views_match_mgraph() {
        type ER = edge::Dir<()>;
        type Slot = <ER as Edge>::Slot;

        fn nbrs<G: graph::Graph<(), ER>>(g: &G, n: Id) -> Vec<(Id, Slot)> {
            let mut v: Vec<(Id, Slot)> = g
                .neighbors(id::N(n))
                .unwrap()
                .map(|(nb, slot, _)| (*nb, slot))
                .collect();
            v.sort_unstable();
            v
        }

        fn between<G: graph::Graph<(), ER>>(g: &G, a: Id, b: Id) -> Vec<Slot> {
            let mut v: Vec<Slot> =
                g.edges_between(id::N(a), id::N(b)).map(|(s, _)| s).collect();
            v.sort_unstable();
            v
        }

        let ops = || crate::modify![N(0) >> N(1), n(1) >> n(0), N(2) >> N(3)];

        let mut m: crate::graph::MDir0 = crate::graph::MDir0::default();
        m.modify(ops()).unwrap();
        let v0: VGraph<(), ER> = VGraph::new();
        let (v, _) = v0.modify(ops()).unwrap();

        assert_eq!(v.node_count(), 4);
        assert_eq!(v.edge_count(), 3);
        for n in 0..4 as Id {
            assert_eq!(nbrs(&v, n), nbrs(&m, n), "neighbors of {n}");
        }
        for (a, b) in [(0, 1), (1, 0), (2, 3), (3, 2)] {
            assert_eq!(between(&v, a, b), between(&m, a, b), "edges_between {a},{b}");
        }
    }

    #[test]
    fn mgraph_parity_over_op_sequence() {
        type ER = edge::Undir<()>;
        type Ops = Vec<crate::modify::Node<u32, ER>>;

        fn view<G: graph::Graph<u32, ER>>(g: &G) -> Vec<(Id, u32, Vec<Id>)> {
            let mut view: Vec<(Id, u32, Vec<Id>)> = g
                .iter_nodes()
                .map(|(n, val, adj)| {
                    let mut nbs: Vec<Id> = adj.map(|nb| *nb).collect();
                    nbs.sort_unstable();
                    (*n, *val, nbs)
                })
                .collect();
            view.sort_unstable();
            view
        }

        let steps: Vec<fn() -> Ops> = vec![
            || crate::modify![N(0).val(1u32) ^ N(1).val(2u32), n(1) ^ N(2).val(3u32)],
            || crate::modify![X(0).val(9u32), x(0) ^ x(2)],
            || crate::modify![!X(2)],
            || crate::modify![N(5).val(7u32) ^ x(0)],
            || crate::modify![x(0) & !e() ^ x(1)],
            || crate::modify![!X(0)],
        ];

        let mut m: crate::graph::MUndirN<u32> = crate::graph::MUndirN::default();
        let mut v: VGraph<u32, ER> = VGraph::new();

        for (step, ops) in steps.into_iter().enumerate() {
            m.modify(ops()).unwrap();
            let (next, _) = v.modify(ops()).unwrap();
            v = next;
            assert_eq!(view(&v), view(&m), "step {step}");
            assert_eq!(v.edge_count(), m.edge_count(), "step {step}");
            assert_eq!(v.id_space_len(), m.id_space_len(), "step {step}");
        }
    }

    #[test]
    fn multi_free_slot_reuse_matches_mgraph() {
        type ER = edge::Undir<()>;
        type Ops = Vec<crate::modify::Node<(), ER>>;

        fn allocated(m: &Modification<(), ER>) -> (Vec<(Id, Id)>, Vec<(Id, Id, Id)>) {
            let mut nodes: Vec<(Id, Id)> =
                m.new_node_ids.iter().map(|(l, n)| (l.0, **n)).collect();
            nodes.sort_unstable();
            let edges: Vec<(Id, Id, Id)> =
                m.added_edges.iter().map(|(e, a, b, _)| (**e, **a, **b)).collect();
            (nodes, edges)
        }

        let steps: Vec<fn() -> Ops> = vec![
            || crate::modify![N(0) ^ N(1), N(2) ^ N(3), N(4) ^ n(0)],
            || crate::modify![!X(1), !X(3)],
            || crate::modify![N(7) ^ N(8)],
            || crate::modify![!X(0), !X(4)],
            || crate::modify![N(11), N(12), N(13)],
        ];

        let mut m: crate::graph::MUndir0 = crate::graph::MUndir0::default();
        let mut v: VGraph<(), ER> = VGraph::new();

        for (step, ops) in steps.into_iter().enumerate() {
            let m_mod = m.modify(ops()).unwrap();
            let (next, v_mod) = v.modify(ops()).unwrap();
            v = next;

            assert_eq!(allocated(&v_mod), allocated(&m_mod), "step {step}");
            assert_eq!(v.id_space_len(), m.id_space_len(), "step {step}");

            let mut v_ids: Vec<Id> = v.iter_node_ids().map(|n| *n).collect();
            let mut m_ids: Vec<Id> = m.iter_node_ids().map(|n| *n).collect();
            v_ids.sort_unstable();
            m_ids.sort_unstable();
            assert_eq!(v_ids, m_ids, "step {step}");
        }
    }

    #[test]
    fn removed_edge_gone_from_both_endpoints() {
        let g0: VGraph<(), edge::Undir<()>> = VGraph::new();
        let (g1, _) = g0.modify(crate::modify![N(0) ^ N(1)]).unwrap();
        let (g2, _) = g1.modify(crate::modify![x(0) & !e() ^ x(1)]).unwrap();
        assert_eq!(g2.edge_count(), 0);
        assert_eq!(g2.edges_between(id::N(0), id::N(1)).count(), 0);
        assert_eq!(g2.neighbors(id::N(0)).map(|it| it.count()).unwrap(), 0);
        assert_eq!(g2.neighbors(id::N(1)).map(|it| it.count()).unwrap(), 0);
        assert_eq!(g2.degree(id::N(0)), Some(0));
        assert_eq!(g2.degree(id::N(1)), Some(0));
        assert_eq!(g1.edge_count(), 1);
    }
}
