use super::{IntoOp, Op};
use super::{HasRawVal, IsFullVal, IntoVal};
use crate::composite::{Composite, Included, KindOf, Kinded, PartKind};
use crate::graph;
use crate::graph::error;
use crate::modify::error::apply::IncludeRefused;
use crate::graph::edge::{DirSlot, SlotVal, Src, Tgt, Und, UndirSlot};
use std::marker::PhantomData;
use std::ops::{BitXor, Shl, Shr};

pub struct Edge<EV, NV, ER: graph::Edge>(pub(crate) EV, pub(crate) PhantomData<(NV, ER)>);

pub struct Connected<NV, ER: graph::Edge> {
    pub(crate) slot: ER::Slot,
    pub(crate) val: Result<ER::Val, error::Part>,
    pub(crate) target: Op<NV, ER>,
}

pub struct Parts<C>(pub(crate) Result<C, error::Part>);

impl<NV, ER: graph::Edge> Edge<(), NV, ER> {
    pub fn val<V>(self, v: V) -> Edge<HasRawVal<V>, NV, ER> {
        Edge(HasRawVal(v), PhantomData)
    }

    pub fn include<T, C>(self, part: T) -> Edge<Parts<C>, NV, ER>
    where
        C: Composite,
        T: Into<C::Part>,
    {
        Edge(Parts(Ok(C::from_part(part.into()))), PhantomData)
    }
}

impl<NV, ER: graph::Edge, C> Edge<Parts<C>, NV, ER>
where
    C: Composite,
    KindOf<C>: Send + Sync + 'static,
    C::IncludeRefused: Send + Sync + 'static,
{
    pub fn include<T>(self, part: T) -> Edge<Parts<C>, NV, ER>
    where
        T: Into<C::Part>,
    {
        let held = self.0.0.and_then(|held| assemble(held, part.into()));
        Edge(Parts(held), PhantomData)
    }
}

fn assemble<C>(held: C, part: C::Part) -> Result<C, error::Part>
where
    C: Composite,
    KindOf<C>: Send + Sync + 'static,
    C::IncludeRefused: Send + Sync + 'static,
{
    let kind = part.kind();
    match held.part(&kind) {
        Some(_) => Err(error::Part::Repeated(PartKind::new(kind))),
        None => match held.with_part(part) {
            Ok(Included::Added(next)) => Ok(next),
            Ok(Included::Held) => Ok(held),
            Err(refused) => Err(error::Part::Refused(IncludeRefused::new(refused))),
        },
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
                        val: Ok(self.0.into_val()),
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
                        val: Ok(ER::wrap_slot_val((self.0).0)),
                        target: rhs.into_op(),
                    },
                    PhantomData,
                )
            }
        }
    };
}

for_each_dir!(impl_connect_op!());

macro_rules! impl_connect_parts_op {
    ($Dir:ident, $SlotDir:ident, $Op:ident, $op:ident) => {
        impl<C, NV, ER: graph::Edge + $Dir + SlotVal<$SlotDir, SlotType = C>, RHS: IntoOp<NV, ER>> $Op<RHS>
            for Edge<Parts<C>, NV, ER>
        {
            type Output = Edge<Connected<NV, ER>, NV, ER>;
            fn $op(self, rhs: RHS) -> Self::Output {
                Edge(
                    Connected {
                        slot: <ER as $Dir>::SLOT,
                        val: (self.0).0.map(ER::wrap_slot_val),
                        target: rhs.into_op(),
                    },
                    PhantomData,
                )
            }
        }
    };
}

for_each_dir!(impl_connect_parts_op!());
