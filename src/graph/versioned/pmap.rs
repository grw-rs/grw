use std::hash::{Hash, Hasher};
use std::sync::Arc;

use rustc_hash::FxHasher;
use smallvec::SmallVec;

use super::trie::PVec;
use crate::graph::error;
use crate::graph::index::{Cardinality, Catalogue, IdSet, IdView, IndexDecl, IndexHit, IndexName, KeyBytes, KeyTag};
use crate::graph::index;
use crate::id;

type Bucket<V> = SmallVec<[(KeyBytes, V); 2]>;

fn slot_of(k: &KeyBytes) -> u32 {
    let mut hasher = FxHasher::default();
    k.hash(&mut hasher);
    hasher.finish() as u32
}

pub(crate) struct PMap<V: Clone> {
    slots: PVec<Arc<Bucket<V>>>,
    len: usize,
}

impl<V: Clone> Clone for PMap<V> {
    fn clone(&self) -> Self {
        PMap { slots: self.slots.clone(), len: self.len }
    }
}

impl<V: Clone> PMap<V> {
    pub(crate) fn new() -> Self {
        PMap { slots: PVec::new(), len: 0 }
    }

    fn get(&self, k: &KeyBytes) -> Option<&V> {
        self.slots.get(slot_of(k))?.iter().find(|(kb, _)| kb == k).map(|(_, v)| v)
    }

    fn set(&self, k: KeyBytes, v: V) -> (Self, Option<V>) {
        let slot = slot_of(&k);
        let mut bucket: Bucket<V> = self.slots.get(slot).map(|b| (**b).clone()).unwrap_or_default();
        let old = match bucket.iter().position(|(kb, _)| *kb == k) {
            Some(pos) => Some(std::mem::replace(&mut bucket[pos].1, v)),
            None => {
                bucket.push((k, v));
                None
            }
        };
        let slots = self.slots.set(slot, Arc::new(bucket));
        let len = if old.is_some() { self.len } else { self.len + 1 };
        (PMap { slots, len }, old)
    }

    fn remove(&self, k: &KeyBytes) -> (Self, Option<V>) {
        let slot = slot_of(k);
        let Some(bucket) = self.slots.get(slot) else {
            return (self.clone(), None);
        };
        let Some(pos) = bucket.iter().position(|(kb, _)| kb == k) else {
            return (self.clone(), None);
        };
        let mut bucket = (**bucket).clone();
        let (_, old) = bucket.remove(pos);
        let slots =
            if bucket.is_empty() { self.slots.remove(slot) } else { self.slots.set(slot, Arc::new(bucket)) };
        (PMap { slots, len: self.len - 1 }, Some(old))
    }

    fn len(&self) -> usize {
        self.len
    }

    fn iter(&self) -> impl Iterator<Item = (&KeyBytes, &V)> {
        self.slots.iter().flat_map(|(_, bucket)| bucket.iter().map(|(k, v)| (k, v)))
    }
}

pub(crate) struct VIndices<NV> {
    decls: Vec<IndexDecl<NV>>,
    unique: Vec<PMap<id::N>>,
    multi: Vec<PMap<IdSet<id::N>>>,
}

impl<NV> Clone for VIndices<NV> {
    fn clone(&self) -> Self {
        VIndices { decls: self.decls.clone(), unique: self.unique.clone(), multi: self.multi.clone() }
    }
}

impl<NV> VIndices<NV> {
    pub(crate) fn new() -> Self {
        VIndices { decls: Vec::new(), unique: Vec::new(), multi: Vec::new() }
    }

    pub(crate) fn from_parts(decls: Vec<IndexDecl<NV>>) -> Self {
        let unique_count = decls.iter().filter(|d| d.cardinality() == Cardinality::Unique).count();
        let multi_count = decls.len() - unique_count;
        VIndices {
            decls,
            unique: (0..unique_count).map(|_| PMap::new()).collect(),
            multi: (0..multi_count).map(|_| PMap::new()).collect(),
        }
    }

    pub(crate) fn decls(&self) -> &[IndexDecl<NV>] {
        &self.decls
    }

