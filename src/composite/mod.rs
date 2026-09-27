mod erased;
mod type_set;
mod typed;

#[cfg(test)]
mod tests;

pub use erased::PartKind;
pub use type_set::{KindPresent, NotCanonical, NotHeld, TypeSet};
pub use typed::{Has, PartOf, Typed};
pub use grw_derive::Part;

pub trait Kinded {
    type Kind: Ord + Clone + std::fmt::Debug;

    fn kind(&self) -> Self::Kind;
}

pub type KindOf<C> = <<C as Composite>::Part as Kinded>::Kind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Included<C> {
    Added(C),
    Held,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removed<C> {
    Remains(C),
    Vacant,
}

pub trait Composite: Sized {
    type Part: Kinded;
    type IncludeRefused: std::error::Error;
    type ExcludeRefused: std::error::Error;

    fn from_part(part: Self::Part) -> Self;

    fn parts(&self) -> impl Iterator<Item = &Self::Part>;

    fn part(&self, kind: &KindOf<Self>) -> Option<&Self::Part>;

    fn with_part(&self, part: Self::Part) -> Result<Included<Self>, Self::IncludeRefused>;

    fn without_part(&self, kind: &KindOf<Self>) -> Result<Removed<Self>, Self::ExcludeRefused>;
}
