use crate::graph;
use crate::id;
use crate::Id;
use crate::search::Morphism;
use crate::search::query::Query;
use super::{Match, State, Ctx, CsrAdj, Index, ReverseLookup, feature};

pub struct Seq;

impl Seq {
    pub fn search<'g, NV: 'g, ER: graph::Edge + 'g>(
        query: &'g Query<NV, ER>,
        target: &'g super::Graph<'g, NV, ER>,
    ) -> Iter<'g, NV, ER> {
        Iter::new(query, target)
    }

    pub fn search_watched<'g, NV: 'g, ER: graph::Edge + 'g, W: crate::watch::Watcher<NV, ER>>(
        query: &'g Query<NV, ER>,
        target: &'g super::Graph<'g, NV, ER>,
        watcher: W,
    ) -> WatchedIter<'g, NV, ER, W> {
        let ctx = Ctx { query, index: target, target: target.graph };
        let state = State::new(query, target.graph, target, Vec::new());
        WatchedIter { ctx, state, watcher }
    }

    pub fn search_bound<'g, NV: 'g, ER: graph::Edge + 'g>(
        query: &'g Query<NV, ER>,
        target: &'g super::Graph<'g, NV, ER>,
        bindings: Vec<Option<id::N>>,
    ) -> Iter<'g, NV, ER> {
        Iter::new_bound(query, target, bindings)
    }
}

pub struct Iter<'g, NV, ER: graph::Edge> {
    ctx: Ctx<'g, NV, ER, super::Graph<'g, NV, ER>>,
    state: State<Vec<u32>>,
    watcher: crate::watch::Silent,
}

impl<'g, NV, ER: graph::Edge> Iter<'g, NV, ER> {
    fn new(
        query: &'g Query<NV, ER>,
        target: &'g super::Graph<'g, NV, ER>,
    ) -> Self {
        let ctx = Ctx { query, index: target, target: target.graph };
        let state = State::new(query, target.graph, target, Vec::new());
        Iter { ctx, state, watcher: crate::watch::Silent }
    }

    fn new_bound(
        query: &'g Query<NV, ER>,
        target: &'g super::Graph<'g, NV, ER>,
        bindings: Vec<Option<id::N>>,
    ) -> Self {
        let ctx = Ctx { query, index: target, target: target.graph };
        let state = State::new(query, target.graph, target, bindings);
        Iter { ctx, state, watcher: crate::watch::Silent }
    }
}

pub struct IntoIter<'g, NV, ER: graph::Edge> {
    query: Query<NV, ER>,
    indexed: super::Graph<'g, NV, ER>,
    state: State<Vec<u32>>,
    watcher: crate::watch::Silent,
}

impl<'g, NV: Clone, ER: graph::Edge> IntoIter<'g, NV, ER>
where
    ER::Val: Clone,
{
    pub(crate) fn from_session(session: super::Session<'g, NV, ER>) -> Self {
        let state = State::new(&session.query, session.indexed.graph, &session.indexed, session.bindings);
        IntoIter { query: session.query, indexed: session.indexed, state, watcher: crate::watch::Silent }
    }
}

pub struct OwnedIter<NV, ER: graph::Edge> {
    source_graph: graph::Graph<NV, ER>,
    query: Query<NV, ER>,
    csr: CsrAdj<NV, ER>,
    state: State<Vec<u32>>,
    watcher: crate::watch::Silent,
}

impl<NV: Clone, ER: graph::Edge> OwnedIter<NV, ER>
where
    ER::Val: Clone,
{
    pub fn from_graph_and_search(
        source_graph: graph::Graph<NV, ER>,
        search: crate::search::query::Search<NV, ER>,
    ) -> Result<Self, crate::search::error::Search> {
        match search {
            crate::search::query::Search::Resolved(r) => {
                let csr = CsrAdj::build(&source_graph);
                let state = State::new(&r.query, &source_graph, &csr, r.bindings);
                Ok(OwnedIter { source_graph, query: r.query, csr, state, watcher: crate::watch::Silent })
            }
            crate::search::query::Search::Unresolved(_) => Err(crate::search::error::Search::BoundPatternInSession),
        }
    }
}

