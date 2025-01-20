use super::edge::{self, Edge};
use super::{HasVal, IntoExistNode, IntoNode, IntoOptional, IntoVal, LocalId, Node, UndirPending};
use crate::graph;
use crate::graph::edge::{Src, Tgt, Und};
use crate::id;
use std::ops::{BitXor, Shl, Shr};

pub mod new {
    use super::*;

    pub struct Node<NV, V, ER: graph::Edge> {
        pub(crate) id: Option<LocalId>,
        pub(crate) v: V,
        pub(crate) edges: Vec<Edge<NV, ER>>,
    }

    pub struct Ref<NV, ER: graph::Edge> {
        pub(crate) id: LocalId,
        pub(crate) edges: Vec<Edge<NV, ER>>,
    }

    impl<NV, ER: graph::Edge> Node<NV, (), ER> {
        pub fn val(self, v: NV) -> Node<NV, HasVal<NV>, ER> {
            Node {
                id: self.id,
                v: HasVal(v),
                edges: self.edges,
            }
        }
    }
}

pub mod exist {
    use super::*;
    use std::marker::PhantomData;
    use std::ops::Not;

    pub struct Node<NV, V, ER: graph::Edge> {
        pub(crate) id: id::N,
        pub(crate) v: V,
        pub(crate) edges: Vec<Edge<NV, ER>>,
    }

    pub struct Ref<NV, ER: graph::Edge> {
        pub(crate) id: id::N,
        pub(crate) edges: Vec<Edge<NV, ER>>,
    }

    pub struct Rem<NV, ER: graph::Edge>(pub(crate) id::N, pub PhantomData<(NV, ER)>);

    impl<NV, ER: graph::Edge> Node<NV, (), ER> {
        pub fn val(self, v: NV) -> Node<NV, HasVal<NV>, ER> {
            Node {
                id: self.id,
                v: HasVal(v),
                edges: self.edges,
            }
        }
    }

    impl<NV, V, ER: graph::Edge> Not for Node<NV, V, ER> {
        type Output = Rem<NV, ER>;

        fn not(self) -> Rem<NV, ER> {
            Rem(self.id, PhantomData)
        }
    }
}

pub enum New<NV> {
    Add { id: Option<LocalId>, val: NV },
    Ref { id: LocalId },
}

pub enum Bind<NV> {
    Pass,
    Swap(NV),
    Ref,
}

pub enum Exist<NV, ER: graph::Edge> {
    Bind {
        id: id::N,
        op: Bind<NV>,
        edges: Vec<Edge<NV, ER>>,
    },
    Rem {
        id: id::N,
    },
}

pub mod translated {
    use super::*;
    use std::marker::PhantomData;
    use std::ops::Not;

    pub struct Node<NV, V, ER: graph::Edge> {
        pub(crate) id: LocalId,
        pub(crate) v: V,
        pub(crate) edges: Vec<Edge<NV, ER>>,
    }

    pub struct Ref<NV, ER: graph::Edge> {
        pub(crate) id: LocalId,
        pub(crate) edges: Vec<Edge<NV, ER>>,
    }

    pub struct Rem<NV, ER: graph::Edge>(pub(crate) LocalId, pub PhantomData<(NV, ER)>);

    impl<NV, ER: graph::Edge> Node<NV, (), ER> {
        pub fn val(self, v: NV) -> Node<NV, HasVal<NV>, ER> {
            Node {
                id: self.id,
                v: HasVal(v),
                edges: self.edges,
            }
        }
    }

    impl<NV, V, ER: graph::Edge> Not for Node<NV, V, ER> {
        type Output = Rem<NV, ER>;

        fn not(self) -> Rem<NV, ER> {
            Rem(self.id, PhantomData)
        }
    }
}

pub enum Translated<NV, ER: graph::Edge> {
    Bind {
        id: LocalId,
        op: Bind<NV>,
        edges: Vec<Edge<NV, ER>>,
    },
    Rem {
        id: LocalId,
    },
}

impl<NV, V: IntoVal<NV>, ER: graph::Edge> From<new::Node<NV, V, ER>> for Node<NV, ER> {
    fn from(n: new::Node<NV, V, ER>) -> Self {
        Node::New(
            New::Add {
                id: n.id,
                val: n.v.into_val(),
            },
            n.edges,
        )
    }
}

impl<NV, ER: graph::Edge> From<new::Ref<NV, ER>> for Node<NV, ER> {
    fn from(n: new::Ref<NV, ER>) -> Self {
        Node::New(New::Ref { id: n.id }, n.edges)
    }
}

