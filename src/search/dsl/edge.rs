use super::{IntoOp, Op};
use super::HasPred;
use crate::graph::dsl::{HasVal, HasRawVal, IntoVal};
use crate::graph;
use crate::graph::edge::{Src, Tgt, Und, SlotVal, UndirSlot, DirSlot};
use std::marker::PhantomData;
use std::ops::{BitXor, Not, Rem, Shl, Shr};

/// Type-state: edge carries a typed predicate on slot value type P.
/// NOT a ValEdge — has separate operator impls constrained by SlotVal.
pub struct Pred<P>(pub(crate) Box<dyn Fn(&P) -> bool + Send + Sync>);

/// Marker: edge value types that are NOT Pred<P> (typed predicates).
/// Used to prevent blanket impl overlap with Pred-specific resolution.
pub trait IsValEdge {}
impl<V> IsValEdge for HasVal<V> {}
impl IsValEdge for HasPred {}
impl IsValEdge for () {}

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
    pub(crate) path: Option<crate::search::path::Config<(), crate::search::path::Unset, ER::Val>>,
}

impl<NV, ER: graph::Edge> Edge<(), NV, ER> {
    /// Accept a slot-specific value. The direction operator (`^`, `>>`, `<<`)
    /// constrains V to match the slot's type via `SlotVal` and wraps it.
    pub fn val<V>(self, v: V) -> Edge<HasRawVal<V>, NV, ER> {
        Edge(HasRawVal(v), PhantomData, None)
    }
}

impl<NV, ER: graph::Edge> Edge<(), NV, ER> {
    /// Typed predicate — the closure's input type P is enforced by the
    /// direction operator at compile time:
    /// - `^` requires P = undirected value type
    /// - `>>` / `<<` requires P = directed value type
    pub fn test<P: 'static>(self, f: impl Fn(&P) -> bool + Send + Sync + 'static) -> Edge<Pred<P>, NV, ER> {
        Edge(Pred(Box::new(f)), PhantomData, None)
    }

}

macro_rules! impl_connect_op {
    ($Dir:ident, $SlotDir:ident, $Op:ident, $op:ident) => {
        // Slot-typed value: E().val(slot_val) ^ Node
        // V is constrained to match SlotVal<$SlotDir>::SlotType by the direction.
        impl<V: PartialEq + Copy + Send + Sync + 'static, NV, ER: graph::Edge + $Dir + SlotVal<$SlotDir, SlotType = V>, RHS: IntoOp<NV, ER>>
            $Op<RHS> for Edge<HasRawVal<V>, NV, ER>
        {
            type Output = Edge<Connected<NV, ER>, NV, ER>;
            fn $op(self, rhs: RHS) -> Self::Output {
                let v = (self.0).0;
                let pred: Box<dyn Fn(&ER::Val) -> bool + Send + Sync> =
                    Box::new(move |ev| ER::extract_slot_val(ev).map_or(false, |s| *s == v));
                Edge(
                    Connected {
                        slot: <ER as $Dir>::SLOT,
                        val: ER::wrap_slot_val(v),
                        pred: Some(pred),
                        target: rhs.into_op(),
                        any_slot: false,
                        path: None,
                    },
                    PhantomData,
                    None,
                )
            }
        }

        // Slot-typed path: E().val(slot_val) ^ ..node.dfs()
        impl<V: PartialEq + Copy + Send + Sync + 'static, NV, ER: graph::Edge + $Dir + SlotVal<$SlotDir, SlotType = V>, RHS: IntoOp<NV, ER>, Mode>
            $Op<std::ops::RangeTo<crate::search::path::Config<RHS, Mode, ER::Val>>> for Edge<HasRawVal<V>, NV, ER>
        {
            type Output = Edge<Connected<NV, ER>, NV, ER>;
            fn $op(self, rhs: std::ops::RangeTo<crate::search::path::Config<RHS, Mode, ER::Val>>) -> Self::Output {
                let v = (self.0).0;
                let pred: Box<dyn Fn(&ER::Val) -> bool + Send + Sync> =
                    Box::new(move |ev| ER::extract_slot_val(ev).map_or(false, |s| *s == v));
                let cfg = rhs.end;
                let path_cfg = crate::search::path::Config {
                    target: (),
                    min_len: cfg.min_len,
                    max_len: cfg.max_len,
                    guard: cfg.guard,
                    driver: cfg.driver,
                    _mode: std::marker::PhantomData,
                };
                Edge(
                    Connected {
                        slot: <ER as $Dir>::SLOT,
                        val: ER::wrap_slot_val(v),
                        pred: Some(pred),
                        target: cfg.target.into_op(),
                        any_slot: false,
                        path: Some(path_cfg),
                    },
                    PhantomData,
                    None,
                )
            }
        }

        // No-value / HasPred edge: E() ^ Node
        impl<EV: IntoVal<ER::Val> + IsValEdge, NV, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>>
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
                        path: None,
                    },
                    PhantomData,
                    None,
                )
            }
        }

        // No-value / HasPred path: E() ^ ..node.dfs()
        impl<EV: IntoVal<ER::Val> + IsValEdge, NV, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>, Mode>
            $Op<std::ops::RangeTo<crate::search::path::Config<RHS, Mode, ER::Val>>> for Edge<EV, NV, ER>
        {
            type Output = Edge<Connected<NV, ER>, NV, ER>;
            fn $op(self, rhs: std::ops::RangeTo<crate::search::path::Config<RHS, Mode, ER::Val>>) -> Self::Output {
                let cfg = rhs.end;
                let path_cfg = crate::search::path::Config {
                    target: (),
                    min_len: cfg.min_len,
                    max_len: cfg.max_len,
                    guard: cfg.guard,
                    driver: cfg.driver,
                    _mode: std::marker::PhantomData,
                };
                Edge(
                    Connected {
                        slot: <ER as $Dir>::SLOT,
                        val: self.0.into_val(),
                        pred: self.2,
                        target: cfg.target.into_op(),
                        any_slot: false,
                        path: Some(path_cfg),
                    },
                    PhantomData,
                    None,
                )
            }
        }

    };
}

