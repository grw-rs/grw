use std::sync::Arc;

const BITS: u32 = 5;
const FAN: usize = 32;
const MASK: u32 = FAN as u32 - 1;

enum TrieNode<T> {
    Internal([Option<Arc<TrieNode<T>>>; FAN]),
    Leaf([Option<T>; FAN]),
}

pub struct PVec<T> {
    root: Option<Arc<TrieNode<T>>>,
    levels: u32,
    len: u32,
}

impl<T> Clone for PVec<T> {
    fn clone(&self) -> Self {
        PVec { root: self.root.clone(), levels: self.levels, len: self.len }
    }
}

fn capacity(levels: u32) -> u64 {
    (FAN as u64).pow(levels + 1)
}

fn empty_children<T>() -> [Option<Arc<TrieNode<T>>>; FAN] {
    std::array::from_fn(|_| None)
}

fn empty_leaf<T>() -> [Option<T>; FAN] {
    std::array::from_fn(|_| None)
}

fn set_rec<T: Clone>(
    node: Option<&Arc<TrieNode<T>>>,
    slot: u32,
    shift: u32,
    value: T,
) -> (Arc<TrieNode<T>>, bool) {
    if shift == 0 {
        let mut arr = match node {
            Some(n) => match n.as_ref() {
                TrieNode::Leaf(a) => a.clone(),
                TrieNode::Internal(_) => unreachable!("shift 0 must reach a leaf"),
            },
            None => empty_leaf(),
        };
        let idx = (slot & MASK) as usize;
        let was_present = arr[idx].is_some();
        arr[idx] = Some(value);
        (Arc::new(TrieNode::Leaf(arr)), was_present)
    } else {
        let mut children = match node {
            Some(n) => match n.as_ref() {
                TrieNode::Internal(c) => c.clone(),
                TrieNode::Leaf(_) => unreachable!("nonzero shift must be internal"),
            },
            None => empty_children(),
        };
        let idx = ((slot >> shift) & MASK) as usize;
        let (new_child, was_present) = set_rec(children[idx].as_ref(), slot, shift - BITS, value);
        children[idx] = Some(new_child);
        (Arc::new(TrieNode::Internal(children)), was_present)
    }
}

fn remove_rec<T: Clone>(node: &Arc<TrieNode<T>>, slot: u32, shift: u32) -> Arc<TrieNode<T>> {
    match node.as_ref() {
        TrieNode::Leaf(arr) => {
            let mut arr = arr.clone();
            let idx = (slot & MASK) as usize;
            arr[idx] = None;
            Arc::new(TrieNode::Leaf(arr))
        }
        TrieNode::Internal(children) => {
            let mut children = children.clone();
            let idx = ((slot >> shift) & MASK) as usize;
            let child = children[idx].as_ref().expect("path to occupied slot must exist");
            children[idx] = Some(remove_rec(child, slot, shift - BITS));
            Arc::new(TrieNode::Internal(children))
        }
    }
}

fn walk<'a, T>(node: &'a TrieNode<T>, base: u32, shift: u32, out: &mut Vec<(u32, &'a T)>) {
    match node {
        TrieNode::Leaf(arr) => {
            for (i, v) in arr.iter().enumerate() {
                if let Some(v) = v {
                    out.push((base + i as u32, v));
                }
            }
        }
        TrieNode::Internal(children) => {
            for (i, c) in children.iter().enumerate() {
                if let Some(c) = c {
                    walk(c, base + ((i as u32) << shift), shift - BITS, out);
                }
            }
        }
    }
}

impl<T> PVec<T> {
    pub fn new() -> Self {
        PVec { root: None, levels: 0, len: 0 }
    }

    pub fn get(&self, slot: u32) -> Option<&T> {
        if slot as u64 >= capacity(self.levels) {
            return None;
        }
        let mut node = self.root.as_ref()?;
        let mut shift = BITS * self.levels;
        loop {
            match node.as_ref() {
                TrieNode::Leaf(arr) => {
                    let idx = (slot & MASK) as usize;
                    return arr[idx].as_ref();
                }
                TrieNode::Internal(children) => {
                    let idx = ((slot >> shift) & MASK) as usize;
                    node = children[idx].as_ref()?;
                    shift -= BITS;
                }
            }
        }
    }

    pub fn set(&self, slot: u32, value: T) -> PVec<T>
    where
        T: Clone,
    {
        let mut levels = self.levels;
        while slot as u64 >= capacity(levels) {
            levels += 1;
        }
        let mut root = self.root.clone();
        for _ in self.levels..levels {
            let mut children = empty_children();
            children[0] = root.take();
            root = Some(Arc::new(TrieNode::Internal(children)));
        }
        let shift = BITS * levels;
        let (new_root, was_present) = set_rec(root.as_ref(), slot, shift, value);
        PVec {
            root: Some(new_root),
            levels,
            len: if was_present { self.len } else { self.len + 1 },
        }
    }

