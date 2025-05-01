use crate::graph;
use crate::graph::collections::IdSpace;
use crate::graph::dsl::LocalId;
use crate::Id;
use crate::search::{Decision, Morphism};
use crate::search::dsl::{ClusterOps, Op, check_pattern};
use crate::search::error;
use super::{Query, PatternNode, PatternEdge, BanCluster, Cluster, NodeKind, Search, Resolved, Unresolved};
use std::collections::BTreeMap;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum EdgeSlot<S: Ord> {
    Specific(S),
    Any,
}

fn normalize_key<ER: graph::Edge>(
    src: usize,
    tgt: usize,
    slot: EdgeSlot<ER::Slot>,
) -> (usize, usize, EdgeSlot<ER::Slot>) {
    if src <= tgt {
        (src, tgt, slot)
    } else {
        let rev = match slot {
            EdgeSlot::Specific(s) => EdgeSlot::Specific(ER::reverse_slot(s)),
            EdgeSlot::Any => EdgeSlot::Any,
        };
        (tgt, src, rev)
    }
}

fn collect_user_ids<NV, ER: graph::Edge>(op: &Op<NV, ER>, ids: &mut Vec<Id>) {
    match op {
        Op::Free { id: Some(lid), edges, .. } => { ids.push(lid.0); for e in edges { collect_user_ids(&e.target, ids); } }
        Op::Free { id: None, edges, .. } => { for e in edges { collect_user_ids(&e.target, ids); } }
        Op::FreeRef { id, edges } => { ids.push(id.0); for e in edges { collect_user_ids(&e.target, ids); } }
        Op::Exist { id, edges, .. } => { ids.push(**id); for e in edges { collect_user_ids(&e.target, ids); } }
        Op::ExistRef { id, edges } => { ids.push(**id); for e in edges { collect_user_ids(&e.target, ids); } }
        Op::Context { id, edges, .. } => { ids.push(id.0); for e in edges { collect_user_ids(&e.target, ids); } }
        Op::ContextRef { id, edges } => { ids.push(id.0); for e in edges { collect_user_ids(&e.target, ids); } }
    }
}

struct Flattener<NV, ER: graph::Edge> {
    node_map: FxHashMap<LocalId, usize>,
    nodes: Vec<PatternNode>,
    edge_map: BTreeMap<(usize, usize, bool, EdgeSlot<ER::Slot>), usize>,
    edges: Vec<PatternEdge<ER>>,
    node_preds: Vec<Option<Box<dyn Fn(&NV) -> bool + Send + Sync>>>,
    edge_preds: Vec<Option<Box<dyn Fn(&ER::Val) -> bool + Send + Sync>>>,
    exist_bindings: FxHashMap<usize, crate::id::N>,
    id_space: IdSpace,
}

impl<NV, ER: graph::Edge> Flattener<NV, ER> {
    fn new(cluster_ops: &[ClusterOps<NV, ER>]) -> Self {
        let mut user_ids = Vec::new();
        for cluster in cluster_ops {
            for op in &cluster.ops {
                collect_user_ids(op, &mut user_ids);
            }
        }
        let mut id_space = IdSpace::default();
        for id in user_ids {
            id_space.remove_id(id);
        }
        Flattener {
            node_map: FxHashMap::default(),
            nodes: Vec::new(),
            edge_map: BTreeMap::new(),
            edges: Vec::new(),
            node_preds: Vec::new(),
            edge_preds: Vec::new(),
            exist_bindings: FxHashMap::default(),
            id_space,
        }
    }

    fn ensure_node(&mut self, local_id: LocalId, kind: NodeKind, negated: bool) -> usize {
        let nodes = &mut self.nodes;
        let node_preds = &mut self.node_preds;
        *self.node_map.entry(local_id).or_insert_with(|| {
            let idx = nodes.len();
            nodes.push(PatternNode { local_id, kind, negated, ban_only: false });
            node_preds.push(None);
            idx
        })
    }

    fn fresh_anon(&mut self) -> LocalId {
        LocalId(self.id_space.pop_id().expect("pattern id space exhausted"))
    }

