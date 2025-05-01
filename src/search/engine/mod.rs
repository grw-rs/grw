pub(crate) mod feature;
pub mod seq;
pub mod par;

use std::sync::Arc;

use crate::graph;
use crate::graph::dsl::LocalId;
use crate::id;
use crate::Id;
use crate::search::Morphism;
use crate::search::query::{Query, BanCluster};

pub(crate) trait Index<NV, ER: graph::Edge> {
    type Neighbors<'a>: ExactSizeIterator<Item = u32> + 'a where Self: 'a;
    type Reverse: ReverseLookup;

    fn degree(&self, n: u32) -> u32;
    fn is_adjacent(&self, n1: u32, n2: u32) -> bool;
    fn has_edge_in_slot(&self, n1: u32, n2: u32, slot: ER::Slot, any_slot: bool) -> bool;
    fn neighbors(&self, n: u32) -> Self::Neighbors<'_>;
    fn node_val(&self, n: u32) -> &NV;

    fn check_edge(
        &self, n1: u32, n2: u32,
        slot: ER::Slot, any_slot: bool, negated: bool,
        pred: Option<&(dyn Fn(&ER::Val) -> bool + Send + Sync)>,
    ) -> bool;

    fn occupied_edge_slots(&self, n1: u32, n2: u32) -> smallvec::SmallVec<[ER::Slot; 3]>;

    fn all_node_ids(&self) -> &[id::N];
    fn node_vals_len(&self) -> usize;
    fn reverse_len(&self) -> usize;
    fn create_reverse(&self) -> Self::Reverse;

    fn compute_search_order(&self, query: &Query<NV, ER>) -> Vec<usize>;
    fn compute_val_filtered(&self, query: &Query<NV, ER>) -> Vec<Option<Vec<id::N>>>;

    fn csr_adj(&self) -> &CsrAdj<NV, ER> { unreachable!() }
}

pub(crate) trait ReverseLookup: Send {
    fn get(&self, n: u32) -> u32;
    fn set(&mut self, n: u32, val: u32);
    fn clear(&mut self, n: u32);
}

impl ReverseLookup for Vec<u32> {
    #[inline(always)]
    fn get(&self, n: u32) -> u32 { self[n as usize] }
    #[inline(always)]
    fn set(&mut self, n: u32, val: u32) { self[n as usize] = val; }
    #[inline(always)]
    fn clear(&mut self, n: u32) { self[n as usize] = UNMAPPED; }
}

impl ReverseLookup for rustc_hash::FxHashMap<u32, u32> {
    #[inline(always)]
    fn get(&self, n: u32) -> u32 {
        match rustc_hash::FxHashMap::get(self, &n) {
            Some(&v) => v,
            None => UNMAPPED,
        }
    }
    #[inline(always)]
    fn set(&mut self, n: u32, val: u32) { self.insert(n, val); }
    #[inline(always)]
    fn clear(&mut self, n: u32) { self.remove(&n); }
}

pub struct Raw;
pub struct Rev;
pub struct RevCsr;
pub struct RevCsrVal;

pub trait Tier<NV, ER: graph::Edge> {
    type Data;
    fn build(graph: &graph::Graph<NV, ER>) -> Self::Data;
}

pub struct Indexed<'g, NV, ER: graph::Edge, D> {
    pub(crate) graph: &'g graph::Graph<NV, ER>,
    pub(crate) data: D,
}


impl<NV: Clone, ER: graph::Edge> Tier<NV, ER> for RevCsr
where
    ER::Val: Clone,
{
    type Data = CsrAdj<NV, ER>;
    fn build(graph: &graph::Graph<NV, ER>) -> Self::Data {
        CsrAdj::build(graph)
    }
}

pub struct RevData {
    pub(crate) reverse_len: usize,
    pub(crate) all_node_ids: Vec<id::N>,
}

impl<NV, ER: graph::Edge> Tier<NV, ER> for Rev {
    type Data = RevData;
    fn build(graph: &graph::Graph<NV, ER>) -> Self::Data {
        let mut all_node_ids: Vec<id::N> = graph.nodes.nodes_iter()
            .map(|(n, _)| n)
            .collect();
        all_node_ids.sort_unstable();
        RevData {
            reverse_len: graph.nodes.store.len(),
            all_node_ids,
        }
    }
}

pub struct RawData {
    pub(crate) all_node_ids: Vec<id::N>,
}

impl<NV, ER: graph::Edge> Tier<NV, ER> for Raw {
    type Data = RawData;
    fn build(graph: &graph::Graph<NV, ER>) -> Self::Data {
        let mut all_node_ids: Vec<id::N> = graph.nodes.nodes_iter()
            .map(|(n, _)| n)
            .collect();
        all_node_ids.sort_unstable();
        RawData { all_node_ids }
    }
}

impl<NV, ER: graph::Edge> graph::Graph<NV, ER> {
    pub fn index<T: Tier<NV, ER>>(&self, _tier: T) -> Indexed<'_, NV, ER, T::Data> {
        Indexed { graph: self, data: T::build(self) }
    }
}

impl<NV, ER: graph::Edge> Index<NV, ER> for Indexed<'_, NV, ER, RevData> {
    type Neighbors<'a> = std::vec::IntoIter<u32> where Self: 'a;
    type Reverse = Vec<u32>;

    #[inline(always)]
    fn degree(&self, n: u32) -> u32 {
        match self.graph.nodes.get_node(id::N(n as Id)) {
            Some(node) => node.adj.len() as u32,
            None => 0,
        }
    }

    #[inline(always)]
    fn is_adjacent(&self, n1: u32, n2: u32) -> bool {
        self.graph.is_adjacent(n1 as Id, n2 as Id)
    }

    #[inline(always)]
    fn has_edge_in_slot(&self, n1: u32, n2: u32, slot: ER::Slot, any_slot: bool) -> bool {
        if any_slot || ER::SLOT_COUNT == 1 {
            return self.is_adjacent(n1, n2);
        }
        let n1_id = id::N(n1 as Id);
        let n2_id = id::N(n2 as Id);
        let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
        self.graph.edges_between(n1_id, n2_id)
            .any(|(s, _)| s == actual_slot)
    }

    fn neighbors(&self, n: u32) -> Self::Neighbors<'_> {
        match self.graph.nodes.get_node(id::N(n as Id)) {
            Some(node) => node.adj.iter().map(|n| *n as u32).collect::<Vec<_>>().into_iter(),
            None => Vec::new().into_iter(),
        }
    }

    #[inline(always)]
    fn node_val(&self, n: u32) -> &NV {
        self.graph.nodes.get(id::N(n as Id)).unwrap()
    }

    fn check_edge(
        &self, n1: u32, n2: u32,
        slot: ER::Slot, any_slot: bool, negated: bool,
        pred: Option<&(dyn Fn(&ER::Val) -> bool + Send + Sync)>,
    ) -> bool {
        let n1_id = id::N(n1 as Id);
        let n2_id = id::N(n2 as Id);
        let satisfied = if any_slot {
            let mut has_any = false;
            let mut pred_match = false;
            for (_, val) in self.graph.edges_between(n1_id, n2_id) {
                has_any = true;
                if let Some(p) = pred {
                    if p(val) { pred_match = true; break; }
                } else {
                    pred_match = true; break;
                }
            }
            if pred.is_some() { pred_match } else { has_any }
        } else {
            let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
            let edge_def = ER::edge(actual_slot, (n1 as Id, n2 as Id));
            match self.graph.get_edge_val(edge_def) {
                Some(val) => match pred {
                    Some(p) => p(val),
                    None => true,
                },
                None => false,
            }
        };
        if negated { !satisfied } else { satisfied }
    }

    fn occupied_edge_slots(&self, n1: u32, n2: u32) -> smallvec::SmallVec<[ER::Slot; 3]> {
        let n1_id = id::N(n1 as Id);
        let n2_id = id::N(n2 as Id);
        self.graph.edges_between(n1_id, n2_id)
            .map(|(slot, _)| slot)
            .collect()
    }

    #[inline(always)]
    fn all_node_ids(&self) -> &[id::N] {
        &self.data.all_node_ids
    }

    #[inline(always)]
    fn node_vals_len(&self) -> usize {
        self.graph.nodes.store.len()
    }

    #[inline(always)]
    fn reverse_len(&self) -> usize {
        self.data.reverse_len
    }

    fn create_reverse(&self) -> Self::Reverse {
        vec![UNMAPPED; self.data.reverse_len]
    }

    fn compute_search_order(&self, query: &Query<NV, ER>) -> Vec<usize> {
        query.search_order.clone()
    }

    fn compute_val_filtered(&self, query: &Query<NV, ER>) -> Vec<Option<Vec<id::N>>> {
        query.node_preds.iter()
            .map(|pred_opt| {
                pred_opt.as_ref().map(|pred| {
                    self.data.all_node_ids.iter().copied()
                        .filter(|&n| pred(self.graph.nodes.get(n).unwrap()))
                        .collect()
                })
            })
            .collect()
    }
}