for_each_dir!(impl_connect_op!());

// Typed predicate operators: E().typed(|p: &P| ...) >> / << / ^ N(1)
// The direction operator constrains P via SlotVal.

// >> (Src): P must be the directed value type
impl<P: 'static, NV, ER: graph::Edge + Src + SlotVal<DirSlot, SlotType = P>, RHS: IntoOp<NV, ER>>
    Shr<RHS> for Edge<Pred<P>, NV, ER>
where ER::Val: Default,
{
    type Output = Edge<Connected<NV, ER>, NV, ER>;
    fn shr(self, rhs: RHS) -> Self::Output {
        let pred = self.0.0;
        let wrapped: Box<dyn Fn(&ER::Val) -> bool + Send + Sync> =
            Box::new(move |ev| ER::extract_slot_val(ev).map_or(false, |p| pred(p)));
        Edge(Connected {
            slot: <ER as Src>::SLOT,
            val: ER::Val::default(),
            pred: Some(wrapped),
            target: rhs.into_op(),
            any_slot: false,
            path: None,
        }, PhantomData, None)
    }
}

// >> path variant: E().test(pred) >> ..Target.dfs()
impl<P: 'static, NV, ER: graph::Edge + Src + SlotVal<DirSlot, SlotType = P>, RHS: IntoOp<NV, ER>, Mode>
    Shr<std::ops::RangeTo<crate::search::path::Config<RHS, Mode, ER::Val>>> for Edge<Pred<P>, NV, ER>
where ER::Val: Default,
{
    type Output = Edge<Connected<NV, ER>, NV, ER>;
    fn shr(self, rhs: std::ops::RangeTo<crate::search::path::Config<RHS, Mode, ER::Val>>) -> Self::Output {
        let pred = self.0.0;
        let wrapped: Box<dyn Fn(&ER::Val) -> bool + Send + Sync> =
            Box::new(move |ev| ER::extract_slot_val(ev).map_or(false, |p| pred(p)));
        let cfg = rhs.end;
        let path_cfg = crate::search::path::Config {
            target: (), min_len: cfg.min_len, max_len: cfg.max_len,
            guard: cfg.guard, driver: cfg.driver,
            _mode: std::marker::PhantomData,
        };
        Edge(Connected {
            slot: <ER as Src>::SLOT,
            val: ER::Val::default(),
            pred: Some(wrapped),
            target: cfg.target.into_op(),
            any_slot: false,
            path: Some(path_cfg),
        }, PhantomData, None)
    }
}

// << (Tgt): P must be the directed value type
impl<P: 'static, NV, ER: graph::Edge + Tgt + SlotVal<DirSlot, SlotType = P>, RHS: IntoOp<NV, ER>>
    Shl<RHS> for Edge<Pred<P>, NV, ER>
