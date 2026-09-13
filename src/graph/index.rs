use crate::graph::collections::FxHashMap;
use crate::id;
use std::sync::Arc;

pub(crate) use crate::graph::collections::IdSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndexName(pub &'static str);

impl std::fmt::Display for IndexName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    Unique,
    Multi,
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyBytes(Box<[u8]>);

impl KeyBytes {
    pub fn of<K: serde::Serialize>(key: &K) -> Self {
        KeyBytes(bincode::serialize(key).expect("index key serializes with bincode").into_boxed_slice())
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub(crate) fn from_bytes(bytes: Vec<u8>) -> Self {
        KeyBytes(bytes.into_boxed_slice())
    }
}

impl std::fmt::Debug for KeyBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("KeyBytes({} bytes)", self.0.len()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyTag(pub u64);

impl KeyTag {
    pub fn of<K: 'static>() -> Self {
        let name = std::any::type_name::<K>();
        KeyTag(crate::layout::fnv_hash_bytes(crate::layout::FNV_OFFSET, name.as_bytes()))
    }
}

pub trait KeyOf<NV>: Send + Sync + 'static {
    type Key: serde::Serialize + 'static;
    fn key(&self, v: &NV) -> Option<Self::Key>;
}

impl<NV, F, K> KeyOf<NV> for F
where
    F: Fn(&NV) -> Option<K> + Send + Sync + 'static,
    K: serde::Serialize + 'static,
{
    type Key = K;
    fn key(&self, v: &NV) -> Option<K> {
        self(v)
    }
}

type Extract<NV> = Arc<dyn Fn(&NV) -> Option<KeyBytes> + Send + Sync>;

pub struct IndexDecl<NV> {
    name: IndexName,
    cardinality: Cardinality,
    tag: KeyTag,
    extract: Extract<NV>,
}

impl<NV> IndexDecl<NV> {
    pub fn new<X: KeyOf<NV>>(name: IndexName, cardinality: Cardinality, x: X) -> Self {
        let tag = KeyTag::of::<X::Key>();
        let extract: Extract<NV> = Arc::new(move |v: &NV| x.key(v).map(|k| KeyBytes::of(&k)));
        IndexDecl { name, cardinality, tag, extract }
    }

    pub fn name(&self) -> IndexName {
        self.name
    }
    pub fn cardinality(&self) -> Cardinality {
        self.cardinality
    }
    pub fn tag(&self) -> KeyTag {
        self.tag
    }

    pub(crate) fn key_of(&self, v: &NV) -> Option<KeyBytes> {
        (self.extract)(v)
    }
}

impl<NV> Clone for IndexDecl<NV> {
    fn clone(&self) -> Self {
        IndexDecl { name: self.name, cardinality: self.cardinality, tag: self.tag, extract: self.extract.clone() }
    }
}

/// A borrowed read view over the node ids a `Multi` index holds under one
/// key. The backing set is a crate-private structure; this view is the whole
/// public surface over it.
#[derive(Clone, Copy)]
pub struct IdView<'a>(&'a IdSet<id::N>);

impl<'a> IdView<'a> {
    pub(crate) fn of(set: &'a IdSet<id::N>) -> Self {
        IdView(set)
    }

    pub fn iter(&self) -> impl Iterator<Item = id::N> + '_ {
        self.0.iter().map(id::N)
    }

    pub fn len(&self) -> usize {
        self.0.len() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains(&self, n: id::N) -> bool {
        self.0.contains_key(&n)
    }
}

impl std::fmt::Debug for IdView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

pub enum IndexHit<'a> {
    One(id::N),
    Many(IdView<'a>),
    None,
}

#[derive(Debug, Clone)]
pub struct Catalogue(Vec<(IndexName, Cardinality, KeyTag)>);

impl Catalogue {
    pub fn iter(&self) -> impl Iterator<Item = (IndexName, Cardinality, KeyTag)> + '_ {
        self.0.iter().copied()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub(crate) fn build_catalogue<NV>(decls: &[IndexDecl<NV>]) -> Catalogue {
    let mut entries: Vec<(IndexName, Cardinality, KeyTag)> =
        decls.iter().map(|d| (d.name(), d.cardinality(), d.tag())).collect();
    entries.sort_by_key(|(name, _, _)| name.0);
    Catalogue(entries)
}

pub(crate) struct Indices<NV> {
    decls: Vec<IndexDecl<NV>>,
    unique: Vec<FxHashMap<KeyBytes, id::N>>,
    multi: Vec<FxHashMap<KeyBytes, IdSet<id::N>>>,
}

impl<NV> Indices<NV> {
    /// The catalogue of a graph that declares no index. Spelled out at every
    /// construction site; there is no `Default` to fall back on.
    pub(crate) fn empty() -> Self {
        Indices { decls: Vec::new(), unique: Vec::new(), multi: Vec::new() }
    }

    pub(crate) fn decls(&self) -> &[IndexDecl<NV>] {
        &self.decls
    }
    pub(crate) fn unique_tables(&self) -> &[FxHashMap<KeyBytes, id::N>] {
        &self.unique
    }
    pub(crate) fn multi_tables(&self) -> &[FxHashMap<KeyBytes, IdSet<id::N>>] {
        &self.multi
    }

