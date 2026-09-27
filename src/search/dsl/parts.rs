use super::HasPred;
use crate::composite::{Composite, Kinded, PartKind, PartOf};
use crate::graph;
use crate::graph::dsl::HasVal;
use crate::graph::edge::SlotVal;
use std::marker::PhantomData;

pub(crate) enum Tested {
    Bare,
    Tested,
}

pub(crate) type PartCheck<V> = Box<dyn Fn(&V) -> bool + Send + Sync>;

pub(crate) struct Required<V> {
    pub(crate) kind: PartKind,
    pub(crate) tested: Tested,
    pub(crate) holds: PartCheck<V>,
}

pub struct Parts<V>(pub(crate) Vec<Required<V>>);

impl<V> Parts<V> {
    pub(crate) fn none() -> Self {
        Parts(Vec::new())
    }
}

type Project<P, F> = Box<dyn for<'a> Fn(&'a P) -> Option<&'a F> + Send + Sync>;

type PartTest<F> = Box<dyn Fn(&F) -> bool + Send + Sync>;

pub(crate) struct Need<P: Kinded> {
    kind: P::Kind,
    tested: Tested,
    holds: PartCheck<P>,
}

impl<P> Need<P>
where
    P: Kinded + 'static,
    P::Kind: Send + Sync + 'static,
{
    fn through<ER, S, C>(self) -> Required<ER::Val>
    where
        ER: SlotVal<S, SlotType = C>,
        C: Composite<Part = P>,
    {
        let Need { kind, tested, holds } = self;
        let erased = PartKind::new(kind.clone());
        let check: PartCheck<ER::Val> = Box::new(move |v| match ER::extract_slot_val(v).and_then(|c| c.part(&kind)) {
            Some(part) => holds(part),
            None => false,
        });
        Required { kind: erased, tested, holds: check }
    }
}

pub struct Focus<P: Kinded, F> {
    held: Vec<Need<P>>,
    kind: P::Kind,
    project: Project<P, F>,
    tests: Vec<PartTest<F>>,
}

impl<P: Kinded + 'static, F: 'static> Focus<P, F> {
    pub(crate) fn typed(held: Vec<Need<P>>) -> Self
    where
        P: PartOf<F>,
    {
        Focus { held, kind: <P as PartOf<F>>::kind(), project: Box::new(<P as PartOf<F>>::project), tests: Vec::new() }
    }

    pub(crate) fn test(mut self, f: impl Fn(&F) -> bool + Send + Sync + 'static) -> Self {
        self.tests.push(Box::new(f));
        self
    }

    pub(crate) fn finish(self) -> Vec<Need<P>> {
        let Focus { mut held, kind, project, tests } = self;
        let tested = match tests.is_empty() {
            true => Tested::Bare,
            false => Tested::Tested,
        };
        let holds: PartCheck<P> = Box::new(move |p| match project(p) {
            Some(part) => tests.iter().all(|t| t(part)),
            None => false,
        });
        held.push(Need { kind, tested, holds });
        held
    }
}

impl<P: Kinded + 'static> Focus<P, P> {
    pub(crate) fn kinded(held: Vec<Need<P>>, kind: P::Kind) -> Self {
        Focus { held, kind, project: Box::new(|p| Some(p)), tests: Vec::new() }
    }
}

pub struct InSlot<ER, S>(PhantomData<fn() -> (ER, S)>);

pub trait SlotEdge {
    type Edge: graph::Edge;
}

impl<ER: graph::Edge, S> SlotEdge for InSlot<ER, S> {
    type Edge = ER;
}

type ValOf<At> = <<At as SlotEdge>::Edge as graph::Edge>::Val;

pub trait EdgeTerm<At: SlotEdge> {
    fn into_parts(self) -> Parts<ValOf<At>>;
}

impl<At: SlotEdge> EdgeTerm<At> for () {
    fn into_parts(self) -> Parts<ValOf<At>> {
        Parts::none()
    }
}

impl<At: SlotEdge, X> EdgeTerm<At> for HasVal<X> {
    fn into_parts(self) -> Parts<ValOf<At>> {
        Parts::none()
    }
}

impl<At: SlotEdge> EdgeTerm<At> for HasPred {
    fn into_parts(self) -> Parts<ValOf<At>> {
        Parts::none()
    }
}

impl<ER, S, C, P, F> EdgeTerm<InSlot<ER, S>> for Focus<P, F>
where
    ER: SlotVal<S, SlotType = C>,
    C: Composite<Part = P>,
    P: Kinded + 'static,
    P::Kind: Send + Sync + 'static,
    F: 'static,
{
    fn into_parts(self) -> Parts<ER::Val> {
        Parts(self.finish().into_iter().map(Need::through::<ER, S, C>).collect())
    }
}
