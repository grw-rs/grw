#![allow(unused)]

use crate::Id;

use core::hash::BuildHasherDefault;
use derive_more::{Deref, DerefMut};
use indexmap::{IndexMap, IndexSet};
use range_set_blaze::RangeSetBlaze;
use rustc_hash::FxHasher;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign};
use std::ops::{Sub, SubAssign};

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

pub(crate) use rustc_hash::FxHashMap;
pub(crate) use rustc_hash::FxHashSet;

pub(crate) type FxIndexSet<K> = IndexSet<K, BuildHasherDefault<FxHasher>>;
pub(crate) type FxIndexMap<K, V> = IndexMap<K, V, BuildHasherDefault<FxHasher>>;

pub(crate) mod untyped {
    #[cfg(feature = "id32")]
    pub type IdSet = roaring::RoaringBitmap;
    #[cfg(feature = "id32")]
    pub type IdIter<'a> = roaring::bitmap::Iter<'a>;

    #[cfg(feature = "id64")]
    pub type IdSet = roaring::RoaringTreemap;
    #[cfg(feature = "id64")]
    pub type IdIter<'a> = roaring::treemap::Iter<'a>;
}

#[derive(Deref, DerefMut, PartialEq, Clone)]
pub(crate) struct IdSet<K: Eq + From<Id>>(
    #[deref]
    #[deref_mut]
    untyped::IdSet,
    PhantomData<K>,
);

impl<K: Eq + From<Id>> serde::Serialize for IdSet<K> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de, K: Eq + From<Id>> serde::Deserialize<'de> for IdSet<K> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let inner = untyped::IdSet::deserialize(deserializer)?;
        Ok(Self(inner, PhantomData))
    }
}

impl<K: Eq + From<Id>> IdSet<K> {
    pub(crate) fn from_id(id: Id) -> Self {
        Self(untyped::IdSet::from([id]), PhantomData)
    }
    pub(crate) fn insert_id(&mut self, id: &Id) -> bool {
        self.0.insert(*id)
    }
    pub(crate) fn remove_id(&mut self, id: &Id) -> bool {
        self.0.remove(*id)
    }
    pub(crate) fn insert<'a>(&mut self, k: &'a K) -> bool
    where
        Id: From<&'a K>,
    {
        self.0.insert(k.into())
    }
    pub(crate) fn remove<'a>(&mut self, k: &'a K) -> bool
    where
        Id: From<&'a K>,
    {
        self.0.remove(k.into())
    }
    pub(crate) fn pop_first(&mut self) -> Option<K> {
        self.0.iter().next().map(|k| {
            self.remove_id(&k);
            k.into()
        })
    }

    pub(crate) fn from_raw_ids(ids: impl IntoIterator<Item = Id>) -> Self {
        Self(untyped::IdSet::from_iter(ids), PhantomData)
    }

    pub(crate) fn is_disjoint(&self, other: &Self) -> bool {
        self.0.is_disjoint(&other.0)
    }

    pub(crate) fn is_subset(&self, other: &Self) -> bool {
        self.0.is_subset(&other.0)
    }

    pub(crate) fn is_superset(&self, other: &Self) -> bool {
        self.0.is_superset(&other.0)
    }

    pub(crate) fn intersection_len(&self, other: &Self) -> u64 {
        self.0.intersection_len(&other.0)
    }

    pub(crate) fn union_len(&self, other: &Self) -> u64 {
        self.0.union_len(&other.0)
    }

    pub(crate) fn difference_len(&self, other: &Self) -> u64 {
        self.0.difference_len(&other.0)
    }

    pub(crate) fn symmetric_difference_len(&self, other: &Self) -> u64 {
        self.0.symmetric_difference_len(&other.0)
    }
}

impl<K: Eq + From<Id>> fmt::Debug for IdSet<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.len() < 16 {
            write!(f, "IdSet<{:?}>", self.iter().collect::<Vec<Id>>())
        } else {
            write!(
                f,
                "IdSet<{:?} values between {:?} and {:?}>",
                self.len(),
                self.0.min().unwrap(),
                self.0.max().unwrap()
            )
        }
    }
}

impl<K: Eq + From<Id>> Default for IdSet<K> {
    fn default() -> Self {
        Self(Default::default(), Default::default())
    }
}

impl<K: Eq + From<Id>> Eq for IdSet<K> {}

impl<K: Eq + From<Id>> Hash for IdSet<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for id in self.0.iter() {
            id.hash(state);
        }
    }
}