impl<NV, ER: graph::Edge> Index<NV, ER> for Indexed<'_, NV, ER, RawData> {
    type Neighbors<'a> = std::vec::IntoIter<u32> where Self: 'a;
    type Reverse = rustc_hash::FxHashMap<u32, u32>;

    #[inline(always)]
    fn degree(&self, n: u32) -> u32 {
        match self.graph.nodes.get_node(id::N(n as Id)) {
            Some(node) => node.adj.len() as u32,
            None => 0,
        }
    }

    #[inline(always)]
    fn is_adjacent(&self, n1: u32, n2: u32) -> bool {
        self.graph.is_adjacent(n1 as Id, n2 as Id)
    }

    #[inline(always)]
    fn has_edge_in_slot(&self, n1: u32, n2: u32, slot: ER::Slot, any_slot: bool) -> bool {
        if any_slot || ER::SLOT_COUNT == 1 {
            return self.is_adjacent(n1, n2);
        }
        let n1_id = id::N(n1 as Id);
        let n2_id = id::N(n2 as Id);
        let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
        self.graph.edges_between(n1_id, n2_id)
            .any(|(s, _)| s == actual_slot)
    }

    fn neighbors(&self, n: u32) -> Self::Neighbors<'_> {
        match self.graph.nodes.get_node(id::N(n as Id)) {
            Some(node) => node.adj.iter().map(|n| *n as u32).collect::<Vec<_>>().into_iter(),
            None => Vec::new().into_iter(),
        }
    }

    #[inline(always)]
    fn node_val(&self, n: u32) -> &NV {
        self.graph.nodes.get(id::N(n as Id)).unwrap()
    }

    fn check_edge(
        &self, n1: u32, n2: u32,
        slot: ER::Slot, any_slot: bool, negated: bool,
        pred: Option<&(dyn Fn(&ER::Val) -> bool + Send + Sync)>,
    ) -> bool {
        let n1_id = id::N(n1 as Id);
        let n2_id = id::N(n2 as Id);
        let satisfied = if any_slot {
            let mut has_any = false;
            let mut pred_match = false;
            for (_, val) in self.graph.edges_between(n1_id, n2_id) {
                has_any = true;
                if let Some(p) = pred {
                    if p(val) { pred_match = true; break; }
                } else {
                    pred_match = true; break;
                }
            }
            if pred.is_some() { pred_match } else { has_any }
        } else {
            let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
            let edge_def = ER::edge(actual_slot, (n1 as Id, n2 as Id));
            match self.graph.get_edge_val(edge_def) {
                Some(val) => match pred {
                    Some(p) => p(val),
                    None => true,
                },
                None => false,
            }
        };
        if negated { !satisfied } else { satisfied }
    }

    fn occupied_edge_slots(&self, n1: u32, n2: u32) -> smallvec::SmallVec<[ER::Slot; 3]> {
        let n1_id = id::N(n1 as Id);
        let n2_id = id::N(n2 as Id);
        self.graph.edges_between(n1_id, n2_id)
            .map(|(slot, _)| slot)
            .collect()
    }

    #[inline(always)]
    fn all_node_ids(&self) -> &[id::N] {
        &self.data.all_node_ids
    }

    #[inline(always)]
    fn node_vals_len(&self) -> usize {
        self.graph.nodes.store.len()
    }

    #[inline(always)]
    fn reverse_len(&self) -> usize {
        self.graph.nodes.store.len()
    }

    fn create_reverse(&self) -> Self::Reverse {
        rustc_hash::FxHashMap::with_capacity_and_hasher(0, rustc_hash::FxBuildHasher)
    }

    fn compute_search_order(&self, query: &Query<NV, ER>) -> Vec<usize> {
        query.search_order.clone()
    }

    fn compute_val_filtered(&self, query: &Query<NV, ER>) -> Vec<Option<Vec<id::N>>> {
        query.node_preds.iter()
            .map(|pred_opt| {
                pred_opt.as_ref().map(|pred| {
                    self.data.all_node_ids.iter().copied()
                        .filter(|&n| pred(self.graph.nodes.get(n).unwrap()))
                        .collect()
                })
            })
            .collect()
    }
}

impl<NV, ER: graph::Edge> Index<NV, ER> for Indexed<'_, NV, ER, CsrAdj<NV, ER>> {
    type Neighbors<'a> = std::iter::Copied<std::slice::Iter<'a, u32>> where Self: 'a;
    type Reverse = Vec<u32>;

    #[inline(always)]
    fn degree(&self, n: u32) -> u32 {
        self.data.degree(n)
    }

    #[inline(always)]
    fn is_adjacent(&self, n1: u32, n2: u32) -> bool {
        self.data.is_adjacent(n1, n2)
    }

    #[inline(always)]
    fn has_edge_in_slot(&self, n1: u32, n2: u32, slot: ER::Slot, any_slot: bool) -> bool {
        if any_slot || ER::SLOT_COUNT == 1 {
            return self.data.is_adjacent(n1, n2);
        }
        let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
        match self.data.edge_store_at(n1, n2) {
            Some(store) => ER::csr_store_val(store, actual_slot).is_some(),
            None => false,
        }
    }

    #[inline(always)]
    fn neighbors(&self, n: u32) -> Self::Neighbors<'_> {
        self.data.neighbors(n).iter().copied()
    }

    #[inline(always)]
    fn node_val(&self, n: u32) -> &NV {
        self.data.node_val(n)
    }

    #[inline(always)]
    fn check_edge(
        &self, n1: u32, n2: u32,
        slot: ER::Slot, any_slot: bool, negated: bool,
        pred: Option<&(dyn Fn(&ER::Val) -> bool + Send + Sync)>,
    ) -> bool {
        let store = self.data.edge_store_at(n1, n2);
        let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
        edge_check::<ER>(store, any_slot, negated, actual_slot, pred)
    }

    #[inline(always)]
    fn occupied_edge_slots(&self, n1: u32, n2: u32) -> smallvec::SmallVec<[ER::Slot; 3]> {
        match self.data.edge_store_at(n1, n2) {
            Some(store) => ER::csr_occupied_slots(store),
            None => smallvec::SmallVec::new(),
        }
    }

    #[inline(always)]
    fn all_node_ids(&self) -> &[id::N] {
        self.data.all_node_ids()
    }

    #[inline(always)]
    fn node_vals_len(&self) -> usize {
        self.data.node_vals.len()
    }

    #[inline(always)]
    fn reverse_len(&self) -> usize {
        if self.data.offsets.len() > 1 { self.data.offsets.len() - 1 } else { 0 }
    }

    fn create_reverse(&self) -> Self::Reverse {
        vec![UNMAPPED; self.reverse_len()]
    }

    fn compute_search_order(&self, query: &Query<NV, ER>) -> Vec<usize> {
        compute_search_order(query, &self.data)
    }

    fn compute_val_filtered(&self, query: &Query<NV, ER>) -> Vec<Option<Vec<id::N>>> {
        query.node_preds.iter()
            .map(|pred_opt| {
                pred_opt.as_ref().map(|pred| {
                    self.data.all_node_ids().iter().copied()
                        .filter(|&n| pred(self.data.node_val(*n)))
                        .collect()
                })
            })
            .collect()
    }

    fn csr_adj(&self) -> &CsrAdj<NV, ER> { &self.data }
}

pub struct RevCsrValData<NV, ER: graph::Edge> {
    pub(crate) csr: CsrAdj<NV, ER>,
    pub(crate) value_groups: rustc_hash::FxHashMap<NV, Vec<id::N>>,
}

impl<NV: Clone + Eq + std::hash::Hash, ER: graph::Edge> Tier<NV, ER> for RevCsrVal
where
    ER::Val: Clone,
{
    type Data = RevCsrValData<NV, ER>;
    fn build(graph: &graph::Graph<NV, ER>) -> Self::Data {
        let csr = CsrAdj::build(graph);
        let mut value_groups: rustc_hash::FxHashMap<NV, Vec<id::N>> =
            rustc_hash::FxHashMap::with_capacity_and_hasher(0, rustc_hash::FxBuildHasher);
        for &n in csr.all_node_ids() {
            let val = csr.node_val(*n).clone();
            value_groups.entry(val).or_insert_with(Vec::new).push(n);
        }
        RevCsrValData { csr, value_groups }
    }
}