impl<NV, V: IntoOptional<NV>, ER: graph::Edge> From<exist::Node<NV, V, ER>> for Node<NV, ER> {
    fn from(n: exist::Node<NV, V, ER>) -> Self {
        let op = match n.v.into_optional() {
            Some(v) => Bind::Swap(v),
            None => Bind::Pass,
        };
        Node::Exist(Exist::Bind {
            id: n.id,
            op,
            edges: n.edges,
        })
    }
}

impl<NV, ER: graph::Edge> From<exist::Ref<NV, ER>> for Node<NV, ER> {
    fn from(n: exist::Ref<NV, ER>) -> Self {
        Node::Exist(Exist::Bind {
            id: n.id,
            op: Bind::Ref,
            edges: n.edges,
        })
    }
}

impl<NV, ER: graph::Edge> From<exist::Rem<NV, ER>> for Node<NV, ER> {
    fn from(n: exist::Rem<NV, ER>) -> Self {
        Node::Exist(Exist::Rem { id: n.0 })
    }
}

impl<NV, V: IntoOptional<NV>, ER: graph::Edge> From<translated::Node<NV, V, ER>> for Node<NV, ER> {
    fn from(n: translated::Node<NV, V, ER>) -> Self {
        let op = match n.v.into_optional() {
            Some(v) => Bind::Swap(v),
            None => Bind::Pass,
        };
        Node::Translated(Translated::Bind {
            id: n.id,
            op,
            edges: n.edges,
        })
    }
}

impl<NV, ER: graph::Edge> From<translated::Ref<NV, ER>> for Node<NV, ER> {
    fn from(n: translated::Ref<NV, ER>) -> Self {
        Node::Translated(Translated::Bind {
            id: n.id,
            op: Bind::Ref,
            edges: n.edges,
        })
    }
}

impl<NV, ER: graph::Edge> From<translated::Rem<NV, ER>> for Node<NV, ER> {
    fn from(n: translated::Rem<NV, ER>) -> Self {
        Node::Translated(Translated::Rem { id: n.0 })
    }
}

// Anon edge ops: node >> rhs, node << rhs, node ^ rhs
// Adds a new edge with ER::Val::default() in the given direction.

macro_rules! impl_anon_new_edge_op {
    ($Self:ty, $V:ident, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, $V, ER: graph::Edge + $Dir, RHS: IntoNode<NV, ER>> $Op<RHS> for $Self
        where
            ER::Val: Default,
        {
            type Output = Self;
            fn $op(mut self, rhs: RHS) -> Self {
                self.edges.push(Edge::New {
                    slot: <ER as $Dir>::SLOT,
                    val: ER::Val::default(),
                    target: rhs.into_node(),
                });
                self
            }
        }
    };
    ($Self:ty, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, ER: graph::Edge + $Dir, RHS: IntoNode<NV, ER>> $Op<RHS> for $Self
        where
            ER::Val: Default,
        {
            type Output = Self;
            fn $op(mut self, rhs: RHS) -> Self {
                self.edges.push(Edge::New {
                    slot: <ER as $Dir>::SLOT,
                    val: ER::Val::default(),
                    target: rhs.into_node(),
                });
                self
            }
        }
    };
}

for_each_dir!(impl_anon_new_edge_op!(new::Node<NV, V, ER>, V));
for_each_dir!(impl_anon_new_edge_op!(exist::Node<NV, V, ER>, V));
for_each_dir!(impl_anon_new_edge_op!(translated::Node<NV, V, ER>, V));
for_each_dir!(impl_anon_new_edge_op!(new::Ref<NV, ER>));
for_each_dir!(impl_anon_new_edge_op!(exist::Ref<NV, ER>));
for_each_dir!(impl_anon_new_edge_op!(translated::Ref<NV, ER>));

// UndirPending ops: (node & edge) >> rhs, etc.
// Resolves the pending undirected edge with an explicit direction.

// With a new::Edge — uses into_val() on the edge value typestate.