    pub fn remove(&self, slot: u32) -> PVec<T>
    where
        T: Clone,
    {
        if self.get(slot).is_none() {
            return self.clone();
        }
        let shift = BITS * self.levels;
        let new_root = remove_rec(self.root.as_ref().expect("get(slot) confirmed occupied"), slot, shift);
        PVec { root: Some(new_root), levels: self.levels, len: self.len - 1 }
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Lowest occupied slot. `remove` does not prune emptied subtrees, so the
    /// descent must keep scanning siblings rather than stop at the first child.
    pub fn first(&self) -> Option<u32> {
        fn descend<T>(node: &TrieNode<T>, base: u32, shift: u32) -> Option<u32> {
            match node {
                TrieNode::Leaf(arr) => {
                    arr.iter().position(|v| v.is_some()).map(|i| base + i as u32)
                }
                TrieNode::Internal(children) => children.iter().enumerate().find_map(|(i, c)| {
                    let c = c.as_ref()?;
                    descend(c, base + ((i as u32) << shift), shift - BITS)
                }),
            }
        }
        descend(self.root.as_ref()?, 0, BITS * self.levels)
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> + '_ {
        let mut items = Vec::new();
        if let Some(root) = &self.root {
            walk(root, 0, BITS * self.levels, &mut items);
        }
        items.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn set_get_remove_roundtrip() {
        let v0: PVec<u64> = PVec::new();
        let v1 = v0.set(0, 10).set(31, 20).set(32, 30).set(1024, 40);
        assert_eq!(v1.get(0), Some(&10));
        assert_eq!(v1.get(31), Some(&20));
        assert_eq!(v1.get(32), Some(&30));
        assert_eq!(v1.get(1024), Some(&40));
        assert_eq!(v1.get(7), None);
        assert_eq!(v1.len(), 4);
        let v2 = v1.remove(31);
        assert_eq!(v2.get(31), None);
        assert_eq!(v2.len(), 3);
        assert_eq!(v1.get(31), Some(&20));
    }

    #[test]
    fn iteration_in_slot_order() {
        let v = PVec::new().set(100, 1).set(3, 2).set(4000, 3);
        let got: Vec<u32> = v.iter().map(|(s, _)| s).collect();
        assert_eq!(got, vec![3, 100, 4000]);
    }

    #[test]
    fn old_versions_immutable_under_writes() {
        let mut versions = vec![PVec::new()];
        for i in 0..2000u32 {
            let next = versions.last().unwrap().set(i % 300, i as u64);
            versions.push(next);
        }
        let snapshot: Vec<Option<u64>> = (0..300).map(|s| versions[1000].get(s).copied()).collect();
        let _more = versions.last().unwrap().set(5, 999999);
        let again: Vec<Option<u64>> = (0..300).map(|s| versions[1000].get(s).copied()).collect();
        assert_eq!(snapshot, again);
    }

    #[test]
    fn structural_sharing_ptr_eq() {
        let a = PVec::new().set(0, Arc::new(1u64)).set(1000, Arc::new(2u64));
        let b = a.set(0, Arc::new(3u64));
        assert!(Arc::ptr_eq(a.get(1000).unwrap(), b.get(1000).unwrap()));
    }

    #[test]
    fn set_overwrite_keeps_len() {
        let v0: PVec<u64> = PVec::new().set(5, 100);
        assert_eq!(v0.len(), 1);
        let v1 = v0.set(5, 200);
        assert_eq!(v1.len(), 1);
        assert_eq!(v1.get(5), Some(&200));
        assert_eq!(v0.get(5), Some(&100));
    }

    #[test]
    fn remove_absent_slot_is_noop() {
        let v0: PVec<u64> = PVec::new().set(1, 10).set(2, 20);
        let v1 = v0.remove(999);
        assert_eq!(v1.len(), v0.len());
        let a: Vec<(u32, u64)> = v0.iter().map(|(s, v)| (s, *v)).collect();
        let b: Vec<(u32, u64)> = v1.iter().map(|(s, v)| (s, *v)).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn remove_then_set_same_slot() {
        let v0: PVec<u64> = PVec::new().set(7, 1);
        assert_eq!(v0.len(), 1);
        let v1 = v0.remove(7);
        assert_eq!(v1.len(), 0);
        assert_eq!(v1.get(7), None);
        let v2 = v1.set(7, 2);
        assert_eq!(v2.len(), 1);
        assert_eq!(v2.get(7), Some(&2));
        assert_eq!(v1.get(7), None);
    }

    #[test]
    fn first_is_lowest_occupied_slot() {
        let v0: PVec<u64> = PVec::new();
        assert_eq!(v0.first(), None);
        let v1 = v0.set(4000, 1).set(100, 2).set(3, 3);
        assert_eq!(v1.first(), Some(3));
        let v2 = v1.remove(3);
        assert_eq!(v2.first(), Some(100));
        assert_eq!(v1.first(), Some(3));
        let v3 = v2.remove(100).remove(4000);
        assert_eq!(v3.first(), None);
        assert_eq!(v3.set(31, 9).first(), Some(31));
    }

    #[test]
    fn growth_boundary_stress() {
        let mut v: PVec<u64> = PVec::new();
        for i in 0..65u32 {
            v = v.set(i, i as u64 * 10);
        }
        v = v.set(40000, 999);
        for i in 0..65u32 {
            assert_eq!(v.get(i), Some(&(i as u64 * 10)));
        }
        assert_eq!(v.get(40000), Some(&999));
        let got: Vec<u32> = v.iter().map(|(s, _)| s).collect();
        let mut expected: Vec<u32> = (0..65).collect();
        expected.push(40000);
        assert_eq!(got, expected);
        assert_eq!(v.len(), 66);
    }
}