pub struct WatchedIter<'g, NV, ER: graph::Edge, W: crate::watch::Watcher<NV, ER>> {
    ctx: Ctx<'g, NV, ER, super::Graph<'g, NV, ER>>,
    state: State<Vec<u32>>,
    watcher: W,
}

impl<'g, NV, ER: graph::Edge, W: crate::watch::Watcher<NV, ER>> WatchedIter<'g, NV, ER, W> {
    pub fn into_watcher(self) -> W {
        self.watcher
    }
}

impl<R: ReverseLookup> State<R> {
    pub(crate) fn initial_candidates<NV, ER: graph::Edge, I: Index<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>) -> Vec<id::N> {
        let depth0_idx = self.search_order[0];
        let morphism = ctx.query.node_morphism[depth0_idx];
        let pattern_degree = ctx.query.pattern_degrees[depth0_idx];
        let is_injective = ctx.query.is_injective[depth0_idx];

        if let Some(&Some(target_n)) = self.bindings.get(depth0_idx) {
            let mut result = Vec::new();
            let target_degree = ctx.index.degree(*target_n) as usize;
            let degree_ok = match morphism {
                Morphism::Iso => target_degree == pattern_degree,
                Morphism::SubIso | Morphism::EpiMono | Morphism::Mono => target_degree >= pattern_degree,
                Morphism::Epi | Morphism::Homo => true,
            };
            if degree_ok {
                if !(is_injective && self.reverse.get(*target_n) != super::UNMAPPED) {
                    if let Some(pred) = &ctx.query.node_preds[depth0_idx] {
                        if pred(ctx.index.node_val(*target_n)) {
                            result.push(target_n);
                        }
                    } else {
                        result.push(target_n);
                    }
                }
            }
            return result;
        }

        let pool: &[id::N] = match &self.val_filtered[depth0_idx] {
            Some(filtered) => filtered,
            None => ctx.index.all_node_ids(),
        };
        let profile = &ctx.query.neighbor_degree_profile[depth0_idx];
        let check_profile = !profile.is_empty() && morphism != Morphism::Iso && morphism.is_injective();
        let mut result = Vec::new();
        let mut nbr_degs: Vec<usize> = Vec::new();
        for &n in pool {
            let target_degree = ctx.index.degree(*n) as usize;
            match morphism {
                Morphism::Iso => { if target_degree != pattern_degree { continue; } }
                Morphism::SubIso | Morphism::EpiMono | Morphism::Mono => { if target_degree < pattern_degree { continue; } }
                Morphism::Epi | Morphism::Homo => {}
            }
            if is_injective && self.reverse.get(*n) != super::UNMAPPED { continue; }
            if let Some(pred) = &ctx.query.node_preds[depth0_idx] {
                if !pred(ctx.index.node_val(*n)) { continue; }
            }
            if check_profile {
                nbr_degs.clear();
                nbr_degs.extend(
                    ctx.index.neighbors(*n)
                        .map(|raw| ctx.index.degree(raw) as usize)
                );
                nbr_degs.sort_unstable_by(|a, b| b.cmp(a));
                let mut ok = true;
                for (i, &req) in profile.iter().enumerate() {
                    if i >= nbr_degs.len() || nbr_degs[i] < req {
                        ok = false;
                        break;
                    }
                }
                if !ok { continue; }
            }
            result.push(n);
        }
        result
    }

    fn candidates_for_into<NV, ER: graph::Edge, EP: feature::Edge, I: Index<NV, ER>>(&mut self, ctx: &Ctx<'_, NV, ER, I>, depth: usize, out: &mut Vec<id::N>) {
        out.clear();
        let pattern_idx = self.search_order[depth];

        if let Some(&Some(target_n)) = self.bindings.get(pattern_idx) {
            if (*target_n as usize) >= ctx.index.node_vals_len()
                || ctx.index.degree(*target_n) == 0
                    && !ctx.target.nodes.has(target_n)
            {
                return;
            }
            if ctx.query.is_injective[pattern_idx] && self.reverse.get(*target_n) != super::UNMAPPED {
                return;
            }
            if let Some(pred) = &ctx.query.node_preds[pattern_idx] {
                if !pred(ctx.index.node_val(*target_n)) {
                    return;
                }
            }
            out.push(target_n);
            return;
        }

        if let Some(anchor) = self.best_mapped_neighbor(ctx, pattern_idx) {
            let morphism = ctx.query.node_morphism[pattern_idx];
            let pattern_degree = ctx.query.pattern_degrees[pattern_idx];
            let is_injective = ctx.query.is_injective[pattern_idx];
            let anchor_raw: u32 = *anchor;
            let can_fast_verify = ER::SLOT_COUNT == 1
                && !ctx.query.node_has_predicates[pattern_idx]
                && pattern_idx < 64;
            let mut other_mapped = [0u32; 63];
            let mut other_count = 0usize;
            if can_fast_verify {
                let mut adj_bits = ctx.query.pattern_adj_bits[pattern_idx];
                while adj_bits != 0 {
                    let neighbor = adj_bits.trailing_zeros() as usize;
                    adj_bits &= adj_bits - 1;
                    let mapped = self.mapping[neighbor];
                    if mapped != super::UNMAPPED && mapped != anchor_raw {
                        other_mapped[other_count] = mapped;
                        other_count += 1;
                    }
                }
            }
            self.forward_verified_depths |= 1u64 << depth;
            let nbrs: Vec<u32> = ctx.index.neighbors(anchor_raw).collect();
            out.extend(nbrs
                .iter()
                .copied()
                .filter(|&raw| {
                    if is_injective && self.reverse.get(raw) != super::UNMAPPED { return false; }
                    let target_degree = ctx.index.degree(raw) as usize;
                    match morphism {
                        Morphism::Iso => {
                            if target_degree != pattern_degree { return false; }
                        }
                        Morphism::SubIso | Morphism::EpiMono | Morphism::Mono => {
                            if target_degree < pattern_degree { return false; }
                        }
                        Morphism::Epi | Morphism::Homo => {}
                    }
                    if let Some(pred) = &ctx.query.node_preds[pattern_idx] {
                        if !pred(ctx.index.node_val(raw)) { return false; }
                    }
                    if can_fast_verify {
                        for i in 0..other_count {
                            if !ctx.index.is_adjacent(raw, other_mapped[i]) {
                                return false;
                            }
                        }
                        for &(neighbor_idx, slot, any_slot, negated) in &ctx.query.adj_check[pattern_idx] {
                            if !negated { continue; }
                            let mapped_raw = self.mapping[neighbor_idx];
                            if mapped_raw == super::UNMAPPED { continue; }
                            if ctx.index.has_edge_in_slot(raw, mapped_raw, slot, any_slot) == negated {
                                return false;
                            }
                        }
                    } else {
                        for &(neighbor_idx, slot, any_slot, negated) in &ctx.query.adj_check[pattern_idx] {
                            let mapped_raw = self.mapping[neighbor_idx];
                            if mapped_raw == super::UNMAPPED { continue; }
                            if ctx.index.has_edge_in_slot(raw, mapped_raw, slot, any_slot) == negated {
                                return false;
                            }
                        }
                        if EP::HAS_PREDICATES {
                            for &(neighbor_idx, slot, negated, edge_idx) in &ctx.query.adj_pred[pattern_idx] {
                                let mapped_raw = self.mapping[neighbor_idx];
                                if mapped_raw == super::UNMAPPED { continue; }
                                let any_slot = ctx.query.edges[edge_idx].any_slot;
                                if !ctx.index.check_edge(raw, mapped_raw, slot, any_slot, negated, ctx.query.edge_preds[edge_idx].as_deref()) {
                                    return false;
                                }
                            }
                        }
                    }
                    true
                })
                .map(|raw| id::N(raw as Id)));
            return;
        }

        let pattern_degree = ctx.query.pattern_degrees[pattern_idx];
        let morphism = ctx.query.node_morphism[pattern_idx];
        let is_injective = ctx.query.is_injective[pattern_idx];

        let pool: &[id::N] = match &self.val_filtered[pattern_idx] {
            Some(filtered) => filtered,
            None => ctx.index.all_node_ids(),
        };
        out.extend(pool.iter()
            .copied()
            .filter(|&n| {
                let target_degree = ctx.index.degree(*n) as usize;
                match morphism {
                    Morphism::Iso => {
                        if target_degree != pattern_degree { return false; }
                    }
                    Morphism::SubIso | Morphism::EpiMono | Morphism::Mono => {
                        if target_degree < pattern_degree { return false; }
                    }
                    Morphism::Epi | Morphism::Homo => {}
                }
                if is_injective && self.reverse.get(*n) != super::UNMAPPED { return false; }
                if let Some(pred) = &ctx.query.node_preds[pattern_idx] {
                    if !pred(ctx.index.node_val(*n)) { return false; }
                }
                true
            }));

        out.sort_unstable();
    }

    fn check_ban_clusters<NV, ER: graph::Edge, I: Index<NV, ER>, W: crate::watch::Watcher<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>, watcher: &mut W) -> bool {
        for (idx, ban) in ctx.query.ban_clusters.iter().enumerate() {
            if self.ban_cluster_satisfiable(ctx, ban) {
                if W::ACTIVE {
                    watcher.on_ban_verdict(idx, crate::watch::BanVerdict::Fired);
                }
                return false;
            }
        }
        true
    }

    fn ban_cluster_satisfiable<NV, ER: graph::Edge, I: Index<NV, ER>>(
        &self,
        ctx: &Ctx<'_, NV, ER, I>,
        ban: &super::super::query::BanCluster,
    ) -> bool {
        if !self.ban_shared_edges_satisfied(ctx, ban) {
            return false;
        }
        if ban.ban_only_nodes.is_empty() {
            return true;
        }
        let mut ban_mapping: Vec<Option<id::N>> = vec![None; ban.ban_only_nodes.len()];
        self.ban_backtrack(ctx, ban, &mut ban_mapping, 0)
    }

    fn ban_backtrack<NV, ER: graph::Edge, I: Index<NV, ER>>(
        &self,
        ctx: &Ctx<'_, NV, ER, I>,
        ban: &super::super::query::BanCluster,
        ban_mapping: &mut Vec<Option<id::N>>,
        depth: usize,
    ) -> bool {
        if depth == ban.ban_only_nodes.len() {
            return true;
        }

        let pattern_idx = ban.ban_only_nodes[depth];
        let candidates: Vec<id::N> = ctx.index.all_node_ids().to_vec();

        for n in candidates {
            match ban.morphism {
                Morphism::Iso | Morphism::SubIso | Morphism::EpiMono | Morphism::Mono => {
                    if self.reverse.get(*n) != super::UNMAPPED {
                        continue;
                    }
                    if ban_mapping.iter().any(|m| *m == Some(n)) {
                        continue;
                    }
                }
                Morphism::Epi | Morphism::Homo => {}
            }

            if let Some(pred) = &ctx.query.node_preds[pattern_idx] {
                if !pred(ctx.index.node_val(*n)) {
                    continue;
                }
            }

            if !self.ban_node_feasible(ctx, ban, ban_mapping, depth, n) {
                continue;
            }

            ban_mapping[depth] = Some(n);
            if self.ban_backtrack(ctx, ban, ban_mapping, depth + 1) {
                ban_mapping[depth] = None;
                return true;
            }
            ban_mapping[depth] = None;
        }

        false
    }

    #[inline(always)]
    fn count_leaf_fused<NV, ER: graph::Edge, EP: feature::Edge, I: Index<NV, ER>, W: crate::watch::Watcher<NV, ER>>(&self, ctx: &Ctx<'_, NV, ER, I>, leaf_depth: usize, watcher: &mut W) -> Option<usize> {
        let leaf_pi = self.search_order[leaf_depth];

        if let Some(&Some(_)) = self.bindings.get(leaf_pi) {
            return None;
        }

        let anchor = self.best_mapped_neighbor(ctx, leaf_pi)?;
        let anchor_raw: u32 = *anchor;

        let morphism = ctx.query.node_morphism[leaf_pi];
        let pattern_degree = ctx.query.pattern_degrees[leaf_pi];
        let is_injective = ctx.query.is_injective[leaf_pi];
        let can_fast = ER::SLOT_COUNT == 1 && !ctx.query.node_has_predicates[leaf_pi] && leaf_pi < 64;

        let mut other_mapped = [0u32; 63];
        let mut other_count = 0usize;
        if can_fast {
            let mut adj_bits = ctx.query.pattern_adj_bits[leaf_pi];
            while adj_bits != 0 {
                let neighbor = adj_bits.trailing_zeros() as usize;
                adj_bits &= adj_bits - 1;
                let mapped = self.mapping[neighbor];
                if mapped != super::UNMAPPED && mapped != anchor_raw {
                    other_mapped[other_count] = mapped;
                    other_count += 1;
                }
            }
        }

        let mut count = 0usize;

        for raw in ctx.index.neighbors(anchor_raw) {
            if is_injective && self.reverse.get(raw) != super::UNMAPPED {
                continue;
            }

            let target_degree = ctx.index.degree(raw) as usize;
            match morphism {
                Morphism::Iso => {
                    if target_degree != pattern_degree { continue; }
                }
                Morphism::SubIso | Morphism::EpiMono | Morphism::Mono => {
                    if target_degree < pattern_degree { continue; }
                }
                Morphism::Epi | Morphism::Homo => {}
            }

            if let Some(pred) = &ctx.query.node_preds[leaf_pi] {
                if !pred(ctx.index.node_val(raw)) { continue; }
            }

            if can_fast {
                let mut ok = true;
                for i in 0..other_count {
                    if !ctx.index.is_adjacent(raw, other_mapped[i]) {
                        ok = false;
                        break;
                    }
                }
                if !ok { continue; }
                for &(neighbor_idx, slot, any_slot, negated) in &ctx.query.adj_check[leaf_pi] {
                    if !negated { continue; }
                    let mapped_raw = self.mapping[neighbor_idx];
                    if mapped_raw == super::UNMAPPED { continue; }
                    if ctx.index.has_edge_in_slot(raw, mapped_raw, slot, any_slot) == negated {
                        ok = false;
                        break;
                    }
                }
                if !ok { continue; }

                if morphism == Morphism::Iso || morphism == Morphism::SubIso {
                    if !self.is_feasible_reverse_only(ctx, leaf_pi, raw) {
                        continue;
                    }
                }
            } else {
                let mut ok = true;
                for &(neighbor_idx, slot, any_slot, negated) in &ctx.query.adj_check[leaf_pi] {
                    let mapped_raw = self.mapping[neighbor_idx];
                    if mapped_raw == super::UNMAPPED { continue; }
                    if ctx.index.has_edge_in_slot(raw, mapped_raw, slot, any_slot) == negated {
                        ok = false;
                        break;
                    }
                }
                if !ok { continue; }
                if EP::HAS_PREDICATES {
                    for &(neighbor_idx, slot, negated, edge_idx) in &ctx.query.adj_pred[leaf_pi] {
                        let mapped_raw = self.mapping[neighbor_idx];
                        if mapped_raw == super::UNMAPPED { continue; }
                        let any_slot = ctx.query.edges[edge_idx].any_slot;
                        if !ctx.index.check_edge(raw, mapped_raw, slot, any_slot, negated, ctx.query.edge_preds[edge_idx].as_deref()) {
                            ok = false; break;
                        }
                    }
                    if !ok { continue; }
                }
                if morphism == Morphism::Iso || morphism == Morphism::SubIso {
                    if ER::SLOT_COUNT > 1 {
                        if !self.is_feasible(ctx, leaf_pi, id::N(raw as Id), watcher) {
                            continue;
                        }
                    } else {
                        if !self.is_feasible_reverse_only(ctx, leaf_pi, raw) {
                            continue;
                        }
                    }
                }
            }


            count += 1;
        }

        Some(count)
    }

    pub(crate) fn advance<
        'a, NV,
        ER: graph::Edge,
        EP: feature::Edge,
        BP: feature::Ban,
        EM: feature::Emit,
        I: Index<NV, ER>,
        W: crate::watch::Watcher<NV, ER>,
    >(&mut self, ctx: &Ctx<'a, NV, ER, I>, watcher: &mut W) -> (Option<Match>, usize) {
        if self.exhausted {
            return (None, 0);
        }

        let search_len = self.search_order.len();

        if search_len == 0 {
            self.exhausted = true;
            let ban_ok = !BP::ACTIVE || self.check_ban_clusters(ctx, watcher);
            if ban_ok {
                return if EM::COUNT_ONLY { (None, 1) } else { (Some(Match(Vec::new())), 0) };
            }
            return (None, 0);
        }

        if self.stack.is_empty() {
            let candidates = self.initial_candidates(ctx);
            self.stack.push(super::StackFrame { depth: 0, candidates, candidate_idx: 0 });
        }

        let mut total = 0usize;

        loop {
            let frame = match self.stack.last_mut() {
                Some(f) => f,
                None => {
                    self.exhausted = true;
                    return (None, total);
                }
            };

            if frame.candidate_idx >= frame.candidates.len() {
                let depth = frame.depth;
                if let Some(popped) = self.stack.pop() {
                    let mut recycled = popped.candidates;
                    recycled.clear();
                    self.candidate_pool.push(recycled);
                }

                if depth > 0 {
                    let prev_pattern_idx = self.search_order[depth - 1];
                    let prev_target = self.mapping[prev_pattern_idx];
                    if prev_target != super::UNMAPPED {
                        if W::ACTIVE {
                            let prev_local_id = ctx.query.nodes[prev_pattern_idx].local_id;
                            watcher.on_unbind(depth - 1, prev_local_id);
                        }
                        self.mapping[prev_pattern_idx] = super::UNMAPPED;
                        self.reverse.clear(prev_target);
                    }
                }

                continue;
            }

            let depth = frame.depth;
            let candidate = frame.candidates[frame.candidate_idx];
            frame.candidate_idx += 1;

            let pattern_idx = self.search_order[depth];

            if (self.forward_verified_depths >> depth) & 1 == 1 {
                let morphism = ctx.query.node_morphism[pattern_idx];
                if morphism == Morphism::Iso || morphism == Morphism::SubIso {
                    if ER::SLOT_COUNT > 1 {
                        if !self.is_feasible(ctx, pattern_idx, candidate, watcher) {
                            continue;
                        }
                    } else if W::ACTIVE {
                        if !self.is_feasible(ctx, pattern_idx, candidate, watcher) {
                            continue;
                        }
                    } else {
                        if !self.is_feasible_reverse_only(ctx, pattern_idx, *candidate) {
                            continue;
                        }
                    }
                }
            } else {
                if !self.is_feasible(ctx, pattern_idx, candidate, watcher) {
                    continue;
                }
            }

            if depth + 1 == search_len
                && EM::COUNT_ONLY
                && !BP::ACTIVE
                && !W::ACTIVE
                && !ctx.query.has_surjective
            {
                total += 1;
                continue;
            }

            self.mapping[pattern_idx] = *candidate;
            self.reverse.set(*candidate, pattern_idx as u32);

            if W::ACTIVE {
                let local_id = ctx.query.nodes[pattern_idx].local_id;
                match watcher.on_bind(depth, local_id, candidate) {
                    crate::watch::Control::Stop => {
                        self.exhausted = true;
                        return (None, total);
                    }
                    _ => {}
                }
            }

            if depth + 2 < search_len {
                if !self.lookahead_ok(ctx, depth) {
                    if W::ACTIVE {
                        let local_id = ctx.query.nodes[pattern_idx].local_id;
                        watcher.on_unbind(depth, local_id);
                    }
                    self.mapping[pattern_idx] = super::UNMAPPED;
                    self.reverse.clear(*candidate);
                    continue;
                }
            }

            if depth + 1 == search_len {
                if (!BP::ACTIVE || self.check_ban_clusters(ctx, watcher))
                    && (!ctx.query.has_surjective || self.check_surjective(ctx))
                {
                    if EM::COUNT_ONLY {
                        total += 1;
                    } else {
                        let result = self.build_match(ctx);
                        if W::ACTIVE {
                            let control = watcher.on_match(&result.0);
                            let local_id = ctx.query.nodes[pattern_idx].local_id;
                            watcher.on_unbind(depth, local_id);
                            self.mapping[pattern_idx] = super::UNMAPPED;
                            self.reverse.clear(*candidate);
                            match control {
                                crate::watch::Control::Stop => {
                                    self.exhausted = true;
                                    return (None, total);
                                }
                                _ => return (Some(result), 0),
                            }
                        } else {
                            self.mapping[pattern_idx] = super::UNMAPPED;
                            self.reverse.clear(*candidate);
                            return (Some(result), 0);
                        }
                    }
                }
                if W::ACTIVE {
                    let local_id = ctx.query.nodes[pattern_idx].local_id;
                    watcher.on_unbind(depth, local_id);
                }
                self.mapping[pattern_idx] = super::UNMAPPED;
                self.reverse.clear(*candidate);
                continue;
            }

            if EM::COUNT_ONLY
                && depth + 2 == search_len
                && !BP::ACTIVE
                && !ctx.query.has_surjective
            {
                if let Some(leaf_count) = self.count_leaf_fused::<NV, ER, EP, I, W>(ctx, depth + 1, watcher) {
                    total += leaf_count;
                } else {
                    let mut buf = self.candidate_pool.pop().unwrap_or_default();
                    self.candidates_for_into::<NV, ER, EP, I>(ctx, depth + 1, &mut buf);
                    let leaf_pi = self.search_order[depth + 1];
                    let fv = (self.forward_verified_depths >> (depth + 1)) & 1 == 1;
                    if fv {
                        let m = ctx.query.node_morphism[leaf_pi];
                        if m == Morphism::Iso || m == Morphism::SubIso {
                            if ER::SLOT_COUNT > 1 {
                                for &c in &buf {
                                    if self.is_feasible(ctx, leaf_pi, c, watcher) {
                                        total += 1;
                                    }
                                }
                            } else {
                                for &c in &buf {
                                    if self.is_feasible_reverse_only(ctx, leaf_pi, *c) {
                                        total += 1;
                                    }
                                }
                            }
                        } else {
                            total += buf.len();
                        }
                    } else {
                        for &c in &buf {
                            if self.is_feasible(ctx, leaf_pi, c, watcher) {
                                total += 1;
                            }
                        }
                    }
                    buf.clear();
                    self.candidate_pool.push(buf);
                }
                if W::ACTIVE {
                    let local_id = ctx.query.nodes[pattern_idx].local_id;
                    watcher.on_unbind(depth, local_id);
                }
                self.mapping[pattern_idx] = super::UNMAPPED;
                self.reverse.clear(*candidate);
                continue;
            }

            let mut buf = self.candidate_pool.pop().unwrap_or_default();
            self.candidates_for_into::<NV, ER, EP, I>(ctx, depth + 1, &mut buf);

            self.stack.push(super::StackFrame {
                depth: depth + 1,
                candidates: buf,
                candidate_idx: 0,
            });
        }
    }
}

