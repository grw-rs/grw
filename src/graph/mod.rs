pub(crate) mod collections;
pub mod dsl;
pub mod edge;
pub mod layout;
pub mod mutable;
pub(crate) mod node;
pub mod persist;
pub mod versioned;
pub mod watcher;

pub use mutable::MGraph;
pub use versioned::VGraph;

#[cfg(test)]
mod tests;

use std::fmt::Debug;

pub(crate) mod batch;
pub mod error;

pub(crate) use crate::{Id, NR, id};
pub(crate) use collections::*;
pub use edge::{Edge, HasRel};
pub use node::{AdjIds, Node};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Nodes<V> {
    pub(crate) store: Vec<Option<Node<V>>>,
    pub(crate) free_ids: IdSpace,
    pub(crate) count: usize,
}

impl<V: Clone> Clone for Nodes<V> {
    fn clone(&self) -> Self {
        Nodes { store: self.store.clone(), free_ids: self.free_ids.clone(), count: self.count }
    }
}

impl<V> Debug for Nodes<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Nodes {{ count: {} }}", self.count))
    }
}

impl<V> Default for Nodes<V> {
    fn default() -> Self {
        Self {
            store: Vec::new(),
            free_ids: IdSpace::default(),
            count: 0,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "S: serde::Serialize, V: serde::Serialize",
    deserialize = "S: serde::de::DeserializeOwned, V: serde::de::DeserializeOwned",
))]
pub(crate) struct EdgeRec<S, V> {
    pub(crate) n1: id::N,
    pub(crate) n2: id::N,
    pub(crate) slot: S,
    pub(crate) val: V,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "E::Slot: serde::Serialize, E::Val: serde::Serialize",
    deserialize = "E::Slot: serde::de::DeserializeOwned, E::Val: serde::de::DeserializeOwned",
))]
pub struct Edges<E: Edge> {
    pub(crate) store: Vec<Option<EdgeRec<E::Slot, E::Val>>>,
    pub(crate) free_ids: IdSpace,
    pub(crate) count: usize,
}

impl<S: Clone, V: Clone> Clone for EdgeRec<S, V> {
    fn clone(&self) -> Self {
        EdgeRec { n1: self.n1, n2: self.n2, slot: self.slot.clone(), val: self.val.clone() }
    }
}

impl<E: Edge> Clone for Edges<E> where E::Slot: Clone, E::Val: Clone {
    fn clone(&self) -> Self {
        Edges { store: self.store.clone(), free_ids: self.free_ids.clone(), count: self.count }
    }
}

impl<E: Edge> Debug for Edges<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Edges {{ {} }}", self.count))
    }
}

impl<E: Edge> Edges<E> {
    pub fn len(&self) -> usize {
        self.count
    }

    pub(crate) fn get_by_id(&self, eid: id::E) -> Option<&EdgeRec<E::Slot, E::Val>> {
        self.store.get(*eid as usize).and_then(|opt| opt.as_ref())
    }

    pub fn iter(&self) -> impl Iterator<Item = (E::Def, &E::Val)> {
        self.store
            .iter()
            .filter_map(|opt| {
                opt.as_ref().map(|rec| {
                    (E::edge(rec.slot, (*rec.n1, *rec.n2)), &rec.val)
                })
            })
    }
}

impl<E: Edge> Default for Edges<E> {
    fn default() -> Self {
        Self {
            store: Vec::new(),
            free_ids: IdSpace::default(),
            count: 0,
        }
    }
}

/// Read surface every graph representation exposes. Every method is
/// statically dispatched — the search engine is generic over `G: Graph` and
/// monomorphizes, so no implementor may introduce indirection here.
pub trait Graph<NV, E: Edge> {
    /// Distinct neighbour ids of one node, named so `iter_nodes` can yield it
    /// alongside the node value without nesting `impl Trait`.
    type NodeAdj<'a>: Iterator<Item = id::N>
    where
        Self: 'a;

