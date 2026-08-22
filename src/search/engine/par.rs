use std::sync::Arc;

use crate::graph;
use crate::search::query::Query;
use super::{Match, State, Shared, Ctx};
use rayon::prelude::*;
use rayon::iter::plumbing::{bridge_unindexed, UnindexedConsumer, UnindexedProducer, Folder};

pub struct Par;

impl Par {
    pub fn search<'g, NV: Sync + Send + Clone + 'g, ER: graph::Edge + 'g>(
        query: &'g Query<NV, ER>,
        target: &'g super::Graph<'g, NV, ER>,
    ) -> ParIter<'g, NV, ER>
    where
        ER::Val: Send + Sync + Clone,
        ER::Slot: Send + Sync,
        ER::CsrStore: Send + Sync,
    {
        ParIter { query, indexed: target }
    }
}

pub struct ParIter<'g, NV, ER: graph::Edge> {
    query: &'g Query<NV, ER>,
    indexed: &'g super::Graph<'g, NV, ER>,
}

impl<'g, NV, ER: graph::Edge> ParIter<'g, NV, ER> {
    pub(crate) fn new(query: &'g Query<NV, ER>, indexed: &'g super::Graph<'g, NV, ER>) -> Self {
        ParIter { query, indexed }
    }
}

struct SearchProducer<'g, NV, ER: graph::Edge> {
    candidates: Vec<crate::id::N>,
    query: &'g Query<NV, ER>,
    indexed: &'g super::Graph<'g, NV, ER>,
    shared: Arc<Shared>,
    /// Splitting floor: every leaf pays one O(node_count) State build, so
    /// unbounded splitting (rayon splits on steal pressure) re-creates the
    /// quadratic cost with many threads. Sized so there are ~4 leaves per
    /// thread for stealing balance, but never fewer than 256 roots per leaf.
    min_chunk: usize,
}

unsafe impl<'g, NV: Sync, ER: graph::Edge> Send for SearchProducer<'g, NV, ER>
where
    ER::Val: Sync,
    ER::Slot: Sync,
    ER::CsrStore: Sync,
{}

impl<'g, NV: Sync + Send + Clone, ER: graph::Edge> UnindexedProducer for SearchProducer<'g, NV, ER>
where
    ER::Val: Send + Sync + Clone,
    ER::Slot: Send + Sync,
    ER::CsrStore: Send + Sync,
{
    type Item = Match;

    fn split(mut self) -> (Self, Option<Self>) {
        if self.candidates.len() <= self.min_chunk.max(1) {
            return (self, None);
        }
        let mid = self.candidates.len() / 2;
        let right = self.candidates.split_off(mid);
        let other = SearchProducer {
            candidates: right,
            query: self.query,
            indexed: self.indexed,
            shared: Arc::clone(&self.shared),
            min_chunk: self.min_chunk,
        };
        (self, Some(other))
    }

    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        if self.candidates.is_empty() {
            return folder;
        }

        let depth0_idx = self.shared.search_order[0];
        let node_count = self.query.nodes.len();
        let ctx = Ctx { query: self.query, index: self.indexed, target: self.indexed.graph };
        let mut folder = folder;

        // One State per fold_with call, rebound per root: `reverse` is an
        // O(node_count) buffer, so constructing it per root makes the whole
        // parallel search O(V²). Rebinding clears ≤ pattern_count entries.
        let mut state: Option<State<_>> = None;
        for &root in &self.candidates {
            if folder.full() { break; }
            let mut bindings = vec![None; node_count];
            bindings[depth0_idx] = Some(root);
            let st = match state.as_mut() {
                None => {
                    state = Some(State::new_from_shared(
                        self.query, self.indexed.graph, self.indexed, &self.shared, bindings,
                    ));
                    state.as_mut().expect("just set")
                }
                Some(st) => {
                    st.rebind(self.shared.exhausted, bindings);
                    st
                }
            };
            if st.exhausted { continue; }
            folder = st.stream_fold(&ctx, folder, |f, m| {
                let f = f.consume(m);
                let full = f.full();
                (f, !full)
            });
        }
        folder
    }
}