impl<K: Eq + From<Id>> PartialOrd for IdSet<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: Eq + From<Id>> Ord for IdSet<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        use Ordering::*;

        match self.len().cmp(&other.len()) {
            res @ (Less | Greater) => res,
            Equal => {
                let mut a = self.iter();
                let mut b = other.iter();

                loop {
                    match (a.next(), b.next()) {
                        (None, None) => break Equal,
                        (Some(x), Some(y)) => match x.cmp(&y) {
                            res @ (Less | Greater) => break res,
                            Equal => continue,
                        },
                        _ => unreachable!(
                            "same length iterators producing different number of elements"
                        ),
                    }
                }
            }
        }
    }
}

impl<K: Eq + From<Id>> From<untyped::IdSet> for IdSet<K> {
    fn from(value: untyped::IdSet) -> Self {
        Self(value, PhantomData)
    }
}

impl<K: Eq + From<Id>> From<IdSet<K>> for untyped::IdSet {
    fn from(value: IdSet<K>) -> Self {
        value.0
    }
}

impl<K: Eq + From<Id>> BitAnd<IdSet<K>> for IdSet<K> {
    type Output = IdSet<K>;

    fn bitand(self, rhs: IdSet<K>) -> Self::Output {
        (&*self).bitand(&*rhs).into()
    }
}
impl<K: Eq + From<Id>> BitAnd<&IdSet<K>> for IdSet<K> {
    type Output = IdSet<K>;

    fn bitand(self, rhs: &IdSet<K>) -> Self::Output {
        (&*self).bitand(&**rhs).into()
    }
}
impl<K: Eq + From<Id>> BitAnd<IdSet<K>> for &IdSet<K> {
    type Output = IdSet<K>;

    fn bitand(self, rhs: IdSet<K>) -> Self::Output {
        rhs.bitand(self)
    }
}
impl<K: Eq + From<Id>> BitAnd<&IdSet<K>> for &IdSet<K> {
    type Output = IdSet<K>;

    fn bitand(self, rhs: &IdSet<K>) -> Self::Output {
        (&**self).bitand(&**rhs).into()
    }
}

impl<K: Eq + From<Id>> BitAndAssign<IdSet<K>> for IdSet<K> {
    fn bitand_assign(&mut self, mut rhs: IdSet<K>) {
        self.0.bitand_assign(rhs.0);
    }
}
impl<K: Eq + From<Id>> BitAndAssign<&IdSet<K>> for IdSet<K> {
    fn bitand_assign(&mut self, rhs: &IdSet<K>) {
        self.0.bitand_assign(&rhs.0);
    }
}

impl<K: Eq + From<Id>> BitOr<IdSet<K>> for IdSet<K> {
    type Output = IdSet<K>;

    fn bitor(self, rhs: IdSet<K>) -> Self::Output {
        (&*self).bitor(&*rhs).into()
    }
}
impl<K: Eq + From<Id>> BitOr<&IdSet<K>> for IdSet<K> {
    type Output = IdSet<K>;

    fn bitor(self, rhs: &IdSet<K>) -> Self::Output {
        (&*self).bitor(&**rhs).into()
    }
}
impl<K: Eq + From<Id>> BitOr<IdSet<K>> for &IdSet<K> {
    type Output = IdSet<K>;

    fn bitor(self, rhs: IdSet<K>) -> Self::Output {
        rhs.bitor(self)
    }
}
impl<K: Eq + From<Id>> BitOr<&IdSet<K>> for &IdSet<K> {
    type Output = IdSet<K>;

    fn bitor(self, rhs: &IdSet<K>) -> Self::Output {
        (&**self).bitor(&**rhs).into()
    }
}

impl<K: Eq + From<Id>> BitOrAssign<IdSet<K>> for IdSet<K> {
    fn bitor_assign(&mut self, mut rhs: IdSet<K>) {
        self.0.bitor_assign(rhs.0);
    }
}
impl<K: Eq + From<Id>> BitOrAssign<&IdSet<K>> for IdSet<K> {
    fn bitor_assign(&mut self, rhs: &IdSet<K>) {
        self.0.bitor_assign(&rhs.0);
    }
}

impl<K: Eq + From<Id>> Sub<&IdSet<K>> for &IdSet<K> {
    type Output = IdSet<K>;

    fn sub(self, rhs: &IdSet<K>) -> Self::Output {
        (&self.0 - &rhs.0).into()
    }
}