macro_rules! impl_undir_new_op {
    ($NodeTy:ty, $V:ident, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, $V, EV: IntoVal<ER::Val>, ER: graph::Edge + $Dir, RHS: IntoNode<NV, ER>>
            $Op<RHS> for UndirPending<$NodeTy, edge::new::Edge<EV, NV, ER>>
        {
            type Output = $NodeTy;
            fn $op(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(Edge::New {
                    slot: <ER as $Dir>::SLOT,
                    val: self.1.0.into_val(),
                    target: rhs.into_node(),
                });
                node
            }
        }
    };
    ($NodeTy:ty, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, EV: IntoVal<ER::Val>, ER: graph::Edge + $Dir, RHS: IntoNode<NV, ER>>
            $Op<RHS> for UndirPending<$NodeTy, edge::new::Edge<EV, NV, ER>>
        {
            type Output = $NodeTy;
            fn $op(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(Edge::New {
                    slot: <ER as $Dir>::SLOT,
                    val: self.1.0.into_val(),
                    target: rhs.into_node(),
                });
                node
            }
        }
    };
}

for_each_dir!(impl_undir_new_op!(new::Node<NV, V, ER>, V));
for_each_dir!(impl_undir_new_op!(exist::Node<NV, V, ER>, V));
for_each_dir!(impl_undir_new_op!(translated::Node<NV, V, ER>, V));
for_each_dir!(impl_undir_new_op!(new::Ref<NV, ER>));
for_each_dir!(impl_undir_new_op!(exist::Ref<NV, ER>));
for_each_dir!(impl_undir_new_op!(translated::Ref<NV, ER>));

// With an exist::Edge — uses into_optional() to produce Pass or Swap.

macro_rules! impl_undir_exist_op {
    ($NodeTy:ty, $V:ident, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, $V, EV: IntoOptional<ER::Val>, ER: graph::Edge + $Dir, RHS: IntoExistNode<NV, ER>>
            $Op<RHS> for UndirPending<$NodeTy, edge::exist::Edge<EV, NV, ER>>
        {
            type Output = $NodeTy;
            fn $op(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                let op = edge::Exist::Bind(match self.1.0.into_optional() {
                    Some(v) => edge::Bind::Swap(v),
                    None => edge::Bind::Pass,
                });
                node.edges.push(Edge::Exist {
                    slot: <ER as $Dir>::SLOT,
                    op,
                    target: rhs.into_exist_node(),
                });
                node
            }
        }
    };
    ($NodeTy:ty, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, EV: IntoOptional<ER::Val>, ER: graph::Edge + $Dir, RHS: IntoExistNode<NV, ER>>
            $Op<RHS> for UndirPending<$NodeTy, edge::exist::Edge<EV, NV, ER>>
        {
            type Output = $NodeTy;
            fn $op(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                let op = edge::Exist::Bind(match self.1.0.into_optional() {
                    Some(v) => edge::Bind::Swap(v),
                    None => edge::Bind::Pass,
                });
                node.edges.push(Edge::Exist {
                    slot: <ER as $Dir>::SLOT,
                    op,
                    target: rhs.into_exist_node(),
                });
                node
            }
        }
    };
}

for_each_dir!(impl_undir_exist_op!(exist::Node<NV, V, ER>, V));
for_each_dir!(impl_undir_exist_op!(exist::Ref<NV, ER>));
for_each_dir!(impl_undir_exist_op!(translated::Node<NV, V, ER>, V));
for_each_dir!(impl_undir_exist_op!(translated::Ref<NV, ER>));

// With an exist::Rem — removes an existing edge.

macro_rules! impl_undir_rem_op {
    ($NodeTy:ty, $V:ident, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, $V, ER: graph::Edge + $Dir, RHS: IntoExistNode<NV, ER>>
            $Op<RHS> for UndirPending<$NodeTy, edge::exist::Rem<(), NV, ER>>
        {
            type Output = $NodeTy;
            fn $op(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(Edge::Exist {
                    slot: <ER as $Dir>::SLOT,
                    op: edge::Exist::Rem,
                    target: rhs.into_exist_node(),
                });
                node
            }
        }
    };
    ($NodeTy:ty, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, ER: graph::Edge + $Dir, RHS: IntoExistNode<NV, ER>>
            $Op<RHS> for UndirPending<$NodeTy, edge::exist::Rem<(), NV, ER>>
        {
            type Output = $NodeTy;
            fn $op(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(Edge::Exist {
                    slot: <ER as $Dir>::SLOT,
                    op: edge::Exist::Rem,
                    target: rhs.into_exist_node(),
                });
                node
            }
        }
    };
}

for_each_dir!(impl_undir_rem_op!(exist::Node<NV, V, ER>, V));
for_each_dir!(impl_undir_rem_op!(exist::Ref<NV, ER>));
for_each_dir!(impl_undir_rem_op!(translated::Node<NV, V, ER>, V));
for_each_dir!(impl_undir_rem_op!(translated::Ref<NV, ER>));