    fn flatten_and_collect(
        &mut self,
        op: Op<NV, ER>,
        cluster_nodes: &mut FxHashSet<usize>,
        cluster_edges: &mut Vec<usize>,
    ) -> Result<usize, error::Search> {
        let (local_id, kind, negated, node_pred, edges, exist_target) = match op {
            Op::Free { id, node_pred, negated, edges, .. } => {
                let lid = match id {
                    Some(lid) => lid,
                    None => self.fresh_anon(),
                };
                (lid, NodeKind::New, negated, node_pred, edges, None)
            }
            Op::FreeRef { id, edges } => (id, NodeKind::New, false, None, edges, None),
            Op::Exist { id, node_pred, negated, edges } => {
                let lid = LocalId(*id);
                (lid, NodeKind::Exist, negated, node_pred, edges, Some(id))
            }
            Op::ExistRef { id, edges } => {
                let lid = LocalId(*id);
                (lid, NodeKind::Exist, false, None, edges, Some(id))
            }
            Op::Context { id, node_pred, negated, edges } => {
                (id, NodeKind::Translated, negated, node_pred, edges, None)
            }
            Op::ContextRef { id, edges } => (id, NodeKind::Translated, false, None, edges, None),
        };

        let node_idx = self.ensure_node(local_id, kind, negated);
        cluster_nodes.insert(node_idx);

        if let Some(target) = exist_target {
            self.exist_bindings.insert(node_idx, target);
        }

        if node_pred.is_some() && self.node_preds[node_idx].is_none() {
            self.node_preds[node_idx] = node_pred;
        }

        for edge_op in edges {
            let target_idx = self.flatten_and_collect(
                edge_op.target, cluster_nodes, cluster_edges,
            )?;
            let edge_negated = edge_op.negated
                || self.nodes[node_idx].negated
                || self.nodes[target_idx].negated;

            let edge_slot = if edge_op.any_slot {
                EdgeSlot::Any
            } else {
                EdgeSlot::Specific(edge_op.slot)
            };
            let (lo, hi, norm_slot) = normalize_key::<ER>(node_idx, target_idx, edge_slot);
            let key = (lo, hi, edge_negated, norm_slot);

            if let Some(&existing_idx) = self.edge_map.get(&key) {
                let src_lid = self.nodes[lo].local_id.0;
                let tgt_lid = self.nodes[hi].local_id.0;
                if cluster_edges.contains(&existing_idx) {
                    return Err(error::Edge::Duplicate {
                        src: src_lid,
                        tgt: tgt_lid,
                    }.into());
                }
                let existing_has_pred = self.edge_preds[existing_idx].is_some();
                let new_has_pred = edge_op.edge_pred.is_some();
                if existing_has_pred || new_has_pred {
                    return Err(error::Edge::ConflictingPred {
                        src: src_lid,
                        tgt: tgt_lid,
                    }.into());
                }
                cluster_edges.push(existing_idx);
            } else {
                let edge_idx = self.edges.len();
                self.edges.push(PatternEdge {
                    source: node_idx,
                    target: target_idx,
                    slot: edge_op.slot,
                    negated: edge_negated,
                    ban_only: false,
                    any_slot: edge_op.any_slot,
                });
                self.edge_preds.push(edge_op.edge_pred);
                self.edge_map.insert(key, edge_idx);
                cluster_edges.push(edge_idx);
            }
        }

        Ok(node_idx)
    }
}

