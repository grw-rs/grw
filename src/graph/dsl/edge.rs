use super::{IntoOp, Op};
use super::{HasRawVal, IsFullVal, IntoVal};
use crate::graph;
use crate::graph::edge::{DirSlot, SlotVal, Src, Tgt, Und, UndirSlot};
use std::marker::PhantomData;
use std::ops::{BitXor, Shl, Shr};

pub struct Edge<EV, NV, ER: graph::Edge>(pub(crate) EV, pub(crate) PhantomData<(NV, ER)>);

pub struct Connected<NV, ER: graph::Edge> {
    pub(crate) slot: ER::Slot,
    pub(crate) val: ER::Val,
    pub(crate) target: Op<NV, ER>,
}

impl<NV, ER: graph::Edge> Edge<(), NV, ER> {
    pub fn val<V>(self, v: V) -> Edge<HasRawVal<V>, NV, ER> {
        Edge(HasRawVal(v), PhantomData)
    }
}

macro_rules! impl_connect_op {
    ($Dir:ident, $SlotDir:ident, $Op:ident, $op:ident) => {
        // No-value edge: E() >> Node (uses IntoVal, requires Default)
        impl<EV: IntoVal<ER::Val> + IsFullVal, NV, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>>
            $Op<RHS> for Edge<EV, NV, ER>
        {
            type Output = Edge<Connected<NV, ER>, NV, ER>;
            fn $op(self, rhs: RHS) -> Self::Output {
                Edge(
                    Connected {
                        slot: <ER as $Dir>::SLOT,
                        val: self.0.into_val(),
                        target: rhs.into_op(),
                    },
                    PhantomData,
                )
            }
        }

        // Slot-typed value: E().val(slot_val) >> Node
        // V is constrained to match SlotVal<$SlotDir>::SlotType by the direction.
        impl<V, NV, ER: graph::Edge + $Dir + SlotVal<$SlotDir, SlotType = V>, RHS: IntoOp<NV, ER>>
            $Op<RHS> for Edge<HasRawVal<V>, NV, ER>
        {
            type Output = Edge<Connected<NV, ER>, NV, ER>;
            fn $op(self, rhs: RHS) -> Self::Output {
                Edge(
                    Connected {
                        slot: <ER as $Dir>::SLOT,
                        val: ER::wrap_slot_val((self.0).0),
                        target: rhs.into_op(),
                    },
                    PhantomData,
                )
            }
        }
    };
}

for_each_dir!(impl_connect_op!());