impl<K: Eq + From<Id>> Sub<IdSet<K>> for &IdSet<K> {
    type Output = IdSet<K>;

    fn sub(self, rhs: IdSet<K>) -> Self::Output {
        Sub::sub(self, &rhs)
    }
}
impl<K: Eq + From<Id>> Sub<&IdSet<K>> for IdSet<K> {
    type Output = IdSet<K>;

    fn sub(mut self, rhs: &IdSet<K>) -> Self::Output {
        SubAssign::sub_assign(&mut self, rhs);
        self
    }
}
impl<K: Eq + From<Id>> Sub<IdSet<K>> for IdSet<K> {
    type Output = IdSet<K>;

    fn sub(mut self, rhs: IdSet<K>) -> Self::Output {
        SubAssign::sub_assign(&mut self, &rhs);
        self
    }
}

impl<K: Eq + From<Id>> SubAssign<IdSet<K>> for IdSet<K> {
    fn sub_assign(&mut self, rhs: IdSet<K>) {
        SubAssign::sub_assign(self, &rhs)
    }
}
impl<K: Eq + From<Id>> SubAssign<&IdSet<K>> for IdSet<K> {
    fn sub_assign(&mut self, rhs: &IdSet<K>) {
        SubAssign::sub_assign(&mut self.0, &rhs.0);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct IdSpace(pub(crate) RangeSetBlaze<Id>);

impl serde::Serialize for IdSpace {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let ranges: Vec<(Id, Id)> = self.0.ranges().map(|r| (*r.start(), *r.end())).collect();
        let mut seq = serializer.serialize_seq(Some(ranges.len()))?;
        for pair in &ranges {
            seq.serialize_element(pair)?;
        }
        seq.end()
    }
}

impl<'de> serde::Deserialize<'de> for IdSpace {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let ranges: Vec<(Id, Id)> = serde::Deserialize::deserialize(deserializer)?;
        let mut rsb = RangeSetBlaze::new();
        for (start, end) in ranges {
            rsb.ranges_insert(start..=end);
        }
        Ok(IdSpace(rsb))
    }
}

impl Default for IdSpace {
    fn default() -> Self {
        let mut rs = RangeSetBlaze::new();
        rs.ranges_insert(0..=Id::MAX);
        Self(rs)
    }
}

impl IdSpace {
    pub(crate) fn new_starting_at(first_id: Id) -> Self {
        let mut rs = RangeSetBlaze::new();
        rs.ranges_insert(first_id..=Id::MAX);
        Self(rs)
    }

    pub(crate) fn remove_id(&mut self, id: Id) {
        self.0.remove(id);
    }

    pub(crate) fn pop_id(&mut self) -> Option<Id> {
        self.0.pop_first()
    }

    pub(crate) fn push_id(&mut self, id: Id) {
        self.0.insert(id);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use itertools::Itertools;

    use range_set_blaze::{prelude::*, *};

    use super::*;
    use crate::id;

    #[test]
    fn t_idset_fmt_debug() {
        let s = IdSet::<id::N>::from(untyped::IdSet::from([10, 20, 30, 500]));
        dbg!(s);
    }

    #[test]
    fn t_idset_partial_eq() {
        let ids1: IdSet<id::N> = untyped::IdSet::from_iter(0..10u32).into();
        let ids2: IdSet<id::N> = untyped::IdSet::from_iter(0..10u32).into();
        let ids3: IdSet<id::N> = untyped::IdSet::from_iter(1..11u32).into();

        assert!(ids1 == ids2);
        assert!(ids1 != ids3);
    }

    #[test]
    fn t_idset_partial_ord() {
        let ids1: IdSet<id::N> = untyped::IdSet::from_iter(0..10u32).into();
        let ids2: IdSet<id::N> = untyped::IdSet::from_iter(0..10u32).into();
        let ids3: IdSet<id::N> = untyped::IdSet::from_iter(1..11u32).into();
        let ids4: IdSet<id::N> = untyped::IdSet::from_iter(0..9u32).into();
        let ids5: IdSet<id::N> = untyped::IdSet::from([10, 20, 30, 40, 49]).into();
        let ids6: IdSet<id::N> = untyped::IdSet::from([10, 20, 30, 40, 50]).into();

        assert!(!(ids1 < ids2));
        assert!(!(ids1 > ids2));
        assert!(ids1 < ids3);
        assert!(ids4 < ids3);
        assert!(ids5 < ids6);

        let mut set = BTreeSet::new();

        set.insert(ids1);
        set.insert(ids5);

        assert!(set.contains(&ids2));
        assert!(set.contains(&untyped::IdSet::from([10, 20, 30, 40, 49]).into()));

        assert!(!set.contains(&ids3));
        assert!(!set.contains(&ids6));
    }

    #[test]
    fn t_idset_bitand() {
        assert_eq!(
            IdSet::<id::N>::from(untyped::IdSet::from_iter(5..7u32)),
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..7u32))
                & IdSet::<id::N>::from(untyped::IdSet::from_iter(5..11u32))
        );