impl<'g, NV: Sync + Send + Clone + 'g, ER: graph::Edge + 'g> ParallelIterator for ParIter<'g, NV, ER>
where
    ER::Val: Send + Sync + Clone,
    ER::Slot: Send + Sync,
    ER::CsrStore: Send + Sync,
{
    type Item = Match;

    /// Count without materializing `Match`: rayon's default `count()` drives
    /// the full Collect pipeline and allocates every match. This override
    /// mirrors `drive_unindexed`'s setup, then sums per-root counts via the
    /// engine's `feature::Count` path (no `build_match`, fused leaf counting),
    /// with one reused `State` per chunk.
    fn count(self) -> usize {
        let shared = Arc::new(Shared::precompute(self.query, self.indexed.graph, self.indexed));
        if shared.exhausted {
            return 0;
        }
        if shared.search_order.is_empty() {
            // mirrors drive_unindexed: empty pattern emits the single empty
            // match unless ban clusters exist (which get an empty producer).
            if self.query.ban_clusters.is_empty() { return 1; }
            return 0;
        }

        let ctx = Ctx { query: self.query, index: self.indexed, target: self.indexed.graph };
        let probe = State::new_from_shared(self.query, self.indexed.graph, self.indexed, &shared, Vec::new());
        let initial = probe.initial_candidates(&ctx);

        let threads = rayon::current_num_threads().max(1);
        let chunk = (initial.len() / (threads * 4)).max(256).max(1);
        let depth0_idx = shared.search_order[0];
        let node_count = self.query.nodes.len();

        initial
            .par_chunks(chunk)
            .map(|roots| {
                let ctx = Ctx { query: self.query, index: self.indexed, target: self.indexed.graph };
                let mut state: Option<State<_>> = None;
                let mut sum = 0usize;
                for &root in roots {
                    let mut bindings = vec![None; node_count];
                    bindings[depth0_idx] = Some(root);
                    let st = match state.as_mut() {
                        None => {
                            state = Some(State::new_from_shared(
                                self.query, self.indexed.graph, self.indexed, &shared, bindings,
                            ));
                            state.as_mut().expect("just set")
                        }
                        Some(st) => {
                            st.rebind(shared.exhausted, bindings);
                            st
                        }
                    };
                    if st.exhausted { continue; }
                    sum += st.stream_count(&ctx);
                }
                sum
            })
            .sum()
    }

    fn drive_unindexed<C: UnindexedConsumer<Self::Item>>(self, consumer: C) -> C::Result {
        let shared = Arc::new(Shared::precompute(self.query, self.indexed.graph, self.indexed));

        if shared.exhausted {
            let producer = SearchProducer {
                candidates: Vec::new(),
                query: self.query,
                indexed: self.indexed,
                shared,
                min_chunk: 1,
            };
            return bridge_unindexed(producer, consumer);
        }

        if shared.search_order.is_empty() {
            if self.query.ban_clusters.is_empty() {
                return rayon::iter::once(Match { bindings: Vec::new(), path_alts: Vec::new(), path_cursors: Vec::new(), path_edge_indices: Vec::new() }).drive_unindexed(consumer);
            }
            let producer = SearchProducer {
                candidates: Vec::new(),
                query: self.query,
                indexed: self.indexed,
                shared,
                min_chunk: 1,
            };
            return bridge_unindexed(producer, consumer);
        }

        let ctx = Ctx { query: self.query, index: self.indexed, target: self.indexed.graph };
        let probe = State::new_from_shared(self.query, self.indexed.graph, self.indexed, &shared, Vec::new());
        let initial = probe.initial_candidates(&ctx);

        let threads = rayon::current_num_threads().max(1);
        let min_chunk = (initial.len() / (threads * 4)).max(256);
        let producer = SearchProducer {
            candidates: initial,
            query: self.query,
            indexed: self.indexed,
            shared,
            min_chunk,
        };
        bridge_unindexed(producer, consumer)
    }
}
