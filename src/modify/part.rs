use crate::composite::{Composite, Included, KindOf, Removed};
use crate::graph::edge::SlotVal;
use crate::graph::edge::{DirSlot, UndirSlot};
use crate::modify::error::apply::{ExcludeRefused, IncludeRefused, SlotClassKind};
use std::marker::PhantomData;

pub enum Part<V> {
    Include(IncludePart<V>),
    Exclude(ExcludePart<V>),
}

pub struct IncludePart<V>(pub(crate) Box<dyn Includes<V> + Send + Sync>);

pub struct ExcludePart<V>(pub(crate) Box<dyn Excludes<V> + Send + Sync>);

impl<C> IncludePart<C>
where
    C: Composite + PartialEq + 'static,
    C::Part: Send + Sync + 'static,
    C::IncludeRefused: Send + Sync + 'static,
{
    pub fn new(part: C::Part) -> Self {
        IncludePart::slotted::<Itself<C>>(part)
    }
}

impl<V> IncludePart<V> {
    pub(crate) fn slotted<Pr>(part: <Pr::Held as Composite>::Part) -> Self
    where
        Pr: Projection<Whole = V> + 'static,
        V: PartialEq + 'static,
        <Pr::Held as Composite>::Part: Send + Sync + 'static,
        <Pr::Held as Composite>::IncludeRefused: Send + Sync + 'static,
    {
        IncludePart(Box::new(Slotted::<Pr, _> { op: Including::<Pr::Held> { part }, projection: PhantomData }))
    }
}

impl<C> ExcludePart<C>
where
    C: Composite + PartialEq + 'static,
    KindOf<C>: Send + Sync + 'static,
    C::ExcludeRefused: Send + Sync + 'static,
{
    pub fn new(kind: KindOf<C>) -> Self {
        ExcludePart::slotted::<Itself<C>>(kind)
    }
}

impl<V> ExcludePart<V> {
    pub(crate) fn slotted<Pr>(kind: KindOf<Pr::Held>) -> Self
    where
        Pr: Projection<Whole = V> + 'static,
        V: PartialEq + 'static,
        KindOf<Pr::Held>: Send + Sync + 'static,
        <Pr::Held as Composite>::ExcludeRefused: Send + Sync + 'static,
    {
        ExcludePart(Box::new(Slotted::<Pr, _> { op: Excluding::<Pr::Held> { kind }, projection: PhantomData }))
    }
}

impl<V> From<IncludePart<V>> for Part<V> {
    fn from(op: IncludePart<V>) -> Self {
        Part::Include(op)
    }
}

impl<V> From<ExcludePart<V>> for Part<V> {
    fn from(op: ExcludePart<V>) -> Self {
        Part::Exclude(op)
    }
}

pub(crate) enum Link<'g, V> {
    Vacant,
    Present(Present<'g, V>),
}

pub(crate) enum Present<'g, V> {
    Held(&'g V),
    Staged(V),
}

impl<V> Present<'_, V> {
    fn value(&self) -> &V {
        match self {
            Present::Held(held) => held,
            Present::Staged(staged) => staged,
        }
    }
}

impl<'g, V: PartialEq> Present<'g, V> {
    fn settled(next: V, stored: Option<&'g V>) -> Self {
        match stored {
            Some(held) if *held == next => Present::Held(held),
            _ => Present::Staged(next),
        }
    }
}

pub(crate) enum Refused<R> {
    Part(R),
    SlotClass(SlotClassKind),
}

mod sealed { pub trait Sealed {} }

impl sealed::Sealed for UndirSlot {}
impl sealed::Sealed for DirSlot {}

pub trait SlotClassed: sealed::Sealed {
    const CLASS: SlotClassKind;
}

impl SlotClassed for UndirSlot {
    const CLASS: SlotClassKind = SlotClassKind::Undir;
}

impl SlotClassed for DirSlot {
    const CLASS: SlotClassKind = SlotClassKind::Dir;
}

