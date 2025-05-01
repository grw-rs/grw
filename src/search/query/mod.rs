mod compile;

use crate::graph;
use crate::graph::dsl::LocalId;
use crate::search::{Morphism, Decision};
use crate::{Id, id};

pub use compile::compile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeKind {
    New,
    Exist,
    Translated,
}

pub(crate) struct PatternNode {
    pub(crate) local_id: LocalId,
    pub(crate) kind: NodeKind,
    pub(crate) negated: bool,
    pub(crate) ban_only: bool,
}

pub(crate) struct PatternEdge<ER: graph::Edge> {
    pub(crate) source: usize,
    pub(crate) target: usize,
    pub(crate) slot: ER::Slot,
    pub(crate) negated: bool,
    pub(crate) ban_only: bool,
    pub(crate) any_slot: bool,
}

#[allow(dead_code)]
pub(crate) struct Cluster {
    pub(crate) morphism: Morphism,
    pub(crate) decision: Decision,
    pub(crate) node_indices: Vec<usize>,
}

pub(crate) struct BanCluster {
    pub(crate) shared_nodes: Vec<usize>,
    pub(crate) ban_only_nodes: Vec<usize>,
    pub(crate) edge_indices: Vec<usize>,
    pub(crate) morphism: Morphism,
}

pub struct Query<NV, ER: graph::Edge> {
    pub(crate) nodes: Vec<PatternNode>,
    pub(crate) edges: Vec<PatternEdge<ER>>,
    pub(crate) adj: Vec<Vec<(usize, ER::Slot, bool, usize)>>,
    pub(crate) adj_check: Vec<Vec<(usize, ER::Slot, bool, bool)>>,
    pub(crate) adj_pred: Vec<Vec<(usize, ER::Slot, bool, usize)>>,
    pub(crate) clusters: Vec<Cluster>,
    pub(crate) exist_indices: Vec<usize>,
    pub(crate) translated_indices: Vec<usize>,
    pub(crate) node_morphism: Vec<Morphism>,
    pub(crate) node_preds: Vec<Option<Box<dyn Fn(&NV) -> bool + Send + Sync>>>,
    pub(crate) edge_preds: Vec<Option<Box<dyn Fn(&ER::Val) -> bool + Send + Sync>>>,
    pub(crate) ban_clusters: Vec<BanCluster>,
    pub(crate) search_order: Vec<usize>,
    pub(crate) pattern_degrees: Vec<usize>,
    pub(crate) has_ban_clusters: bool,
    pub(crate) has_surjective: bool,
    pub(crate) is_injective: Vec<bool>,
    pub(crate) pattern_adj_bits: Vec<u64>,
    pub(crate) has_predicates: bool,
    pub(crate) node_has_predicates: Vec<bool>,
    pub(crate) node_has_neg_adj: Vec<bool>,
    pub(crate) neighbor_degree_profile: Vec<Vec<usize>>,
}

impl<NV, ER: graph::Edge> Query<NV, ER> {
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn cluster_count(&self) -> usize {
        self.clusters.len()
    }

    pub fn node_local_id(&self, index: usize) -> LocalId {
        self.nodes[index].local_id
    }

    pub fn has_exist_nodes(&self) -> bool {
        !self.exist_indices.is_empty()
    }

    pub fn has_translated_nodes(&self) -> bool {
        !self.translated_indices.is_empty()
    }
}

pub struct Resolved<NV, ER: graph::Edge> {
    pub(crate) query: Query<NV, ER>,
    pub(crate) bindings: Vec<Option<id::N>>,
}

impl<NV, ER: graph::Edge> Resolved<NV, ER> {
    pub fn query(&self) -> &Query<NV, ER> { &self.query }
    pub fn bindings(&self) -> &[Option<id::N>] { &self.bindings }
    pub fn into_query(self) -> Query<NV, ER> { self.query }
}

pub struct Unresolved<NV, ER: graph::Edge> {
    pub(crate) query: Query<NV, ER>,
    pub(crate) translated_indices: Vec<usize>,
    pub(crate) base_bindings: Vec<Option<id::N>>,
}

