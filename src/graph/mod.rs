pub(crate) mod collections;
pub mod dsl;
pub mod edge;
pub(crate) mod node;

#[cfg(test)]
mod tests;

use std::fmt::Debug;

pub(crate) mod batch;
pub mod error;

pub(crate) use crate::{Id, NR, id};
pub(crate) use collections::*;
pub use edge::{Edge, HasRel};
pub use node::Node;

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

pub type Anydir0 = Graph<(), edge::Anydir<()>>;
pub type AnydirN<NV> = Graph<NV, edge::Anydir<()>>;
pub type AnydirE<EV> = Graph<(), edge::Anydir<EV>>;
pub type Anydir<NV, EV> = Graph<NV, edge::Anydir<EV>>;

pub type Dir0 = Graph<(), edge::Dir<()>>;
pub type DirN<NV> = Graph<NV, edge::Dir<()>>;
pub type DirE<EV> = Graph<(), edge::Dir<EV>>;
pub type Dir<NV, EV> = Graph<NV, edge::Dir<EV>>;

pub type Undir0 = Graph<(), edge::Undir<()>>;
pub type UndirN<NV> = Graph<NV, edge::Undir<()>>;
pub type UndirE<EV> = Graph<(), edge::Undir<EV>>;
pub type Undir<NV, EV> = Graph<NV, edge::Undir<EV>>;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "NV: serde::Serialize, E::Slot: serde::Serialize, E::Val: serde::Serialize",
    deserialize = "NV: serde::de::DeserializeOwned, E::Slot: serde::de::DeserializeOwned, E::Val: serde::de::DeserializeOwned",
))]
pub struct Graph<NV, E: Edge> {
    pub(crate) nodes: Nodes<NV>,
    pub(crate) edges: Edges<E>,
    pub(crate) degrees: Vec<(Id, IdSet<id::N>)>,
}

impl<NV: Clone, E: Edge> Clone for Graph<NV, E> where E::Slot: Clone, E::Val: Clone {
    fn clone(&self) -> Self {
        Graph { nodes: self.nodes.clone(), edges: self.edges.clone(), degrees: self.degrees.clone() }
    }
}

pub trait HasKey<NV, E: Edge> {
    fn has_in(self, graph: &Graph<NV, E>) -> bool;
}

pub trait GetKey<NV, E: Edge> {
    type Val;
    fn get_from(self, graph: &Graph<NV, E>) -> Option<&Self::Val>;
    fn get_mut_from(self, graph: &mut Graph<NV, E>) -> Option<&mut Self::Val>;
}

impl<NV, E: Edge> HasKey<NV, E> for id::N {
    fn has_in(self, graph: &Graph<NV, E>) -> bool {
        graph.nodes.has(self)
    }
}

impl<NV, E: Edge> GetKey<NV, E> for id::N {
    type Val = NV;
    fn get_from(self, graph: &Graph<NV, E>) -> Option<&NV> {
        graph.nodes.get(self)
    }
    fn get_mut_from(self, graph: &mut Graph<NV, E>) -> Option<&mut NV> {
        graph.nodes.get_node_mut(self).map(|n| &mut n.val)
    }
}

impl<NV, E: Edge> HasKey<NV, E> for Id {
    fn has_in(self, graph: &Graph<NV, E>) -> bool {
        id::N(self).has_in(graph)
    }
}

impl<NV, E: Edge> GetKey<NV, E> for Id {
    type Val = NV;
    fn get_from(self, graph: &Graph<NV, E>) -> Option<&NV> {
        id::N(self).get_from(graph)
    }
    fn get_mut_from(self, graph: &mut Graph<NV, E>) -> Option<&mut NV> {
        id::N(self).get_mut_from(graph)
    }
}