macro_rules! dispatch_advance {
    ($state:expr, $ctx:expr, $emit:ty, $watcher:expr) => {
        match (
            $ctx.query.has_predicates,
            $ctx.query.has_ban_clusters,
        ) {
            (false, false) => $state.advance::<_, _, feature::PlainEdges, feature::NoBans, $emit, _, _>($ctx, $watcher),
            (false, true)  => $state.advance::<_, _, feature::PlainEdges, feature::WithBans, $emit, _, _>($ctx, $watcher),
            (true, false)  => $state.advance::<_, _, feature::PredEdges, feature::NoBans, $emit, _, _>($ctx, $watcher),
            (true, true)   => $state.advance::<_, _, feature::PredEdges, feature::WithBans, $emit, _, _>($ctx, $watcher),
        }
    };
}

impl<'g, NV, ER: graph::Edge> Iterator for Iter<'g, NV, ER> {
    type Item = Match;

    fn next(&mut self) -> Option<Match> {
        dispatch_advance!(self.state, &self.ctx, feature::Collect, &mut self.watcher).0
    }

    fn count(mut self) -> usize {
        dispatch_advance!(self.state, &self.ctx, feature::Count, &mut self.watcher).1
    }
}

impl<'g, NV, ER: graph::Edge> Iterator for IntoIter<'g, NV, ER> {
    type Item = Match;