impl<NV: Clone + Eq + std::hash::Hash, ER: graph::Edge> Index<NV, ER> for Indexed<'_, NV, ER, RevCsrValData<NV, ER>>
where
    ER::Val: Clone,
{
    type Neighbors<'a> = std::iter::Copied<std::slice::Iter<'a, u32>> where Self: 'a;
    type Reverse = Vec<u32>;

    #[inline(always)]
    fn degree(&self, n: u32) -> u32 {
        self.data.csr.degree(n)
    }

    #[inline(always)]
    fn is_adjacent(&self, n1: u32, n2: u32) -> bool {
        self.data.csr.is_adjacent(n1, n2)
    }

    #[inline(always)]
    fn has_edge_in_slot(&self, n1: u32, n2: u32, slot: ER::Slot, any_slot: bool) -> bool {
        if any_slot || ER::SLOT_COUNT == 1 {
            return self.data.csr.is_adjacent(n1, n2);
        }
        let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
        match self.data.csr.edge_store_at(n1, n2) {
            Some(store) => ER::csr_store_val(store, actual_slot).is_some(),
            None => false,
        }
    }

    #[inline(always)]
    fn neighbors(&self, n: u32) -> Self::Neighbors<'_> {
        self.data.csr.neighbors(n).iter().copied()
    }

    #[inline(always)]
    fn node_val(&self, n: u32) -> &NV {
        self.data.csr.node_val(n)
    }

    #[inline(always)]
    fn check_edge(
        &self, n1: u32, n2: u32,
        slot: ER::Slot, any_slot: bool, negated: bool,
        pred: Option<&(dyn Fn(&ER::Val) -> bool + Send + Sync)>,
    ) -> bool {
        let store = self.data.csr.edge_store_at(n1, n2);
        let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
        edge_check::<ER>(store, any_slot, negated, actual_slot, pred)
    }

    #[inline(always)]
    fn occupied_edge_slots(&self, n1: u32, n2: u32) -> smallvec::SmallVec<[ER::Slot; 3]> {
        match self.data.csr.edge_store_at(n1, n2) {
            Some(store) => ER::csr_occupied_slots(store),
            None => smallvec::SmallVec::new(),
        }
    }

    #[inline(always)]
    fn all_node_ids(&self) -> &[id::N] {
        self.data.csr.all_node_ids()
    }

    #[inline(always)]
    fn node_vals_len(&self) -> usize {
        self.data.csr.node_vals.len()
    }

    #[inline(always)]
    fn reverse_len(&self) -> usize {
        if self.data.csr.offsets.len() > 1 { self.data.csr.offsets.len() - 1 } else { 0 }
    }

    fn create_reverse(&self) -> Self::Reverse {
        vec![UNMAPPED; self.reverse_len()]
    }

    fn compute_search_order(&self, query: &Query<NV, ER>) -> Vec<usize> {
        let node_count = query.nodes.len();
        let has_preds = query.node_preds.iter().any(|p| p.is_some());
        if !has_preds {
            return query.search_order.clone();
        }

        let natural: Vec<usize> = (0..node_count)
            .filter(|&i| !query.nodes[i].negated && !query.nodes[i].ban_only)
            .collect();
        if natural.is_empty() {
            return Vec::new();
        }

        let mut cand_counts = vec![0usize; node_count];
        for &ni in &natural {
            let pattern_degree = query.pattern_degrees[ni];
            let morphism = query.node_morphism[ni];
            let pred = &query.node_preds[ni];
            let mut count = 0usize;
            if let Some(p) = pred {
                for (val, group) in &self.data.value_groups {
                    if !p(val) { continue; }
                    for &n in group {
                        let target_degree = self.data.csr.degree(*n) as usize;
                        let degree_ok = match morphism {
                            Morphism::Iso => target_degree == pattern_degree,
                            Morphism::SubIso | Morphism::EpiMono | Morphism::Mono => target_degree >= pattern_degree,
                            Morphism::Epi | Morphism::Homo => true,
                        };
                        if degree_ok { count += 1; }
                    }
                }
            } else {
                for &n in self.data.csr.all_node_ids() {
                    let target_degree = self.data.csr.degree(*n) as usize;
                    let degree_ok = match morphism {
                        Morphism::Iso => target_degree == pattern_degree,
                        Morphism::SubIso | Morphism::EpiMono | Morphism::Mono => target_degree >= pattern_degree,
                        Morphism::Epi | Morphism::Homo => true,
                    };
                    if degree_ok { count += 1; }
                }
            }
            cand_counts[ni] = count;
        }

        let mut order = Vec::with_capacity(natural.len());
        let mut in_order = vec![false; node_count];
        let first = *natural.iter()
            .min_by_key(|&&ni| cand_counts[ni])
            .unwrap();
        order.push(first);
        in_order[first] = true;

        while order.len() < natural.len() {
            let mut best = None;
            let mut best_conn = 0usize;
            let mut best_cands = usize::MAX;
            for &ni in &natural {
                if in_order[ni] { continue; }
                let conn = query.adj[ni].iter().filter(|&&(neighbor, _, neg, ei)| {
                    !neg && !query.edges[ei].ban_only && in_order[neighbor]
                }).count();
                let cands = cand_counts[ni];
                if conn > best_conn
                    || (conn == best_conn && cands < best_cands)
                    || (best.is_none() && conn == 0)
                {
                    best_conn = conn;
                    best_cands = cands;
                    best = Some(ni);
                }
            }
            if let Some(ni) = best {
                order.push(ni);
                in_order[ni] = true;
            }
        }

        order
    }

    fn compute_val_filtered(&self, query: &Query<NV, ER>) -> Vec<Option<Vec<id::N>>> {
        query.node_preds.iter()
            .map(|pred_opt| {
                pred_opt.as_ref().map(|pred| {
                    let mut result = Vec::new();
                    for (val, group) in &self.data.value_groups {
                        if pred(val) {
                            result.extend_from_slice(group);
                        }
                    }
                    result.sort_unstable();
                    result
                })
            })
            .collect()
    }

    fn csr_adj(&self) -> &CsrAdj<NV, ER> { &self.data.csr }
}

pub use seq::Seq;
pub use par::Par;

pub struct MatchedEdge<S> {
    pub src: id::N,
    pub tgt: id::N,
    pub slot: S,
    pub any_slot: bool,
}

struct MatchCtx<'a, NV, ER: graph::Edge> {
    query: &'a super::query::Query<NV, ER>,
    csr: &'a CsrAdj<NV, ER>,
}

#[derive(Debug)]
pub struct Match(Vec<(LocalId, id::N)>);

impl Match {
    pub fn get(&self, pattern_local: impl Into<LocalId>) -> Option<id::N> {
        let lid = pattern_local.into();
        self.0.binary_search_by_key(&lid.0, |(l, _)| l.0)
            .ok()
            .map(|i| self.0[i].1)
    }

    pub fn iter(&self) -> impl Iterator<Item = &(LocalId, id::N)> {
        self.0.iter()
    }

    pub fn values(&self) -> impl Iterator<Item = id::N> + '_ {
        self.0.iter().map(|&(_, n)| n)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn as_slice(&self) -> &[(LocalId, id::N)] {
        &self.0
    }

    pub fn edges<'a, NV, ER: graph::Edge>(
        &'a self,
        query: &'a super::query::Query<NV, ER>,
        csr: &'a CsrAdj<NV, ER>,
    ) -> impl Iterator<Item = MatchedEdge<ER::Slot>> + 'a {
        let specific = query.edges.iter().enumerate().filter_map(move |(_, pe)| {
            if pe.negated || pe.ban_only || pe.any_slot { return None }
            let src_lid = query.nodes[pe.source].local_id;
            let tgt_lid = query.nodes[pe.target].local_id;
            let src_nid = self.get(src_lid)?;
            let tgt_nid = self.get(tgt_lid)?;
            Some(MatchedEdge { src: src_nid, tgt: tgt_nid, slot: pe.slot, any_slot: false })
        });
        let any = query.edges.iter().enumerate().filter_map(move |(ei, pe)| {
            if !pe.any_slot || pe.negated || pe.ban_only { return None }
            let src_nid = self.get(query.nodes[pe.source].local_id)?;
            let tgt_nid = self.get(query.nodes[pe.target].local_id)?;
            Some((ei, src_nid, tgt_nid))
        }).flat_map(move |(ei, src_nid, tgt_nid)| {
            let n1 = std::cmp::min(*src_nid, *tgt_nid);
            let n2 = std::cmp::max(*src_nid, *tgt_nid);
            let edge_pred = &query.edge_preds[ei];
            let store = csr.edge_store_at(n1, n2);
            let slots: smallvec::SmallVec<[ER::Slot; 3]> = match store {
                Some(s) => {
                    let mut occupied = ER::csr_occupied_slots(s);
                    if let Some(pred) = edge_pred {
                        occupied.retain(|slot| {
                            ER::csr_store_val(s, *slot)
                                .map(|v| pred(v))
                                .unwrap_or(false)
                        });
                    }
                    occupied
                }
                None => smallvec::SmallVec::new(),
            };
            slots.into_iter().map(move |slot| MatchedEdge { src: src_nid, tgt: tgt_nid, slot, any_slot: true })
        });
        specific.chain(any)
    }

    pub fn translate<'a, NV, ER: graph::Edge>(
        &'a self,
        query: &'a super::query::Query<NV, ER>,
        graph: &'a graph::Graph<NV, ER>,
    ) -> TranslatedMatch<'a, NV, ER> {
        TranslatedMatch { m: self, query, graph }
    }
}

