use crate::{Id, NR, id};
use std::fmt::Debug;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Node {
    #[error("duplicate node {0:?}")]
    Duplicate(id::N),
    #[error("duplicate local id {0}")]
    DuplicateLocalId(Id),
    #[error("undefined reference {0}")]
    UndefinedRef(Id),
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Edge<S: Debug + Eq> {
    #[error("duplicate edge {0:?} slot {1:?}")]
    Duplicate(NR<id::N>, S),
    #[error("edge references missing node {0:?}")]
    NodeNotFound(id::N),
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Build<S: Debug + Eq> {
    #[error(transparent)]
    Node(#[from] Node),
    #[error(transparent)]
    Edge(Edge<S>),
}

impl<S: Debug + Eq> From<Edge<S>> for Build<S> {
    fn from(e: Edge<S>) -> Self {
        Build::Edge(e)
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Index {
    #[error("duplicate index name {0}")]
    DuplicateName(super::index::IndexName),
    #[error("no such index {0}")]
    NoSuchIndex(super::index::IndexName),
    #[error("index {index} key tag mismatch: expected {expected:?}, got {got:?}")]
    KeyTagMismatch { index: super::index::IndexName, expected: super::index::KeyTag, got: super::index::KeyTag },
    #[error("index {index} is not unique across nodes {nodes:?}")]
    NotUnique { index: super::index::IndexName, nodes: Vec<id::N> },
}