impl<NV, ER: graph::Edge> Unresolved<NV, ER> {
    pub fn query(&self) -> &Query<NV, ER> { &self.query }
    pub fn into_query(self) -> Query<NV, ER> { self.query }
    pub fn translated_indices(&self) -> &[usize] { &self.translated_indices }

    pub fn bind(&self, table: &[(Id, Id)]) -> Result<Bound<'_, NV, ER>, BindError> {
        let mut bindings = self.base_bindings.clone();
        let mut mapped_count = 0usize;

        for &(local, target) in table {
            let ti = self.translated_indices.iter()
                .find(|&&ti| self.query.nodes[ti].local_id.0 == local);
            match ti {
                None => return Err(BindError::NotFound(local)),
                Some(&ti) => {
                    if bindings[ti].is_some() {
                        return Err(BindError::Duplicate(local));
                    }
                    bindings[ti] = Some(id::N(target));
                    mapped_count += 1;
                }
            }
        }

        if mapped_count != self.translated_indices.len() {
            let missing: Vec<Id> = self.translated_indices.iter()
                .filter(|&&ti| bindings[ti].is_none())
                .map(|&ti| self.query.nodes[ti].local_id.0)
                .collect();
            return Err(BindError::Missing(missing));
        }

        let all_pinned: Vec<usize> = self.query.exist_indices.iter()
            .chain(self.translated_indices.iter())
            .copied().collect();
        for (i, &ci) in all_pinned.iter().enumerate() {
            let morphism_i = self.query.node_morphism[ci];
            if morphism_i == Morphism::Homo { continue; }
            let target_i = bindings[ci].expect("validated above");
            for &cj in &all_pinned[(i + 1)..] {
                let morphism_j = self.query.node_morphism[cj];
                if morphism_j == Morphism::Homo { continue; }
                let target_j = bindings[cj].expect("validated above");
                if target_i == target_j {
                    return Err(BindError::Collision {
                        n1: self.query.nodes[ci].local_id.0,
                        n2: self.query.nodes[cj].local_id.0,
                        target: *target_i,
                    });
                }
            }
        }

        Ok(Bound { query: &self.query, bindings })
    }
}

pub struct Bound<'a, NV, ER: graph::Edge> {
    pub(crate) query: &'a Query<NV, ER>,
    pub(crate) bindings: Vec<Option<id::N>>,
}

impl<NV, ER: graph::Edge> Bound<'_, NV, ER> {
    pub fn query(&self) -> &Query<NV, ER> { self.query }
    pub fn bindings(&self) -> &[Option<id::N>] { &self.bindings }
}

#[derive(Debug)]
pub enum BindError {
    NotFound(Id),
    Duplicate(Id),
    Missing(Vec<Id>),
    Collision { n1: Id, n2: Id, target: Id },
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "no translated node with local id {id}"),
            Self::Duplicate(id) => write!(f, "duplicate binding for local id {id}"),
            Self::Missing(ids) => write!(f, "missing bindings for: {ids:?}"),
            Self::Collision { n1, n2, target } => write!(f, "injectivity collision: nodes {n1} and {n2} both map to {target}"),
        }
    }
}

impl std::error::Error for BindError {}