where ER::Val: Default,
{
    type Output = Edge<Connected<NV, ER>, NV, ER>;
    fn shl(self, rhs: RHS) -> Self::Output {
        let pred = self.0.0;
        let wrapped: Box<dyn Fn(&ER::Val) -> bool + Send + Sync> =
            Box::new(move |ev| ER::extract_slot_val(ev).map_or(false, |p| pred(p)));
        Edge(Connected {
            slot: <ER as Tgt>::SLOT,
            val: ER::Val::default(),
            pred: Some(wrapped),
            target: rhs.into_op(),
            any_slot: false,
            path: None,
        }, PhantomData, None)
    }
}

// << path variant: E().test(pred) << ..Target.dfs()
impl<P: 'static, NV, ER: graph::Edge + Tgt + SlotVal<DirSlot, SlotType = P>, RHS: IntoOp<NV, ER>, Mode>
    Shl<std::ops::RangeTo<crate::search::path::Config<RHS, Mode, ER::Val>>> for Edge<Pred<P>, NV, ER>
where ER::Val: Default,
{
    type Output = Edge<Connected<NV, ER>, NV, ER>;
    fn shl(self, rhs: std::ops::RangeTo<crate::search::path::Config<RHS, Mode, ER::Val>>) -> Self::Output {
        let pred = self.0.0;
        let wrapped: Box<dyn Fn(&ER::Val) -> bool + Send + Sync> =
            Box::new(move |ev| ER::extract_slot_val(ev).map_or(false, |p| pred(p)));
        let cfg = rhs.end;
        let path_cfg = crate::search::path::Config {
            target: (), min_len: cfg.min_len, max_len: cfg.max_len,
            guard: cfg.guard, driver: cfg.driver,
            _mode: std::marker::PhantomData,
        };
        Edge(Connected {
            slot: <ER as Tgt>::SLOT,
            val: ER::Val::default(),
            pred: Some(wrapped),
            target: cfg.target.into_op(),
            any_slot: false,
            path: Some(path_cfg),
        }, PhantomData, None)
    }
}

// ^ (Und): P must be the undirected value type
impl<P: 'static, NV, ER: graph::Edge + Und + SlotVal<UndirSlot, SlotType = P>, RHS: IntoOp<NV, ER>>
    BitXor<RHS> for Edge<Pred<P>, NV, ER>
where ER::Val: Default,
{
    type Output = Edge<Connected<NV, ER>, NV, ER>;
    fn bitxor(self, rhs: RHS) -> Self::Output {
        let pred = self.0.0;
        let wrapped: Box<dyn Fn(&ER::Val) -> bool + Send + Sync> =
            Box::new(move |ev| ER::extract_slot_val(ev).map_or(false, |p| pred(p)));
        Edge(Connected {
            slot: <ER as Und>::SLOT,
            val: ER::Val::default(),
            pred: Some(wrapped),
            target: rhs.into_op(),
            any_slot: false,
            path: None,
        }, PhantomData, None)
    }
}

// ^ path variant: E().test(pred) ^ ..Target.dfs()
impl<P: 'static, NV, ER: graph::Edge + Und + SlotVal<UndirSlot, SlotType = P>, RHS: IntoOp<NV, ER>, Mode>
    BitXor<std::ops::RangeTo<crate::search::path::Config<RHS, Mode, ER::Val>>> for Edge<Pred<P>, NV, ER>
where ER::Val: Default,
{
    type Output = Edge<Connected<NV, ER>, NV, ER>;
    fn bitxor(self, rhs: std::ops::RangeTo<crate::search::path::Config<RHS, Mode, ER::Val>>) -> Self::Output {
        let pred = self.0.0;
        let wrapped: Box<dyn Fn(&ER::Val) -> bool + Send + Sync> =
            Box::new(move |ev| ER::extract_slot_val(ev).map_or(false, |p| pred(p)));
        let cfg = rhs.end;
        let path_cfg = crate::search::path::Config {
            target: (), min_len: cfg.min_len, max_len: cfg.max_len,
            guard: cfg.guard, driver: cfg.driver,
            _mode: std::marker::PhantomData,
        };
        Edge(Connected {
            slot: <ER as Und>::SLOT,
            val: ER::Val::default(),
            pred: Some(wrapped),
            target: cfg.target.into_op(),
            any_slot: false,
            path: Some(path_cfg),
        }, PhantomData, None)
    }
}

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
    ($Dir:ident, $_SlotDir:ident, $Op:ident, $op:ident) => {
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
                        path: None,
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
                path: None,
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
                path: None,
            },
            PhantomData,
            None,
        )
    }
}
