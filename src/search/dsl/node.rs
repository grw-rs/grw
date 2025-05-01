use super::edge::{self, Connected};
use super::{EdgeOp, IntoOp, Op, UndirPending};
use super::{HasConstraint, HasPred};
use crate::graph::dsl::{HasVal, IntoOptional, IntoVal, LocalId};
use crate::graph;
use crate::graph::edge::{Src, Tgt, Und};
use std::ops::{BitAnd, BitXor, Not, Rem, Shl, Shr};

pub struct Free<NV, V, ER: graph::Edge> {
    pub(crate) id: Option<LocalId>,
    pub(crate) v: V,
    pub(crate) pred: Option<Box<dyn Fn(&NV) -> bool + Send + Sync>>,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

pub struct FreeRef<NV, ER: graph::Edge> {
    pub(crate) id: LocalId,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

pub struct Context<NV, V, ER: graph::Edge> {
    pub(crate) id: LocalId,
    pub(crate) v: V,
    pub(crate) pred: Option<Box<dyn Fn(&NV) -> bool + Send + Sync>>,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

pub struct ContextRef<NV, ER: graph::Edge> {
    pub(crate) id: LocalId,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

pub struct NegFree<NV, V, ER: graph::Edge> {
    pub(crate) id: Option<LocalId>,
    pub(crate) v: V,
    pub(crate) pred: Option<Box<dyn Fn(&NV) -> bool + Send + Sync>>,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

#[allow(dead_code)]
pub struct NegContext<NV, V, ER: graph::Edge> {
    pub(crate) id: LocalId,
    pub(crate) v: V,
    pub(crate) pred: Option<Box<dyn Fn(&NV) -> bool + Send + Sync>>,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

// --- Free value methods ---

impl<NV: PartialEq + Copy + Send + Sync + 'static, ER: graph::Edge> Free<NV, (), ER> {
    pub fn val(self, v: NV) -> Free<NV, HasVal<NV>, ER> {
        Free {
            id: self.id,
            v: HasVal(v),
            pred: Some(Box::new(move |x| *x == v)),
            edges: self.edges,
        }
    }
}

impl<NV, ER: graph::Edge> Free<NV, (), ER> {
    pub fn test(self, f: impl Fn(&NV) -> bool + Send + Sync + 'static) -> Free<NV, HasPred, ER> {
        Free {
            id: self.id,
            v: HasPred,
            pred: Some(Box::new(f)),
            edges: self.edges,
        }
    }
}

// --- Context value methods ---

impl<NV: PartialEq + Copy + Send + Sync + 'static, ER: graph::Edge> Context<NV, (), ER> {
    pub fn val(self, v: NV) -> Context<NV, HasVal<NV>, ER> {
        Context {
            id: self.id,
            v: HasVal(v),
            pred: Some(Box::new(move |x| *x == v)),
            edges: self.edges,
        }
    }
}

impl<NV, ER: graph::Edge> Context<NV, (), ER> {
    pub fn test(self, f: impl Fn(&NV) -> bool + Send + Sync + 'static) -> Context<NV, HasPred, ER> {
        Context {
            id: self.id,
            v: HasPred,
            pred: Some(Box::new(f)),
            edges: self.edges,
        }
    }
}

// --- NegContext value methods (for (!X(0)).val(42) syntax) ---

impl<NV: PartialEq + Copy + Send + Sync + 'static, ER: graph::Edge> NegContext<NV, (), ER> {
    pub fn val(self, v: NV) -> NegContext<NV, HasVal<NV>, ER> {
        NegContext {
            id: self.id,
            v: HasVal(v),
            pred: Some(Box::new(move |x| *x == v)),
            edges: self.edges,
        }
    }
}

impl<NV, ER: graph::Edge> NegContext<NV, (), ER> {
    pub fn test(self, f: impl Fn(&NV) -> bool + Send + Sync + 'static) -> NegContext<NV, HasPred, ER> {
        NegContext {
            id: self.id,
            v: HasPred,
            pred: Some(Box::new(f)),
            edges: self.edges,
        }
    }
}

// --- Not impls ---

impl<NV, V, ER: graph::Edge> Not for Free<NV, V, ER> {
    type Output = NegFree<NV, V, ER>;
    fn not(self) -> Self::Output {
        NegFree {
            id: self.id,
            v: self.v,
            pred: self.pred,
            edges: self.edges,
        }
    }
}

impl<NV, V, ER: graph::Edge> Not for Context<NV, V, ER> {
    type Output = NegContext<NV, V, ER>;
    fn not(self) -> Self::Output {
        NegContext {
            id: self.id,
            v: self.v,
            pred: self.pred,
            edges: self.edges,
        }
    }
}

// --- IntoOp: Free (positive) ---

impl<NV, V: IntoOptional<NV>, ER: graph::Edge> From<Free<NV, V, ER>> for Op<NV, ER> {
    fn from(n: Free<NV, V, ER>) -> Self {
        Op::Free {
            id: n.id,
            val: n.v.into_optional(),
            node_pred: n.pred,
            negated: false,
            edges: n.edges,
        }
    }
}

impl<NV, V: IntoOptional<NV>, ER: graph::Edge> IntoOp<NV, ER> for Free<NV, V, ER> {
    fn into_op(self) -> Op<NV, ER> {
        self.into()
    }
}

// --- IntoOp: FreeRef ---

impl<NV, ER: graph::Edge> From<FreeRef<NV, ER>> for Op<NV, ER> {
    fn from(n: FreeRef<NV, ER>) -> Self {
        Op::FreeRef {
            id: n.id,
            edges: n.edges,
        }
    }
}

impl<NV, ER: graph::Edge> IntoOp<NV, ER> for FreeRef<NV, ER> {
    fn into_op(self) -> Op<NV, ER> {
        self.into()
    }
}

// --- IntoOp: Context (positive) ---

impl<NV, V: IntoOptional<NV>, ER: graph::Edge> From<Context<NV, V, ER>> for Op<NV, ER> {
    fn from(n: Context<NV, V, ER>) -> Self {
        Op::Context {
            id: n.id,
            node_pred: n.pred,
            negated: false,
            edges: n.edges,
        }
    }
}

impl<NV, V: IntoOptional<NV>, ER: graph::Edge> IntoOp<NV, ER> for Context<NV, V, ER> {
    fn into_op(self) -> Op<NV, ER> {
        self.into()
    }
}

// --- IntoOp: ContextRef ---

impl<NV, ER: graph::Edge> From<ContextRef<NV, ER>> for Op<NV, ER> {
    fn from(n: ContextRef<NV, ER>) -> Self {
        Op::ContextRef {
            id: n.id,
            edges: n.edges,
        }
    }
}

impl<NV, ER: graph::Edge> IntoOp<NV, ER> for ContextRef<NV, ER> {
    fn into_op(self) -> Op<NV, ER> {
        self.into()
    }
}

// --- IntoOp: NegFree (always) ---

impl<NV, V: IntoOptional<NV>, ER: graph::Edge> From<NegFree<NV, V, ER>> for Op<NV, ER> {
    fn from(n: NegFree<NV, V, ER>) -> Self {
        Op::Free {
            id: n.id,
            val: n.v.into_optional(),
            node_pred: n.pred,
            negated: true,
            edges: n.edges,
        }
    }
}

impl<NV, V: IntoOptional<NV>, ER: graph::Edge> IntoOp<NV, ER> for NegFree<NV, V, ER> {
    fn into_op(self) -> Op<NV, ER> {
        self.into()
    }
}

// --- IntoOp: NegContext (only when V: HasConstraint) ---

impl<NV, V: IntoOptional<NV> + HasConstraint, ER: graph::Edge> From<NegContext<NV, V, ER>>
    for Op<NV, ER>
{
    fn from(n: NegContext<NV, V, ER>) -> Self {
        Op::Context {
            id: n.id,
            node_pred: n.pred,
            negated: true,
            edges: n.edges,
        }
    }
}

impl<NV, V: IntoOptional<NV> + HasConstraint, ER: graph::Edge> IntoOp<NV, ER>
    for NegContext<NV, V, ER>
{
    fn into_op(self) -> Op<NV, ER> {
        self.into()
    }
}

// =============================================================================
// Operator macros: positive anonymous edge
// =============================================================================

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
                    edge_pred: None,
                    target: rhs.into_op(),
                    negated: false,
                    any_slot: false,
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
                    edge_pred: None,
                    target: rhs.into_op(),
                    negated: false,
                    any_slot: false,
                });
                self
            }
        }
    };
}