    /// Restores tables from parsed snapshot entries rather than rebuilding
    /// them by scanning node values through each declaration's extractor.
    pub(crate) fn from_tables(
        decls: Vec<IndexDecl<NV>>,
        unique: Vec<Vec<(KeyBytes, id::N)>>,
        multi: Vec<Vec<(KeyBytes, IdSet<id::N>)>>,
    ) -> Self {
        let expected_unique = decls.iter().filter(|d| d.cardinality() == Cardinality::Unique).count();
        let expected_multi = decls.len() - expected_unique;
        assert_eq!(unique.len(), expected_unique, "VIndices::from_tables: unique table count does not match catalogue");
        assert_eq!(multi.len(), expected_multi, "VIndices::from_tables: multi table count does not match catalogue");
        VIndices {
            decls,
            unique: unique.into_iter().map(|entries| entries.into_iter().fold(PMap::new(), |m, (k, v)| m.set(k, v).0)).collect(),
            multi: multi.into_iter().map(|entries| entries.into_iter().fold(PMap::new(), |m, (k, v)| m.set(k, v).0)).collect(),
        }
    }

    pub(crate) fn unique_entries(&self) -> Vec<Vec<(KeyBytes, id::N)>> {
        self.unique.iter().map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect()).collect()
    }

    pub(crate) fn multi_entries(&self) -> Vec<Vec<(KeyBytes, IdSet<id::N>)>> {
        self.multi.iter().map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).collect()
    }

    fn table_pos(&self, i: usize) -> usize {
        self.decls[..i].iter().filter(|d| d.cardinality() == self.decls[i].cardinality()).count()
    }

    pub(crate) fn catalogue(&self) -> Catalogue {
        index::build_catalogue(&self.decls)
    }

    pub(crate) fn unique_get(&self, pos: usize, key: &KeyBytes) -> Option<id::N> {
        self.unique[pos].get(key).copied()
    }

    pub(crate) fn hit<'a>(
        &'a self,
        name: IndexName,
        key: &KeyBytes,
        tag: KeyTag,
    ) -> Result<IndexHit<'a>, error::Index> {
        let idx = self.decls.iter().position(|d| d.name() == name).ok_or(error::Index::NoSuchIndex(name))?;
        let decl = &self.decls[idx];
        if decl.tag() != tag {
            return Err(error::Index::KeyTagMismatch { index: name, expected: decl.tag(), got: tag });
        }
        let pos = self.table_pos(idx);
        Ok(match decl.cardinality() {
            Cardinality::Unique => match self.unique[pos].get(key) {
                Some(&n) => IndexHit::One(n),
                None => IndexHit::None,
            },
            Cardinality::Multi => match self.multi[pos].get(key) {
                Some(set) if !set.is_empty() => IndexHit::Many(IdView::of(set)),
                _ => IndexHit::None,
            },
        })
    }

    pub(crate) fn insert_key_for(&mut self, i: usize, id: id::N, val: &NV) {
        let Some(key) = self.decls[i].key_of(val) else { return };
        let pos = self.table_pos(i);
        match self.decls[i].cardinality() {
            Cardinality::Unique => {
                let (next, _) = self.unique[pos].set(key, id);
                self.unique[pos] = next;
            }
            Cardinality::Multi => {
                let mut set = self.multi[pos].get(&key).cloned().unwrap_or_default();
                set.insert(&id);
                let (next, _) = self.multi[pos].set(key, set);
                self.multi[pos] = next;
            }
        }
    }

    pub(crate) fn remove_key_for(&mut self, i: usize, id: id::N, val: &NV) {
        let Some(key) = self.decls[i].key_of(val) else { return };
        let pos = self.table_pos(i);
        match self.decls[i].cardinality() {
            Cardinality::Unique => {
                let (next, _) = self.unique[pos].remove(&key);
                self.unique[pos] = next;
            }
            Cardinality::Multi => {
                if let Some(set) = self.multi[pos].get(&key) {
                    let mut set = set.clone();
                    set.remove(&id);
                    let (next, _) =
                        if set.is_empty() { self.multi[pos].remove(&key) } else { self.multi[pos].set(key, set) };
                    self.multi[pos] = next;
                }
            }
        }
    }

    pub(crate) fn insert_node(&mut self, id: id::N, val: &NV) {
        for i in 0..self.decls.len() {
            self.insert_key_for(i, id, val);
        }
    }

    pub(crate) fn remove_node(&mut self, id: id::N, val: &NV) {
        for i in 0..self.decls.len() {
            self.remove_key_for(i, id, val);
        }
    }

    pub(crate) fn push_decl(&mut self, decl: IndexDecl<NV>) -> Result<(), error::Index> {
        if self.decls.iter().any(|d| d.name() == decl.name()) {
            return Err(error::Index::DuplicateName(decl.name()));
        }
        match decl.cardinality() {
            Cardinality::Unique => self.unique.push(PMap::new()),
            Cardinality::Multi => self.multi.push(PMap::new()),
        }
        self.decls.push(decl);
        Ok(())
    }

    pub(crate) fn remove_decl(&mut self, name: IndexName) -> Result<IndexDecl<NV>, error::Index> {
        let idx = self.decls.iter().position(|d| d.name() == name).ok_or(error::Index::NoSuchIndex(name))?;
        let pos = self.table_pos(idx);
        let decl = self.decls.remove(idx);
        match decl.cardinality() {
            Cardinality::Unique => {
                self.unique.remove(pos);
            }
            Cardinality::Multi => {
                self.multi.remove(pos);
            }
        }
        Ok(decl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_remove_roundtrip() {
        let m0: PMap<u32> = PMap::new();
        let (m1, prior) = m0.set(KeyBytes::of(&1u32), 10);
        assert_eq!(prior, None);
        let (m2, prior) = m1.set(KeyBytes::of(&2u32), 20);
        assert_eq!(prior, None);
        assert_eq!(m2.get(&KeyBytes::of(&1u32)), Some(&10));
        assert_eq!(m2.get(&KeyBytes::of(&2u32)), Some(&20));
        assert_eq!(m2.get(&KeyBytes::of(&3u32)), None);
        assert_eq!(m2.len(), 2);

        let (m3, removed) = m2.remove(&KeyBytes::of(&1u32));
        assert_eq!(removed, Some(10));
        assert_eq!(m3.get(&KeyBytes::of(&1u32)), None);
        assert_eq!(m3.len(), 1);
        assert_eq!(m2.get(&KeyBytes::of(&1u32)), Some(&10), "prior version untouched");
        assert_eq!(m2.len(), 2, "prior version length untouched");
    }

    #[test]
    fn set_overwrite_keeps_len_and_returns_old() {
        let m0: PMap<u32> = PMap::new();
        let (m1, prior) = m0.set(KeyBytes::of(&5u32), 100);
        assert_eq!(prior, None);
        assert_eq!(m1.len(), 1);
        let (m2, prior) = m1.set(KeyBytes::of(&5u32), 200);
        assert_eq!(prior, Some(100));
        assert_eq!(m2.len(), 1);
        assert_eq!(m2.get(&KeyBytes::of(&5u32)), Some(&200));
        assert_eq!(m1.get(&KeyBytes::of(&5u32)), Some(&100));
    }

    #[test]
    fn remove_absent_key_is_noop() {
        let m0: PMap<u32> = PMap::new();
        let (m1, _) = m0.set(KeyBytes::of(&1u32), 1);
        let (m2, removed) = m1.remove(&KeyBytes::of(&999u32));
        assert_eq!(removed, None);
        assert_eq!(m2.len(), m1.len());
    }

    #[test]
    fn iter_yields_every_entry() {
        let m0: PMap<u32> = PMap::new();
        let (m1, _) = m0.set(KeyBytes::of(&1u32), 10);
        let (m2, _) = m1.set(KeyBytes::of(&2u32), 20);
        let (m3, _) = m2.set(KeyBytes::of(&3u32), 30);
        let mut got: Vec<u32> = m3.iter().map(|(_, v)| *v).collect();
        got.sort_unstable();
        assert_eq!(got, vec![10, 20, 30]);
    }

    #[test]
    fn untouched_bucket_shares_pointer_across_versions() {
        let m0: PMap<u32> = PMap::new();
        let (m1, _) = m0.set(KeyBytes::of(&11u32), 1);
        let (m1, _) = m1.set(KeyBytes::of(&2_000_003u32), 2);
        let (m2, _) = m1.set(KeyBytes::of(&11u32), 99);

        let untouched_key = KeyBytes::of(&2_000_003u32);
        let before = m1.get(&untouched_key).expect("present before");
        let after = m2.get(&untouched_key).expect("present after");
        assert!(std::ptr::eq(before, after), "untouched bucket must share memory across versions");
        assert_eq!(*after, 2);

        let touched_key = KeyBytes::of(&11u32);
        assert_eq!(m1.get(&touched_key), Some(&1));
        assert_eq!(m2.get(&touched_key), Some(&99));
    }
}
