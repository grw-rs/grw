pub mod fragment {
    use crate::id;
    use crate::modify::LocalId;

    #[derive(Debug, thiserror::Error)]
    pub enum Node {
        #[error("remove conflict on node {0:?}")]
        RemoveConflict(id::N),
        #[error("duplicate existing node {0:?}")]
        DuplicateExist(id::N),
        #[error("duplicate new node {0:?}")]
        DuplicateNew(LocalId),
        #[error("undefined reference {0:?}")]
        UndefinedRef(LocalId),
        #[error("duplicate translated node {0:?}")]
        DuplicateTranslated(LocalId),
        #[error("undefined translated reference {0:?}")]
        UndefinedTranslatedRef(LocalId),
        #[error("translated remove conflict on node {0:?}")]
        TranslatedRemoveConflict(LocalId),
    }
}

pub mod apply {
    use crate::id;

    #[derive(Debug, thiserror::Error)]
    pub enum Node {
        #[error("node not found {0:?}")]
        NotFound(id::N),
        #[error("cascade conflict on node {0:?}")]
        CascadeConflict(id::N),
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Edge {
        #[error("edge not found {0:?}-{1:?}")]
        NotFound(id::N, id::N),
        #[error("duplicate edge {0:?}-{1:?}")]
        Duplicate(id::N, id::N),
        #[error("swap conflict on edge {0:?}-{1:?}")]
        SwapConflict(id::N, id::N),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Fragment {
    #[error(transparent)]
    Node(#[from] fragment::Node),
}

#[derive(Debug, thiserror::Error)]
pub enum Apply {
    #[error(transparent)]
    Node(#[from] apply::Node),
    #[error(transparent)]
    Edge(#[from] apply::Edge),
}

#[derive(Debug, thiserror::Error)]
pub enum Modify {
    #[error(transparent)]
    Fragment(#[from] Fragment),
    #[error(transparent)]
    Apply(#[from] Apply),
}
