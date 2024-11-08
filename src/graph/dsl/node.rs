use super::edge::{self, Connected};
use super::{EdgeOp, IntoOp, Op, UndirPending};
use super::{HasVal, IntoVal, LocalId};
use crate::graph;
use crate::graph::edge::{Src, Tgt, Und};
use std::ops::{BitAnd, BitXor, Shl, Shr};

pub struct Node<NV, V, ER: graph::Edge> {
    pub(crate) id: Option<LocalId>,
    pub(crate) v: V,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

pub struct Ref<NV, ER: graph::Edge> {
    pub(crate) id: LocalId,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

impl<NV, ER: graph::Edge> Node<NV, (), ER> {
    pub fn val(self, v: NV) -> Node<NV, HasVal<NV>, ER> {
        Node {
            id: self.id,
            v: HasVal(v),
            edges: self.edges,
        }
    }
}

impl<NV, V: IntoVal<NV>, ER: graph::Edge> From<Node<NV, V, ER>> for Op<NV, ER> {
    fn from(n: Node<NV, V, ER>) -> Self {
        Op::Add {
            id: n.id,
            val: n.v.into_val(),
            edges: n.edges,
        }
    }
}

impl<NV, V: IntoVal<NV>, ER: graph::Edge> IntoOp<NV, ER> for Node<NV, V, ER> {
    fn into_op(self) -> Op<NV, ER> {
        self.into()
    }
}

impl<NV, ER: graph::Edge> From<Ref<NV, ER>> for Op<NV, ER> {
    fn from(n: Ref<NV, ER>) -> Self {
        Op::Ref {
            id: n.id,
            edges: n.edges,
        }
    }
}

impl<NV, ER: graph::Edge> IntoOp<NV, ER> for Ref<NV, ER> {
    fn into_op(self) -> Op<NV, ER> {
        self.into()
    }
}

macro_rules! impl_anon_edge_op {
    ($Self:ty, $V:ident, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, $V, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>> $Op<RHS> for $Self
        where
            ER::Val: Default,
        {
            type Output = Self;
            fn $op(mut self, rhs: RHS) -> Self {
                self.edges.push(EdgeOp {
                    slot: <ER as $Dir>::SLOT,
                    val: ER::Val::default(),
                    target: rhs.into_op(),
                });
                self
            }
        }
    };
    ($Self:ty, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>> $Op<RHS> for $Self
        where
            ER::Val: Default,
        {
            type Output = Self;
            fn $op(mut self, rhs: RHS) -> Self {
                self.edges.push(EdgeOp {
                    slot: <ER as $Dir>::SLOT,
                    val: ER::Val::default(),
                    target: rhs.into_op(),
                });
                self
            }
        }
    };
}

for_each_dir!(impl_anon_edge_op!(Node<NV, V, ER>, V));
for_each_dir!(impl_anon_edge_op!(Ref<NV, ER>));

macro_rules! impl_undir_op {
    ($NodeTy:ty, $V:ident, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, $V, EV: IntoVal<ER::Val>, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>>
            $Op<RHS> for UndirPending<$NodeTy, edge::Edge<EV, NV, ER>>
        {
            type Output = $NodeTy;
            fn $op(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(EdgeOp {
                    slot: <ER as $Dir>::SLOT,
                    val: self.1 .0.into_val(),
                    target: rhs.into_op(),
                });
                node
            }
        }
    };
    ($NodeTy:ty, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, EV: IntoVal<ER::Val>, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>>
            $Op<RHS> for UndirPending<$NodeTy, edge::Edge<EV, NV, ER>>
        {
            type Output = $NodeTy;
            fn $op(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(EdgeOp {
                    slot: <ER as $Dir>::SLOT,
                    val: self.1 .0.into_val(),
                    target: rhs.into_op(),
                });
                node
            }
        }
    };
}

for_each_dir!(impl_undir_op!(Node<NV, V, ER>, V));
for_each_dir!(impl_undir_op!(Ref<NV, ER>));

macro_rules! impl_bitand_connected {
    ($Self:ty, $V:ident) => {
        impl<NV, $V, ER: graph::Edge> BitAnd<edge::Edge<Connected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: edge::Edge<Connected<NV, ER>, NV, ER>) -> Self {
                self.edges.push(EdgeOp {
                    slot: arm.0.slot,
                    val: arm.0.val,
                    target: arm.0.target,
                });
                self
            }
        }
    };
    ($Self:ty) => {
        impl<NV, ER: graph::Edge> BitAnd<edge::Edge<Connected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: edge::Edge<Connected<NV, ER>, NV, ER>) -> Self {
                self.edges.push(EdgeOp {
                    slot: arm.0.slot,
                    val: arm.0.val,
                    target: arm.0.target,
                });
                self
            }
        }
    };
}

impl_bitand_connected!(Node<NV, V, ER>, V);
impl_bitand_connected!(Ref<NV, ER>);

macro_rules! impl_bitand_undir_pending {
    ($Self:ty, $EdgeTy:ty, $V:ident) => {
        impl<NV, $V, ER: graph::Edge> BitAnd<$EdgeTy> for $Self {
            type Output = UndirPending<Self, $EdgeTy>;
            fn bitand(self, edge: $EdgeTy) -> Self::Output {
                UndirPending(self, edge)
            }
        }
    };
    ($Self:ty, $EdgeTy:ty) => {
        impl<NV, ER: graph::Edge> BitAnd<$EdgeTy> for $Self {
            type Output = UndirPending<Self, $EdgeTy>;
            fn bitand(self, edge: $EdgeTy) -> Self::Output {
                UndirPending(self, edge)
            }
        }
    };
}

impl_bitand_undir_pending!(Node<NV, V, ER>, edge::Edge<(), NV, ER>, V);
impl_bitand_undir_pending!(Node<NV, V, ER>, edge::Edge<HasVal<ER::Val>, NV, ER>, V);
impl_bitand_undir_pending!(Ref<NV, ER>, edge::Edge<(), NV, ER>);
impl_bitand_undir_pending!(Ref<NV, ER>, edge::Edge<HasVal<ER::Val>, NV, ER>);
