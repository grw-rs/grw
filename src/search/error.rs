use crate::Id;
use crate::search::Morphism;

#[derive(Debug, thiserror::Error)]
pub enum Node {
    #[error("duplicate local id {0}")]
    DuplicateLocalId(Id),
    #[error("undefined reference {0}")]
    UndefinedRef(Id),
    #[error("context node {0} only allowed in ban clusters")]
    ContextOnlyInBan(Id),
    #[error("node {0} not in any cluster")]
    NodeNotInAnyCluster(Id),
}

#[derive(Debug, thiserror::Error)]
pub enum Edge {
    #[error("contradictory edge: node pair ({src}, {tgt}) has both positive and negated edge with same slot")]
    Contradictory {
        src: Id,
        tgt: Id,
    },
    #[error("duplicate edge: node pair ({src}, {tgt}) has conflicting predicates on same slot")]
    ConflictingPred {
        src: Id,
        tgt: Id,
    },
    #[error("duplicate edge: node pair ({src}, {tgt}) has same edge repeated in cluster")]
    Duplicate {
        src: Id,
        tgt: Id,
    },
    #[error("redundant ban edge: n({src})-n({tgt}) already required by get pattern")]
    RedundantInBan {
        src: Id,
        tgt: Id,
    },
    #[error("ban edge n({src})-n({tgt}) covered by get: all slots already required")]
    CoveredByGet {
        src: Id,
        tgt: Id,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum Cluster {
    #[error("invalid cluster conflict: ban[{ban_idx}] ({ban_morphism:?}) vs get[{get_idx}] ({get_morphism:?})")]
    InvalidConflict {
        ban_idx: usize,
        ban_morphism: Morphism,
        get_idx: usize,
        get_morphism: Morphism,
    },
    #[error("ban cluster subsumed by get pattern: always satisfiable")]
    Subsumed,
    #[error("Iso morphism requires exactly one {decision:?} cluster, found {count}")]
    IsoNotSole {
        decision: crate::search::Decision,
        count: usize,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum Context {
    #[error("X({0}) is not a context node in this pattern")]
    NotFound(Id),
    #[error("duplicate mapping for X({0})")]
    Duplicate(Id),
    #[error("missing context mapping for X nodes: {0:?}")]
    Missing(Vec<Id>),
    #[error("context target node {0} does not exist in graph")]
    TargetMissing(Id),
    #[error("X({x1}) and X({x2}) both map to target node {target} under injective morphism")]
    Collision { x1: Id, x2: Id, target: Id },
}

#[derive(Debug, thiserror::Error)]
pub enum Search {
    #[error(transparent)]
    Node(#[from] Node),
    #[error(transparent)]
    Edge(#[from] Edge),
    #[error(transparent)]
    Cluster(#[from] Cluster),
    #[error(transparent)]
    Bind(#[from] crate::search::query::BindError),
    #[error("bound pattern (context nodes) cannot be used with graph-provided search; use the manual ceremony instead")]
    BoundPatternInSession,
    #[error("target node {0} does not exist in graph")]
    TargetMissing(Id),
    #[error("graph build: {0}")]
    GraphBuild(Box<dyn std::error::Error + Send + Sync>),
}
