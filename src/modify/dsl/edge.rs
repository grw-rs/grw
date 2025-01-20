use super::node;
use super::{ExistEdgeSource, HasVal, IntoExistNode, IntoNode, IntoOptional, IntoVal, Node, UndirPending};
use crate::graph;
use crate::graph::edge::{Src, Tgt, Und};
use std::marker::PhantomData;
use std::ops::{BitAnd, BitXor, Shl, Shr};

pub mod new {
    use super::*;

    pub struct Edge<EV, NV, ER: graph::Edge>(pub(crate) EV, pub(crate) PhantomData<(NV, ER)>);

    impl<NV, ER: graph::Edge> Edge<(), NV, ER> {
        pub fn val(self, v: impl Into<ER::Val>) -> Edge<HasVal<ER::Val>, NV, ER> {
            Edge(HasVal(v.into()), PhantomData)
        }
    }
}

pub mod exist {
    use super::*;
    use std::ops::Not;

    pub struct Edge<EV, NV, ER: graph::Edge>(pub(crate) EV, pub(crate) PhantomData<(NV, ER)>);
    pub struct Rem<S, NV, ER: graph::Edge>(pub(crate) S, pub(crate) PhantomData<(NV, ER)>);

    impl<NV, ER: graph::Edge> Edge<(), NV, ER> {
        pub fn val(self, v: impl Into<ER::Val>) -> Edge<HasVal<ER::Val>, NV, ER> {
            Edge(HasVal(v.into()), PhantomData)
        }
    }

    impl<NV, ER: graph::Edge> Not for Edge<(), NV, ER> {
        type Output = Rem<(), NV, ER>;
        fn not(self) -> Rem<(), NV, ER> {
            Rem((), PhantomData)
        }
    }
}

pub struct NewConnected<NV, ER: graph::Edge> {
    pub(crate) slot: ER::Slot,
    pub(crate) val: ER::Val,
    pub(crate) target: Node<NV, ER>,
}

pub struct Connected<NV, ER: graph::Edge> {
    pub(crate) slot: ER::Slot,
    pub(crate) val: Option<ER::Val>,
    pub(crate) target: Node<NV, ER>,
}

pub enum Bind<EV> {
    Pass,
    Swap(EV),
}

pub enum Exist<EV> {
    Bind(Bind<EV>),
    Rem,
}

pub enum Edge<NV, ER: graph::Edge> {
    New {
        slot: ER::Slot,
        val: ER::Val,
        target: Node<NV, ER>,
    },
    Exist {
        slot: ER::Slot,
        op: Exist<ER::Val>,
        target: Node<NV, ER>,
    },
}

// Edge connect ops: edge >> node, edge << node, edge ^ node
// Produces a connected edge builder with the resolved direction and target.

macro_rules! impl_connect_new_op {
    ($Dir:ident, $Op:ident, $op:ident) => {
        impl<EV: IntoVal<ER::Val>, NV, ER: graph::Edge + $Dir, RHS: IntoNode<NV, ER>>
            $Op<RHS> for new::Edge<EV, NV, ER>
        {
            type Output = new::Edge<NewConnected<NV, ER>, NV, ER>;
            fn $op(self, rhs: RHS) -> Self::Output {
                new::Edge(
                    NewConnected {
                        slot: <ER as $Dir>::SLOT,
                        val: self.0.into_val(),
                        target: rhs.into_node(),
                    },
                    PhantomData,
                )
            }
        }
    };
}

for_each_dir!(impl_connect_new_op!());

macro_rules! impl_connect_exist_op {
    ($Dir:ident, $Op:ident, $op:ident) => {
        impl<EV: IntoOptional<ER::Val>, NV, ER: graph::Edge + $Dir, RHS: IntoExistNode<NV, ER>>
            $Op<RHS> for exist::Edge<EV, NV, ER>
        {
            type Output = exist::Edge<Connected<NV, ER>, NV, ER>;
            fn $op(self, rhs: RHS) -> Self::Output {
                exist::Edge(
                    Connected {
                        slot: <ER as $Dir>::SLOT,
                        val: self.0.into_optional(),
                        target: rhs.into_exist_node(),
                    },
                    PhantomData,
                )
            }
        }
    };
}

for_each_dir!(impl_connect_exist_op!());

macro_rules! impl_connect_rem_op {
    ($Dir:ident, $Op:ident, $op:ident) => {
        impl<EV: IntoOptional<ER::Val>, NV, ER: graph::Edge + $Dir, RHS: IntoExistNode<NV, ER>>
            $Op<RHS> for exist::Rem<EV, NV, ER>
        {
            type Output = exist::Rem<Connected<NV, ER>, NV, ER>;
            fn $op(self, rhs: RHS) -> Self::Output {
                exist::Rem(
                    Connected {
                        slot: <ER as $Dir>::SLOT,
                        val: self.0.into_optional(),
                        target: rhs.into_exist_node(),
                    },
                    PhantomData,
                )
            }
        }
    };
}

for_each_dir!(impl_connect_rem_op!());

// node & connected_edge -> node with edge appended

