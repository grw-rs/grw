use crate::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub Id);

impl From<Id> for LocalId {
    fn from(value: Id) -> Self {
        LocalId(value)
    }
}

pub struct HasVal<V>(pub(crate) V);

pub(crate) trait IntoOptional<T> {
    fn into_optional(self) -> Option<T>;
}
impl<T> IntoOptional<T> for () {
    fn into_optional(self) -> Option<T> {
        None
    }
}
impl<T> IntoOptional<T> for HasVal<T> {
    fn into_optional(self) -> Option<T> {
        Some(self.0)
    }
}

#[diagnostic::on_unimplemented(
    message = "`{T}` has no default — an explicit value is required",
    label = "no `Default` impl for `{T}`",
    note = "provide a value with `.val(...)`, or add `#[derive(Default)]` to `{T}`"
)]
pub(crate) trait IntoVal<T> {
    fn into_val(self) -> T;
}
impl<T: Default> IntoVal<T> for () {
    fn into_val(self) -> T {
        T::default()
    }
}
impl<T> IntoVal<T> for HasVal<T> {
    fn into_val(self) -> T {
        self.0
    }
}

macro_rules! for_each_dir {
    ($mac:ident ! ($($arg:tt)+)) => {
        $mac!($($arg)+, Src, Shr, shr);
        $mac!($($arg)+, Tgt, Shl, shl);
        $mac!($($arg)+, Und, BitXor, bitxor);
    };
    ($mac:ident ! ()) => {
        $mac!(Src, Shr, shr);
        $mac!(Tgt, Shl, shl);
        $mac!(Und, BitXor, bitxor);
    };
}

pub mod edge;
pub mod node;

use crate::graph::{self, EdgeRec};
use crate::{Edges, FxHashMap, IdSpace, NR, Node, Nodes, id};
use std::collections::BTreeSet;
use std::marker::PhantomData;

pub struct UndirPending<N, E>(pub N, pub E);

pub enum Op<NV, ER: graph::Edge> {
    Add {
        id: Option<LocalId>,
        val: NV,
        edges: Vec<EdgeOp<NV, ER>>,
    },
    Ref {
        id: LocalId,
        edges: Vec<EdgeOp<NV, ER>>,
    },
}

pub struct EdgeOp<NV, ER: graph::Edge> {
    pub slot: ER::Slot,
    pub val: ER::Val,
    pub target: Op<NV, ER>,
}

pub(crate) trait IntoOp<NV, ER: graph::Edge> {
    fn into_op(self) -> Op<NV, ER>;
}

fn walk_collect_ids<NV, ER: graph::Edge>(
    op: &Op<NV, ER>,
    explicit: &mut BTreeSet<Id>,
    auto_count: &mut usize,
) -> Result<(), super::error::Build<ER::Slot>> {
    match op {
        Op::Add { id, edges, .. } => {
            if let Some(lid) = id {
                if !explicit.insert(lid.0) {
                    return Err(super::error::Node::DuplicateLocalId(lid.0).into());
                }
            } else {
                *auto_count += 1;
            }
            for e in edges {
                walk_collect_ids(&e.target, explicit, auto_count)?;
            }
        }
        Op::Ref { edges, .. } => {
            for e in edges {
                walk_collect_ids(&e.target, explicit, auto_count)?;
            }
        }
    }
    Ok(())
}

fn assign_auto_ids(explicit: &BTreeSet<Id>, auto_count: usize) -> Vec<Id> {
    let mut auto_ids = Vec::with_capacity(auto_count);
    let mut candidate: Id = 0;
    for _ in 0..auto_count {
        while explicit.contains(&candidate) {
            candidate = candidate.checked_add(1).unwrap();
        }
        auto_ids.push(candidate);
        candidate = candidate.checked_add(1).unwrap();
    }
    auto_ids
}

struct Flattener<'a, NV, ER: graph::Edge> {
    id_map: &'a FxHashMap<Id, id::N>,
    auto_ids: &'a [Id],
    auto_cursor: usize,
    nodes_out: Vec<(id::N, NV)>,
    edges_out: Vec<((NR<id::N>, ER::Slot), ER::Val)>,
    edge_set: BTreeSet<(NR<id::N>, ER::Slot)>,
}

impl<'a, NV, ER: graph::Edge> Flattener<'a, NV, ER> {
    fn resolve_id(&mut self, op: &Op<NV, ER>) -> Result<id::N, super::error::Build<ER::Slot>> {
        match op {
            Op::Add { id: Some(lid), .. } => Ok(*self.id_map.get(&lid.0).unwrap()),
            Op::Add { id: None, .. } => {
                let aid = self.auto_ids[self.auto_cursor];
                self.auto_cursor += 1;
                Ok(id::N(aid))
            }
            Op::Ref { id: lid, .. } => match self.id_map.get(&lid.0) {
                Some(&n) => Ok(n),
                None => Err(super::error::Node::UndefinedRef(lid.0).into()),
            },
        }
    }