macro_rules! impl_edge_key {
    ($name:ident, $mod:ident) => {
        impl<NV, EV> HasKey<NV, edge::$name<EV>> for edge::$mod::E<Id> {
            fn has_in(self, graph: &Graph<NV, edge::$name<EV>>) -> bool {
                let (nr, slot) = self.into();
                graph.edges_between(*nr.n1(), *nr.n2())
                    .any(|(s, _)| s == slot)
            }
        }

        impl<NV, EV> GetKey<NV, edge::$name<EV>> for edge::$mod::E<Id> {
            type Val = EV;
            fn get_from(self, graph: &Graph<NV, edge::$name<EV>>) -> Option<&EV> {
                let (nr, slot) = self.into();
                graph.edges_between(*nr.n1(), *nr.n2())
                    .find(|(s, _)| *s == slot)
                    .map(|(_, v)| v)
            }
            fn get_mut_from(self, graph: &mut Graph<NV, edge::$name<EV>>) -> Option<&mut EV> {
                let (nr, slot) = self.into();
                let n1 = *nr.n1();
                let n2 = *nr.n2();
                let Graph { nodes, edges, .. } = graph;
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
impl_edge_key!(Anydir, anydir);

impl<NV, E: Edge> HasKey<NV, E> for (id::N, id::N) {
    fn has_in(self, graph: &Graph<NV, E>) -> bool {
        graph.nodes.get_node(self.0)
            .map(|n| n.adj.contains(self.1))
            .unwrap_or(false)
    }
}

impl<NV, E: Edge> HasKey<NV, E> for (Id, Id) {
    fn has_in(self, graph: &Graph<NV, E>) -> bool {
        (id::N(self.0), id::N(self.1)).has_in(graph)
    }
}

impl<NV, E: Edge> Graph<NV, E> {
    pub fn get<K: GetKey<NV, E>>(&self, key: K) -> Option<&K::Val> {
        key.get_from(self)
    }
    pub fn get_mut<K: GetKey<NV, E>>(&mut self, key: K) -> Option<&mut K::Val> {
        key.get_mut_from(self)
    }
    pub fn has<K: HasKey<NV, E>>(&self, key: K) -> bool {
        key.has_in(self)
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn node_iter(&self) -> impl Iterator<Item = (id::N, &NV)> {
        self.nodes.iter()
    }

    pub fn edge_iter(&self) -> impl Iterator<Item = (E::Def, &E::Val)> {
        self.edges.iter()
    }

    pub fn is_adjacent(&self, n1: Id, n2: Id) -> bool {
        self.nodes.get_node(id::N(n1))
            .map(|n| n.adj.contains(id::N(n2)))
            .unwrap_or(false)
    }

    pub(crate) fn get_edge_val(&self, e: E::Def) -> Option<&E::Val> {
        let (nr, slot) = e.into();
        self.edges_between(*nr.n1(), *nr.n2())
            .find(|(s, _)| *s == slot)
            .map(|(_, v)| v)
    }

    pub(crate) fn edges_between(&self, n1: id::N, n2: id::N) -> impl Iterator<Item = (E::Slot, &E::Val)> + '_ {
        self.nodes.get_node(n1)
            .into_iter()
            .flat_map(move |node| {
                node.adj.edges_to(n2).map(move |eid| {
                    let rec = self.edges.store[*eid as usize].as_ref().unwrap();
                    (rec.slot, &rec.val)
                })
            })
    }

    pub(crate) fn edge_ids_between(&self, n1: id::N, n2: id::N) -> impl Iterator<Item = id::E> + '_ {
        self.nodes.get_node(n1)
            .into_iter()
            .flat_map(move |node| node.adj.edges_to(n2))
    }

    pub(crate) fn degrees_insert(&mut self, node: id::N, degree: Id) {
        let pos = self.degrees.partition_point(|&(d, _)| d > degree);
        if pos < self.degrees.len() && self.degrees[pos].0 == degree {
            self.degrees[pos].1.insert(&node);
        } else {
            let mut set = IdSet::<id::N>::default();
            set.insert(&node);
            self.degrees.insert(pos, (degree, set));
        }
    }

    pub(crate) fn degrees_remove(&mut self, node: id::N, degree: Id) {
        let pos = self.degrees.partition_point(|&(d, _)| d > degree);
        if pos < self.degrees.len() && self.degrees[pos].0 == degree {
            self.degrees[pos].1.remove(&node);
            if self.degrees[pos].1.is_empty() {
                self.degrees.remove(pos);
            }
        }
    }

    pub(crate) fn degrees_move(&mut self, node: id::N, old_degree: Id, new_degree: Id) {
        if old_degree == new_degree {
            return;
        }
        self.degrees_remove(node, old_degree);
        self.degrees_insert(node, new_degree);
    }

    pub(crate) fn nodes_with_min_degree(&self, min_degree: Id) -> impl Iterator<Item = &IdSet<id::N>> {
        let end = self.degrees.partition_point(|&(d, _)| d >= min_degree);
        self.degrees[..end].iter().map(|(_, set)| set)
    }


    pub(crate) fn build_degrees(&mut self) {
        self.degrees.clear();
        let entries: Vec<(id::N, Id)> = self.nodes.nodes_iter()
            .map(|(n, node)| (n, node.adj.len()))
            .collect();
        for (n, deg) in entries {
            self.degrees_insert(n, deg);
        }
    }

}

impl<NV: Clone, E: Edge> Graph<NV, E>
where
    E::Val: Clone,
{
    pub fn to_vecs(&self) -> (Vec<(Id, NV)>, Vec<(E::Def, E::Val)>) {
        let nodes = self.nodes.iter().map(|(n, v)| (*n, v.clone())).collect();
        let edges = self.edges.iter().map(|(def, val)| (def, val.clone())).collect();
        (nodes, edges)
    }
}

impl<NV, E: Edge> From<Graph<NV, E>> for (Vec<(Id, NV)>, Vec<(E::Def, E::Val)>) {
    fn from(g: Graph<NV, E>) -> Self {
        let Graph { nodes, edges, degrees: _ } = g;
        let nodes = nodes.store.into_iter().enumerate()
            .filter_map(|(i, opt)| opt.map(|node| (i as Id, node.val)))
            .collect();
        let edges = edges
            .store
            .into_iter()
            .filter_map(|opt| {
                opt.map(|rec| (E::edge(rec.slot, (*rec.n1, *rec.n2)), rec.val))
            })
            .collect();
        (nodes, edges)
    }
}

impl<NV, E: HasRel> Graph<NV, E> {
    pub fn rel(&self, ns: impl Into<NR<id::N>>) -> E::Rel<'_> {
        let nr = ns.into();
        E::rel(nr, self.edges_between(*nr.n1(), *nr.n2()))
    }
    pub fn rel_mut(&mut self, ns: impl Into<NR<id::N>>) -> E::RelMut<'_> {
        let nr = ns.into();
        let n1 = *nr.n1();
        let n2 = *nr.n2();
        let Graph { nodes, edges, .. } = self;
        let eids: smallvec::SmallVec<[id::E; 4]> = nodes.get_node(n1)
            .map(|node| node.adj.edges_to(n2).collect())
            .unwrap_or_default();
        E::rel_mut(nr, eids.into_iter().map(|eid| {
            let idx = *eid as usize;
            let rec_ref = unsafe { &mut *edges.store.as_mut_ptr().add(idx) };
            let rec = rec_ref.as_mut().unwrap();
            (rec.slot, &mut rec.val)
        }))
    }
}

impl<NV, E: Edge> Default for Graph<NV, E>
where
    NV: Default,
    E::Val: Default,
{
    fn default() -> Self {
        Self {
            nodes: Nodes::default(),
            edges: Edges::default(),
            degrees: Vec::new(),
        }
    }
}

pub(crate) fn from_edges<E: Edge>(
    edges: impl Into<edge::Batch<E>>,
) -> Result<Graph<(), E>, error::Edge<E::Slot>> {
    let edge::Batch(evs) = edges.into();
    let (nodes, edges) = batch::collect_derived_nodes_from_edges::<E>(&mut evs.into_iter())?;
    let mut g = Graph { nodes, edges, degrees: Vec::new() };
    g.build_degrees();
    Ok(g)
}

pub(crate) fn from_nodes_edges<NV, E: Edge>(
    nodes: impl Into<node::Batch<NV>>,
    edges: impl Into<edge::Batch<E>>,
) -> Result<Graph<NV, E>, error::Build<E::Slot>> {
    let mut nodes = nodes.into().0?;
    let edge::Batch(evs) = edges.into();
    let edges = batch::collect_edges::<NV, E>(&mut nodes, &mut evs.into_iter())?;
    let mut g = Graph { nodes, edges, degrees: Vec::new() };
    g.build_degrees();
    Ok(g)
}

macro_rules! impl_graph_from_edges {
    ($name:ident, $mod:ident) => {
        impl TryFrom<Vec<edge::$mod::E<Id>>> for Graph<(), edge::$name<()>> {
            type Error = error::Edge<edge::$mod::Slot>;
            fn try_from(es: Vec<edge::$mod::E<Id>>) -> Result<Self, Self::Error> {
                from_edges(es)
            }
        }

        impl<EV> TryFrom<Vec<(edge::$mod::E<Id>, EV)>> for Graph<(), edge::$name<EV>> {
            type Error = error::Edge<edge::$mod::Slot>;
            fn try_from(evs: Vec<(edge::$mod::E<Id>, EV)>) -> Result<Self, Self::Error> {
                from_edges(evs)
            }
        }
    };
}

macro_rules! impl_graph_from_nodes_edges {
    ($name:ident, $mod:ident) => {
        impl TryFrom<(Id, Vec<edge::$mod::E<Id>>)> for Graph<(), edge::$name<()>> {
            type Error = error::Build<edge::$mod::Slot>;
            fn try_from((ns_len, es): (Id, Vec<edge::$mod::E<Id>>)) -> Result<Self, Self::Error> {
                from_nodes_edges(ns_len, es)
            }
        }

        impl TryFrom<(Vec<Id>, Vec<edge::$mod::E<Id>>)> for Graph<(), edge::$name<()>> {
            type Error = error::Build<edge::$mod::Slot>;
            fn try_from((ns, es): (Vec<Id>, Vec<edge::$mod::E<Id>>)) -> Result<Self, Self::Error> {
                from_nodes_edges(ns, es)
            }
        }

        impl<NV> TryFrom<(Vec<(Id, NV)>, Vec<edge::$mod::E<Id>>)>
            for Graph<NV, edge::$name<()>>
        {
            type Error = error::Build<edge::$mod::Slot>;
            fn try_from(
                (nvs, es): (Vec<(Id, NV)>, Vec<edge::$mod::E<Id>>),
            ) -> Result<Self, Self::Error> {
                from_nodes_edges(nvs, es)
            }
        }

        impl<EV> TryFrom<(Id, Vec<(edge::$mod::E<Id>, EV)>)> for Graph<(), edge::$name<EV>> {
            type Error = error::Build<edge::$mod::Slot>;
            fn try_from(
                (ns_len, evs): (Id, Vec<(edge::$mod::E<Id>, EV)>),
            ) -> Result<Self, Self::Error> {
                from_nodes_edges(ns_len, evs)
            }
        }

        impl<EV> TryFrom<(Vec<Id>, Vec<(edge::$mod::E<Id>, EV)>)>
            for Graph<(), edge::$name<EV>>
        {
            type Error = error::Build<edge::$mod::Slot>;
            fn try_from(
                (ns, evs): (Vec<Id>, Vec<(edge::$mod::E<Id>, EV)>),
            ) -> Result<Self, Self::Error> {
                from_nodes_edges(ns, evs)
            }
        }

        impl<NV, EV> TryFrom<(Vec<(Id, NV)>, Vec<(edge::$mod::E<Id>, EV)>)>
            for Graph<NV, edge::$name<EV>>
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