    pub(crate) fn from_parts(
        decls: Vec<IndexDecl<NV>>,
        unique: Vec<FxHashMap<KeyBytes, id::N>>,
        multi: Vec<FxHashMap<KeyBytes, IdSet<id::N>>>,
    ) -> Self {
        let expected_unique = decls.iter().filter(|d| d.cardinality() == Cardinality::Unique).count();
        let expected_multi = decls.len() - expected_unique;
        assert_eq!(unique.len(), expected_unique, "Indices::from_parts: unique table count does not match catalogue");
        assert_eq!(multi.len(), expected_multi, "Indices::from_parts: multi table count does not match catalogue");
        Indices { decls, unique, multi }
    }

    pub(crate) fn catalogue(&self) -> Catalogue {
        build_catalogue(&self.decls)
    }

    fn table_pos(&self, i: usize) -> usize {
        self.decls[..i].iter().filter(|d| d.cardinality() == self.decls[i].cardinality()).count()
    }

    pub(crate) fn hit<'a>(
        &'a self,
        name: IndexName,
        key: &KeyBytes,
        tag: KeyTag,
    ) -> Result<IndexHit<'a>, super::error::Index> {
        let idx = self.decls.iter().position(|d| d.name() == name).ok_or(super::error::Index::NoSuchIndex(name))?;
        let decl = &self.decls[idx];
        if decl.tag() != tag {
            return Err(super::error::Index::KeyTagMismatch { index: name, expected: decl.tag(), got: tag });
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
                self.unique[pos].insert(key, id);
            }
            Cardinality::Multi => {
                self.multi[pos].entry(key).or_default().insert(&id);
            }
        }
    }

    pub(crate) fn remove_key_for(&mut self, i: usize, id: id::N, val: &NV) {
        let Some(key) = self.decls[i].key_of(val) else { return };
        let pos = self.table_pos(i);
        match self.decls[i].cardinality() {
            Cardinality::Unique => {
                self.unique[pos].remove(&key);
            }
            Cardinality::Multi => {
                if let Some(set) = self.multi[pos].get_mut(&key) {
                    set.remove(&id);
                    if set.is_empty() {
                        self.multi[pos].remove(&key);
                    }
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

    pub(crate) fn push_decl(&mut self, decl: IndexDecl<NV>) -> Result<(), super::error::Index> {
        if self.decls.iter().any(|d| d.name() == decl.name()) {
            return Err(super::error::Index::DuplicateName(decl.name()));
        }
        match decl.cardinality() {
            Cardinality::Unique => self.unique.push(FxHashMap::default()),
            Cardinality::Multi => self.multi.push(FxHashMap::default()),
        }
        self.decls.push(decl);
        Ok(())
    }

    pub(crate) fn remove_decl(&mut self, name: IndexName) -> Result<IndexDecl<NV>, super::error::Index> {
        let idx = self.decls.iter().position(|d| d.name() == name).ok_or(super::error::Index::NoSuchIndex(name))?;
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

impl<NV> Clone for Indices<NV> {
    fn clone(&self) -> Self {
        Indices { decls: self.decls.clone(), unique: self.unique.clone(), multi: self.multi.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_tag_stable_and_type_distinct() {
        assert_eq!(KeyTag::of::<u32>(), KeyTag::of::<u32>());
        assert_ne!(KeyTag::of::<u32>(), KeyTag::of::<u8>());
    }

    #[test]
    fn key_bytes_equal_for_equal_values() {
        assert_eq!(KeyBytes::of(&7u32), KeyBytes::of(&7u32));
        assert_ne!(KeyBytes::of(&7u32), KeyBytes::of(&8u32));
    }

    #[test]
    fn catalogue_sorted_by_name() {
        let decls = vec![
            IndexDecl::new(IndexName("zeta"), Cardinality::Unique, |v: &u32| Some(*v)),
            IndexDecl::new(IndexName("alpha"), Cardinality::Multi, |v: &u32| Some(*v % 2)),
        ];
        let indices = Indices::from_parts(decls, vec![FxHashMap::default()], vec![FxHashMap::default()]);
        let names: Vec<&str> = indices.catalogue().iter().map(|(n, _, _)| n.0).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }

    #[test]
    fn insert_hit_remove_roundtrip() {
        let decls = vec![IndexDecl::new(IndexName("by_val"), Cardinality::Unique, |v: &u32| Some(*v))];
        let mut indices = Indices::from_parts(decls, vec![FxHashMap::default()], vec![]);
        indices.insert_node(id::N(0), &42u32);
        let tag = KeyTag::of::<u32>();
        match indices.hit(IndexName("by_val"), &KeyBytes::of(&42u32), tag).unwrap() {
            IndexHit::One(n) => assert_eq!(n, id::N(0)),
            _ => panic!("expected One"),
        }
        indices.remove_node(id::N(0), &42u32);
        assert!(matches!(indices.hit(IndexName("by_val"), &KeyBytes::of(&42u32), tag).unwrap(), IndexHit::None));
    }

    #[test]
    fn from_parts_panics_on_cardinality_mismatch() {
        let decls = vec![IndexDecl::new(IndexName("by_val"), Cardinality::Unique, |v: &u32| Some(*v))];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Indices::from_parts(decls, vec![], vec![]);
        }));
        assert!(result.is_err());
    }
}
