use super::node;
use super::{ExistEdgeSource, HasVal, IntoExistNode, IntoNode, IntoOptional, IntoVal, Node, UndirPending};
use crate::graph;
use crate::graph::dsl::{HasRawVal, IsFullVal};
use crate::graph::edge::{DirSlot, SlotVal, Src, Tgt, Und, UndirSlot};
use crate::composite::{Composite, Has, KindOf, Kinded};
use crate::modify::part::{ExcludePart, IncludePart, Part, SlotClassed, Through};
use std::marker::PhantomData;
use std::ops::{BitAnd, BitXor, Shl, Shr};

pub mod new {
    use super::*;

    pub struct Edge<EV, NV, ER: graph::Edge>(pub(crate) EV, pub(crate) PhantomData<(NV, ER)>);

    pub struct PendingInclude<T>(pub(crate) T);

    pub struct PendingExclude<T>(pub(crate) PhantomData<fn() -> T>);

    pub struct PendingExcludeKind<K>(pub(crate) K);

    impl<NV, ER: graph::Edge> Edge<(), NV, ER> {
        pub fn val<V>(self, v: V) -> Edge<HasRawVal<V>, NV, ER> {
            Edge(HasRawVal(v), PhantomData)
        }

        pub fn include<T>(self, part: T) -> Edge<PendingInclude<T>, NV, ER> {
            Edge(PendingInclude(part), PhantomData)
        }

        pub fn exclude<T>(self) -> Edge<PendingExclude<T>, NV, ER> {
            Edge(PendingExclude(PhantomData), PhantomData)
        }

        pub fn exclude_kind<K>(self, kind: K) -> Edge<PendingExcludeKind<K>, NV, ER> {
            Edge(PendingExcludeKind(kind), PhantomData)
        }
    }
}

mod sealed { pub trait Sealed {} }

impl<T> sealed::Sealed for new::PendingInclude<T> {}
impl<T> sealed::Sealed for new::PendingExclude<T> {}
impl<K> sealed::Sealed for new::PendingExcludeKind<K> {}

pub trait PartTerm<ER: graph::Edge, S>: sealed::Sealed {
    fn into_part(self) -> Part<ER::Val>;
}

impl<ER, S, C, T> PartTerm<ER, S> for new::PendingInclude<T>
where
    ER: graph::Edge + SlotVal<S, SlotType = C> + 'static,
    S: SlotClassed + 'static,
    ER::Val: PartialEq + 'static,
    C: Composite,
    T: Into<C::Part>,
    C::Part: Send + Sync + 'static,
    C::IncludeRefused: Send + Sync + 'static,
{
    fn into_part(self) -> Part<ER::Val> {
        IncludePart::slotted::<Through<ER, S>>(self.0.into()).into()
    }
}

impl<ER, S, C, T> PartTerm<ER, S> for new::PendingExclude<T>
where
    ER: graph::Edge + SlotVal<S, SlotType = C> + 'static,
    S: SlotClassed + 'static,
    ER::Val: PartialEq + 'static,
    C: Has<T>,
    KindOf<C>: Send + Sync + 'static,
    C::ExcludeRefused: Send + Sync + 'static,
{
    fn into_part(self) -> Part<ER::Val> {
        ExcludePart::slotted::<Through<ER, S>>(<C as Has<T>>::part_kind()).into()
    }
}

impl<ER, S, C, K> PartTerm<ER, S> for new::PendingExcludeKind<K>
where
    ER: graph::Edge + SlotVal<S, SlotType = C> + 'static,
    S: SlotClassed + 'static,
    ER::Val: PartialEq + 'static,
    C: Composite,
    C::Part: Kinded<Kind = K>,
    K: Send + Sync + 'static,
    C::ExcludeRefused: Send + Sync + 'static,
{
    fn into_part(self) -> Part<ER::Val> {
        ExcludePart::slotted::<Through<ER, S>>(self.0).into()
    }
}

pub mod exist {
    use super::*;
    use std::ops::Not;

    pub struct Edge<EV, NV, ER: graph::Edge>(pub(crate) EV, pub(crate) PhantomData<(NV, ER)>);
    pub struct Rem<S, NV, ER: graph::Edge>(pub(crate) S, pub(crate) PhantomData<(NV, ER)>);

