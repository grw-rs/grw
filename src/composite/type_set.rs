use super::{Composite, Included, KindOf, Kinded, Removed};
use smallvec::SmallVec;

type Parts<P> = SmallVec<[P; 2]>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a different part of kind {kind:?} is already held")]
pub struct KindPresent<K: std::fmt::Debug> {
    pub kind: K,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("no part of kind {kind:?} is held")]
pub struct NotHeld<K: std::fmt::Debug> {
    pub kind: K,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeSet<P: Kinded> {
    parts: Parts<P>,
}

impl<P: Kinded> TypeSet<P> {
    fn single(part: P) -> Self {
        let mut parts = Parts::new();
        parts.push(part);
        TypeSet { parts }
    }

    fn position(&self, kind: &P::Kind) -> Result<usize, usize> {
        self.parts.binary_search_by(|held| held.kind().cmp(kind))
    }
}

impl<P: Kinded + Clone + PartialEq> Composite for TypeSet<P> {
    type Part = P;
    type IncludeRefused = KindPresent<P::Kind>;
    type ExcludeRefused = NotHeld<P::Kind>;

    fn from_part(part: P) -> Self {
        TypeSet::single(part)
    }

    fn parts(&self) -> impl Iterator<Item = &P> {
        self.parts.iter()
    }

    fn part(&self, kind: &KindOf<Self>) -> Option<&P> {
        match self.position(kind) {
            Ok(at) => Some(&self.parts[at]),
            Err(_) => None,
        }
    }

    fn with_part(&self, part: P) -> Result<Included<Self>, KindPresent<P::Kind>> {
        let kind = part.kind();
        match self.position(&kind) {
            Ok(at) => change::held(&self.parts[at], &part, kind),
            Err(at) => Ok(Included::Added(change::inserted(self, at, part))),
        }
    }

    fn without_part(&self, kind: &KindOf<Self>) -> Result<Removed<Self>, NotHeld<P::Kind>> {
        match self.position(kind) {
            Ok(at) => Ok(change::removed(self, at)),
            Err(_) => Err(NotHeld { kind: kind.clone() }),
        }
    }
}

mod change {
    use super::{Included, KindPresent, Kinded, Removed, TypeSet};

    pub(super) fn held<P: Kinded + PartialEq, C>(
        held: &P,
        offered: &P,
        kind: P::Kind,
    ) -> Result<Included<C>, KindPresent<P::Kind>> {
        match held == offered {
            true => Ok(Included::Held),
            false => Err(KindPresent { kind }),
        }
    }

    pub(super) fn inserted<P: Kinded + Clone>(set: &TypeSet<P>, at: usize, part: P) -> TypeSet<P> {
        let mut parts = set.parts.clone();
        parts.insert(at, part);
        TypeSet { parts }
    }

    pub(super) fn removed<P: Kinded + Clone>(set: &TypeSet<P>, at: usize) -> Removed<TypeSet<P>> {
        match set.parts.len() {
            1 => Removed::Vacant,
            _ => {
                let mut parts = set.parts.clone();
                parts.remove(at);
                Removed::Remains(TypeSet { parts })
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotCanonical<K: std::fmt::Debug> {
    #[error("a type set holds at least one part")]
    Empty,
    #[error("part of kind {after:?} follows part of kind {before:?}")]
    OutOfOrder { before: K, after: K },
    #[error("two parts of kind {kind:?}")]
    DuplicateKind { kind: K },
}

mod canonical {
    use super::{NotCanonical, Parts, TypeSet};
    use crate::composite::Kinded;
    use std::cmp::Ordering;

    pub(super) fn checked<P: Kinded>(parts: Parts<P>) -> Result<TypeSet<P>, NotCanonical<P::Kind>> {
        match parts.is_empty() {
            true => Err(NotCanonical::Empty),
            false => ordered(&parts).map(|()| TypeSet { parts }),
        }
    }

    fn ordered<P: Kinded>(parts: &[P]) -> Result<(), NotCanonical<P::Kind>> {
        parts.windows(2).try_for_each(|pair| step(pair[0].kind(), pair[1].kind()))
    }

    fn step<K: Ord + std::fmt::Debug>(before: K, after: K) -> Result<(), NotCanonical<K>> {
        match before.cmp(&after) {
            Ordering::Less => Ok(()),
            Ordering::Equal => Err(NotCanonical::DuplicateKind { kind: after }),
            Ordering::Greater => Err(NotCanonical::OutOfOrder { before, after }),
        }
    }
}

mod wire {
    use super::{Parts, TypeSet, canonical};
    use crate::composite::Kinded;

    impl<P: Kinded + serde::Serialize> serde::Serialize for TypeSet<P> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.collect_seq(self.parts.iter())
        }
    }

    impl<'de, P: Kinded + serde::Deserialize<'de>> serde::Deserialize<'de> for TypeSet<P> {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let parts = Parts::<P>::deserialize(deserializer)?;
            canonical::checked(parts).map_err(serde::de::Error::custom)
        }
    }
}

mod layout_val {
    use super::TypeSet;
    use crate::composite::Kinded;
    use crate::layout::{self, FieldInfo, FieldType, Val};

    const TYPE_SET_TAG: u8 = 16;

    impl<P: Kinded + Val> Val for TypeSet<P> {
        fn fields() -> &'static [FieldInfo] {
            &[]
        }

        fn field_type() -> FieldType {
            FieldType::Set(P::field_type)
        }

        fn layout_hash() -> u64 {
            let h = layout::fnv_hash_byte(layout::FNV_OFFSET, TYPE_SET_TAG);
            layout::fnv_hash_u64(h, P::layout_hash())
        }

        fn size() -> usize {
            std::mem::size_of::<TypeSet<P>>()
        }

        fn align() -> usize {
            std::mem::align_of::<TypeSet<P>>()
        }
    }
}