    fn walk(
        &mut self,
        op: Op<NV, ER>,
        _src: Option<id::N>,
    ) -> Result<id::N, super::error::Build<ER::Slot>> {
        let real_id = self.resolve_id(&op)?;

        let edges = match op {
            Op::Add { val, edges, .. } => {
                self.nodes_out.push((real_id, val));
                edges
            }
            Op::Ref { edges, .. } => edges,
        };

        for e in edges {
            let tgt = self.walk(e.target, Some(real_id))?;
            let (rel, slot) = graph::edge::normalized_val(
                [real_id, tgt],
                e.slot,
                ER::reverse_slot(e.slot),
            );
            let key = (rel, slot);
            if !self.edge_set.insert(key) {
                return Err(super::error::Edge::Duplicate(rel, slot).into());
            }
            self.edges_out.push((key, e.val));
        }

        Ok(real_id)
    }
}

pub fn from_fragment<NV, ER: graph::Edge>(
    ops: Vec<Op<NV, ER>>,
) -> Result<graph::Graph<NV, ER>, super::error::Build<ER::Slot>> {
    let mut explicit = BTreeSet::new();
    let mut auto_count = 0usize;

    for op in &ops {
        walk_collect_ids(op, &mut explicit, &mut auto_count)?;
    }

    let auto_ids = assign_auto_ids(&explicit, auto_count);

    let mut id_map = FxHashMap::default();
    for &eid in &explicit {
        id_map.insert(eid, id::N(eid));
    }
    for &aid in &auto_ids {
        id_map.insert(aid, id::N(aid));
    }

    let total_nodes = explicit.len() + auto_count;
    let mut flat = Flattener::<NV, ER> {
        id_map: &id_map,
        auto_ids: &auto_ids,
        auto_cursor: 0,
        nodes_out: Vec::with_capacity(total_nodes),
        edges_out: Vec::new(),
        edge_set: BTreeSet::new(),
    };

    for op in ops {
        flat.walk(op, None)?;
    }

    let mut nodes = {
        let mut store: Vec<Option<Node<NV>>> = Vec::new();
        let mut count = 0usize;
        let mut free_ids = IdSpace::default();

        for (n, val) in flat.nodes_out {
            let idx = *n as usize;
            if idx >= store.len() {
                store.resize_with(idx + 1, || None);
            }
            if store[idx].is_some() {
                continue;
            }
            free_ids.remove_id(*n);
            store[idx] = Some(Node::new(val));
            count += 1;
        }

        Nodes { store, free_ids, count }
    };

    let mut edge_store: Vec<Option<EdgeRec<ER::Slot, ER::Val>>> = Vec::new();
    let mut edge_free_ids = IdSpace::default();
    let mut edge_count: usize = 0;

    for ((rel, slot), val) in flat.edges_out {
        let edge_id = id::E(edge_free_ids.pop_id().unwrap());
        let n1 = *rel.n1();
        let n2 = *rel.n2();

        if !rel.is_cycle() {
            let ns = rel.ns_refs();
            let [ref_a, ref_b] = nodes.get_two_mut(*ns[0], *ns[1]);
            let node_a = ref_a.unwrap();
            let node_b = ref_b.unwrap();
            node_a.adj.insert(*ns[1], edge_id);
            node_b.adj.insert(*ns[0], edge_id);
        } else {
            let node = nodes.get_node_mut(*rel.n1()).unwrap();
            node.adj.insert(n1, edge_id);
        }

        edge_store.push(Some(EdgeRec { n1, n2, slot, val }));
        edge_count += 1;
    }

    let edges = Edges { store: edge_store, free_ids: edge_free_ids, count: edge_count };

    let mut g = graph::Graph {
        nodes,
        edges,
        degrees: Vec::new(),
    };
    g.build_degrees();
    Ok(g)
}

#[allow(non_snake_case)]
pub fn N<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::Node<NV, (), ER> {
    node::Node {
        id: Some(local.into()),
        v: (),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn N_<NV, ER: graph::Edge>() -> node::Node<NV, (), ER> {
    node::Node {
        id: None,
        v: (),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn n<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::Ref<NV, ER> {
    node::Ref {
        id: local.into(),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn E<NV, ER: graph::Edge>() -> edge::Edge<(), NV, ER> {
    edge::Edge((), PhantomData)
}

#[macro_export]
macro_rules! graph {
    [<$nv:ty, $er:ty>; $($expr:expr),* $(,)?] => {{
        #[allow(unused_imports)]
        use $crate::graph::dsl::*;
        $crate::graph::dsl::from_fragment::<$nv, $er>(vec![$($expr.into()),*])
    }};
    [$($expr:expr),* $(,)?] => {{
        #[allow(unused_imports)]
        use $crate::graph::dsl::*;
        $crate::graph::dsl::from_fragment(vec![$($expr.into()),*])
    }};
}