        assert_eq!(
            IdSet::<id::N>::from(untyped::IdSet::from_iter(5..7u32)),
            &IdSet::<id::N>::from(untyped::IdSet::from_iter(0..7u32))
                & IdSet::<id::N>::from(untyped::IdSet::from_iter(5..11u32))
        );

        assert_eq!(
            IdSet::<id::N>::from(untyped::IdSet::from_iter(5..7u32)),
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..7u32))
                & &IdSet::<id::N>::from(untyped::IdSet::from_iter(5..11u32))
        );

        assert_eq!(
            IdSet::<id::N>::from(untyped::IdSet::from_iter(5..7u32)),
            &IdSet::<id::N>::from(untyped::IdSet::from_iter(0..7u32))
                & &IdSet::<id::N>::from(untyped::IdSet::from_iter(5..11u32))
        );

        let mut ids1 = IdSet::<id::N>::from(untyped::IdSet::from_iter(5..17u32));
        ids1 &= IdSet::<id::N>::from(untyped::IdSet::from_iter(15..19u32));
        assert_eq!(
            ids1,
            IdSet::<id::N>::from(untyped::IdSet::from_iter(15..17u32))
        );

        let mut ids1 = IdSet::<id::N>::from(untyped::IdSet::from_iter(15..19u32));
        ids1 &= IdSet::<id::N>::from(untyped::IdSet::from_iter(5..17u32));
        assert_eq!(
            ids1,
            IdSet::<id::N>::from(untyped::IdSet::from_iter(15..17u32))
        );

        let mut ids1 = IdSet::<id::N>::from(untyped::IdSet::from_iter(5..17u32));
        ids1 &= &IdSet::<id::N>::from(untyped::IdSet::from_iter(15..19u32));
        assert_eq!(
            ids1,
            IdSet::<id::N>::from(untyped::IdSet::from_iter(15..17u32))
        );

        let mut ids1 = IdSet::<id::N>::from(untyped::IdSet::from_iter(15..19u32));
        ids1 &= &IdSet::<id::N>::from(untyped::IdSet::from_iter(5..17u32));
        assert_eq!(
            ids1,
            IdSet::<id::N>::from(untyped::IdSet::from_iter(15..17u32))
        );
    }

    #[test]
    fn t_idset_bitor() {
        assert_eq!(
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..11u32)),
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..7u32))
                | IdSet::<id::N>::from(untyped::IdSet::from_iter(5..11u32))
        );

        assert_eq!(
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..11u32)),
            &IdSet::<id::N>::from(untyped::IdSet::from_iter(0..7u32))
                | IdSet::<id::N>::from(untyped::IdSet::from_iter(5..11u32))
        );

        assert_eq!(
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..11u32)),
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..7u32))
                | &IdSet::<id::N>::from(untyped::IdSet::from_iter(5..11u32))
        );

        assert_eq!(
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..11u32)),
            &IdSet::<id::N>::from(untyped::IdSet::from_iter(0..7u32))
                | &IdSet::<id::N>::from(untyped::IdSet::from_iter(5..11u32))
        );

        let mut ids1 = IdSet::<id::N>::from(untyped::IdSet::from_iter(5..17u32));
        ids1 |= IdSet::<id::N>::from(untyped::IdSet::from_iter(15..19u32));
        assert_eq!(
            ids1,
            IdSet::<id::N>::from(untyped::IdSet::from_iter(5..19u32))
        );

        let mut ids1 = IdSet::<id::N>::from(untyped::IdSet::from_iter(15..19u32));
        ids1 |= IdSet::<id::N>::from(untyped::IdSet::from_iter(5..17u32));
        assert_eq!(
            ids1,
            IdSet::<id::N>::from(untyped::IdSet::from_iter(5..19u32))
        );

        let mut ids1 = IdSet::<id::N>::from(untyped::IdSet::from_iter(5..17u32));
        ids1 |= &IdSet::<id::N>::from(untyped::IdSet::from_iter(15..19u32));
        assert_eq!(
            ids1,
            IdSet::<id::N>::from(untyped::IdSet::from_iter(5..19u32))
        );

        let mut ids1 = IdSet::<id::N>::from(untyped::IdSet::from_iter(15..19u32));
        ids1 |= &IdSet::<id::N>::from(untyped::IdSet::from_iter(5..17u32));
        assert_eq!(
            ids1,
            IdSet::<id::N>::from(untyped::IdSet::from_iter(5..19u32))
        );
    }

    #[test]
    fn t_idset_sub() {
        assert_eq!(
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..5u32)),
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..7u32))
                - IdSet::<id::N>::from(untyped::IdSet::from_iter(5..11u32))
        );

        assert_eq!(
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..5u32)),
            &IdSet::<id::N>::from(untyped::IdSet::from_iter(0..7u32))
                - IdSet::<id::N>::from(untyped::IdSet::from_iter(5..11u32))
        );

        assert_eq!(
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..5u32)),
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..7u32))
                - &IdSet::<id::N>::from(untyped::IdSet::from_iter(5..11u32))
        );

        assert_eq!(
            IdSet::<id::N>::from(untyped::IdSet::from_iter(0..5u32)),
            &IdSet::<id::N>::from(untyped::IdSet::from_iter(0..7u32))
                - &IdSet::<id::N>::from(untyped::IdSet::from_iter(5..11u32))
        );

        let mut ids1 = IdSet::<id::N>::from(untyped::IdSet::from_iter(5..17u32));
        ids1 -= IdSet::<id::N>::from(untyped::IdSet::from_iter(15..19u32));
        assert_eq!(
            ids1,
            IdSet::<id::N>::from(untyped::IdSet::from_iter(5..15u32))
        );

        let mut ids1 = IdSet::<id::N>::from(untyped::IdSet::from_iter(15..19u32));
        ids1 -= IdSet::<id::N>::from(untyped::IdSet::from_iter(5..17u32));
        assert_eq!(
            ids1,
            IdSet::<id::N>::from(untyped::IdSet::from_iter(17..19u32))
        );

        let mut ids1 = IdSet::<id::N>::from(untyped::IdSet::from_iter(5..17u32));
        ids1 -= &IdSet::<id::N>::from(untyped::IdSet::from_iter(15..19u32));
        assert_eq!(
            ids1,
            IdSet::<id::N>::from(untyped::IdSet::from_iter(5..15u32))
        );

        let mut ids1 = IdSet::<id::N>::from(untyped::IdSet::from_iter(15..19u32));
        ids1 -= &IdSet::<id::N>::from(untyped::IdSet::from_iter(5..17u32));
        assert_eq!(
            ids1,
            IdSet::<id::N>::from(untyped::IdSet::from_iter(17..19u32))
        );
    }

    #[test]
    fn t_idspace() {
        let mut s = IdSpace::default();
        dbg!(s.pop_id());
        dbg!(s.pop_id());
        dbg!(s.pop_id());
        dbg!(s.pop_id());
        dbg!(s.pop_id());

        dbg!(s.push_id(4));
        dbg!(&s.0);

        dbg!(s.push_id(2));
        dbg!(&s.0);

        dbg!(s.push_id(0));
        dbg!(&s.0);

        dbg!(s.push_id(1));
        dbg!(&s.0);

        dbg!(s.push_id(3));
        dbg!(&s.0);

        dbg!(s.pop_id());
        dbg!(s.pop_id());
        dbg!(s.pop_id());
        dbg!(s.pop_id());
        dbg!(s.pop_id());
    }

    #[test]
    fn t_pop2() {
        let mut s = IdSpace::new_starting_at(6);
        dbg!(&s);
        dbg!(s.pop_id());
        dbg!(&s);
        dbg!(s.pop_id());
    }

    #[test]

    fn t_pop3() {
        let mut ids = IdSpace::default();
        dbg!(&ids);

        let x1 = dbg!(ids.pop_id());
        let x2 = dbg!(ids.pop_id());
        let x3 = dbg!(ids.pop_id());
        let x4 = dbg!(ids.pop_id());
        let x5 = dbg!(ids.pop_id());

        dbg!(&ids);

        ids.push_id(0);
        dbg!(&ids);

        ids.push_id(3);
        dbg!(&ids);

        let x1 = dbg!(ids.pop_id());
    }
}