    fn node_count(&self) -> usize;
    fn edge_count(&self) -> usize;
    /// Upper bound of the raw node-id space: every live `id::N` is `< this`.
    /// Reverse-lookup tables are sized by it, not by `node_count`.
    fn id_space_len(&self) -> usize;
    fn has_node(&self, n: id::N) -> bool;
    fn node_val(&self, n: id::N) -> Option<&NV>;
    fn degree(&self, n: id::N) -> Option<Id>;
    fn neighbors<'a>(&'a self, n: id::N) -> Option<impl Iterator<Item = (id::N, E::Slot, &'a E::Val)> + 'a>
    where
        E::Val: 'a;
    fn neighbor_ids(&self, n: id::N) -> Option<Self::NodeAdj<'_>>;
    fn is_adjacent(&self, n1: Id, n2: Id) -> bool;
    fn get_edge_val(&self, e: E::Def) -> Option<&E::Val>;
    fn edges_between<'a>(&'a self, n1: id::N, n2: id::N) -> impl Iterator<Item = (E::Slot, &'a E::Val)> + 'a
    where
        E::Val: 'a;
    /// Every live edge exactly once: endpoints as stored (lo, hi), slot as
    /// stored — not adjusted for query direction the way `neighbors` and
    /// `edges_between` adjust it. Deterministic order per implementor:
    /// `MGraph` walks its edge store by index (the order `persist::save`
    /// serializes), `VGraph` walks its edge `PVec` by ascending slot.
    /// WARNING: yielded (lo, hi) is in STORAGE order, not semantic (source, target).
    /// For a directed edge stored (0,1) with slot TGT, the semantic direction is 1→0.
    /// Direction-adjusted semantic views are provided by `neighbors` and `edges_between`.
    fn iter_edges<'a>(&'a self) -> impl Iterator<Item = (id::N, id::N, E::Slot, &'a E::Val)> + 'a
    where
        E::Val: 'a;
    fn iter_node_ids(&self) -> impl Iterator<Item = id::N> + '_;
    /// Value and adjacency of every live node in one probe per node. Index
    /// builders need both together; per-field getters re-probe the node store.
    fn iter_nodes<'a>(&'a self) -> impl Iterator<Item = (id::N, &'a NV, Self::NodeAdj<'a>)> + 'a
    where
        NV: 'a;
    fn rel(&self, ns: impl Into<NR<id::N>>) -> E::Rel<'_>
    where
        E: HasRel;

    fn index<T>(&self, _tier: T) -> crate::search::engine::Indexed<'_, NV, E, Self, T::Data>
    where
        T: crate::search::engine::Tier<NV, E>,
        Self: Sized,
    {
        crate::search::engine::Indexed::new(self, T::build(self))
    }
}

pub type MAnydir0 = MGraph<(), edge::Anydir<()>>;
pub type MAnydirN<NV> = MGraph<NV, edge::Anydir<()>>;
pub type MAnydirE<EV> = MGraph<(), edge::Anydir<EV>>;
pub type MAnydir<NV, EV> = MGraph<NV, edge::Anydir<EV>>;

pub type MDir0 = MGraph<(), edge::Dir<()>>;
pub type MDirN<NV> = MGraph<NV, edge::Dir<()>>;
pub type MDirE<EV> = MGraph<(), edge::Dir<EV>>;
pub type MDir<NV, EV> = MGraph<NV, edge::Dir<EV>>;

pub type MUndir0 = MGraph<(), edge::Undir<()>>;
pub type MUndirN<NV> = MGraph<NV, edge::Undir<()>>;
pub type MUndirE<EV> = MGraph<(), edge::Undir<EV>>;
pub type MUndir<NV, EV> = MGraph<NV, edge::Undir<EV>>;

pub type VAnydir0 = VGraph<(), edge::Anydir<()>>;
pub type VAnydirN<NV> = VGraph<NV, edge::Anydir<()>>;
pub type VAnydirE<EV> = VGraph<(), edge::Anydir<EV>>;
pub type VAnydir<NV, EV> = VGraph<NV, edge::Anydir<EV>>;

pub type VDir0 = VGraph<(), edge::Dir<()>>;
pub type VDirN<NV> = VGraph<NV, edge::Dir<()>>;
pub type VDirE<EV> = VGraph<(), edge::Dir<EV>>;
pub type VDir<NV, EV> = VGraph<NV, edge::Dir<EV>>;

pub type VUndir0 = VGraph<(), edge::Undir<()>>;
pub type VUndirN<NV> = VGraph<NV, edge::Undir<()>>;
pub type VUndirE<EV> = VGraph<(), edge::Undir<EV>>;
pub type VUndir<NV, EV> = VGraph<NV, edge::Undir<EV>>;

pub trait HasKey<NV, E: Edge> {
    fn has_in(self, graph: &MGraph<NV, E>) -> bool;
}

pub trait GetKey<NV, E: Edge> {
    type Val;
    fn get_from(self, graph: &MGraph<NV, E>) -> Option<&Self::Val>;
    fn get_mut_from(self, graph: &mut MGraph<NV, E>) -> Option<&mut Self::Val>;
}

impl<NV, E: Edge> HasKey<NV, E> for id::N {
    fn has_in(self, graph: &MGraph<NV, E>) -> bool {
        graph.nodes.has(self)
    }
}

impl<NV, E: Edge> GetKey<NV, E> for id::N {
    type Val = NV;
    fn get_from(self, graph: &MGraph<NV, E>) -> Option<&NV> {
        graph.nodes.get(self)
    }
    fn get_mut_from(self, graph: &mut MGraph<NV, E>) -> Option<&mut NV> {
        graph.nodes.get_node_mut(self).map(|n| &mut n.val)
    }
}