pub fn compile<NV, ER: graph::Edge>(
    cluster_ops: Vec<ClusterOps<NV, ER>>,
) -> Result<Search<NV, ER>, error::Search> {
    check_pattern(&cluster_ops)?;

    let mut flat: Flattener<NV, ER> = Flattener::new(&cluster_ops);

    let mut get_cluster_ops = Vec::new();
    let mut ban_cluster_ops = Vec::new();
    for cluster in cluster_ops {
        match cluster.decision {
            Decision::Get => get_cluster_ops.push(cluster),
            Decision::Ban => ban_cluster_ops.push(cluster),
        }
    }
    let mut clusters = Vec::new();

    for cluster in get_cluster_ops {
        let mut node_indices = FxHashSet::default();
        let mut edge_indices = Vec::new();

        for op in cluster.ops {
            flat.flatten_and_collect(op, &mut node_indices, &mut edge_indices)?;
        }

        clusters.push(Cluster {
            morphism: cluster.morphism,
            decision: Decision::Get,
            node_indices: {
                let mut v: Vec<usize> = node_indices.into_iter().collect();
                v.sort_unstable();
                v
            },
        });
    }

    let get_node_set: FxHashSet<usize> = (0..flat.nodes.len()).collect();
    let get_edge_count = flat.edges.len();

    let mut ban_clusters = Vec::new();

    for cluster in ban_cluster_ops {
        let mut node_indices = FxHashSet::default();
        let mut edge_indices = Vec::new();

        for op in cluster.ops {
            flat.flatten_and_collect(op, &mut node_indices, &mut edge_indices)?;
        }

        let mut ban_only_nodes: Vec<usize> = node_indices
            .iter()
            .filter(|&&ni| !get_node_set.contains(&ni) && !flat.nodes[ni].negated)
            .copied()
            .collect();
        ban_only_nodes.sort_unstable();

        let mut shared_nodes: Vec<usize> = node_indices
            .iter()
            .filter(|&&ni| get_node_set.contains(&ni))
            .copied()
            .collect();
        shared_nodes.sort_unstable();

        for &ni in &ban_only_nodes {
            flat.nodes[ni].ban_only = true;
        }

        ban_clusters.push(BanCluster {
            shared_nodes,
            ban_only_nodes,
            edge_indices,
            morphism: cluster.morphism,
        });

        clusters.push(Cluster {
            morphism: cluster.morphism,
            decision: Decision::Ban,
            node_indices: {
                let mut v: Vec<usize> = node_indices.into_iter().collect();
                v.sort_unstable();
                v
            },
        });
    }

    for i in get_edge_count..flat.edges.len() {
        flat.edges[i].ban_only = true;
    }

    let get_count = clusters.iter().filter(|c| c.decision == Decision::Get).count();
    let ban_count = clusters.iter().filter(|c| c.decision == Decision::Ban).count();
    if get_count > 1 && clusters.iter().any(|c| c.decision == Decision::Get && c.morphism == Morphism::Iso) {
        return Err(error::Cluster::IsoNotSole { decision: Decision::Get, count: get_count }.into());
    }
    if ban_count > 1 && clusters.iter().any(|c| c.decision == Decision::Ban && c.morphism == Morphism::Iso) {
        return Err(error::Cluster::IsoNotSole { decision: Decision::Ban, count: ban_count }.into());
    }

    for ni in 0..flat.nodes.len() {
        let in_any = clusters.iter().any(|c| c.node_indices.contains(&ni));
        if !in_any {
            return Err(error::Node::NodeNotInAnyCluster(flat.nodes[ni].local_id.0).into());
        }
    }

    for ban in &ban_clusters {
        for &ei in &ban.edge_indices {
            let e = &flat.edges[ei];
            if !e.ban_only && !e.negated {
                let src_lid = flat.nodes[e.source].local_id.0;
                let tgt_lid = flat.nodes[e.target].local_id.0;
                return Err(error::Edge::RedundantInBan {
                    src: src_lid,
                    tgt: tgt_lid,
                }.into());
            }
        }
    }

    for ban in &ban_clusters {
        for &ei in &ban.edge_indices {
            let e = &flat.edges[ei];
            if !e.ban_only || e.negated { continue; }
            let src_in_get = get_node_set.contains(&e.source);
            let tgt_in_get = get_node_set.contains(&e.target);
            if !src_in_get || !tgt_in_get { continue; }

            let src_lid = flat.nodes[e.source].local_id.0;
            let tgt_lid = flat.nodes[e.target].local_id.0;

            let sub_iso_covers = clusters.iter()
                .filter(|c| c.decision == Decision::Get)
                .any(|c| {
                    (c.morphism == Morphism::SubIso || c.morphism == Morphism::Iso)
                        && c.node_indices.contains(&e.source)
                        && c.node_indices.contains(&e.target)
                });
            if sub_iso_covers {
                return Err(error::Edge::CoveredByGet {
                    src: src_lid,
                    tgt: tgt_lid,
                }.into());
            }

            if e.any_slot {
                let (lo, hi) = if e.source <= e.target {
                    (e.source, e.target)
                } else {
                    (e.target, e.source)
                };
                let covered_slots = flat.edge_map.range(
                    (lo, hi, false, EdgeSlot::Specific(ER::SLOT_MIN))
                        ..=(lo, hi, false, EdgeSlot::Specific(ER::SLOT_MAX))
                ).filter(|&(_, &edge_idx)| edge_idx < get_edge_count)
                .count();
                if covered_slots >= ER::SLOT_COUNT {
                    return Err(error::Edge::CoveredByGet {
                        src: src_lid,
                        tgt: tgt_lid,
                    }.into());
                }
            }
        }
    }

    // --- Contradiction checks ---
    let positive_entries: Vec<_> = flat.edge_map.iter()
        .filter(|&(&(_, _, neg, _), _)| !neg)
        .map(|(&k, &v)| (k, v))
        .collect();

    for ((lo, hi, _neg, pos_slot), _pos_ei) in &positive_entries {
        let neg_range = flat.edge_map.range(
            (*lo, *hi, true, EdgeSlot::Specific(ER::SLOT_MIN))
                ..=(*lo, *hi, true, EdgeSlot::Any)
        );

        for (&(_, _, _, ref neg_slot), &neg_ei) in neg_range {
            let neg_has_pred = flat.edge_preds[neg_ei].is_some();
            if neg_has_pred {
                continue;
            }

            let src_lid = flat.nodes[*lo].local_id.0;
            let tgt_lid = flat.nodes[*hi].local_id.0;

            match (pos_slot, neg_slot) {
                (EdgeSlot::Specific(ps), EdgeSlot::Specific(ns)) if ps == ns => {
                    return Err(error::Edge::Contradictory {
                        src: src_lid,
                        tgt: tgt_lid,
                    }.into());
                }

                (EdgeSlot::Specific(_), EdgeSlot::Any) => {
                    return Err(error::Edge::Contradictory {
                        src: src_lid,
                        tgt: tgt_lid,
                    }.into());
                }

                (EdgeSlot::Any, EdgeSlot::Any) => {
                    return Err(error::Edge::Contradictory {
                        src: src_lid,
                        tgt: tgt_lid,
                    }.into());
                }

                (EdgeSlot::Any, EdgeSlot::Specific(_)) => {
                    let neg_specific_count = flat.edge_map.range(
                        (*lo, *hi, true, EdgeSlot::Specific(ER::SLOT_MIN))
                            ..=(*lo, *hi, true, EdgeSlot::Specific(ER::SLOT_MAX))
                    ).filter(|(_, ei)| flat.edge_preds[**ei].is_none()).count();
                    if neg_specific_count >= ER::SLOT_COUNT {
                        return Err(error::Edge::Contradictory {
                            src: src_lid,
                            tgt: tgt_lid,
                        }.into());
                    }
                }

                _ => {}
            }
        }
    }

    'ban_loop: for (ban_idx, ban) in ban_clusters.iter().enumerate() {
        let ban_node_indices = &clusters[get_count + ban_idx].node_indices;
        let has_negated_node = ban_node_indices.iter()
            .any(|&ni| flat.nodes[ni].negated);
        if has_negated_node {
            continue;
        }

        let has_negated_edge = ban.edge_indices.iter()
            .any(|&ei| flat.edges[ei].negated);
        if has_negated_edge {
            continue;
        }

        let has_any_slot_edge = ban.edge_indices.iter()
            .any(|&ei| flat.edges[ei].any_slot);
        if has_any_slot_edge {
            continue;
        }

        let has_pred = ban_node_indices.iter()
            .any(|&ni| flat.node_preds[ni].is_some())
            || ban.edge_indices.iter()
            .any(|&ei| flat.edge_preds[ei].is_some());
        if has_pred {
            continue;
        }

        let shared_edges_covered = ban.edge_indices.iter()
            .filter(|&&ei| {
                let e = &flat.edges[ei];
                get_node_set.contains(&e.source) && get_node_set.contains(&e.target)
            })
            .all(|&ei| !flat.edges[ei].ban_only);
        if !shared_edges_covered {
            continue;
        }

        let has_edge_between_ban_only = ban.edge_indices.iter().any(|&ei| {
            let e = &flat.edges[ei];
            ban.ban_only_nodes.contains(&e.source) && ban.ban_only_nodes.contains(&e.target)
        });
        if has_edge_between_ban_only {
            continue;
        }

        let ban_injective = matches!(
            ban.morphism,
            Morphism::Iso | Morphism::SubIso | Morphism::Mono
        );
        if ban_injective && !ban.ban_only_nodes.is_empty() {
            continue;
        }

        for &bn in &ban.ban_only_nodes {
            let mut candidates: Option<FxHashSet<usize>> = None;
            for &ei in &ban.edge_indices {
                let e = &flat.edges[ei];
                let (shared, slot, bn_is_target) =
                    if e.source == bn && get_node_set.contains(&e.target) {
                        (e.target, e.slot, false)
                    } else if e.target == bn && get_node_set.contains(&e.source) {
                        (e.source, e.slot, true)
                    } else {
                        continue;
                    };

                let mut edge_cands: FxHashSet<usize> = FxHashSet::default();
                for gei in 0..get_edge_count {
                    let ge = &flat.edges[gei];
                    if ge.negated { continue; }
                    if bn_is_target {
                        if ge.source == shared && ge.slot == slot {
                            edge_cands.insert(ge.target);
                        }
                    } else {
                        if ge.target == shared && ge.slot == slot {
                            edge_cands.insert(ge.source);
                        }
                    }
                }

                candidates = Some(match candidates {
                    None => edge_cands,
                    Some(prev) => prev.intersection(&edge_cands).copied().collect(),
                });
            }

            match candidates {
                None => continue 'ban_loop,
                Some(c) if c.is_empty() => continue 'ban_loop,
                Some(_) => {}
            }
        }

        return Err(error::Cluster::Subsumed.into());
    }

    let node_count = flat.nodes.len();
    let mut adj: Vec<Vec<(usize, ER::Slot, bool, usize)>> = vec![Vec::new(); node_count];

    for (edge_idx, edge) in flat.edges.iter().enumerate() {
        adj[edge.source].push((edge.target, edge.slot, edge.negated, edge_idx));
        adj[edge.target].push((edge.source, ER::reverse_slot(edge.slot), edge.negated, edge_idx));
    }

    let exist_indices: Vec<usize> = flat.nodes.iter().enumerate()
        .filter(|(_, n)| n.kind == NodeKind::Exist)
        .map(|(i, _)| i).collect();
    let translated_indices: Vec<usize> = flat.nodes.iter().enumerate()
        .filter(|(_, n)| n.kind == NodeKind::Translated)
        .map(|(i, _)| i).collect();

    let mut node_morphism = vec![Morphism::Homo; node_count];
    for cluster in &clusters {
        if cluster.decision == Decision::Get {
            for &ni in &cluster.node_indices {
                node_morphism[ni] = node_morphism[ni].meet(cluster.morphism);
            }
        }
    }

    let mut pattern_degrees = vec![0usize; node_count];
    for i in 0..node_count {
        let mut seen = smallvec::SmallVec::<[usize; 8]>::new();
        for &(neighbor_idx, _, negated, edge_idx) in &adj[i] {
            if !negated
                && !flat.edges[edge_idx].ban_only
                && !seen.contains(&neighbor_idx)
            {
                seen.push(neighbor_idx);
            }
        }
        pattern_degrees[i] = seen.len();
    }

    let neighbor_degree_profile: Vec<Vec<usize>> = (0..node_count).map(|i| {
        if flat.nodes[i].negated || flat.nodes[i].ban_only { return Vec::new(); }
        let mut seen = smallvec::SmallVec::<[usize; 8]>::new();
        let mut reqs = Vec::new();
        for &(neighbor_idx, _, negated, edge_idx) in &adj[i] {
            if !negated
                && !flat.edges[edge_idx].ban_only
                && !seen.contains(&neighbor_idx)
            {
                seen.push(neighbor_idx);
                if node_morphism[neighbor_idx] != Morphism::Homo {
                    reqs.push(pattern_degrees[neighbor_idx]);
                }
            }
        }
        reqs.sort_unstable_by(|a, b| b.cmp(a));
        reqs
    }).collect();

    let search_order: Vec<usize> = {
        let natural: Vec<usize> = (0..node_count)
            .filter(|&i| !flat.nodes[i].negated && !flat.nodes[i].ban_only)
            .collect();
        if natural.is_empty() {
            natural
        } else {
            let mut order = Vec::with_capacity(natural.len());
            let mut in_order = vec![false; node_count];
            let first = *natural.iter()
                .max_by_key(|&&ni| pattern_degrees[ni])
                .unwrap();
            order.push(first);
            in_order[first] = true;
            while order.len() < natural.len() {
                let mut best = None;
                let mut best_conn = 0usize;
                let mut best_deg = 0usize;
                for &ni in &natural {
                    if in_order[ni] { continue; }
                    let conn = adj[ni].iter().filter(|&&(neighbor, _, neg, ei)| {
                        !neg && !flat.edges[ei].ban_only && in_order[neighbor]
                    }).count();
                    let deg = pattern_degrees[ni];
                    if conn > best_conn
                        || (conn == best_conn && deg > best_deg)
                        || (best.is_none() && conn == 0)
                    {
                        best_conn = conn;
                        best_deg = deg;
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
    };

    let all_negated_nodes: Vec<usize> = (0..node_count)
        .filter(|&i| flat.nodes[i].negated)
        .collect();

    if !all_negated_nodes.is_empty() {
        let negated_set: FxHashSet<usize> = all_negated_nodes.iter().copied().collect();

        let mut anchor_nodes: Vec<usize> = Vec::new();
        for &ni in &all_negated_nodes {
            for &(neighbor, _, _, _) in &adj[ni] {
                if !negated_set.contains(&neighbor) && !flat.nodes[neighbor].ban_only {
                    if !anchor_nodes.contains(&neighbor) {
                        anchor_nodes.push(neighbor);
                    }
                }
            }
        }
        anchor_nodes.sort_unstable();

        let mut implicit_ban_edge_indices: Vec<usize> = Vec::new();
        for (ei, edge) in flat.edges.iter_mut().enumerate() {
            if edge.ban_only { continue; }
            let src_neg = negated_set.contains(&edge.source);
            let tgt_neg = negated_set.contains(&edge.target);
            if src_neg || tgt_neg {
                edge.ban_only = true;
                edge.negated = false;
                implicit_ban_edge_indices.push(ei);
            }
        }

        let morphism = clusters.iter()
            .filter(|c| c.decision == Decision::Get)
            .map(|c| c.morphism)
            .reduce(|a, b| a.meet(b))
            .unwrap_or(Morphism::Mono);

        for &ni in &all_negated_nodes {
            flat.nodes[ni].ban_only = true;
        }

        ban_clusters.push(BanCluster {
            shared_nodes: anchor_nodes,
            ban_only_nodes: all_negated_nodes,
            edge_indices: implicit_ban_edge_indices,
            morphism,
        });
    }

    let has_ban_clusters = !ban_clusters.is_empty();
    let has_surjective = node_morphism.iter().any(|m| m.is_surjective());
    let is_injective: Vec<bool> = node_morphism.iter()
        .map(|m| m.is_injective())
        .collect();
    let has_predicates = flat.edge_preds.iter().any(|p| p.is_some());

    let mut node_has_predicates = vec![false; node_count];
    for (ni, _node) in flat.nodes.iter().enumerate() {
        for &(_, _, _negated, edge_idx) in &adj[ni] {
            if flat.edge_preds[edge_idx].is_some() {
                node_has_predicates[ni] = true;
                break;
            }
        }
    }

    let mut adj_check: Vec<Vec<(usize, ER::Slot, bool, bool)>> = vec![Vec::new(); node_count];
    let mut adj_pred: Vec<Vec<(usize, ER::Slot, bool, usize)>> = vec![Vec::new(); node_count];
    for ni in 0..node_count {
        for &(neighbor_idx, slot, negated, edge_idx) in &adj[ni] {
            if flat.edges[edge_idx].ban_only {
                continue;
            }
            if flat.edge_preds[edge_idx].is_some() {
                adj_pred[ni].push((neighbor_idx, slot, negated, edge_idx));
            } else {
                let any_slot = flat.edges[edge_idx].any_slot;
                adj_check[ni].push((neighbor_idx, slot, any_slot, negated));
            }
        }
    }

    let node_has_neg_adj: Vec<bool> = (0..node_count).map(|ni| {
        flat.nodes[ni].negated
            || adj_check[ni].iter().any(|&(_, _, _, negated)| negated)
    }).collect();

    let mut pattern_adj_bits = vec![0u64; node_count];
    for edge in flat.edges.iter() {
        if edge.negated || edge.ban_only {
            continue;
        }
        if edge.source < 64 && edge.target < 64 {
            pattern_adj_bits[edge.source] |= 1u64 << edge.target;
            pattern_adj_bits[edge.target] |= 1u64 << edge.source;
        }
    }

    let mut query = Query {
        nodes: flat.nodes,
        edges: flat.edges,
        adj,
        adj_check,
        adj_pred,
        clusters,
        exist_indices,
        translated_indices,
        node_morphism,
        node_preds: flat.node_preds,
        edge_preds: flat.edge_preds,
        ban_clusters,
        search_order,
        pattern_degrees,
        has_ban_clusters,
        has_surjective,
        is_injective,
        pattern_adj_bits,
        has_predicates,
        node_has_predicates,
        node_has_neg_adj,
        neighbor_degree_profile,
    };

    let mut base_bindings = vec![None; node_count];
    for (&node_idx, &target) in &flat.exist_bindings {
        base_bindings[node_idx] = Some(target);
    }
    if query.translated_indices.is_empty() {
        Ok(Search::Resolved(Resolved { query, bindings: base_bindings }))
    } else {
        let ti = std::mem::take(&mut query.translated_indices);
        Ok(Search::Unresolved(Unresolved { query, translated_indices: ti, base_bindings }))
    }
}
