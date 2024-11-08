use super::{IntoOp, Op};
use super::{HasVal, IntoVal};
use crate::graph;
use crate::graph::edge::{Src, Tgt, Und};
use std::marker::PhantomData;
use std::ops::{BitXor, Shl, Shr};

pub struct Edge<EV, NV, ER: graph::Edge>(pub(crate) EV, pub(crate) PhantomData<(NV, ER)>);

pub struct Connected<NV, ER: graph::Edge> {
    pub(crate) slot: ER::Slot,
    pub(crate) val: ER::Val,
    pub(crate) target: Op<NV, ER>,
}

impl<NV, ER: graph::Edge> Edge<(), NV, ER> {
    pub fn val(self, v: impl Into<ER::Val>) -> Edge<HasVal<ER::Val>, NV, ER> {
        Edge(HasVal(v.into()), PhantomData)
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
                        target: rhs.into_op(),
                    },
                    PhantomData,
                )
            }
        }
    };
}

for_each_dir!(impl_connect_op!());
