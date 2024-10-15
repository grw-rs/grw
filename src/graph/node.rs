use smallvec::SmallVec;
use std::hash::{Hash, Hasher};

use crate::Id;
use crate::id;

#[derive(Debug, Clone)]
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct Adjacents(SmallVec<[(id::N, id::E); 8]>);

impl PartialEq for Adjacents {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl Eq for Adjacents {}

impl Hash for Adjacents {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut count: usize = 0;
        let mut last = None;
        for &(n, _) in &self.0 {
            if last != Some(n) {
                last = Some(n);
                n.hash(state);
                count += 1;
            }
        }
        count.hash(state);
    }
}

impl Adjacents {
    pub(crate) fn new() -> Self {
        Self(SmallVec::new())
    }

    pub(crate) fn insert(&mut self, n: id::N, e: id::E) {
        match self.0.binary_search_by_key(&(n, e), |&(node, edge)| (node, edge)) {
            Ok(_) => {}
            Err(pos) => self.0.insert(pos, (n, e)),
        }
    }

    pub(crate) fn remove_entry(&mut self, n: id::N, e: id::E) -> bool {
        match self.0.binary_search_by_key(&(n, e), |&(node, edge)| (node, edge)) {
            Ok(pos) => {
                self.0.remove(pos);
                true
            }
            Err(_) => false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn remove(&mut self, n: &id::N) -> bool {
        let start = self.0.partition_point(|&(node, _)| node < *n);
        let end = start + self.0[start..].iter().take_while(|&&(node, _)| node == *n).count();
        if start == end {
            return false;
        }
        self.0.drain(start..end);
        true
    }

    pub(crate) fn contains(&self, n: id::N) -> bool {
        let pos = self.0.partition_point(|&(node, _)| node < n);
        pos < self.0.len() && self.0[pos].0 == n
    }

    pub(crate) fn edges_to(&self, n: id::N) -> impl Iterator<Item = id::E> + '_ {
        let pos = self.0.partition_point(|&(node, _)| node < n);
        self.0[pos..]
            .iter()
            .take_while(move |&&(node, _)| node == n)
            .map(|&(_, e)| e)
    }

    pub(crate) fn entries_iter(&self) -> impl Iterator<Item = (id::N, id::E)> + '_ {
        self.0.iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> Id {
        self.iter().count() as Id
    }

    pub(crate) fn entry_count(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = id::N> + '_ {
        let mut last = None;
        self.0.iter().filter_map(move |&(n, _)| {
            if last == Some(n) {
                None
            } else {
                last = Some(n);
                Some(n)
            }
        })
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Node<V> {
    pub(crate) val: V,
    pub(crate) adj: Adjacents,
}

impl<V> std::fmt::Debug for Node<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Node(deg={})", self.adj.len()))
    }
}

impl<V> Node<V> {
    pub(crate) fn new(val: V) -> Self {
        Self {
            val,
            adj: Adjacents::new(),
        }
    }
    pub fn has_adjacents(&self) -> bool {
        !self.adj.is_empty()
    }
    pub fn adjacents_len(&self) -> Id {
        self.adj.len()
    }
    pub fn adjacents_iter(&self) -> impl Iterator<Item = id::N> + '_ {
        self.adj.iter()
    }
}

impl<V> super::Nodes<V> {
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn has(&self, n: id::N) -> bool {
        self.store.get(*n as usize).is_some_and(|opt| opt.is_some())
    }

    pub fn get(&self, n: id::N) -> Option<&V> {
        self.store.get(*n as usize)?.as_ref().map(|node| &node.val)
    }

    pub fn degree(&self, n: id::N) -> Option<Id> {
        self.store.get(*n as usize)?.as_ref().map(|node| node.adj.len())
    }

