use crate::graph;
use crate::graph::dsl::LocalId;
use crate::search::Decision;
use crate::search::error;
use super::{ClusterOps, Op};
use rustc_hash::FxHashSet;

fn collect_from_op<NV, ER: graph::Edge>(
    op: &Op<NV, ER>,
    cluster_defs: &mut FxHashSet<LocalId>,
    cluster_context_defs: &mut FxHashSet<LocalId>,
    cluster_refs: &mut FxHashSet<LocalId>,
) -> Result<(), error::Node> {
    match op {
        Op::Free { id, edges, .. } => {
            if let Some(lid) = id {
                if !cluster_defs.insert(*lid) {
                    return Err(error::Node::DuplicateLocalId(lid.0));
                }
            }
            for edge in edges {
                collect_from_op(&edge.target, cluster_defs, cluster_context_defs, cluster_refs)?;
            }
        }
        Op::FreeRef { id, edges } => {
            cluster_refs.insert(*id);
            for edge in edges {
                collect_from_op(&edge.target, cluster_defs, cluster_context_defs, cluster_refs)?;
            }
        }
        Op::Context { id, edges, .. } => {
            if !cluster_defs.insert(*id) {
                return Err(error::Node::DuplicateLocalId(id.0));
            }
            cluster_context_defs.insert(*id);
            for edge in edges {
                collect_from_op(&edge.target, cluster_defs, cluster_context_defs, cluster_refs)?;
            }
        }
        Op::ContextRef { id, edges } => {
            cluster_refs.insert(*id);
            for edge in edges {
                collect_from_op(&edge.target, cluster_defs, cluster_context_defs, cluster_refs)?;
            }
        }
        Op::Exist { id, edges, .. } => {
            let lid = LocalId(**id);
            if !cluster_defs.insert(lid) {
                return Err(error::Node::DuplicateLocalId(lid.0));
            }
            cluster_context_defs.insert(lid);
            for edge in edges {
                collect_from_op(&edge.target, cluster_defs, cluster_context_defs, cluster_refs)?;
            }
        }
        Op::ExistRef { id, edges } => {
            cluster_refs.insert(LocalId(**id));
            for edge in edges {
                collect_from_op(&edge.target, cluster_defs, cluster_context_defs, cluster_refs)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn check_pattern<NV, ER: graph::Edge>(
    clusters: &[ClusterOps<NV, ER>],
) -> Result<(), error::Node> {
    let mut all_defs: FxHashSet<LocalId> = FxHashSet::default();
    let mut all_refs: FxHashSet<LocalId> = FxHashSet::default();
    let mut context_in_get: FxHashSet<LocalId> = FxHashSet::default();
    let mut all_context_defs: FxHashSet<LocalId> = FxHashSet::default();

    for cluster in clusters {
        let mut cluster_defs: FxHashSet<LocalId> = FxHashSet::default();
        let mut cluster_context_defs: FxHashSet<LocalId> = FxHashSet::default();
        let mut cluster_refs: FxHashSet<LocalId> = FxHashSet::default();

        for op in &cluster.ops {
            collect_from_op(op, &mut cluster_defs, &mut cluster_context_defs, &mut cluster_refs)?;
        }

        all_defs.extend(&cluster_defs);
        all_refs.extend(&cluster_refs);
        all_context_defs.extend(&cluster_context_defs);

        if cluster.decision == Decision::Get {
            context_in_get.extend(&cluster_context_defs);
        }
    }

    for ref_id in &all_refs {
        if !all_defs.contains(ref_id) {
            return Err(error::Node::UndefinedRef(ref_id.0));
        }
    }

    for ctx_id in &all_context_defs {
        if !context_in_get.contains(ctx_id) {
            return Err(error::Node::ContextOnlyInBan(ctx_id.0));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge;
    use crate::search::Morphism;
    use crate::search::dsl;

    type ER = edge::Undir<()>;

    fn get_cluster(morphism: Morphism, ops: Vec<dsl::Op<(), ER>>) -> ClusterOps<(), ER> {
        dsl::get(morphism, ops)
    }

    fn ban_cluster(morphism: Morphism, ops: Vec<dsl::Op<(), ER>>) -> ClusterOps<(), ER> {
        dsl::ban(morphism, ops)
    }

    fn free(id: u32) -> dsl::Op<(), ER> {
        dsl::Op::Free {
            id: Some(LocalId(id)),
            val: None,
            node_pred: None,
            negated: false,
            edges: Vec::new(),
        }
    }

    fn free_with_edge(id: u32, target: dsl::Op<(), ER>) -> dsl::Op<(), ER> {
        dsl::Op::Free {
            id: Some(LocalId(id)),
            val: None,
            node_pred: None,
            negated: false,
            edges: vec![dsl::EdgeOp {
                slot: edge::undir::Slot,
                val: (),
                edge_pred: None,
                target,
                negated: false,
                any_slot: false,
            }],
        }
    }

    fn free_ref(id: u32) -> dsl::Op<(), ER> {
        dsl::Op::FreeRef {
            id: LocalId(id),
            edges: Vec::new(),
        }
    }

    fn context(id: u32) -> dsl::Op<(), ER> {
        dsl::Op::Context {
            id: LocalId(id),
            node_pred: None,
            negated: false,
            edges: Vec::new(),
        }
    }

    fn context_ref(id: u32) -> dsl::Op<(), ER> {
        dsl::Op::ContextRef {
            id: LocalId(id),
            edges: Vec::new(),
        }
    }

    #[test]
    fn valid_single_get_cluster() {
        let clusters = vec![get_cluster(
            Morphism::Mono,
            vec![free_with_edge(0, free(1))],
        )];
        assert!(check_pattern(&clusters).is_ok());
    }

    #[test]
    fn valid_get_with_ref_in_same_cluster() {
        let clusters = vec![get_cluster(
            Morphism::Mono,
            vec![
                free_with_edge(0, free(1)),
                free_ref(0),
            ],
        )];
        assert!(check_pattern(&clusters).is_ok());
    }

    #[test]
    fn valid_get_and_ban_sharing_nodes() {
        let clusters = vec![
            get_cluster(Morphism::Mono, vec![free_with_edge(0, free(1))]),
            ban_cluster(Morphism::Homo, vec![free_with_edge(0, free(2))]),
        ];
        assert!(check_pattern(&clusters).is_ok());
    }

    #[test]
    fn valid_context_in_get_cluster() {
        let clusters = vec![get_cluster(
            Morphism::Mono,
            vec![free_with_edge(0, context(10))],
        )];
        assert!(check_pattern(&clusters).is_ok());
    }

    #[test]
    fn valid_context_ref_in_ban_with_def_in_get() {
        let clusters = vec![
            get_cluster(Morphism::Mono, vec![free_with_edge(0, context(10))]),
            ban_cluster(Morphism::Homo, vec![context_ref(10)]),
        ];
        assert!(check_pattern(&clusters).is_ok());
    }

    #[test]
    fn invalid_duplicate_def_in_same_cluster() {
        let clusters = vec![get_cluster(
            Morphism::Mono,
            vec![free(0), free(0)],
        )];
        assert!(matches!(
            check_pattern(&clusters),
            Err(error::Node::DuplicateLocalId(0))
        ));
    }

    #[test]
    fn invalid_undefined_ref() {
        let clusters = vec![get_cluster(
            Morphism::Mono,
            vec![free_ref(99)],
        )];
        assert!(matches!(
            check_pattern(&clusters),
            Err(error::Node::UndefinedRef(99))
        ));
    }

    #[test]
    fn invalid_context_only_in_ban() {
        let clusters = vec![ban_cluster(
            Morphism::Homo,
            vec![context(10)],
        )];
        assert!(matches!(
            check_pattern(&clusters),
            Err(error::Node::ContextOnlyInBan(10))
        ));
    }

    #[test]
    fn invalid_context_ref_without_def() {
        let clusters = vec![get_cluster(
            Morphism::Mono,
            vec![context_ref(10)],
        )];
        assert!(matches!(
            check_pattern(&clusters),
            Err(error::Node::UndefinedRef(10))
        ));
    }
}