pub struct TranslatedMatch<'a, NV, ER: graph::Edge> {
    m: &'a Match,
    query: &'a super::query::Query<NV, ER>,
    graph: &'a graph::Graph<NV, ER>,
}

impl<'a, NV, ER: graph::Edge> TranslatedMatch<'a, NV, ER> {
    pub fn node(&self, pattern_lid: impl Into<LocalId>) -> Option<(id::N, &'a NV)> {
        let nid = self.m.get(pattern_lid)?;
        let nv = self.graph.get(nid)?;
        Some((nid, nv))
    }

    pub fn nodes(&self) -> impl Iterator<Item = (LocalId, id::N, &'a NV)> + '_ {
        self.m.iter().filter_map(move |&(lid, nid)| {
            let nv = self.graph.get(nid)?;
            Some((lid, nid, nv))
        })
    }

    pub fn pattern_edges(&self) -> impl Iterator<Item = MatchedEdge<ER::Slot>> + '_ {
        self.query.edges.iter().filter_map(move |pe| {
            if pe.negated || pe.ban_only { return None }
            let src_lid = self.query.nodes[pe.source].local_id;
            let tgt_lid = self.query.nodes[pe.target].local_id;
            let src_nid = self.m.get(src_lid)?;
            let tgt_nid = self.m.get(tgt_lid)?;
            Some(MatchedEdge { src: src_nid, tgt: tgt_nid, slot: pe.slot, any_slot: pe.any_slot })
        })
    }

    pub fn adjacent_ids(&self, pattern_lid: impl Into<LocalId>) -> impl Iterator<Item = (LocalId, id::N)> + '_ {
        let lid = pattern_lid.into();
        let src_idx = self.query.nodes.iter().position(|n| n.local_id == lid);
        self.query.edges.iter().filter_map(move |pe| {
            if pe.negated || pe.ban_only { return None }
            let src_idx = src_idx?;
            let other_idx = if pe.source == src_idx {
                pe.target
            } else if pe.target == src_idx {
                pe.source
            } else {
                return None;
            };
            let other_lid = self.query.nodes[other_idx].local_id;
            let other_nid = self.m.get(other_lid)?;
            Some((other_lid, other_nid))
        })
    }

    pub fn match_ref(&self) -> &Match {
        self.m
    }
}

impl<'a, NV, ER: graph::Edge + graph::HasRel> TranslatedMatch<'a, NV, ER> {
    pub fn edges(&self) -> Vec<MatchedEdge<ER::Slot>> {
        let mut result = Vec::new();
        let mut claimed: std::collections::HashMap<(u32, u32), smallvec::SmallVec<[ER::Slot; 3]>> =
            std::collections::HashMap::new();

        for pe in &self.query.edges {
            if pe.negated || pe.ban_only || pe.any_slot { continue }
            let src_nid = match self.m.get(self.query.nodes[pe.source].local_id) { Some(n) => n, None => continue };
            let tgt_nid = match self.m.get(self.query.nodes[pe.target].local_id) { Some(n) => n, None => continue };
            let key = (std::cmp::min(*src_nid, *tgt_nid), std::cmp::max(*src_nid, *tgt_nid));
            claimed.entry(key).or_default().push(pe.slot);
            result.push(MatchedEdge { src: src_nid, tgt: tgt_nid, slot: pe.slot, any_slot: false });
        }

        for (ei, pe) in self.query.edges.iter().enumerate() {
            if !pe.any_slot || pe.negated || pe.ban_only { continue }
            let src_nid = match self.m.get(self.query.nodes[pe.source].local_id) { Some(n) => n, None => continue };
            let tgt_nid = match self.m.get(self.query.nodes[pe.target].local_id) { Some(n) => n, None => continue };
            let key = (std::cmp::min(*src_nid, *tgt_nid), std::cmp::max(*src_nid, *tgt_nid));
            let already_claimed: smallvec::SmallVec<[ER::Slot; 3]> = claimed.get(&key)
                .cloned()
                .unwrap_or_default();
            let edge_pred = &self.query.edge_preds[ei];

            let ns = crate::NR::from([id::N(*src_nid), id::N(*tgt_nid)]);
            for (def, val) in self.graph.rel(ns) {
                let (_, slot): (crate::NR<id::N>, ER::Slot) = def.into();
                if already_claimed.contains(&slot) { continue }
                if let Some(pred) = edge_pred {
                    if !pred(val) { continue }
                }
                claimed.entry(key).or_default().push(slot);
                result.push(MatchedEdge { src: src_nid, tgt: tgt_nid, slot, any_slot: true });
            }
        }

        result
    }

    pub fn adjacents(&self, pattern_lid: impl Into<LocalId>) -> impl Iterator<Item = (LocalId, id::N, &'a NV, ER::Slot)> + '_ {
        let lid = pattern_lid.into();
        let src_idx = self.query.nodes.iter().position(|n| n.local_id == lid);
        self.query.edges.iter().filter_map(move |pe| {
            if pe.negated || pe.ban_only { return None }
            let src_idx = src_idx?;
            let other_idx = if pe.source == src_idx {
                pe.target
            } else if pe.target == src_idx {
                pe.source
            } else {
                return None;
            };
            let other_lid = self.query.nodes[other_idx].local_id;
            let other_nid = self.m.get(other_lid)?;
            let other_nv = self.graph.get(other_nid)?;
            Some((other_lid, other_nid, other_nv, pe.slot))
        })
    }
}

impl std::ops::Index<LocalId> for Match {
    type Output = id::N;
    fn index(&self, lid: LocalId) -> &id::N {
        let i = self.0.binary_search_by_key(&lid.0, |(l, _)| l.0)
            .expect("pattern node not in match");
        &self.0[i].1
    }
}

impl std::ops::Index<Id> for Match {
    type Output = id::N;
    fn index(&self, id: Id) -> &id::N {
        &self[LocalId(id)]
    }
}

pub(crate) struct StackFrame {
    pub(crate) depth: usize,
    pub(crate) candidates: Vec<id::N>,
    pub(crate) candidate_idx: usize,
}

pub struct CsrAdj<NV, ER: graph::Edge> {
    offsets: Vec<u32>,
    neighbors: Vec<u32>,
    pub(crate) node_vals: Vec<NV>,
    pub(crate) edge_stores: Vec<ER::CsrStore>,
    all_node_ids: Vec<id::N>,
}

pub(crate) const UNMAPPED: u32 = u32::MAX;

#[inline(always)]
pub(crate) fn edge_check<ER: graph::Edge>(
    store: Option<&ER::CsrStore>,
    any_slot: bool,
    negated: bool,
    actual_slot: ER::Slot,
    pred: Option<&(dyn Fn(&ER::Val) -> bool + Send + Sync)>,
) -> bool {
    let satisfied = if any_slot {
        match store {
            None => false,
            Some(s) => pred.map_or(true, |p| ER::csr_any_match(s, p)),
        }
    } else if ER::SLOT_COUNT == 1 && pred.is_none() {
        store.is_some()
    } else {
        match store.and_then(|s| ER::csr_store_val(s, actual_slot)) {
            None => false,
            Some(val) => pred.map_or(true, |p| p(val)),
        }
    };
    if negated { !satisfied } else { satisfied }
}

impl<NV: Clone, ER: graph::Edge> CsrAdj<NV, ER>
where
    ER::Val: Clone,
{
    fn build(target: &graph::Graph<NV, ER>) -> Self {
        let max_id = target.nodes.store.len();
        let mut offsets = vec![0u32; max_id + 2];
        for (node_id, node) in target.nodes.nodes_iter() {
            offsets[*node_id as usize + 1] = node.adj.len() as u32;
        }
        for i in 1..offsets.len() {
            offsets[i] += offsets[i - 1];
        }
        let total = *offsets.last().unwrap_or(&0) as usize;
        let mut neighbors = vec![0u32; total];
        let mut edge_stores: Vec<std::mem::MaybeUninit<ER::CsrStore>> =
            (0..total).map(|_| std::mem::MaybeUninit::uninit()).collect();
        let mut node_vals: Vec<std::mem::MaybeUninit<NV>> =
            (0..max_id + 1).map(|_| std::mem::MaybeUninit::uninit()).collect();

        for (node_id, node) in target.nodes.nodes_iter() {
            let nid = *node_id as usize;
            let start = offsets[nid] as usize;
            node_vals[nid] = std::mem::MaybeUninit::new(node.val.clone());

            let mut pairs: Vec<(u32, ER::CsrStore)> = Vec::with_capacity(node.adj.len() as usize);
            for adj in node.adj.iter() {
                let edges_iter = target.edges_between(node_id, adj)
                    .map(|(slot, val)| (slot, val.clone()));
                let store = ER::build_csr_store(edges_iter);
                pairs.push((*adj as u32, store));
            }
            pairs.sort_unstable_by_key(|(n, _)| *n);

            for (i, (adj_raw, store)) in pairs.into_iter().enumerate() {
                neighbors[start + i] = adj_raw;
                edge_stores[start + i] = std::mem::MaybeUninit::new(store);
            }
        }

        let node_vals = unsafe {
            std::mem::transmute::<Vec<std::mem::MaybeUninit<NV>>, Vec<NV>>(node_vals)
        };
        let edge_stores = unsafe {
            std::mem::transmute::<Vec<std::mem::MaybeUninit<ER::CsrStore>>, Vec<ER::CsrStore>>(edge_stores)
        };

        let mut all_node_ids: Vec<id::N> = target.nodes.nodes_iter()
            .map(|(n, _)| n)
            .collect();
        all_node_ids.sort_unstable();

        CsrAdj { offsets, neighbors, node_vals, edge_stores, all_node_ids }
    }
}

impl<NV, ER: graph::Edge> CsrAdj<NV, ER> {
    #[inline(always)]
    pub(crate) fn degree(&self, n: u32) -> u32 {
        self.offsets[n as usize + 1] - self.offsets[n as usize]
    }