for_each_dir!(impl_anon_edge_op!(Free<NV, V, ER>, V));
for_each_dir!(impl_anon_edge_op!(FreeRef<NV, ER>));
for_each_dir!(impl_anon_edge_op!(Context<NV, V, ER>, V));
for_each_dir!(impl_anon_edge_op!(ContextRef<NV, ER>));

// =============================================================================
// Operator macros: negated anonymous edge (for NegFree/NegContext)
// =============================================================================

macro_rules! impl_neg_anon_edge_op {
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
                    edge_pred: None,
                    target: rhs.into_op(),
                    negated: true,
                    any_slot: false,
                });
                self
            }
        }
    };
}

for_each_dir!(impl_neg_anon_edge_op!(NegFree<NV, V, ER>, V));
for_each_dir!(impl_neg_anon_edge_op!(NegContext<NV, V, ER>, V));

// =============================================================================
// Operator macros: resolve undirected pending (positive edge)
// =============================================================================

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
                    edge_pred: self.1 .2,
                    target: rhs.into_op(),
                    negated: false,
                    any_slot: false,
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
                    edge_pred: self.1 .2,
                    target: rhs.into_op(),
                    negated: false,
                    any_slot: false,
                });
                node
            }
        }
    };
}

for_each_dir!(impl_undir_op!(Free<NV, V, ER>, V));
for_each_dir!(impl_undir_op!(FreeRef<NV, ER>));
for_each_dir!(impl_undir_op!(Context<NV, V, ER>, V));
for_each_dir!(impl_undir_op!(ContextRef<NV, ER>));