    impl<NV, ER: graph::Edge> Edge<(), NV, ER> {
        pub fn val<V>(self, v: V) -> Edge<HasRawVal<V>, NV, ER> {
            Edge(HasRawVal(v), PhantomData)
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

pub struct PartConnected<NV, ER: graph::Edge> {
    pub(crate) slot: ER::Slot,
    pub(crate) op: Part<ER::Val>,
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
    Part {
        slot: ER::Slot,
        op: Part<ER::Val>,
        target: Node<NV, ER>,
    },
}

// Edge connect ops: edge >> node, edge << node, edge ^ node
// Produces a connected edge builder with the resolved direction and target.

macro_rules! impl_connect_new_op {
    ($Dir:ident, $SlotDir:ident, $Op:ident, $op:ident) => {
        // No-value edge: E() >> Node
        impl<EV: IntoVal<ER::Val> + IsFullVal, NV, ER: graph::Edge + $Dir, RHS: IntoNode<NV, ER>>
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

        // Slot-typed value: E().val(slot_val) >> Node
        impl<V, NV, ER: graph::Edge + $Dir + SlotVal<$SlotDir, SlotType = V>, RHS: IntoNode<NV, ER>>
            $Op<RHS> for new::Edge<HasRawVal<V>, NV, ER>
        {
            type Output = new::Edge<NewConnected<NV, ER>, NV, ER>;
            fn $op(self, rhs: RHS) -> Self::Output {
                new::Edge(
                    NewConnected {
                        slot: <ER as $Dir>::SLOT,
                        val: ER::wrap_slot_val((self.0).0),
                        target: rhs.into_node(),
                    },
                    PhantomData,
                )
            }
        }
    };
}

for_each_dir!(impl_connect_new_op!());

macro_rules! impl_connect_part_op {
    ($Pending:ident, $Dir:ident, $SlotDir:ident, $Op:ident, $op:ident) => {
        impl<X, NV, ER: graph::Edge + $Dir, RHS: IntoNode<NV, ER>> $Op<RHS> for new::Edge<new::$Pending<X>, NV, ER>
        where
            new::$Pending<X>: PartTerm<ER, $SlotDir>,
        {
            type Output = new::Edge<PartConnected<NV, ER>, NV, ER>;
            fn $op(self, rhs: RHS) -> Self::Output {
                new::Edge(
                    PartConnected {
                        slot: <ER as $Dir>::SLOT,
                        op: self.0.into_part(),
                        target: rhs.into_node(),
                    },
                    PhantomData,
                )
            }
        }
    };
}

for_each_dir!(impl_connect_part_op!(PendingInclude));
for_each_dir!(impl_connect_part_op!(PendingExclude));
for_each_dir!(impl_connect_part_op!(PendingExcludeKind));

macro_rules! impl_connect_exist_op {
    ($Dir:ident, $SlotDir:ident, $Op:ident, $op:ident) => {
        // No-value edge: e() >> ExistNode
        impl<EV: IntoOptional<ER::Val> + IsFullVal, NV, ER: graph::Edge + $Dir, RHS: IntoExistNode<NV, ER>>
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

        // Slot-typed value: e().val(slot_val) >> ExistNode
        impl<V, NV, ER: graph::Edge + $Dir + SlotVal<$SlotDir, SlotType = V>, RHS: IntoExistNode<NV, ER>>
            $Op<RHS> for exist::Edge<HasRawVal<V>, NV, ER>
        {
            type Output = exist::Edge<Connected<NV, ER>, NV, ER>;
            fn $op(self, rhs: RHS) -> Self::Output {
                exist::Edge(
                    Connected {
                        slot: <ER as $Dir>::SLOT,
                        val: Some(ER::wrap_slot_val((self.0).0)),
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
    ($Dir:ident, $_SlotDir:ident, $Op:ident, $op:ident) => {
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

macro_rules! impl_bitand_connected_part {
    ($Self:ty, $V:ident) => {
        impl<NV, $V, ER: graph::Edge> BitAnd<new::Edge<PartConnected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: new::Edge<PartConnected<NV, ER>, NV, ER>) -> Self {
                self.edges.push(Edge::Part { slot: arm.0.slot, op: arm.0.op, target: arm.0.target });
                self
            }
        }
    };
    ($Self:ty) => {
        impl<NV, ER: graph::Edge> BitAnd<new::Edge<PartConnected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: new::Edge<PartConnected<NV, ER>, NV, ER>) -> Self {
                self.edges.push(Edge::Part { slot: arm.0.slot, op: arm.0.op, target: arm.0.target });
                self
            }
        }
    };
}

impl_bitand_connected_part!(node::new::Node<NV, V, ER>, V);
impl_bitand_connected_part!(node::exist::Node<NV, V, ER>, V);
impl_bitand_connected_part!(node::translated::Node<NV, V, ER>, V);
impl_bitand_connected_part!(node::new::Ref<NV, ER>);
impl_bitand_connected_part!(node::exist::Ref<NV, ER>);
impl_bitand_connected_part!(node::translated::Ref<NV, ER>);

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

macro_rules! impl_bitand_part_pending {
    ($(#[$attr:meta])* $Self:ty, [$($G:ident),*]) => {
        impl_bitand_part_pending!(@one $(#[$attr])* $Self, [$($G),*], PendingInclude);
        impl_bitand_part_pending!(@one $(#[$attr])* $Self, [$($G),*], PendingExclude);
        impl_bitand_part_pending!(@one $(#[$attr])* $Self, [$($G),*], PendingExcludeKind);
    };
    (@one $(#[$attr:meta])* $Self:ty, [$($G:ident),*], $Pending:ident) => {
        $(#[$attr])*
        impl<NV, $($G,)* X, ER: graph::Edge> BitAnd<new::Edge<new::$Pending<X>, NV, ER>> for $Self {
            type Output = UndirPending<Self, new::Edge<new::$Pending<X>, NV, ER>>;
            fn bitand(self, edge: new::Edge<new::$Pending<X>, NV, ER>) -> Self::Output {
                UndirPending(self, edge)
            }
        }
    };
}

impl_bitand_part_pending!(#[diagnostic::do_not_recommend] node::new::Node<NV, V, ER>, [V]);
impl_bitand_part_pending!(#[diagnostic::do_not_recommend] node::new::Ref<NV, ER>, []);
impl_bitand_part_pending!(node::exist::Node<NV, V, ER>, [V]);
impl_bitand_part_pending!(node::exist::Ref<NV, ER>, []);
impl_bitand_part_pending!(node::translated::Node<NV, V, ER>, [V]);
impl_bitand_part_pending!(node::translated::Ref<NV, ER>, []);

// HasRawVal variants: Node & E().val(slot_val) / e().val(slot_val) creates UndirPending
macro_rules! impl_bitand_rawval_pending {
    (@dnr, $Self:ty, $EdgeTy:ident, $V:ident) => {
        #[diagnostic::do_not_recommend]
        impl<NV, $V, V_, ER: graph::Edge> BitAnd<$EdgeTy::Edge<HasRawVal<V_>, NV, ER>> for $Self {
            type Output = UndirPending<Self, $EdgeTy::Edge<HasRawVal<V_>, NV, ER>>;
            fn bitand(self, edge: $EdgeTy::Edge<HasRawVal<V_>, NV, ER>) -> Self::Output {
                UndirPending(self, edge)
            }
        }
    };
    (@dnr, $Self:ty, $EdgeTy:ident) => {
        #[diagnostic::do_not_recommend]
        impl<NV, V_, ER: graph::Edge> BitAnd<$EdgeTy::Edge<HasRawVal<V_>, NV, ER>> for $Self {
            type Output = UndirPending<Self, $EdgeTy::Edge<HasRawVal<V_>, NV, ER>>;
            fn bitand(self, edge: $EdgeTy::Edge<HasRawVal<V_>, NV, ER>) -> Self::Output {
                UndirPending(self, edge)
            }
        }
    };
    ($Self:ty, $EdgeTy:ident, $V:ident) => {
        impl<NV, $V, V_, ER: graph::Edge> BitAnd<$EdgeTy::Edge<HasRawVal<V_>, NV, ER>> for $Self {
            type Output = UndirPending<Self, $EdgeTy::Edge<HasRawVal<V_>, NV, ER>>;
            fn bitand(self, edge: $EdgeTy::Edge<HasRawVal<V_>, NV, ER>) -> Self::Output {
                UndirPending(self, edge)
            }
        }
    };
    ($Self:ty, $EdgeTy:ident) => {
        impl<NV, V_, ER: graph::Edge> BitAnd<$EdgeTy::Edge<HasRawVal<V_>, NV, ER>> for $Self {
            type Output = UndirPending<Self, $EdgeTy::Edge<HasRawVal<V_>, NV, ER>>;
            fn bitand(self, edge: $EdgeTy::Edge<HasRawVal<V_>, NV, ER>) -> Self::Output {
                UndirPending(self, edge)
            }
        }
    };
}

// new::Edge<HasRawVal<V_>>
impl_bitand_rawval_pending!(@dnr, node::new::Node<NV, V, ER>, new, V);
impl_bitand_rawval_pending!(@dnr, node::new::Ref<NV, ER>, new);
impl_bitand_rawval_pending!(node::exist::Node<NV, V, ER>, new, V);
impl_bitand_rawval_pending!(node::exist::Ref<NV, ER>, new);
impl_bitand_rawval_pending!(node::translated::Node<NV, V, ER>, new, V);
impl_bitand_rawval_pending!(node::translated::Ref<NV, ER>, new);

// exist::Edge<HasRawVal<V_>>
impl_bitand_rawval_pending!(node::exist::Node<NV, V, ER>, exist, V);
impl_bitand_rawval_pending!(node::exist::Ref<NV, ER>, exist);
impl_bitand_rawval_pending!(node::translated::Node<NV, V, ER>, exist, V);
impl_bitand_rawval_pending!(node::translated::Ref<NV, ER>, exist);

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