    fn next(&mut self) -> Option<Match> {
        let ctx = Ctx {
            query: &self.query,
            index: &self.indexed,
            target: self.indexed.graph,
        };
        dispatch_advance!(self.state, &ctx, feature::Collect, &mut self.watcher).0
    }

    fn count(mut self) -> usize {
        let ctx = Ctx {
            query: &self.query,
            index: &self.indexed,
            target: self.indexed.graph,
        };
        dispatch_advance!(self.state, &ctx, feature::Count, &mut self.watcher).1
    }
}

impl<NV: 'static, ER: graph::Edge + 'static> Iterator for OwnedIter<NV, ER> {
    type Item = Match;

    fn next(&mut self) -> Option<Match> {
        let ctx = Ctx {
            query: &self.query,
            index: &self.csr,
            target: &self.source_graph,
        };
        dispatch_advance!(self.state, &ctx, feature::Collect, &mut self.watcher).0
    }

    fn count(mut self) -> usize {
        let ctx = Ctx {
            query: &self.query,
            index: &self.csr,
            target: &self.source_graph,
        };
        dispatch_advance!(self.state, &ctx, feature::Count, &mut self.watcher).1
    }
}

impl<'g, NV, ER: graph::Edge, W: crate::watch::Watcher<NV, ER>> Iterator for WatchedIter<'g, NV, ER, W> {
    type Item = Match;

    fn next(&mut self) -> Option<Match> {
        dispatch_advance!(self.state, &self.ctx, feature::Collect, &mut self.watcher).0
    }
}

impl<R: ReverseLookup> State<R> {
    pub(crate) fn stream_fold<'a, NV, ER: graph::Edge, I: Index<NV, ER>, A>(
        &mut self,
        ctx: &Ctx<'a, NV, ER, I>,
        mut acc: A,
        mut f: impl FnMut(A, Match) -> (A, bool),
    ) -> A {
        let mut watcher = crate::watch::Silent;
        loop {
            match dispatch_advance!(self, ctx, feature::Collect, &mut watcher).0 {
                Some(m) => {
                    let (new_acc, cont) = f(acc, m);
                    acc = new_acc;
                    if !cont { return acc; }
                }
                None => return acc,
            }
        }
    }
}