// =============================================================================
// Operator macros: BitAnd with Connected edge (positive)
// =============================================================================

macro_rules! impl_bitand_connected {
    ($Self:ty, $V:ident) => {
        impl<NV, $V, ER: graph::Edge> BitAnd<edge::Edge<Connected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: edge::Edge<Connected<NV, ER>, NV, ER>) -> Self {
                self.edges.push(EdgeOp {
                    slot: arm.0.slot,
                    val: arm.0.val,
                    edge_pred: arm.0.pred,
                    target: arm.0.target,
                    negated: false,
                    any_slot: arm.0.any_slot,
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
                    edge_pred: arm.0.pred,
                    target: arm.0.target,
                    negated: false,
                    any_slot: arm.0.any_slot,
                });
                self
            }
        }
    };
}

impl_bitand_connected!(Free<NV, V, ER>, V);
impl_bitand_connected!(FreeRef<NV, ER>);
impl_bitand_connected!(Context<NV, V, ER>, V);
impl_bitand_connected!(ContextRef<NV, ER>);

// =============================================================================
// Operator macros: BitAnd with pending undirected edge (positive)
// =============================================================================

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

impl_bitand_undir_pending!(Free<NV, V, ER>, edge::Edge<(), NV, ER>, V);
impl_bitand_undir_pending!(Free<NV, V, ER>, edge::Edge<HasVal<ER::Val>, NV, ER>, V);
impl_bitand_undir_pending!(Free<NV, V, ER>, edge::Edge<HasPred, NV, ER>, V);
impl_bitand_undir_pending!(FreeRef<NV, ER>, edge::Edge<(), NV, ER>);
impl_bitand_undir_pending!(FreeRef<NV, ER>, edge::Edge<HasVal<ER::Val>, NV, ER>);
impl_bitand_undir_pending!(FreeRef<NV, ER>, edge::Edge<HasPred, NV, ER>);
impl_bitand_undir_pending!(Context<NV, V, ER>, edge::Edge<(), NV, ER>, V);
impl_bitand_undir_pending!(Context<NV, V, ER>, edge::Edge<HasVal<ER::Val>, NV, ER>, V);
impl_bitand_undir_pending!(Context<NV, V, ER>, edge::Edge<HasPred, NV, ER>, V);
impl_bitand_undir_pending!(ContextRef<NV, ER>, edge::Edge<(), NV, ER>);
impl_bitand_undir_pending!(ContextRef<NV, ER>, edge::Edge<HasVal<ER::Val>, NV, ER>);
impl_bitand_undir_pending!(ContextRef<NV, ER>, edge::Edge<HasPred, NV, ER>);