    pub fn neighbors(&self, n: id::N) -> Option<impl Iterator<Item = id::N> + '_> {
        self.store.get(*n as usize)?.as_ref().map(|node| node.adj.iter())
    }

    pub fn iter(&self) -> impl Iterator<Item = (id::N, &V)> {
        self.store.iter().enumerate()
            .filter_map(|(i, opt)| opt.as_ref().map(|node| (id::N(i as Id), &node.val)))
    }

    pub(crate) fn get_node(&self, n: id::N) -> Option<&Node<V>> {
        self.store.get(*n as usize)?.as_ref()
    }

    pub(crate) fn get_node_mut(&mut self, n: id::N) -> Option<&mut Node<V>> {
        self.store.get_mut(*n as usize)?.as_mut()
    }

    pub(crate) fn nodes_iter(&self) -> impl Iterator<Item = (id::N, &Node<V>)> {
        self.store.iter().enumerate()
            .filter_map(|(i, opt)| opt.as_ref().map(|node| (id::N(i as Id), node)))
    }

    pub(crate) fn insert(&mut self, n: id::N, node: Node<V>) {
        let idx = *n as usize;
        if idx >= self.store.len() {
            self.store.resize_with(idx + 1, || None);
        }
        self.store[idx] = Some(node);
        self.count += 1;
    }

    pub(crate) fn remove(&mut self, n: id::N) -> Option<Node<V>> {
        let idx = *n as usize;
        let old = self.store.get_mut(idx)?.take();
        if old.is_some() { self.count -= 1; }
        old
    }

    pub(crate) fn get_two_mut(&mut self, a: id::N, b: id::N) -> [Option<&mut Node<V>>; 2] {
        let ai = *a as usize;
        let bi = *b as usize;
        assert_ne!(ai, bi);
        let len = self.store.len();
        if ai >= len || bi >= len {
            if ai >= len && bi >= len {
                return [None, None];
            }
            if ai >= len {
                return [None, self.store[bi].as_mut()];
            }
            return [self.store[ai].as_mut(), None];
        }
        if ai < bi {
            let (left, right) = self.store.split_at_mut(bi);
            [left[ai].as_mut(), right[0].as_mut()]
        } else {
            let (left, right) = self.store.split_at_mut(ai);
            [right[0].as_mut(), left[bi].as_mut()]
        }
    }
}

pub struct Batch<NV>(pub(crate) Result<super::Nodes<NV>, super::error::Node>);

impl From<crate::Id> for Batch<()> {
    fn from(count: crate::Id) -> Self {
        Batch(Ok(super::batch::collect_nspace_nodes(count)))
    }
}

impl From<Vec<crate::Id>> for Batch<()> {
    fn from(ids: Vec<crate::Id>) -> Self {
        Batch(super::batch::collect_nodes(
            &mut ids.into_iter().map(|id| (id, ())),
        ))
    }
}

impl<NV> From<Vec<(crate::Id, NV)>> for Batch<NV> {
    fn from(nvs: Vec<(crate::Id, NV)>) -> Self {
        Batch(super::batch::collect_nodes(&mut nvs.into_iter()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_n_deref() {
        let n = id::N(199);
        let id1 = *n;
        let rf = &n;
        let id2 = **rf;
        assert_eq!(id1, id2);
    }

    #[test]
    fn t_insert_sorted() {
        let mut adj = Adjacents::new();
        adj.insert(id::N(5), id::E(0));
        adj.insert(id::N(2), id::E(1));
        adj.insert(id::N(8), id::E(2));

        let ns: Vec<id::N> = adj.iter().collect();
        assert_eq!(ns, vec![id::N(2), id::N(5), id::N(8)]);
    }

    #[test]
    fn t_contains() {
        let mut adj = Adjacents::new();
        adj.insert(id::N(3), id::E(0));
        adj.insert(id::N(7), id::E(1));

        assert!(adj.contains(id::N(3)));
        assert!(adj.contains(id::N(7)));
        assert!(!adj.contains(id::N(5)));
    }

    #[test]
    fn t_eq_hash_ignores_edge_ids() {
        use std::collections::hash_map::DefaultHasher;

        let mut a = Adjacents::new();
        a.insert(id::N(1), id::E(10));
        a.insert(id::N(2), id::E(20));

        let mut b = Adjacents::new();
        b.insert(id::N(1), id::E(99));
        b.insert(id::N(2), id::E(88));

        assert_eq!(a, b);

        let hash = |adj: &Adjacents| {
            let mut h = DefaultHasher::new();
            adj.hash(&mut h);
            h.finish()
        };
        assert_eq!(hash(&a), hash(&b));
    }

    #[test]
    fn t_multi_edge_insert() {
        let mut adj = Adjacents::new();
        adj.insert(id::N(3), id::E(0));
        adj.insert(id::N(3), id::E(1));
        adj.insert(id::N(5), id::E(2));

        assert_eq!(adj.entry_count(), 3);
        assert_eq!(adj.len(), 2);

        let ns: Vec<id::N> = adj.iter().collect();
        assert_eq!(ns, vec![id::N(3), id::N(5)]);

        let edges: Vec<id::E> = adj.edges_to(id::N(3)).collect();
        assert_eq!(edges, vec![id::E(0), id::E(1)]);
    }

    #[test]
    fn t_self_loop() {
        let mut adj = Adjacents::new();
        adj.insert(id::N(2), id::E(0));
        adj.insert(id::N(5), id::E(1));
        adj.insert(id::N(5), id::E(2));

        assert!(adj.contains(id::N(5)));
        assert_eq!(adj.len(), 2);
        assert_eq!(adj.entry_count(), 3);
    }
}