pub(crate) trait Includes<V> {
    fn onto<'g>(
        self: Box<Self>,
        link: Link<'g, V>,
        stored: Option<&'g V>,
    ) -> Result<Present<'g, V>, Refused<IncludeRefused>>;
}

pub(crate) trait Excludes<V> {
    fn off<'g>(
        self: Box<Self>,
        present: Present<'g, V>,
        stored: Option<&'g V>,
    ) -> Result<Link<'g, V>, Refused<ExcludeRefused>>;
}

pub(crate) trait Projection {
    type Whole;
    type Held: Composite;

    fn held(whole: &Self::Whole) -> Result<&Self::Held, SlotClassKind>;

    fn whole(held: Self::Held) -> Self::Whole;
}

pub(crate) struct Through<ER, S>(PhantomData<fn() -> (ER, S)>);

impl<ER, S> Projection for Through<ER, S>
where
    ER: SlotVal<S>,
    S: SlotClassed,
    ER::SlotType: Composite,
{
    type Whole = ER::Val;
    type Held = ER::SlotType;

    fn held(whole: &ER::Val) -> Result<&ER::SlotType, SlotClassKind> {
        match ER::extract_slot_val(whole) {
            Some(held) => Ok(held),
            None => Err(S::CLASS),
        }
    }

    fn whole(held: ER::SlotType) -> ER::Val {
        ER::wrap_slot_val(held)
    }
}

pub(crate) struct Itself<C>(PhantomData<fn() -> C>);

impl<C: Composite> Projection for Itself<C> {
    type Whole = C;
    type Held = C;

    fn held(whole: &C) -> Result<&C, SlotClassKind> {
        Ok(whole)
    }

    fn whole(held: C) -> C {
        held
    }
}

pub(crate) struct Slotted<Pr, Op> {
    op: Op,
    projection: PhantomData<fn() -> Pr>,
}

struct Including<C: Composite> {
    part: C::Part,
}

struct Excluding<C: Composite> {
    kind: KindOf<C>,
}

impl<Pr> Includes<Pr::Whole> for Slotted<Pr, Including<Pr::Held>>
where
    Pr: Projection,
    Pr::Whole: PartialEq,
    <Pr::Held as Composite>::IncludeRefused: Send + Sync + 'static,
{
    fn onto<'g>(
        self: Box<Self>,
        link: Link<'g, Pr::Whole>,
        stored: Option<&'g Pr::Whole>,
    ) -> Result<Present<'g, Pr::Whole>, Refused<IncludeRefused>> {
        let Including { part } = self.op;
        match link {
            Link::Vacant => Ok(Present::settled(Pr::whole(Pr::Held::from_part(part)), stored)),
            Link::Present(present) => match Pr::held(present.value()).map(|held| held.with_part(part)) {
                Err(expected) => Err(Refused::SlotClass(expected)),
                Ok(Ok(Included::Added(next))) => Ok(Present::settled(Pr::whole(next), stored)),
                Ok(Ok(Included::Held)) => Ok(present),
                Ok(Err(refused)) => Err(Refused::Part(IncludeRefused::new(refused))),
            },
        }
    }
}

impl<Pr> Excludes<Pr::Whole> for Slotted<Pr, Excluding<Pr::Held>>
where
    Pr: Projection,
    Pr::Whole: PartialEq,
    <Pr::Held as Composite>::ExcludeRefused: Send + Sync + 'static,
{
    fn off<'g>(
        self: Box<Self>,
        present: Present<'g, Pr::Whole>,
        stored: Option<&'g Pr::Whole>,
    ) -> Result<Link<'g, Pr::Whole>, Refused<ExcludeRefused>> {
        match Pr::held(present.value()).map(|held| held.without_part(&self.op.kind)) {
            Err(expected) => Err(Refused::SlotClass(expected)),
            Ok(Ok(Removed::Remains(next))) => Ok(Link::Present(Present::settled(Pr::whole(next), stored))),
            Ok(Ok(Removed::Vacant)) => Ok(Link::Vacant),
            Ok(Err(refused)) => Err(Refused::Part(ExcludeRefused::new(refused))),
        }
    }
}