// =============================================================================
// Operator macros: BitAnd with NegEdge Connected (negated edge)
// =============================================================================

macro_rules! impl_bitand_neg_connected {
    ($Self:ty, $V:ident) => {
        impl<NV, $V, ER: graph::Edge> BitAnd<edge::NegEdge<Connected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: edge::NegEdge<Connected<NV, ER>, NV, ER>) -> Self {
                self.edges.push(EdgeOp {
                    slot: arm.0.slot,
                    val: arm.0.val,
                    edge_pred: arm.0.pred,
                    target: arm.0.target,
                    negated: true,
                    any_slot: arm.0.any_slot,
                });
                self
            }
        }
    };
    ($Self:ty) => {
        impl<NV, ER: graph::Edge> BitAnd<edge::NegEdge<Connected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: edge::NegEdge<Connected<NV, ER>, NV, ER>) -> Self {
                self.edges.push(EdgeOp {
                    slot: arm.0.slot,
                    val: arm.0.val,
                    edge_pred: arm.0.pred,
                    target: arm.0.target,
                    negated: true,
                    any_slot: arm.0.any_slot,
                });
                self
            }
        }
    };
}

impl_bitand_neg_connected!(Free<NV, V, ER>, V);
impl_bitand_neg_connected!(FreeRef<NV, ER>);
impl_bitand_neg_connected!(Context<NV, V, ER>, V);
impl_bitand_neg_connected!(ContextRef<NV, ER>);
impl_bitand_neg_connected!(NegFree<NV, V, ER>, V);
impl_bitand_neg_connected!(NegContext<NV, V, ER>, V);

// =============================================================================
// Operator macros: BitAnd with pending undirected NegEdge
// =============================================================================