    #[inline(always)]
    pub(crate) fn is_adjacent(&self, n1: u32, n2: u32) -> bool {
        let start = self.offsets[n1 as usize] as usize;
        let end = self.offsets[n1 as usize + 1] as usize;
        self.neighbors[start..end].binary_search(&n2).is_ok()
    }

    #[inline(always)]
    pub(crate) fn neighbors(&self, n: u32) -> &[u32] {
        let start = self.offsets[n as usize] as usize;
        let end = self.offsets[n as usize + 1] as usize;
        &self.neighbors[start..end]
    }

    #[inline(always)]
    pub(crate) fn node_val(&self, n: u32) -> &NV {
        &self.node_vals[n as usize]
    }

    #[inline(always)]
    pub(crate) fn all_node_ids(&self) -> &[id::N] {
        &self.all_node_ids
    }

    #[inline(always)]
    pub(crate) fn edge_store_at(&self, n1: u32, n2: u32) -> Option<&ER::CsrStore> {
        let start = self.offsets[n1 as usize] as usize;
        let end = self.offsets[n1 as usize + 1] as usize;
        match self.neighbors[start..end].binary_search(&n2) {
            Ok(pos) => Some(&self.edge_stores[start + pos]),
            Err(_) => None,
        }
    }
}

pub type Graph<'g, NV, ER> = Indexed<'g, NV, ER, CsrAdj<NV, ER>>;

impl<NV, ER: graph::Edge> Index<NV, ER> for CsrAdj<NV, ER> {
    type Neighbors<'a> = std::iter::Copied<std::slice::Iter<'a, u32>> where Self: 'a;
    type Reverse = Vec<u32>;

    #[inline(always)]
    fn degree(&self, n: u32) -> u32 {
        self.degree(n)
    }

    #[inline(always)]
    fn is_adjacent(&self, n1: u32, n2: u32) -> bool {
        self.is_adjacent(n1, n2)
    }

    #[inline(always)]
    fn has_edge_in_slot(&self, n1: u32, n2: u32, slot: ER::Slot, any_slot: bool) -> bool {
        if any_slot || ER::SLOT_COUNT == 1 {
            return self.is_adjacent(n1, n2);
        }
        let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
        match CsrAdj::edge_store_at(self, n1, n2) {
            Some(store) => ER::csr_store_val(store, actual_slot).is_some(),
            None => false,
        }
    }

    #[inline(always)]
    fn neighbors(&self, n: u32) -> Self::Neighbors<'_> {
        CsrAdj::neighbors(self, n).iter().copied()
    }

    #[inline(always)]
    fn node_val(&self, n: u32) -> &NV {
        CsrAdj::node_val(self, n)
    }

    #[inline(always)]
    fn check_edge(
        &self, n1: u32, n2: u32,
        slot: ER::Slot, any_slot: bool, negated: bool,
        pred: Option<&(dyn Fn(&ER::Val) -> bool + Send + Sync)>,
    ) -> bool {
        let store = CsrAdj::edge_store_at(self, n1, n2);
        let actual_slot = if n1 <= n2 { slot } else { ER::reverse_slot(slot) };
        edge_check::<ER>(store, any_slot, negated, actual_slot, pred)
    }

    #[inline(always)]
    fn occupied_edge_slots(&self, n1: u32, n2: u32) -> smallvec::SmallVec<[ER::Slot; 3]> {
        match CsrAdj::edge_store_at(self, n1, n2) {
            Some(store) => ER::csr_occupied_slots(store),
            None => smallvec::SmallVec::new(),
        }
    }

    #[inline(always)]
    fn all_node_ids(&self) -> &[id::N] {
        CsrAdj::all_node_ids(self)
    }

    #[inline(always)]
    fn node_vals_len(&self) -> usize {
        self.node_vals.len()
    }

    #[inline(always)]
    fn reverse_len(&self) -> usize {
        if self.offsets.len() > 1 { self.offsets.len() - 1 } else { 0 }
    }

    fn create_reverse(&self) -> Self::Reverse {
        vec![UNMAPPED; self.reverse_len()]
    }

    fn compute_search_order(&self, query: &Query<NV, ER>) -> Vec<usize> {
        compute_search_order(query, self)
    }

    fn compute_val_filtered(&self, query: &Query<NV, ER>) -> Vec<Option<Vec<id::N>>> {
        query.node_preds.iter()
            .map(|pred_opt| {
                pred_opt.as_ref().map(|pred| {
                    self.all_node_ids.iter().copied()
                        .filter(|&n| pred(CsrAdj::node_val(self, *n)))
                        .collect()
                })
            })
            .collect()
    }

    fn csr_adj(&self) -> &CsrAdj<NV, ER> { self }
}

pub(crate) struct Ctx<'a, NV, ER: graph::Edge, I> {
    pub(crate) query: &'a Query<NV, ER>,
    pub(crate) target: &'a graph::Graph<NV, ER>,
    pub(crate) index: &'a I,
}

pub struct Session<'g, NV, ER: graph::Edge> {
    query: Query<NV, ER>,
    indexed: Graph<'g, NV, ER>,
    bindings: Vec<Option<id::N>>,
}

impl<'g, NV: Clone + 'g, ER: graph::Edge + 'g> Session<'g, NV, ER>
where
    ER::Val: Clone,
{
    pub fn from_search(
        search: super::query::Search<NV, ER>,
        graph: &'g graph::Graph<NV, ER>,
    ) -> Result<Self, super::error::Search> {
        match search {
            super::query::Search::Resolved(r) => {
                for binding in &r.bindings {
                    if let Some(pinned) = binding {
                        if !graph.has(*pinned) {
                            return Err(super::error::Search::TargetMissing(**pinned));
                        }
                    }
                }
                let bindings = r.bindings;
                let indexed = graph.index(RevCsr);
                Ok(Session { query: r.query, indexed, bindings })
            }
            super::query::Search::Unresolved(_) => Err(super::error::Search::BoundPatternInSession),
        }
    }
}

impl<'g, NV: Clone + 'g, ER: graph::Edge + 'g> Session<'g, NV, ER>
where
    ER::Val: Clone,
{
    pub fn iter(&self) -> seq::Iter<'_, NV, ER> {
        Seq::search_bound(&self.query, &self.indexed, self.bindings.clone())
    }

    pub fn par_iter(&self) -> par::ParIter<'_, NV, ER> {
        par::ParIter::new(&self.query, &self.indexed)
    }

    pub fn query(&self) -> &Query<NV, ER> {
        &self.query
    }

    pub fn indexed(&self) -> &Graph<'g, NV, ER> {
        &self.indexed
    }

    pub fn matched_edges<'a>(&'a self, m: &'a Match) -> impl Iterator<Item = MatchedEdge<ER::Slot>> + 'a {
        m.edges(&self.query, self.indexed.csr_adj())
    }

    pub fn translate<'a>(&'a self, m: &'a Match) -> TranslatedMatch<'a, NV, ER> {
        m.translate(&self.query, self.indexed.graph)
    }

    pub fn graph(&self) -> &graph::Graph<NV, ER> {
        self.indexed.graph
    }
}

impl<'a, 'g: 'a, NV: Clone + 'a, ER: graph::Edge + 'a> IntoIterator for &'a Session<'g, NV, ER>
where
    ER::Val: Clone,
{
    type Item = Match;
    type IntoIter = seq::Iter<'a, NV, ER>;

    fn into_iter(self) -> seq::Iter<'a, NV, ER> {
        Seq::search_bound(&self.query, &self.indexed, self.bindings.clone())
    }
}

