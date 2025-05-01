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

use crate::graph::dsl::{HasVal, IntoOptional, IntoVal};

pub struct HasPred;

pub trait HasConstraint {}
impl<V> HasConstraint for HasVal<V> {}
impl HasConstraint for HasPred {}

impl<T> IntoOptional<T> for HasPred {
    fn into_optional(self) -> Option<T> {
        None
    }
}
impl<T: Default> IntoVal<T> for HasPred {
    fn into_val(self) -> T {
        T::default()
    }
}
mod validate;

pub(crate) use validate::check_pattern;

pub use crate::graph::dsl::LocalId;
use crate::id;
use crate::graph;
pub use crate::search::{Decision, Morphism};
pub use crate::search::Morphism::*;
use std::marker::PhantomData;

pub struct UndirPending<N, E>(pub N, pub E);

pub enum Op<NV, ER: graph::Edge> {
    Free {
        id: Option<LocalId>,
        val: Option<NV>,
        node_pred: Option<Box<dyn Fn(&NV) -> bool + Send + Sync>>,
        negated: bool,
        edges: Vec<EdgeOp<NV, ER>>,
    },
    FreeRef {
        id: LocalId,
        edges: Vec<EdgeOp<NV, ER>>,
    },
    Exist {
        id: id::N,
        node_pred: Option<Box<dyn Fn(&NV) -> bool + Send + Sync>>,
        negated: bool,
        edges: Vec<EdgeOp<NV, ER>>,
    },
    ExistRef {
        id: id::N,
        edges: Vec<EdgeOp<NV, ER>>,
    },
    Context {
        id: LocalId,
        node_pred: Option<Box<dyn Fn(&NV) -> bool + Send + Sync>>,
        negated: bool,
        edges: Vec<EdgeOp<NV, ER>>,
    },
    ContextRef {
        id: LocalId,
        edges: Vec<EdgeOp<NV, ER>>,
    },
}

#[allow(dead_code)]
pub struct EdgeOp<NV, ER: graph::Edge> {
    pub slot: ER::Slot,
    pub val: ER::Val,
    pub edge_pred: Option<Box<dyn Fn(&ER::Val) -> bool + Send + Sync>>,
    pub target: Op<NV, ER>,
    pub negated: bool,
    pub any_slot: bool,
}

pub(crate) trait IntoOp<NV, ER: graph::Edge> {
    fn into_op(self) -> Op<NV, ER>;
}

pub struct ClusterOps<NV, ER: graph::Edge> {
    pub(crate) morphism: Morphism,
    pub(crate) decision: Decision,
    pub(crate) ops: Vec<Op<NV, ER>>,
}

pub fn get<NV, ER: graph::Edge>(morphism: Morphism, ops: Vec<Op<NV, ER>>) -> ClusterOps<NV, ER> {
    ClusterOps {
        morphism,
        decision: Decision::Get,
        ops,
    }
}

pub fn ban<NV, ER: graph::Edge>(morphism: Morphism, ops: Vec<Op<NV, ER>>) -> ClusterOps<NV, ER> {
    ClusterOps {
        morphism,
        decision: Decision::Ban,
        ops,
    }
}

#[allow(non_snake_case)]
pub fn N<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::Free<NV, (), ER> {
    node::Free {
        id: Some(local.into()),
        v: (),
        pred: None,
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn N_<NV, ER: graph::Edge>() -> node::Free<NV, (), ER> {
    node::Free {
        id: None,
        v: (),
        pred: None,
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn n<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::FreeRef<NV, ER> {
    node::FreeRef {
        id: local.into(),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn X<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::Context<NV, (), ER> {
    node::Context {
        id: local.into(),
        v: (),
        pred: None,
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn x<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::ContextRef<NV, ER> {
    node::ContextRef {
        id: local.into(),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn T<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::Context<NV, (), ER> {
    node::Context {
        id: local.into(),
        v: (),
        pred: None,
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn t<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::ContextRef<NV, ER> {
    node::ContextRef {
        id: local.into(),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn E<NV, ER: graph::Edge>() -> edge::Edge<(), NV, ER> {
    edge::Edge((), PhantomData, None)
}