impl<NV, E: Edge> HasKey<NV, E> for Id {
    fn has_in(self, graph: &MGraph<NV, E>) -> bool {
        id::N(self).has_in(graph)
    }
}

impl<NV, E: Edge> GetKey<NV, E> for Id {
    type Val = NV;
    fn get_from(self, graph: &MGraph<NV, E>) -> Option<&NV> {
        id::N(self).get_from(graph)
    }
    fn get_mut_from(self, graph: &mut MGraph<NV, E>) -> Option<&mut NV> {
        id::N(self).get_mut_from(graph)
    }
}

macro_rules! impl_edge_key {
    ($name:ident, $mod:ident) => {
        impl<NV, EV> HasKey<NV, edge::$name<EV>> for edge::$mod::E<Id> {
            fn has_in(self, graph: &MGraph<NV, edge::$name<EV>>) -> bool {
                let (nr, slot) = self.into();
                graph.edges_between(*nr.n1(), *nr.n2())
                    .any(|(s, _)| s == slot)
            }
        }

        impl<NV, EV> GetKey<NV, edge::$name<EV>> for edge::$mod::E<Id> {
            type Val = EV;
            fn get_from(self, graph: &MGraph<NV, edge::$name<EV>>) -> Option<&EV> {
                let (nr, slot) = self.into();
                graph.edges_between(*nr.n1(), *nr.n2())
                    .find(|(s, _)| *s == slot)
                    .map(|(_, v)| v)
            }
            fn get_mut_from(self, graph: &mut MGraph<NV, edge::$name<EV>>) -> Option<&mut EV> {
                let (nr, slot) = self.into();
                let n1 = *nr.n1();
                let n2 = *nr.n2();
                let MGraph { nodes, edges, .. } = graph;
                let eids: smallvec::SmallVec<[id::E; 4]> = nodes.get_node(n1)
                    .map(|node| node.adj.edges_to(n2).collect())
                    .unwrap_or_default();
                let target_eid = eids.into_iter().find(|&eid| {
                    edges.store[*eid as usize]
                        .as_ref()
                        .map(|r| r.slot == slot)
                        .unwrap_or(false)
                })?;
                edges.store[*target_eid as usize].as_mut().map(|r| &mut r.val)
            }
        }
    };
}

impl_edge_key!(Undir, undir);
impl_edge_key!(Dir, dir);

impl<NV, EV> HasKey<NV, edge::Anydir<EV>> for edge::anydir::E<Id> {
    fn has_in(self, graph: &MGraph<NV, edge::Anydir<EV>>) -> bool {
        let (nr, slot) = self.into();
        graph.edges_between(*nr.n1(), *nr.n2())
            .any(|(s, _)| s == slot)
    }
}

impl<NV, EV> GetKey<NV, edge::Anydir<EV>> for edge::anydir::E<Id> {
    type Val = edge::AnyVal<EV>;
    fn get_from(self, graph: &MGraph<NV, edge::Anydir<EV>>) -> Option<&edge::AnyVal<EV>> {
        let (nr, slot) = self.into();
        graph.edges_between(*nr.n1(), *nr.n2())
            .find(|(s, _)| *s == slot)
            .map(|(_, v)| v)
    }
    fn get_mut_from(self, graph: &mut MGraph<NV, edge::Anydir<EV>>) -> Option<&mut edge::AnyVal<EV>> {
        let (nr, slot) = self.into();
        let n1 = *nr.n1();
        let n2 = *nr.n2();
        let MGraph { nodes, edges, .. } = graph;
        let eids: smallvec::SmallVec<[id::E; 4]> = nodes.get_node(n1)
            .map(|node| node.adj.edges_to(n2).collect())
            .unwrap_or_default();
        let target_eid = eids.into_iter().find(|&eid| {
            edges.store[*eid as usize]
                .as_ref()
                .map(|r| r.slot == slot)
                .unwrap_or(false)
        })?;
        edges.store[*target_eid as usize].as_mut().map(|r| &mut r.val)
    }
}

impl<NV, E: Edge> HasKey<NV, E> for (id::N, id::N) {
    fn has_in(self, graph: &MGraph<NV, E>) -> bool {
        graph.nodes.get_node(self.0)
            .map(|n| n.adj.contains(self.1))
            .unwrap_or(false)
    }
}

impl<NV, E: Edge> HasKey<NV, E> for (Id, Id) {
    fn has_in(self, graph: &MGraph<NV, E>) -> bool {
        (id::N(self.0), id::N(self.1)).has_in(graph)
    }
}

