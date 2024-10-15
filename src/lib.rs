#![allow(dead_code)]

#[cfg(all(feature = "id32", feature = "id64"))]
compile_error!("feature \"id32\" and \"id64\" cannot be enabled at the same time");

#[cfg(all(not(feature = "id32"), not(feature = "id64")))]
compile_error!("either feature \"id32\" or \"id64\" must be set");

#[cfg(feature = "id32")]
pub type Id = u32;
#[cfg(feature = "id64")]
pub type Id = u64;

use std::fmt::Debug;

macro_rules! def_id_wrapper {
    ($name:ident) => {
        #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
        #[derive(serde::Serialize, serde::Deserialize)]
        #[repr(transparent)]
        pub struct $name(pub crate::Id);

        impl std::ops::Deref for $name {
            type Target = Id;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl From<Id> for $name {
            fn from(value: Id) -> Self {
                $name(value)
            }
        }
        impl From<$name> for Id {
            fn from(value: $name) -> Self {
                value.0
            }
        }
        impl From<&$name> for Id {
            fn from(value: &$name) -> Self {
                value.0
            }
        }
        impl std::fmt::Debug for $name
        where
            Self: Copy + Eq + std::ops::Deref<Target = Id>,
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(stringify!($name))?;
                f.write_str(&format!("({:?})", **self))
            }
        }
    };
}

pub mod id {
    use super::*;
    def_id_wrapper!(N);
    def_id_wrapper!(E);
}

/// Normalized Relation
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct NR<I: Debug + Copy + Eq>(pub(crate) [I; 2]);

impl<I: Debug + Copy + Eq + std::ops::Deref<Target = Id>> Debug for NR<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [n0, n1]: [Id; 2] = self.0.map(|n| *n);
        f.write_fmt(format_args!("NR({n0:}, {n1:})"))
    }
}

impl<I: Debug + Copy + Eq> NR<I> {
    #[inline]
    pub fn n1(&self) -> &I {
        &self.0[0]
    }
    #[inline]
    pub fn n2(&self) -> &I {
        &self.0[1]
    }
    pub fn is_cycle(&self) -> bool {
        self.n1() == self.n2()
    }
    pub(crate) fn ns_refs(&self) -> [&I; 2] {
        [self.n1(), self.n2()]
    }
}

impl From<[id::N; 2]> for NR<id::N> {
    #[inline]
    fn from(ns: [id::N; 2]) -> Self {
        NR(if ns[0] <= ns[1] { ns } else { [ns[1], ns[0]] })
    }
}

impl From<(id::N, id::N)> for NR<id::N> {
    #[inline]
    fn from((n1, n2): (id::N, id::N)) -> Self {
        [n1, n2].into()
    }
}

impl From<(Id, Id)> for NR<id::N> {
    #[inline]
    fn from((n1, n2): (Id, Id)) -> Self {
        [id::N(n1), id::N(n2)].into()
    }
}

impl From<NR<id::N>> for [id::N; 2] {
    fn from(value: NR<id::N>) -> Self {
        value.0
    }
}

impl std::ops::Deref for NR<id::N> {
    type Target = [id::N; 2];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub mod graph;

pub(crate) use graph::collections::*;
pub(crate) use graph::{Edge, Edges, Node, Nodes};
pub use graph::edge;


pub use graph::Graph;