macro_rules! impl_bitand_neg_undir_pending {
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

impl_bitand_neg_undir_pending!(Free<NV, V, ER>, edge::NegEdge<(), NV, ER>, V);
impl_bitand_neg_undir_pending!(Free<NV, V, ER>, edge::NegEdge<HasVal<ER::Val>, NV, ER>, V);
impl_bitand_neg_undir_pending!(Free<NV, V, ER>, edge::NegEdge<HasPred, NV, ER>, V);
impl_bitand_neg_undir_pending!(FreeRef<NV, ER>, edge::NegEdge<(), NV, ER>);
impl_bitand_neg_undir_pending!(FreeRef<NV, ER>, edge::NegEdge<HasVal<ER::Val>, NV, ER>);
impl_bitand_neg_undir_pending!(FreeRef<NV, ER>, edge::NegEdge<HasPred, NV, ER>);
impl_bitand_neg_undir_pending!(Context<NV, V, ER>, edge::NegEdge<(), NV, ER>, V);
impl_bitand_neg_undir_pending!(Context<NV, V, ER>, edge::NegEdge<HasVal<ER::Val>, NV, ER>, V);
impl_bitand_neg_undir_pending!(Context<NV, V, ER>, edge::NegEdge<HasPred, NV, ER>, V);
impl_bitand_neg_undir_pending!(ContextRef<NV, ER>, edge::NegEdge<(), NV, ER>);
impl_bitand_neg_undir_pending!(ContextRef<NV, ER>, edge::NegEdge<HasVal<ER::Val>, NV, ER>);
impl_bitand_neg_undir_pending!(ContextRef<NV, ER>, edge::NegEdge<HasPred, NV, ER>);
impl_bitand_neg_undir_pending!(NegFree<NV, V, ER>, edge::NegEdge<(), NV, ER>, V);
impl_bitand_neg_undir_pending!(NegFree<NV, V, ER>, edge::NegEdge<HasVal<ER::Val>, NV, ER>, V);
impl_bitand_neg_undir_pending!(NegFree<NV, V, ER>, edge::NegEdge<HasPred, NV, ER>, V);
impl_bitand_neg_undir_pending!(NegContext<NV, V, ER>, edge::NegEdge<(), NV, ER>, V);
impl_bitand_neg_undir_pending!(NegContext<NV, V, ER>, edge::NegEdge<HasVal<ER::Val>, NV, ER>, V);
impl_bitand_neg_undir_pending!(NegContext<NV, V, ER>, edge::NegEdge<HasPred, NV, ER>, V);

// =============================================================================
// Operator macros: resolve negated undirected pending
// =============================================================================

macro_rules! impl_neg_undir_op {
    ($NodeTy:ty, $V:ident, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, $V, EV: IntoVal<ER::Val>, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>>
            $Op<RHS> for UndirPending<$NodeTy, edge::NegEdge<EV, NV, ER>>
        where
            ER::Val: Default,
        {
            type Output = $NodeTy;
            fn $op(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(EdgeOp {
                    slot: <ER as $Dir>::SLOT,
                    val: self.1 .0.into_val(),
                    edge_pred: self.1 .2,
                    target: rhs.into_op(),
                    negated: true,
                    any_slot: false,
                });
                node
            }
        }
    };
    ($NodeTy:ty, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, EV: IntoVal<ER::Val>, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>>
            $Op<RHS> for UndirPending<$NodeTy, edge::NegEdge<EV, NV, ER>>
        where
            ER::Val: Default,
        {
            type Output = $NodeTy;
            fn $op(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(EdgeOp {
                    slot: <ER as $Dir>::SLOT,
                    val: self.1 .0.into_val(),
                    edge_pred: self.1 .2,
                    target: rhs.into_op(),
                    negated: true,
                    any_slot: false,
                });
                node
            }
        }
    };
}

for_each_dir!(impl_neg_undir_op!(Free<NV, V, ER>, V));
for_each_dir!(impl_neg_undir_op!(FreeRef<NV, ER>));
for_each_dir!(impl_neg_undir_op!(Context<NV, V, ER>, V));
for_each_dir!(impl_neg_undir_op!(ContextRef<NV, ER>));
for_each_dir!(impl_neg_undir_op!(NegFree<NV, V, ER>, V));
for_each_dir!(impl_neg_undir_op!(NegContext<NV, V, ER>, V));

// =============================================================================
// Operator macros: anonymous any-edge (%)
// =============================================================================

macro_rules! impl_any_edge_op {
    ($Self:ty, $V:ident) => {
        impl<NV, $V, ER: graph::Edge, RHS: IntoOp<NV, ER>> Rem<RHS> for $Self
        where ER::Val: Default,
        {
            type Output = Self;
            fn rem(mut self, rhs: RHS) -> Self {
                self.edges.push(EdgeOp {
                    slot: ER::SLOT_MIN,
                    val: ER::Val::default(),
                    edge_pred: None,
                    target: rhs.into_op(),
                    negated: false,
                    any_slot: true,
                });
                self
            }
        }
    };
    ($Self:ty) => {
        impl<NV, ER: graph::Edge, RHS: IntoOp<NV, ER>> Rem<RHS> for $Self
        where ER::Val: Default,
        {
            type Output = Self;
            fn rem(mut self, rhs: RHS) -> Self {
                self.edges.push(EdgeOp {
                    slot: ER::SLOT_MIN,
                    val: ER::Val::default(),
                    edge_pred: None,
                    target: rhs.into_op(),
                    negated: false,
                    any_slot: true,
                });
                self
            }
        }
    };
}

impl_any_edge_op!(Free<NV, V, ER>, V);
impl_any_edge_op!(FreeRef<NV, ER>);
impl_any_edge_op!(Context<NV, V, ER>, V);
impl_any_edge_op!(ContextRef<NV, ER>);

// =============================================================================
// Operator macros: negated anonymous any-edge (%)
// =============================================================================

macro_rules! impl_neg_any_edge_op {
    ($Self:ty, $V:ident) => {
        impl<NV, $V, ER: graph::Edge, RHS: IntoOp<NV, ER>> Rem<RHS> for $Self
        where ER::Val: Default,
        {
            type Output = Self;
            fn rem(mut self, rhs: RHS) -> Self {
                self.edges.push(EdgeOp {
                    slot: ER::SLOT_MIN,
                    val: ER::Val::default(),
                    edge_pred: None,
                    target: rhs.into_op(),
                    negated: true,
                    any_slot: true,
                });
                self
            }
        }
    };
}

impl_neg_any_edge_op!(NegFree<NV, V, ER>, V);
impl_neg_any_edge_op!(NegContext<NV, V, ER>, V);

// =============================================================================
// Operator macros: resolve UndirPending via % (positive edge)
// =============================================================================

macro_rules! impl_undir_any_op {
    ($NodeTy:ty, $V:ident) => {
        impl<NV, $V, EV: IntoVal<ER::Val>, ER: graph::Edge, RHS: IntoOp<NV, ER>>
            Rem<RHS> for UndirPending<$NodeTy, edge::Edge<EV, NV, ER>>
        {
            type Output = $NodeTy;
            fn rem(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(EdgeOp {
                    slot: ER::SLOT_MIN,
                    val: self.1 .0.into_val(),
                    edge_pred: self.1 .2,
                    target: rhs.into_op(),
                    negated: false,
                    any_slot: true,
                });
                node
            }
        }
    };
    ($NodeTy:ty) => {
        impl<NV, EV: IntoVal<ER::Val>, ER: graph::Edge, RHS: IntoOp<NV, ER>>
            Rem<RHS> for UndirPending<$NodeTy, edge::Edge<EV, NV, ER>>
        {
            type Output = $NodeTy;
            fn rem(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(EdgeOp {
                    slot: ER::SLOT_MIN,
                    val: self.1 .0.into_val(),
                    edge_pred: self.1 .2,
                    target: rhs.into_op(),
                    negated: false,
                    any_slot: true,
                });
                node
            }
        }
    };
}

impl_undir_any_op!(Free<NV, V, ER>, V);
impl_undir_any_op!(FreeRef<NV, ER>);
impl_undir_any_op!(Context<NV, V, ER>, V);
impl_undir_any_op!(ContextRef<NV, ER>);

// =============================================================================
// Operator macros: resolve UndirPending via % (negated edge)
// =============================================================================

macro_rules! impl_neg_undir_any_op {
    ($NodeTy:ty, $V:ident) => {
        impl<NV, $V, EV: IntoVal<ER::Val>, ER: graph::Edge, RHS: IntoOp<NV, ER>>
            Rem<RHS> for UndirPending<$NodeTy, edge::NegEdge<EV, NV, ER>>
        where ER::Val: Default,
        {
            type Output = $NodeTy;
            fn rem(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(EdgeOp {
                    slot: ER::SLOT_MIN,
                    val: self.1 .0.into_val(),
                    edge_pred: self.1 .2,
                    target: rhs.into_op(),
                    negated: true,
                    any_slot: true,
                });
                node
            }
        }
    };
    ($NodeTy:ty) => {
        impl<NV, EV: IntoVal<ER::Val>, ER: graph::Edge, RHS: IntoOp<NV, ER>>
            Rem<RHS> for UndirPending<$NodeTy, edge::NegEdge<EV, NV, ER>>
        where ER::Val: Default,
        {
            type Output = $NodeTy;
            fn rem(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(EdgeOp {
                    slot: ER::SLOT_MIN,
                    val: self.1 .0.into_val(),
                    edge_pred: self.1 .2,
                    target: rhs.into_op(),
                    negated: true,
                    any_slot: true,
                });
                node
            }
        }
    };
}

impl_neg_undir_any_op!(Free<NV, V, ER>, V);
impl_neg_undir_any_op!(FreeRef<NV, ER>);
impl_neg_undir_any_op!(Context<NV, V, ER>, V);
impl_neg_undir_any_op!(ContextRef<NV, ER>);
impl_neg_undir_any_op!(NegFree<NV, V, ER>, V);
impl_neg_undir_any_op!(NegContext<NV, V, ER>, V);
