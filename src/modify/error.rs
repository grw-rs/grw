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
    use crate::composite::Composite;
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
        #[error("edge {0:?}-{1:?} is changed whole and by part in one batch")]
        PartConflict(id::N, id::N),
        #[error("edge {a:?}-{b:?} refused the part: {refused}")]
        PartRejected { a: id::N, b: id::N, refused: IncludeRefused },
        #[error("edge {a:?}-{b:?} refused to give up the part: {refused}")]
        PartNotHeld { a: id::N, b: id::N, refused: ExcludeRefused },
        #[error(transparent)]
        SlotClass(SlotClass),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SlotClassKind {
        Undir,
        Dir,
    }

    impl std::fmt::Display for SlotClassKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                SlotClassKind::Undir => f.write_str("undirected"),
                SlotClassKind::Dir => f.write_str("directed"),
            }
        }
    }

    #[derive(Debug, thiserror::Error)]
    #[error("the part op on {a:?}-{b:?} needs a {expected} link")]
    pub struct SlotClass {
        pub a: id::N,
        pub b: id::N,
        pub expected: SlotClassKind,
    }

    pub struct IncludeRefused(Box<dyn std::error::Error + Send + Sync>);

    pub struct ExcludeRefused(Box<dyn std::error::Error + Send + Sync>);

    impl IncludeRefused {
        pub(crate) fn new(refused: impl std::error::Error + Send + Sync + 'static) -> Self {
            IncludeRefused(Box::new(refused))
        }

        pub fn of<C>(&self) -> Option<&C::IncludeRefused>
        where
            C: Composite,
            C::IncludeRefused: 'static,
        {
            self.0.downcast_ref()
        }
    }

    impl ExcludeRefused {
        pub(crate) fn new(refused: impl std::error::Error + Send + Sync + 'static) -> Self {
            ExcludeRefused(Box::new(refused))
        }

        pub fn of<C>(&self) -> Option<&C::ExcludeRefused>
        where
            C: Composite,
            C::ExcludeRefused: 'static,
        {
            self.0.downcast_ref()
        }
    }

    macro_rules! refusal_reads_as_its_combinator {
        ($refusal:ident) => {
            impl std::fmt::Debug for $refusal {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    std::fmt::Debug::fmt(&self.0, f)
                }
            }

            impl std::fmt::Display for $refusal {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    std::fmt::Display::fmt(&self.0, f)
                }
            }

            impl std::error::Error for $refusal {}
        };
    }

    refusal_reads_as_its_combinator!(IncludeRefused);
    refusal_reads_as_its_combinator!(ExcludeRefused);

    #[derive(Debug, thiserror::Error)]
    pub enum Index {
        #[error("duplicate key on index {index}: existing node {existing:?}")]
        DuplicateKey { index: crate::graph::index::IndexName, existing: id::N },
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
    #[error(transparent)]
    Index(#[from] apply::Index),
}

#[derive(Debug, thiserror::Error)]
pub enum Version {
    #[error("version {requested} does not advance current version {current}")]
    NotMonotonic { current: u64, requested: u64 },
}

#[derive(Debug, thiserror::Error)]
pub enum Modify {
    #[error(transparent)]
    Fragment(#[from] Fragment),
    #[error(transparent)]
    Apply(#[from] Apply),
    #[error(transparent)]
    Version(#[from] Version),
}
