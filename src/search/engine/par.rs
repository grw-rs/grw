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
        if self.candidates.len() <= 1 {
            return (self, None);
        }
        let mid = self.candidates.len() / 2;
        let right = self.candidates.split_off(mid);
        let other = SearchProducer {
            candidates: right,
            query: self.query,
            indexed: self.indexed,
            shared: Arc::clone(&self.shared),
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

        for &root in &self.candidates {
            if folder.full() { break; }
            let mut bindings = vec![None; node_count];
            bindings[depth0_idx] = Some(root);
            let mut state = State::new_from_shared(
                self.query, self.indexed.graph, self.indexed, &self.shared, bindings,
            );
            if state.exhausted { continue; }
            folder = state.stream_fold(&ctx, folder, |f, m| {
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

    fn drive_unindexed<C: UnindexedConsumer<Self::Item>>(self, consumer: C) -> C::Result {
        let shared = Arc::new(Shared::precompute(self.query, self.indexed.graph, self.indexed));

        if shared.exhausted {
            let producer = SearchProducer {
                candidates: Vec::new(),
                query: self.query,
                indexed: self.indexed,
                shared,
            };
            return bridge_unindexed(producer, consumer);
        }

        if shared.search_order.is_empty() {
            if self.query.ban_clusters.is_empty() {
                return rayon::iter::once(Match(Vec::new())).drive_unindexed(consumer);
            }
            let producer = SearchProducer {
                candidates: Vec::new(),
                query: self.query,
                indexed: self.indexed,
                shared,
            };
            return bridge_unindexed(producer, consumer);
        }

        let ctx = Ctx { query: self.query, index: self.indexed, target: self.indexed.graph };
        let probe = State::new_from_shared(self.query, self.indexed.graph, self.indexed, &shared, Vec::new());
        let initial = probe.initial_candidates(&ctx);

        let producer = SearchProducer {
            candidates: initial,
            query: self.query,
            indexed: self.indexed,
            shared,
        };
        bridge_unindexed(producer, consumer)
    }
}