impl<'g, NV: Clone + 'g, ER: graph::Edge + 'g> IntoIterator for Session<'g, NV, ER>
where
    ER::Val: Clone,
{
    type Item = Match;
    type IntoIter = seq::IntoIter<'g, NV, ER>;

    fn into_iter(self) -> seq::IntoIter<'g, NV, ER> {
        seq::IntoIter::from_session(self)
    }
}

pub(crate) struct Shared {
    pub(crate) search_order: Vec<usize>,
    pub(crate) val_filtered: Arc<Vec<Option<Vec<id::N>>>>,
    pub(crate) exhausted: bool,
}

impl Shared {
    pub(crate) fn precompute<NV, ER: graph::Edge, I: Index<NV, ER>>(
        query: &Query<NV, ER>,
        target: &graph::Graph<NV, ER>,
        index: &I,
    ) -> Self {
        let search_order = index.compute_search_order(query);

        let positive_count = search_order.len();
        let all_iso = search_order.iter()
            .all(|&i| query.node_morphism[i] == Morphism::Iso);
        let any_injective = search_order.iter()
            .any(|&i| query.node_morphism[i].is_injective());

        let exhausted = if all_iso {
            target.nodes.len() != positive_count
        } else if any_injective {
            target.nodes.len() < positive_count
        } else {
            false
        };

        let val_filtered = index.compute_val_filtered(query);

        Shared { search_order, val_filtered: Arc::new(val_filtered), exhausted }
    }
}

pub(crate) struct State<R: ReverseLookup> {
    pub(crate) bindings: Vec<Option<id::N>>,
    pub(crate) mapping: Vec<u32>,
    pub(crate) reverse: R,
    pub(crate) search_order: Vec<usize>,
    pub(crate) stack: Vec<StackFrame>,
    pub(crate) exhausted: bool,
    pub(crate) candidate_pool: Vec<Vec<id::N>>,
    pub(crate) forward_verified_depths: u64,
    pub(crate) val_filtered: Arc<Vec<Option<Vec<id::N>>>>,
}

fn compute_search_order<NV, ER: graph::Edge>(
    query: &Query<NV, ER>,
    csr: &CsrAdj<NV, ER>,
) -> Vec<usize> {
    let node_count = query.nodes.len();
    let has_preds = query.node_preds.iter().any(|p| p.is_some());
    if !has_preds {
        return query.search_order.clone();
    }

    let natural: Vec<usize> = (0..node_count)
        .filter(|&i| !query.nodes[i].negated && !query.nodes[i].ban_only)
        .collect();
    if natural.is_empty() {
        return Vec::new();
    }

    let all_nodes = csr.all_node_ids();
    let mut cand_counts = vec![0usize; node_count];
    for &ni in &natural {
        let pattern_degree = query.pattern_degrees[ni];
        let morphism = query.node_morphism[ni];
        let pred = &query.node_preds[ni];
        let mut count = 0usize;
        for &n in all_nodes {
            let target_degree = csr.degree(*n) as usize;
            let degree_ok = match morphism {
                Morphism::Iso => target_degree == pattern_degree,
                Morphism::SubIso | Morphism::EpiMono | Morphism::Mono => target_degree >= pattern_degree,
                Morphism::Epi | Morphism::Homo => true,
            };
            if !degree_ok { continue; }
            if let Some(p) = pred {
                if !p(csr.node_val(*n)) { continue; }
            }
            count += 1;
        }
        cand_counts[ni] = count;
    }

    let mut order = Vec::with_capacity(natural.len());
    let mut in_order = vec![false; node_count];
    let first = *natural.iter()
        .min_by_key(|&&ni| cand_counts[ni])
        .unwrap();
    order.push(first);
    in_order[first] = true;

    while order.len() < natural.len() {
        let mut best = None;
        let mut best_conn = 0usize;
        let mut best_cands = usize::MAX;
        for &ni in &natural {
            if in_order[ni] { continue; }
            let conn = query.adj[ni].iter().filter(|&&(neighbor, _, neg, ei)| {
                !neg && !query.edges[ei].ban_only && in_order[neighbor]
            }).count();
            let cands = cand_counts[ni];
            if conn > best_conn
                || (conn == best_conn && cands < best_cands)
                || (best.is_none() && conn == 0)
            {
                best_conn = conn;
                best_cands = cands;
                best = Some(ni);
            }
        }
        if let Some(ni) = best {
            order.push(ni);
            in_order[ni] = true;
        }
    }

    order
}

impl<R: ReverseLookup> State<R> {
    pub(crate) fn new<NV, ER: graph::Edge, I: Index<NV, ER, Reverse = R>>(
        query: &Query<NV, ER>,
        target: &graph::Graph<NV, ER>,
        index: &I,
        bindings: Vec<Option<id::N>>,
    ) -> Self {
        let pattern_count = query.nodes.len();
        let search_order = index.compute_search_order(query);

        let positive_count = search_order.len();
        let all_iso = search_order.iter()
            .all(|&i| query.node_morphism[i] == Morphism::Iso);
        let any_injective = search_order.iter()
            .any(|&i| query.node_morphism[i].is_injective());

        let exhausted = if positive_count == 0 {
            false
        } else if all_iso {
            target.nodes.len() != positive_count
        } else if any_injective {
            target.nodes.len() < positive_count
        } else {
            false
        } || (query.has_surjective && positive_count < target.nodes.len());


        let val_filtered: Arc<Vec<Option<Vec<id::N>>>> = Arc::new(index.compute_val_filtered(query));

        State {
            bindings,
            mapping: vec![UNMAPPED; pattern_count],
            reverse: index.create_reverse(),
            search_order,
            stack: Vec::with_capacity(positive_count),
            exhausted,
            candidate_pool: Vec::with_capacity(positive_count),
            forward_verified_depths: 0,
            val_filtered,
        }
    }

    pub(crate) fn new_from_shared<NV, ER: graph::Edge, I: Index<NV, ER, Reverse = R>>(
        query: &Query<NV, ER>,
        _target: &graph::Graph<NV, ER>,
        index: &I,
        shared: &Shared,
        bindings: Vec<Option<id::N>>,
    ) -> Self {
        let pattern_count = query.nodes.len();
        let positive_count = shared.search_order.len();

        let exhausted = shared.exhausted;

        State {
            bindings,
            mapping: vec![UNMAPPED; pattern_count],
            reverse: index.create_reverse(),
            search_order: shared.search_order.clone(),
            stack: Vec::with_capacity(positive_count),
            exhausted,
            candidate_pool: Vec::with_capacity(positive_count),
            forward_verified_depths: 0,
            val_filtered: Arc::clone(&shared.val_filtered),
        }
    }
}

impl<R: ReverseLookup> State<R> {
    pub(crate) fn any_slot_pred_matches<NV, ER: graph::Edge, I: Index<NV, ER>>(
        &self,
        ctx: &Ctx<'_, NV, ER, I>,
        n1: id::N,
        n2: id::N,
        pred: &(dyn Fn(&ER::Val) -> bool + Send + Sync),
    ) -> bool {
        ctx.index.check_edge(*n1, *n2, ER::SLOT_MIN, true, false, Some(pred))
    }