macro_rules! impl_bitand_connected_new {
    (@dnr, $Self:ty, $V:ident) => {
        #[diagnostic::do_not_recommend]
        impl<NV, $V, ER: graph::Edge> BitAnd<new::Edge<NewConnected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: new::Edge<NewConnected<NV, ER>, NV, ER>) -> Self {
                self.edges
                    .push(Edge::New { slot: arm.0.slot, val: arm.0.val, target: arm.0.target });
                self
            }
        }
    };
    (@dnr, $Self:ty) => {
        #[diagnostic::do_not_recommend]
        impl<NV, ER: graph::Edge> BitAnd<new::Edge<NewConnected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: new::Edge<NewConnected<NV, ER>, NV, ER>) -> Self {
                self.edges
                    .push(Edge::New { slot: arm.0.slot, val: arm.0.val, target: arm.0.target });
                self
            }
        }
    };
    ($Self:ty, $V:ident) => {
        impl<NV, $V, ER: graph::Edge> BitAnd<new::Edge<NewConnected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: new::Edge<NewConnected<NV, ER>, NV, ER>) -> Self {
                self.edges
                    .push(Edge::New { slot: arm.0.slot, val: arm.0.val, target: arm.0.target });
                self
            }
        }
    };
    ($Self:ty) => {
        impl<NV, ER: graph::Edge> BitAnd<new::Edge<NewConnected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: new::Edge<NewConnected<NV, ER>, NV, ER>) -> Self {
                self.edges
                    .push(Edge::New { slot: arm.0.slot, val: arm.0.val, target: arm.0.target });
                self
            }
        }
    };
}

impl_bitand_connected_new!(@dnr, node::new::Node<NV, V, ER>, V);
impl_bitand_connected_new!(node::exist::Node<NV, V, ER>, V);
impl_bitand_connected_new!(node::translated::Node<NV, V, ER>, V);
impl_bitand_connected_new!(@dnr, node::new::Ref<NV, ER>);
impl_bitand_connected_new!(node::exist::Ref<NV, ER>);
impl_bitand_connected_new!(node::translated::Ref<NV, ER>);

macro_rules! impl_bitand_connected_exist {
    ($Self:ty, $V:ident) => {
        impl<NV, $V, ER: graph::Edge> BitAnd<exist::Edge<Connected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: exist::Edge<Connected<NV, ER>, NV, ER>) -> Self {
                let op = Exist::Bind(match arm.0.val {
                    Some(v) => Bind::Swap(v),
                    None => Bind::Pass,
                });
                self.edges.push(Edge::Exist { slot: arm.0.slot, op, target: arm.0.target });
                self
            }
        }
    };
    ($Self:ty) => {
        impl<NV, ER: graph::Edge> BitAnd<exist::Edge<Connected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: exist::Edge<Connected<NV, ER>, NV, ER>) -> Self {
                let op = Exist::Bind(match arm.0.val {
                    Some(v) => Bind::Swap(v),
                    None => Bind::Pass,
                });
                self.edges.push(Edge::Exist { slot: arm.0.slot, op, target: arm.0.target });
                self
            }
        }
    };
}

impl_bitand_connected_exist!(node::exist::Node<NV, V, ER>, V);
impl_bitand_connected_exist!(node::exist::Ref<NV, ER>);
impl_bitand_connected_exist!(node::translated::Node<NV, V, ER>, V);
impl_bitand_connected_exist!(node::translated::Ref<NV, ER>);

macro_rules! impl_bitand_connected_rem {
    ($Self:ty, $V:ident) => {
        impl<NV, $V, ER: graph::Edge> BitAnd<exist::Rem<Connected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: exist::Rem<Connected<NV, ER>, NV, ER>) -> Self {
                self.edges.push(Edge::Exist {
                    slot: arm.0.slot,
                    op: Exist::Rem,
                    target: arm.0.target,
                });
                self
            }
        }
    };
    ($Self:ty) => {
        impl<NV, ER: graph::Edge> BitAnd<exist::Rem<Connected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: exist::Rem<Connected<NV, ER>, NV, ER>) -> Self {
                self.edges.push(Edge::Exist {
                    slot: arm.0.slot,
                    op: Exist::Rem,
                    target: arm.0.target,
                });
                self
            }
        }
    };
}

impl_bitand_connected_rem!(node::exist::Node<NV, V, ER>, V);
impl_bitand_connected_rem!(node::exist::Ref<NV, ER>);
impl_bitand_connected_rem!(node::translated::Node<NV, V, ER>, V);
impl_bitand_connected_rem!(node::translated::Ref<NV, ER>);

// node & unconnected_edge -> UndirPending, waiting for a direction operator

