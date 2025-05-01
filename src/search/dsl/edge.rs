use super::{IntoOp, Op};
use super::HasPred;
use crate::graph::dsl::{HasVal, IntoVal};
use crate::graph;
use crate::graph::edge::{Src, Tgt, Und};
use std::marker::PhantomData;
use std::ops::{BitXor, Not, Rem, Shl, Shr};

pub struct Edge<EV, NV, ER: graph::Edge>(
    pub(crate) EV,
    pub(crate) PhantomData<(NV, ER)>,
    pub(crate) Option<Box<dyn Fn(&ER::Val) -> bool + Send + Sync>>,
);

pub struct Connected<NV, ER: graph::Edge> {
    pub(crate) slot: ER::Slot,
    pub(crate) val: ER::Val,
    pub(crate) pred: Option<Box<dyn Fn(&ER::Val) -> bool + Send + Sync>>,
    pub(crate) target: Op<NV, ER>,
    pub(crate) any_slot: bool,
}

impl<NV, ER: graph::Edge> Edge<(), NV, ER>
where
    ER::Val: PartialEq + Copy + Send + Sync + 'static,
{
    pub fn val(self, v: impl Into<ER::Val>) -> Edge<HasVal<ER::Val>, NV, ER> {
        let v = v.into();
        Edge(HasVal(v), PhantomData, Some(Box::new(move |x| *x == v)))
    }
}

impl<NV, ER: graph::Edge> Edge<(), NV, ER> {
    pub fn test(self, f: impl Fn(&ER::Val) -> bool + Send + Sync + 'static) -> Edge<HasPred, NV, ER> {
        Edge(HasPred, PhantomData, Some(Box::new(f)))
    }
}

macro_rules! impl_connect_op {
    ($Dir:ident, $Op:ident, $op:ident) => {
        impl<EV: IntoVal<ER::Val>, NV, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>>
            $Op<RHS> for Edge<EV, NV, ER>
        {
            type Output = Edge<Connected<NV, ER>, NV, ER>;
            fn $op(self, rhs: RHS) -> Self::Output {
                Edge(
                    Connected {
                        slot: <ER as $Dir>::SLOT,
                        val: self.0.into_val(),
                        pred: self.2,
                        target: rhs.into_op(),
                        any_slot: false,
                    },
                    PhantomData,
                    None,
                )
            }
        }
    };
}

for_each_dir!(impl_connect_op!());

pub struct NegEdge<EV, NV, ER: graph::Edge>(
    pub(crate) EV,
    pub(crate) PhantomData<(NV, ER)>,
    pub(crate) Option<Box<dyn Fn(&ER::Val) -> bool + Send + Sync>>,
);

impl<NV, ER: graph::Edge> Not for Edge<(), NV, ER> {
    type Output = NegEdge<(), NV, ER>;
    fn not(self) -> Self::Output {
        NegEdge((), PhantomData, None)
    }
}

impl<NV, ER: graph::Edge> Not for Edge<HasVal<ER::Val>, NV, ER> {
    type Output = NegEdge<HasVal<ER::Val>, NV, ER>;
    fn not(self) -> Self::Output {
        NegEdge(self.0, PhantomData, self.2)
    }
}

impl<NV, ER: graph::Edge> Not for Edge<HasPred, NV, ER> {
    type Output = NegEdge<HasPred, NV, ER>;
    fn not(self) -> Self::Output {
        NegEdge(self.0, PhantomData, self.2)
    }
}

impl<NV, ER: graph::Edge> NegEdge<(), NV, ER>
where
    ER::Val: PartialEq + Copy + Send + Sync + 'static,
{
    pub fn val(self, v: impl Into<ER::Val>) -> NegEdge<HasVal<ER::Val>, NV, ER> {
        let v = v.into();
        NegEdge(HasVal(v), PhantomData, Some(Box::new(move |x| *x == v)))
    }
}

impl<NV, ER: graph::Edge> NegEdge<(), NV, ER> {
    pub fn test(self, f: impl Fn(&ER::Val) -> bool + Send + Sync + 'static) -> NegEdge<HasPred, NV, ER> {
        NegEdge(HasPred, PhantomData, Some(Box::new(f)))
    }
}

macro_rules! impl_neg_connect_op {
    ($Dir:ident, $Op:ident, $op:ident) => {
        impl<EV: IntoVal<ER::Val>, NV, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>>
            $Op<RHS> for NegEdge<EV, NV, ER>
        where
            ER::Val: Default,
        {
            type Output = NegEdge<Connected<NV, ER>, NV, ER>;
            fn $op(self, rhs: RHS) -> Self::Output {
                NegEdge(
                    Connected {
                        slot: <ER as $Dir>::SLOT,
                        val: self.0.into_val(),
                        pred: self.2,
                        target: rhs.into_op(),
                        any_slot: false,
                    },
                    PhantomData,
                    None,
                )
            }
        }
    };
}

for_each_dir!(impl_neg_connect_op!());

impl<EV: IntoVal<ER::Val>, NV, ER: graph::Edge, RHS: IntoOp<NV, ER>>
    Rem<RHS> for Edge<EV, NV, ER>
{
    type Output = Edge<Connected<NV, ER>, NV, ER>;
    fn rem(self, rhs: RHS) -> Self::Output {
        Edge(
            Connected {
                slot: ER::SLOT_MIN,
                val: self.0.into_val(),
                pred: self.2,
                target: rhs.into_op(),
                any_slot: true,
            },
            PhantomData,
            None,
        )
    }
}

impl<EV: IntoVal<ER::Val>, NV, ER: graph::Edge, RHS: IntoOp<NV, ER>>
    Rem<RHS> for NegEdge<EV, NV, ER>
where
    ER::Val: Default,
{
    type Output = NegEdge<Connected<NV, ER>, NV, ER>;
    fn rem(self, rhs: RHS) -> Self::Output {
        NegEdge(
            Connected {
                slot: ER::SLOT_MIN,
                val: self.0.into_val(),
                pred: self.2,
                target: rhs.into_op(),
                any_slot: true,
            },
            PhantomData,
            None,
        )
    }
}