pub(crate) fn from_edges<E: Edge>(
    edges: impl Into<edge::Batch<E>>,
) -> Result<MGraph<(), E>, error::Edge<E::Slot>> {
    let edge::Batch(evs) = edges.into();
    let (nodes, edges) = batch::collect_derived_nodes_from_edges::<E>(&mut evs.into_iter())?;
    let mut g = MGraph { nodes, edges, degrees: Vec::new() };
    g.build_degrees();
    Ok(g)
}

pub(crate) fn from_nodes_edges<NV, E: Edge>(
    nodes: impl Into<node::Batch<NV>>,
    edges: impl Into<edge::Batch<E>>,
) -> Result<MGraph<NV, E>, error::Build<E::Slot>> {
    let mut nodes = nodes.into().0?;
    let edge::Batch(evs) = edges.into();
    let edges = batch::collect_edges::<NV, E>(&mut nodes, &mut evs.into_iter())?;
    let mut g = MGraph { nodes, edges, degrees: Vec::new() };
    g.build_degrees();
    Ok(g)
}

macro_rules! impl_graph_from_edges {
    ($name:ident, $mod:ident) => {
        impl TryFrom<Vec<edge::$mod::E<Id>>> for MGraph<(), edge::$name<()>> {
            type Error = error::Edge<edge::$mod::Slot>;
            fn try_from(es: Vec<edge::$mod::E<Id>>) -> Result<Self, Self::Error> {
                from_edges(es)
            }
        }

        impl<EV> TryFrom<Vec<(edge::$mod::E<Id>, EV)>> for MGraph<(), edge::$name<EV>> {
            type Error = error::Edge<edge::$mod::Slot>;
            fn try_from(evs: Vec<(edge::$mod::E<Id>, EV)>) -> Result<Self, Self::Error> {
                from_edges(evs)
            }
        }
    };
}

macro_rules! impl_graph_from_nodes_edges {
    ($name:ident, $mod:ident) => {
        impl TryFrom<(Id, Vec<edge::$mod::E<Id>>)> for MGraph<(), edge::$name<()>> {
            type Error = error::Build<edge::$mod::Slot>;
            fn try_from((ns_len, es): (Id, Vec<edge::$mod::E<Id>>)) -> Result<Self, Self::Error> {
                from_nodes_edges(ns_len, es)
            }
        }

        impl TryFrom<(Vec<Id>, Vec<edge::$mod::E<Id>>)> for MGraph<(), edge::$name<()>> {
            type Error = error::Build<edge::$mod::Slot>;
            fn try_from((ns, es): (Vec<Id>, Vec<edge::$mod::E<Id>>)) -> Result<Self, Self::Error> {
                from_nodes_edges(ns, es)
            }
        }

        impl<NV> TryFrom<(Vec<(Id, NV)>, Vec<edge::$mod::E<Id>>)>
            for MGraph<NV, edge::$name<()>>
        {
            type Error = error::Build<edge::$mod::Slot>;
            fn try_from(
                (nvs, es): (Vec<(Id, NV)>, Vec<edge::$mod::E<Id>>),
            ) -> Result<Self, Self::Error> {
                from_nodes_edges(nvs, es)
            }
        }

        impl<EV> TryFrom<(Id, Vec<(edge::$mod::E<Id>, EV)>)> for MGraph<(), edge::$name<EV>> {
            type Error = error::Build<edge::$mod::Slot>;
            fn try_from(
                (ns_len, evs): (Id, Vec<(edge::$mod::E<Id>, EV)>),
            ) -> Result<Self, Self::Error> {
                from_nodes_edges(ns_len, evs)
            }
        }

        impl<EV> TryFrom<(Vec<Id>, Vec<(edge::$mod::E<Id>, EV)>)>
            for MGraph<(), edge::$name<EV>>
        {
            type Error = error::Build<edge::$mod::Slot>;
            fn try_from(
                (ns, evs): (Vec<Id>, Vec<(edge::$mod::E<Id>, EV)>),
            ) -> Result<Self, Self::Error> {
                from_nodes_edges(ns, evs)
            }
        }

        impl<NV, EV> TryFrom<(Vec<(Id, NV)>, Vec<(edge::$mod::E<Id>, EV)>)>
            for MGraph<NV, edge::$name<EV>>
        {
            type Error = error::Build<edge::$mod::Slot>;
            fn try_from(
                (nvs, evs): (Vec<(Id, NV)>, Vec<(edge::$mod::E<Id>, EV)>),
            ) -> Result<Self, Self::Error> {
                from_nodes_edges(nvs, evs)
            }
        }
    };
}

impl_graph_from_edges!(Undir, undir);
impl_graph_from_edges!(Dir, dir);
impl_graph_from_edges!(Anydir, anydir);

impl_graph_from_nodes_edges!(Undir, undir);
impl_graph_from_nodes_edges!(Dir, dir);
impl_graph_from_nodes_edges!(Anydir, anydir);