macro_rules! impl_bitand_undir_pending {
    (@dnr, $Self:ty, $EdgeTy:ty, $V:ident) => {
        #[diagnostic::do_not_recommend]
        impl<NV, $V, ER: graph::Edge> BitAnd<$EdgeTy> for $Self {
            type Output = UndirPending<Self, $EdgeTy>;
            fn bitand(self, edge: $EdgeTy) -> Self::Output {
                UndirPending(self, edge)
            }
        }
    };
    (@dnr, $Self:ty, $EdgeTy:ty) => {
        #[diagnostic::do_not_recommend]
        impl<NV, ER: graph::Edge> BitAnd<$EdgeTy> for $Self {
            type Output = UndirPending<Self, $EdgeTy>;
            fn bitand(self, edge: $EdgeTy) -> Self::Output {
                UndirPending(self, edge)
            }
        }
    };
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

impl_bitand_undir_pending!(@dnr, node::new::Node<NV, V, ER>, new::Edge<(), NV, ER>, V);
impl_bitand_undir_pending!(@dnr, node::new::Node<NV, V, ER>, new::Edge<HasVal<ER::Val>, NV, ER>, V);
impl_bitand_undir_pending!(node::exist::Node<NV, V, ER>, new::Edge<(), NV, ER>, V);
impl_bitand_undir_pending!(node::exist::Node<NV, V, ER>, new::Edge<HasVal<ER::Val>, NV, ER>, V);
impl_bitand_undir_pending!(node::exist::Node<NV, V, ER>, exist::Edge<(), NV, ER>, V);
impl_bitand_undir_pending!(node::exist::Node<NV, V, ER>, exist::Edge<HasVal<ER::Val>, NV, ER>, V);
impl_bitand_undir_pending!(node::exist::Node<NV, V, ER>, exist::Rem<(), NV, ER>, V);
impl_bitand_undir_pending!(@dnr, node::new::Ref<NV, ER>, new::Edge<(), NV, ER>);
impl_bitand_undir_pending!(@dnr, node::new::Ref<NV, ER>, new::Edge<HasVal<ER::Val>, NV, ER>);
impl_bitand_undir_pending!(node::exist::Ref<NV, ER>, new::Edge<(), NV, ER>);
impl_bitand_undir_pending!(node::exist::Ref<NV, ER>, new::Edge<HasVal<ER::Val>, NV, ER>);
impl_bitand_undir_pending!(node::exist::Ref<NV, ER>, exist::Edge<(), NV, ER>);
impl_bitand_undir_pending!(node::exist::Ref<NV, ER>, exist::Edge<HasVal<ER::Val>, NV, ER>);
impl_bitand_undir_pending!(node::exist::Ref<NV, ER>, exist::Rem<(), NV, ER>);
impl_bitand_undir_pending!(node::translated::Node<NV, V, ER>, new::Edge<(), NV, ER>, V);
impl_bitand_undir_pending!(node::translated::Node<NV, V, ER>, new::Edge<HasVal<ER::Val>, NV, ER>, V);
impl_bitand_undir_pending!(node::translated::Node<NV, V, ER>, exist::Edge<(), NV, ER>, V);
impl_bitand_undir_pending!(node::translated::Node<NV, V, ER>, exist::Edge<HasVal<ER::Val>, NV, ER>, V);
impl_bitand_undir_pending!(node::translated::Node<NV, V, ER>, exist::Rem<(), NV, ER>, V);
impl_bitand_undir_pending!(node::translated::Ref<NV, ER>, new::Edge<(), NV, ER>);
impl_bitand_undir_pending!(node::translated::Ref<NV, ER>, new::Edge<HasVal<ER::Val>, NV, ER>);
impl_bitand_undir_pending!(node::translated::Ref<NV, ER>, exist::Edge<(), NV, ER>);
impl_bitand_undir_pending!(node::translated::Ref<NV, ER>, exist::Edge<HasVal<ER::Val>, NV, ER>);
impl_bitand_undir_pending!(node::translated::Ref<NV, ER>, exist::Rem<(), NV, ER>);

// Catch-all impls: fire ExistEdgeSource diagnostic when a new node is used as the source
// of an existing-edge op. Never reachable; ExistEdgeSource is not impl'd for new nodes.

impl<NV, V, ER: graph::Edge, EV> BitAnd<exist::Edge<EV, NV, ER>> for node::new::Node<NV, V, ER>
where
    node::new::Node<NV, V, ER>: ExistEdgeSource,
{
    type Output = Self;
    fn bitand(self, _: exist::Edge<EV, NV, ER>) -> Self { unreachable!() }
}

impl<NV, V, ER: graph::Edge, EV> BitAnd<exist::Rem<EV, NV, ER>> for node::new::Node<NV, V, ER>
where
    node::new::Node<NV, V, ER>: ExistEdgeSource,
{
    type Output = Self;
    fn bitand(self, _: exist::Rem<EV, NV, ER>) -> Self { unreachable!() }
}

impl<NV, ER: graph::Edge, EV> BitAnd<exist::Edge<EV, NV, ER>> for node::new::Ref<NV, ER>
where
    node::new::Ref<NV, ER>: ExistEdgeSource,
{
    type Output = Self;
    fn bitand(self, _: exist::Edge<EV, NV, ER>) -> Self { unreachable!() }
}

impl<NV, ER: graph::Edge, EV> BitAnd<exist::Rem<EV, NV, ER>> for node::new::Ref<NV, ER>
where
    node::new::Ref<NV, ER>: ExistEdgeSource,
{
    type Output = Self;
    fn bitand(self, _: exist::Rem<EV, NV, ER>) -> Self { unreachable!() }
}