pub enum Search<NV, ER: graph::Edge> {
    Resolved(Resolved<NV, ER>),
    Unresolved(Unresolved<NV, ER>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge;
    use crate::search::error;

    type ER = edge::Undir<()>;

    #[test]
    fn compile_single_edge() {
        let Search::Resolved(r): Search<(), ER> = crate::search![
            get(Morphism::Mono) {
                N(0) ^ N(1)
            }
        ].unwrap() else { panic!("unexpected context nodes") };
        let q = r.query();
        assert_eq!(q.node_count(), 2);
        assert_eq!(q.edge_count(), 1);
        assert_eq!(q.cluster_count(), 1);
        assert!(!q.has_translated_nodes());
    }

    #[test]
    fn compile_triangle() {
        let Search::Resolved(r): Search<(), ER> = crate::search![
            get(Morphism::Iso) {
                N(0) ^ N(1),
                n(1) ^ N(2),
                n(0) ^ n(2)
            }
        ].unwrap() else { panic!("unexpected context nodes") };
        let q = r.query();
        assert_eq!(q.node_count(), 3);
        assert_eq!(q.edge_count(), 3);
        assert_eq!(q.cluster_count(), 1);
    }

    #[test]
    fn compile_shared_nodes_across_clusters() {
        let Search::Resolved(r): Search<(), ER> = crate::search![
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            ban(Morphism::Mono) {
                n(0) ^ N(2),
                n(2) ^ n(1)
            }
        ].unwrap() else { panic!("unexpected context nodes") };
        let q = r.query();
        assert_eq!(q.node_count(), 3);
        assert_eq!(q.edge_count(), 3);
        assert_eq!(q.cluster_count(), 2);

        assert_eq!(q.clusters[0].decision, Decision::Get);
        assert_eq!(q.clusters[0].node_indices.len(), 2);

        assert_eq!(q.clusters[1].decision, Decision::Ban);
        assert_eq!(q.clusters[1].node_indices.len(), 3);
    }

    #[test]
    fn compile_with_context_node() {
        let Search::Unresolved(u): Search<(), ER> = crate::search![
            get(Morphism::Mono) {
                X(10) ^ N(1)
            }
        ].unwrap() else { panic!("expected context nodes") };
        assert_eq!(u.query().node_count(), 2);
        assert_eq!(u.translated_indices.len(), 1);
    }

    #[test]
    fn compile_deduplicates_edges_across_clusters() {
        let Search::Resolved(r): Search<(), ER> = crate::search![
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            ban(Morphism::Mono) {
                n(0) ^ N(2),
                n(1) ^ n(2)
            }
        ].unwrap() else { panic!("unexpected context nodes") };
        let q = r.query();
        assert_eq!(q.node_count(), 3);
        assert_eq!(q.edge_count(), 3);
    }

    fn morphism_for(q: &Query<(), ER>, local_id: u32) -> Morphism {
        let lid = crate::graph::dsl::LocalId(local_id);
        let idx = q.nodes.iter().position(|n| n.local_id == lid)
            .expect("node not found");
        q.node_morphism[idx]
    }

    #[test]
    fn compile_node_morphism_single_cluster() {
        let Search::Resolved(r): Search<(), ER> = crate::search![
            get(Morphism::Iso) {
                N(0) ^ N(1)
            }
        ].unwrap() else { panic!("unexpected context nodes") };
        let q = r.query();
        assert_eq!(morphism_for(q, 0), Morphism::Iso);
        assert_eq!(morphism_for(q, 1), Morphism::Iso);
    }

    #[test]
    fn compile_node_morphism_takes_most_restrictive() {
        let Search::Resolved(r): Search<(), ER> = crate::search![
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            get(Morphism::SubIso) {
                n(0) ^ N(2)
            }
        ].unwrap() else { panic!("unexpected context nodes") };
        let q = r.query();
        assert_eq!(morphism_for(q, 0), Morphism::SubIso);
        assert_eq!(morphism_for(q, 1), Morphism::Mono);
        assert_eq!(morphism_for(q, 2), Morphism::SubIso);
    }

    #[test]
    fn compile_node_morphism_three_clusters() {
        let Search::Resolved(r): Search<(), ER> = crate::search![
            get(Morphism::Homo) {
                N(0) ^ N(1)
            },
            get(Morphism::Mono) {
                n(0) ^ N(2)
            },
            get(Morphism::SubIso) {
                n(0) ^ N(3)
            }
        ].unwrap() else { panic!("unexpected context nodes") };
        let q = r.query();
        assert_eq!(morphism_for(q, 0), Morphism::SubIso);
        assert_eq!(morphism_for(q, 1), Morphism::Homo);
        assert_eq!(morphism_for(q, 2), Morphism::Mono);
        assert_eq!(morphism_for(q, 3), Morphism::SubIso);
    }

    #[test]
    fn compile_validation_rejects_undefined_ref() {
        let result: Result<Search<(), ER>, error::Search> = crate::search![
            get(Morphism::Mono) {
                n(99)
            }
        ];
        assert!(result.is_err());
    }

    #[test]
    fn compile_iso_must_be_sole_get_cluster() {
        let result: Result<Search<(), ER>, error::Search> = crate::search![
            get(Morphism::Iso) {
                N(0) ^ N(1)
            },
            get(Morphism::Mono) {
                n(0) ^ N(2)
            }
        ];
        assert!(matches!(result, Err(error::Search::Cluster(error::Cluster::IsoNotSole { .. }))));
    }

    fn node_idx_for(q: &Query<(), ER>, local_id: u32) -> usize {
        let lid = crate::graph::dsl::LocalId(local_id);
        q.nodes.iter().position(|n| n.local_id == lid)
            .expect("node not found")
    }

    #[test]
    fn compile_ban_only_nodes_marked() {
        let Search::Resolved(r): Search<(), ER> = crate::search![
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            ban(Morphism::Mono) {
                n(0) ^ N(2),
                n(2) ^ n(1)
            }
        ].unwrap() else { panic!("unexpected context nodes") };
        let q = r.query();
        assert!(!q.nodes[node_idx_for(q, 0)].ban_only);
        assert!(!q.nodes[node_idx_for(q, 1)].ban_only);
        assert!(q.nodes[node_idx_for(q, 2)].ban_only);
    }

    #[test]
    fn compile_ban_only_excluded_from_search_order() {
        let Search::Resolved(r): Search<(), ER> = crate::search![
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            ban(Morphism::Mono) {
                n(0) ^ N(2),
                n(2) ^ n(1)
            }
        ].unwrap() else { panic!("unexpected context nodes") };
        let q = r.query();
        assert_eq!(q.search_order.len(), 2);
        assert!(!q.search_order.contains(&node_idx_for(q, 2)));
    }

    #[test]
    fn compile_ban_only_edges_marked() {
        let Search::Resolved(r): Search<(), ER> = crate::search![
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            ban(Morphism::Mono) {
                n(0) ^ N(2),
                n(2) ^ n(1)
            }
        ].unwrap() else { panic!("unexpected context nodes") };
        let q = r.query();
        let get_edge = q.edges.iter().find(|e| {
            e.source == node_idx_for(q, 0) && e.target == node_idx_for(q, 1)
        }).expect("get edge must exist");
        assert!(!get_edge.ban_only);

        let ban_edges: Vec<_> = q.edges.iter()
            .filter(|e| e.ban_only)
            .collect();
        assert_eq!(ban_edges.len(), 2);
    }

    #[test]
    fn compile_ban_morphism_does_not_affect_node_morphism() {
        let Search::Resolved(r): Search<(), ER> = crate::search![
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            ban(Morphism::Iso) {
                n(0) ^ N(2),
                n(2) ^ n(1)
            }
        ].unwrap() else { panic!("unexpected context nodes") };
        let q = r.query();
        assert_eq!(morphism_for(q, 0), Morphism::Mono);
        assert_eq!(morphism_for(q, 1), Morphism::Mono);
    }

    #[test]
    fn compile_ban_cluster_struct() {
        let Search::Resolved(r): Search<(), ER> = crate::search![
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            ban(Morphism::Mono) {
                n(0) ^ N(2),
                n(2) ^ n(1)
            }
        ].unwrap() else { panic!("unexpected context nodes") };
        let q = r.query();
        assert_eq!(q.ban_clusters.len(), 1);
        let ban = &q.ban_clusters[0];
        assert_eq!(ban.ban_only_nodes.len(), 1);
        assert_eq!(ban.ban_only_nodes[0], node_idx_for(q, 2));
        assert_eq!(ban.edge_indices.len(), 2);
        assert_eq!(ban.morphism, Morphism::Mono);
    }

    #[test]
    fn compile_redundant_shared_edge_in_ban_rejected() {
        let result: Result<Search<(), ER>, error::Search> = crate::search![
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            ban(Morphism::Mono) {
                n(0) ^ n(1)
                     ^ N(2)
            }
        ];
        assert!(matches!(
            result,
            Err(crate::search::error::Search::Edge(
                crate::search::error::Edge::RedundantInBan { src: 0, tgt: 1 }
            ))
        ));
    }

    // =========================================================================
    // Any-edge contradiction tests
    // =========================================================================

    #[test]
    fn any_edge_contradicts_neg_any_undir() {
        let result: Result<Search<(), ER>, error::Search> = crate::search![<(), ER>;
            get(Morphism::Mono) {
                N(0) % N(1),
                n(0) & !E() % n(1)
            }
        ];
        assert!(matches!(
            result,
            Err(error::Search::Edge(error::Edge::Contradictory { .. }))
        ));
    }

    #[test]
    fn any_edge_contradicts_neg_specific_undir() {
        let result: Result<Search<(), ER>, error::Search> = crate::search![<(), ER>;
            get(Morphism::Mono) {
                N(0) % N(1),
                n(0) & !E() ^ n(1)
            }
        ];
        assert!(matches!(
            result,
            Err(error::Search::Edge(error::Edge::Contradictory { .. }))
        ));
    }

    type DER = crate::edge::Dir<()>;

    #[test]
    fn any_edge_valid_with_neg_specific_dir() {
        let result: Result<Search<(), DER>, error::Search> = crate::search![<(), DER>;
            get(Morphism::Mono) {
                N(0) % N(1),
                n(0) & !E() << n(1)
            }
        ];
        assert!(result.is_ok());
    }

    type AER = crate::edge::Anydir<()>;

    #[test]
    fn any_edge_valid_with_neg_specific_anydir() {
        let result: Result<Search<(), AER>, error::Search> = crate::search![<(), AER>;
            get(Morphism::Mono) {
                N(0) % N(1),
                n(0) & !E() ^ n(1)
            }
        ];
        assert!(result.is_ok());
    }

    #[test]
    fn specific_contradicts_neg_any_dir() {
        let result: Result<Search<(), DER>, error::Search> = crate::search![<(), DER>;
            get(Morphism::Mono) {
                N(0) >> N(1),
                n(0) & !E() % n(1)
            }
        ];
        assert!(matches!(
            result,
            Err(error::Search::Edge(error::Edge::Contradictory { .. }))
        ));
    }

    #[test]
    fn specific_contradicts_neg_any_undir() {
        let result: Result<Search<(), ER>, error::Search> = crate::search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ N(1),
                n(0) & !E() % n(1)
            }
        ];
        assert!(matches!(
            result,
            Err(error::Search::Edge(error::Edge::Contradictory { .. }))
        ));
    }

    #[test]
    fn any_contradicts_all_specific_negated_dir() {
        let result: Result<Search<(), DER>, error::Search> = crate::search![<(), DER>;
            get(Morphism::Mono) {
                N(0) % N(1),
                n(0) & !E() >> n(1),
                n(0) & !E() << n(1)
            }
        ];
        assert!(matches!(
            result,
            Err(error::Search::Edge(error::Edge::Contradictory { .. }))
        ));
    }

    #[test]
    fn any_contradicts_all_specific_negated_anydir() {
        let result: Result<Search<(), AER>, error::Search> = crate::search![<(), AER>;
            get(Morphism::Mono) {
                N(0) % N(1),
                n(0) & !E() >> n(1),
                n(0) & !E() << n(1),
                n(0) & !E() ^ n(1)
            }
        ];
        assert!(matches!(
            result,
            Err(error::Search::Edge(error::Edge::Contradictory { .. }))
        ));
    }

    #[test]
    fn any_valid_partial_negated_anydir() {
        let result: Result<Search<(), AER>, error::Search> = crate::search![<(), AER>;
            get(Morphism::Mono) {
                N(0) % N(1),
                n(0) & !E() >> n(1),
                n(0) & !E() << n(1)
            }
        ];
        assert!(result.is_ok());
    }
}