    #[inline(always)]
    pub(crate) fn is_feasible_reverse_only<NV, ER: graph::Edge, I: Index<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>, pattern_idx: usize, candidate_id: Id) -> bool {
        for raw_neighbor in ctx.index.neighbors(candidate_id) {
            let mpi = self.reverse.get(raw_neighbor);
            if mpi != UNMAPPED {
                if (ctx.query.pattern_adj_bits[pattern_idx] >> mpi) & 1 == 0 {
                    return false;
                }
            }
        }
        true
    }

    #[inline(always)]
    pub(crate) fn is_feasible_fast<NV, ER: graph::Edge, I: Index<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>, pattern_idx: usize, candidate_id: Id) -> bool {
        let mut adj_bits = ctx.query.pattern_adj_bits[pattern_idx];
        while adj_bits != 0 {
            let neighbor = adj_bits.trailing_zeros() as usize;
            adj_bits &= adj_bits - 1;
            let mapped = self.mapping[neighbor];
            if mapped != UNMAPPED {
                if !ctx.index.is_adjacent(candidate_id, mapped) {
                    return false;
                }
            }
        }
        let morphism = ctx.query.node_morphism[pattern_idx];
        if morphism == Morphism::Iso || morphism == Morphism::SubIso {
            for raw_neighbor in ctx.index.neighbors(candidate_id) {
                let mpi = self.reverse.get(raw_neighbor);
                if mpi != UNMAPPED {
                    if (ctx.query.pattern_adj_bits[pattern_idx] >> mpi) & 1 == 0 {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub(crate) fn is_feasible<NV, ER: graph::Edge, I: Index<NV, ER>, W: crate::watch::Watcher<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>, pattern_idx: usize, candidate: id::N, watcher: &mut W) -> bool {
        let candidate_id: Id = *candidate;
        if !W::ACTIVE && ER::SLOT_COUNT == 1 && !ctx.query.node_has_predicates[pattern_idx] && !ctx.query.node_has_neg_adj[pattern_idx] && pattern_idx < 64 {
            return self.is_feasible_fast(ctx, pattern_idx, candidate_id);
        }
        let node_negated = ctx.query.nodes[pattern_idx].negated;

        for &(neighbor_idx, slot, negated, edge_idx) in &ctx.query.adj[pattern_idx] {
            if ctx.query.edges[edge_idx].ban_only {
                continue;
            }

            let effective_negated = if node_negated { false } else { negated };
            let any_slot = ctx.query.edges[edge_idx].any_slot;

            let mapped_id = self.mapping[neighbor_idx];
            if mapped_id != UNMAPPED {
                let result = ctx.index.check_edge(candidate_id, mapped_id, slot, any_slot, effective_negated, ctx.query.edge_preds[edge_idx].as_deref());
                if W::ACTIVE {
                    let src = id::N(candidate_id as crate::Id);
                    let tgt = id::N(mapped_id as crate::Id);
                    watcher.on_edge_test(edge_idx, src, tgt, result || effective_negated, result, effective_negated);
                }
                if !result {
                    return false;
                }
            } else if node_negated && neighbor_idx == pattern_idx {
                let result = ctx.index.check_edge(candidate_id, candidate_id, slot, any_slot, false, ctx.query.edge_preds[edge_idx].as_deref());
                if W::ACTIVE {
                    let src = id::N(candidate_id as crate::Id);
                    watcher.on_edge_test(edge_idx, src, src, result, result, false);
                }
                if !result {
                    return false;
                }
            }
        }

        if !node_negated {
            let morphism = ctx.query.node_morphism[pattern_idx];
            match morphism {
                Morphism::Iso | Morphism::SubIso => {
                    for raw_neighbor in ctx.index.neighbors(candidate_id) {
                        let mpi = self.reverse.get(raw_neighbor);
                        if mpi != UNMAPPED {
                            let mapped_pattern_idx = mpi as usize;
                            if pattern_idx < 64 && mapped_pattern_idx < 64 {
                                let has_pattern_edge = (ctx.query.pattern_adj_bits[pattern_idx] >> mpi) & 1 != 0;
                                if !has_pattern_edge {
                                    return false;
                                }
                                if ER::SLOT_COUNT == 1 {
                                    continue;
                                }
                            }
                            let occupied = ctx.index.occupied_edge_slots(candidate_id, raw_neighbor);
                            if occupied.is_empty() {
                                return false;
                            }
                            let all_covered = occupied.iter().all(|&stored_slot| {
                                let candidate_slot = if candidate_id <= raw_neighbor {
                                    stored_slot
                                } else {
                                    ER::reverse_slot(stored_slot)
                                };
                                ctx.query.adj[pattern_idx]
                                    .iter()
                                    .any(|&(ni, s, negated, ei)| {
                                        ni == mapped_pattern_idx
                                            && !negated
                                            && !ctx.query.edges[ei].ban_only
                                            && (ctx.query.edges[ei].any_slot || s == candidate_slot)
                                    })
                            });
                            if !all_covered {
                                return false;
                            }
                            let target_edge_count = occupied.len();
                            let pattern_edge_count = ctx.query.adj[pattern_idx]
                                .iter()
                                .filter(|&&(ni, _, neg, ei)| {
                                    ni == mapped_pattern_idx
                                        && !neg
                                        && !ctx.query.edges[ei].ban_only
                                })
                                .count();
                            if target_edge_count != pattern_edge_count {
                                return false;
                            }
                        }
                    }
                }
                Morphism::EpiMono | Morphism::Mono | Morphism::Epi | Morphism::Homo => {}
            }
        }

        true
    }

    pub(crate) fn lookahead_ok<NV, ER: graph::Edge, I: Index<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>, from_depth: usize) -> bool {
        let search_order = &self.search_order;
        for d in (from_depth + 1)..search_order.len() {
            let pi = search_order[d];
            let mapped_neighbor_count = if pi < 64 {
                let mut adj = ctx.query.pattern_adj_bits[pi];
                let mut count = 0u32;
                while adj != 0 {
                    let ni = adj.trailing_zeros() as usize;
                    adj &= adj - 1;
                    if self.mapping[ni] != UNMAPPED {
                        count += 1;
                    }
                }
                count
            } else {
                ctx.query.adj[pi].iter()
                    .filter(|&&(ni, _, neg, ei)| {
                        !neg && !ctx.query.edges[ei].ban_only && self.mapping[ni] != UNMAPPED
                    })
                    .count() as u32
            };
            if mapped_neighbor_count < 2 { continue; }
            if !self.has_any_candidate(ctx, pi) {
                return false;
            }
        }
        true
    }

    fn has_any_candidate<NV, ER: graph::Edge, I: Index<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>, pattern_idx: usize) -> bool {
        let anchor_raw = match self.best_mapped_neighbor(ctx, pattern_idx) {
            Some(a) => *a,
            None => return true,
        };

        let morphism = ctx.query.node_morphism[pattern_idx];
        let pattern_degree = ctx.query.pattern_degrees[pattern_idx];
        let is_injective = ctx.query.is_injective[pattern_idx];

        let mut other_mapped = [0u32; 63];
        let mut other_count = 0usize;
        if pattern_idx < 64 {
            let mut adj_bits = ctx.query.pattern_adj_bits[pattern_idx];
            while adj_bits != 0 {
                let neighbor = adj_bits.trailing_zeros() as usize;
                adj_bits &= adj_bits - 1;
                let mapped = self.mapping[neighbor];
                if mapped != UNMAPPED && mapped != anchor_raw {
                    other_mapped[other_count] = mapped;
                    other_count += 1;
                }
            }
        } else {
            for &(ni, _, neg, ei) in &ctx.query.adj[pattern_idx] {
                if neg || ctx.query.edges[ei].ban_only { continue; }
                let mapped = self.mapping[ni];
                if mapped != UNMAPPED && mapped != anchor_raw {
                    other_mapped[other_count] = mapped;
                    other_count += 1;
                }
            }
        }

        for raw in ctx.index.neighbors(anchor_raw) {
            if is_injective && self.reverse.get(raw) != UNMAPPED { continue; }
            let target_degree = ctx.index.degree(raw) as usize;
            match morphism {
                Morphism::Iso => { if target_degree != pattern_degree { continue; } }
                Morphism::SubIso | Morphism::EpiMono | Morphism::Mono => { if target_degree < pattern_degree { continue; } }
                Morphism::Epi | Morphism::Homo => {}
            }
            let mut ok = true;
            for i in 0..other_count {
                if !ctx.index.is_adjacent(raw, other_mapped[i]) {
                    ok = false;
                    break;
                }
            }
            if ok { return true; }
        }
        false
    }

    pub(crate) fn best_mapped_neighbor<NV, ER: graph::Edge, I: Index<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>, pattern_idx: usize) -> Option<id::N> {
        let mut best: Option<(id::N, u32)> = None;
        if pattern_idx < 64 && !ctx.query.node_has_predicates[pattern_idx] {
            let mut adj_bits = ctx.query.pattern_adj_bits[pattern_idx];
            while adj_bits != 0 {
                let neighbor = adj_bits.trailing_zeros() as usize;
                adj_bits &= adj_bits - 1;
                let mapped_raw = self.mapping[neighbor];
                if mapped_raw != UNMAPPED {
                    let mapped_target = id::N(mapped_raw as Id);
                    let deg = ctx.index.degree(mapped_raw);
                    match &best {
                        Some((_, best_deg)) if deg >= *best_deg => {}
                        _ => { best = Some((mapped_target, deg)); }
                    }
                }
            }
        } else {
            for &(neighbor_idx, _slot, negated, edge_idx) in &ctx.query.adj[pattern_idx] {
                if negated || ctx.query.edges[edge_idx].ban_only {
                    continue;
                }
                if neighbor_idx == pattern_idx {
                    continue;
                }
                let mapped_raw = self.mapping[neighbor_idx];
                if mapped_raw != UNMAPPED {
                    let mapped_target = id::N(mapped_raw as Id);
                    let deg = ctx.index.degree(mapped_raw);
                    match &best {
                        Some((_, best_deg)) if deg >= *best_deg => {}
                        _ => { best = Some((mapped_target, deg)); }
                    }
                }
            }
        }
        best.map(|(n, _)| n)
    }

    pub(crate) fn check_surjective<NV, ER: graph::Edge, I: Index<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>) -> bool {
        let target_count = ctx.target.nodes.len();
        let mut covered = vec![false; ctx.index.reverse_len()];
        for &idx in &self.search_order {
            let mapped = self.mapping[idx];
            if mapped != UNMAPPED {
                covered[mapped as usize] = true;
            }
        }
        covered.iter().filter(|&&c| c).count() == target_count
    }

    pub(crate) fn get_covered_slots<NV, ER: graph::Edge, I: Index<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>, pattern_src: usize, pattern_tgt: usize, src_target: id::N, tgt_target: id::N) -> smallvec::SmallVec<[ER::Slot; 3]> {
        let source_is_lo = *src_target <= *tgt_target;
        ctx.query.adj[pattern_src].iter()
            .filter(|&&(ni, _, neg, ei)| {
                ni == pattern_tgt && !neg && !ctx.query.edges[ei].ban_only && !ctx.query.edges[ei].any_slot
            })
            .map(|&(_, slot, _, _)| {
                if source_is_lo { slot } else { ER::reverse_slot(slot) }
            })
            .collect()
    }

    pub(crate) fn has_uncovered_edge<NV, ER: graph::Edge, I: Index<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>, src: id::N, tgt: id::N, covered: &[ER::Slot]) -> bool {
        ctx.target.edges_between(src, tgt)
            .any(|(slot, _)| !covered.contains(&slot))
    }

    pub(crate) fn uncovered_pred_matches<NV, ER: graph::Edge, I: Index<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>, src: id::N, tgt: id::N, covered: &[ER::Slot], pred: &dyn Fn(&ER::Val) -> bool) -> bool {
        ctx.target.edges_between(src, tgt)
            .filter(|(slot, _)| !covered.contains(slot))
            .any(|(_, val)| pred(val))
    }

    pub(crate) fn ban_shared_edges_satisfied<NV, ER: graph::Edge, I: Index<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>, ban: &BanCluster) -> bool {
        for &edge_idx in &ban.edge_indices {
            let edge = &ctx.query.edges[edge_idx];

            let src_raw = self.mapping[edge.source];
            if src_raw == UNMAPPED { continue; }
            let tgt_raw = self.mapping[edge.target];
            if tgt_raw == UNMAPPED { continue; }
            let src_target = id::N(src_raw as Id);
            let tgt_target = id::N(tgt_raw as Id);

            let src_id: Id = src_raw;
            let tgt_id: Id = tgt_raw;

            if edge.any_slot {
                let covered = self.get_covered_slots(ctx, edge.source, edge.target, src_target, tgt_target);
                if edge.negated {
                    if let Some(pred) = &ctx.query.edge_preds[edge_idx] {
                        if self.uncovered_pred_matches(ctx, src_target, tgt_target, &covered, pred.as_ref()) {
                            return false;
                        }
                    } else if self.has_uncovered_edge(ctx, src_target, tgt_target, &covered) {
                        return false;
                    }
                } else {
                    if !self.has_uncovered_edge(ctx, src_target, tgt_target, &covered) {
                        return false;
                    }
                    if let Some(pred) = &ctx.query.edge_preds[edge_idx] {
                        if !self.uncovered_pred_matches(ctx, src_target, tgt_target, &covered, pred.as_ref()) {
                            return false;
                        }
                    }
                }
            } else {
                if edge.negated {
                    let adjacent = ctx.target.is_adjacent(src_id, tgt_id);
                    if adjacent {
                        let edge_def = ER::edge(edge.slot, (src_id, tgt_id));
                        match ctx.target.get_edge_val(edge_def) {
                            Some(val) => {
                                if let Some(pred) = &ctx.query.edge_preds[edge_idx] {
                                    if pred(val) {
                                        return false;
                                    }
                                } else {
                                    return false;
                                }
                            }
                            None => {}
                        }
                    }
                } else {
                    if !ctx.target.is_adjacent(src_id, tgt_id) {
                        return false;
                    }
                    let edge_def = ER::edge(edge.slot, (src_id, tgt_id));
                    match ctx.target.get_edge_val(edge_def) {
                        Some(val) => {
                            if let Some(pred) = &ctx.query.edge_preds[edge_idx] {
                                if !pred(val) {
                                    return false;
                                }
                            }
                        }
                        None => return false,
                    }
                }
            }
        }

        if ban.morphism == Morphism::SubIso || ban.morphism == Morphism::Iso {
            for i in 0..ban.shared_nodes.len() {
                for j in (i + 1)..ban.shared_nodes.len() {
                    let ni = ban.shared_nodes[i];
                    let nj = ban.shared_nodes[j];
                    let ti_raw = self.mapping[ni];
                    if ti_raw == UNMAPPED { continue; }
                    let tj_raw = self.mapping[nj];
                    if tj_raw == UNMAPPED { continue; }
                    let ti = id::N(ti_raw as Id);
                    let tj = id::N(tj_raw as Id);

                    let covered = self.get_covered_slots(ctx, ni, nj, ti, tj);
                    let uncovered_target = ctx.target.edges_between(ti, tj)
                        .filter(|(slot, _)| !covered.contains(slot))
                        .count();

                    let ban_edge_count = ban.edge_indices.iter()
                        .filter(|&&ei| {
                            let e = &ctx.query.edges[ei];
                            !e.negated && (
                                (e.source == ni && e.target == nj) ||
                                (e.source == nj && e.target == ni)
                            )
                        })
                        .count();

                    if uncovered_target > ban_edge_count {
                        return false;
                    }
                }
            }
        }

        true
    }

    pub(crate) fn ban_node_feasible<NV, ER: graph::Edge, I: Index<NV, ER>>(
        &self,
        ctx: &Ctx<'_, NV, ER, I>,
        ban: &BanCluster,
        ban_mapping: &[Option<id::N>],
        depth: usize,
        candidate: id::N,
    ) -> bool {
        let pattern_idx = ban.ban_only_nodes[depth];
        let candidate_id: Id = *candidate;

        for &edge_idx in &ban.edge_indices {
            let edge = &ctx.query.edges[edge_idx];

            let (this_is_source, other_pattern_idx) = if edge.source == pattern_idx {
                (true, edge.target)
            } else if edge.target == pattern_idx {
                (false, edge.source)
            } else {
                continue;
            };

            let other_raw = self.mapping[other_pattern_idx];
            let other_target = if other_raw != UNMAPPED {
                id::N(other_raw as Id)
            } else if let Some(pos) = ban.ban_only_nodes.iter().position(|&ni| ni == other_pattern_idx) {
                if let Some(mapped) = ban_mapping[pos] {
                    mapped
                } else {
                    continue;
                }
            } else {
                continue;
            };

            let other_id: Id = *other_target;

            if edge.any_slot {
                let adjacent = ctx.target.is_adjacent(candidate_id, other_id);
                if edge.negated {
                    if adjacent {
                        if let Some(pred) = &ctx.query.edge_preds[edge_idx] {
                            if self.any_slot_pred_matches(ctx, candidate, other_target, pred.as_ref()) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }
                } else {
                    if !adjacent {
                        return false;
                    }
                    if let Some(pred) = &ctx.query.edge_preds[edge_idx] {
                        if !self.any_slot_pred_matches(ctx, candidate, other_target, pred.as_ref()) {
                            return false;
                        }
                    }
                }
            } else {
                let slot = if this_is_source {
                    edge.slot
                } else {
                    ER::reverse_slot(edge.slot)
                };

                if edge.negated {
                    let adjacent = ctx.target.is_adjacent(candidate_id, other_id);
                    if adjacent {
                        let edge_def = ER::edge(slot, (candidate_id, other_id));
                        match ctx.target.get_edge_val(edge_def) {
                            Some(val) => {
                                if let Some(pred) = &ctx.query.edge_preds[edge_idx] {
                                    if pred(val) {
                                        return false;
                                    }
                                } else {
                                    return false;
                                }
                            }
                            None => {}
                        }
                    }
                } else {
                    let adjacent = ctx.target.is_adjacent(candidate_id, other_id);
                    if !adjacent {
                        return false;
                    }
                    let edge_def = ER::edge(slot, (candidate_id, other_id));
                    match ctx.target.get_edge_val(edge_def) {
                        Some(val) => {
                            if let Some(pred) = &ctx.query.edge_preds[edge_idx] {
                                if !pred(val) {
                                    return false;
                                }
                            }
                        }
                        None => return false,
                    }
                }
            }
        }

        true
    }

    pub(crate) fn build_match<NV, ER: graph::Edge, I: Index<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>) -> Match {
        let mut pairs = Vec::with_capacity(self.search_order.len());
        for &idx in &self.search_order {
            let node = &ctx.query.nodes[idx];
            let raw = self.mapping[idx];
            if raw != UNMAPPED {
                pairs.push((node.local_id, id::N(raw as Id)));
            }
        }
        pairs.sort_unstable_by_key(|(lid, _)| lid.0);
        Match(pairs)
    }
}
