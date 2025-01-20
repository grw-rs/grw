macro_rules! for_each_dir {
    ($mac:ident ! ($($arg:tt)+)) => {
        $mac!($($arg)+, Src, Shr, shr);
        $mac!($($arg)+, Tgt, Shl, shl);
        $mac!($($arg)+, Und, BitXor, bitxor);
    };
    ($mac:ident ! ()) => {
        $mac!(Src, Shr, shr);
        $mac!(Tgt, Shl, shl);
        $mac!(Und, BitXor, bitxor);
    };
}

pub mod edge;
pub mod node;
mod validate;

pub(crate) use validate::check_fragment;

pub use crate::graph::dsl::{HasVal, LocalId};
pub(crate) use crate::graph::dsl::{IntoOptional, IntoVal};

use crate::graph;
use crate::id;
use std::marker::PhantomData;

#[macro_export]
macro_rules! modify {
    ($graph:expr, [$($expr:expr),* $(,)?]) => {{
        #[allow(unused_imports)]
        use $crate::modify::dsl::*;
        $graph.modify(vec![$($expr.into()),*])
    }};
    ($($expr:expr),* $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::modify::dsl::*;
        vec![$($expr.into()),*]
    }};
}

pub enum Node<NV, ER: graph::Edge> {
    New(node::New<NV>, Vec<edge::Edge<NV, ER>>),
    Exist(node::Exist<NV, ER>),
    Translated(node::Translated<NV, ER>),
}

#[allow(non_snake_case)]
pub fn N<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::new::Node<NV, (), ER> {
    node::new::Node {
        id: Some(local.into()),
        v: (),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn N_<NV, ER: graph::Edge>() -> node::new::Node<NV, (), ER> {
    node::new::Node {
        id: None,
        v: (),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn n<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::new::Ref<NV, ER> {
    node::new::Ref {
        id: local.into(),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn X<NV, ER: graph::Edge>(id: impl Into<id::N>) -> node::exist::Node<NV, (), ER> {
    node::exist::Node {
        id: id.into(),
        v: (),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn x<NV, ER: graph::Edge>(id: impl Into<id::N>) -> node::exist::Ref<NV, ER> {
    node::exist::Ref {
        id: id.into(),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn T<NV, ER: graph::Edge>(id: impl Into<LocalId>) -> node::translated::Node<NV, (), ER> {
    node::translated::Node {
        id: id.into(),
        v: (),
        edges: Vec::new(),
    }
}

pub fn t<NV, ER: graph::Edge>(id: impl Into<LocalId>) -> node::translated::Ref<NV, ER> {
    node::translated::Ref {
        id: id.into(),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn E<NV, ER: graph::Edge>() -> edge::new::Edge<(), NV, ER> {
    edge::new::Edge((), PhantomData)
}

pub fn e<NV, ER: graph::Edge>() -> edge::exist::Edge<(), NV, ER> {
    edge::exist::Edge((), PhantomData)
}

pub struct UndirPending<N, E>(pub N, pub E);

pub trait IntoNode<NV, ER: graph::Edge> {
    fn into_node(self) -> Node<NV, ER>;
}

#[diagnostic::on_unimplemented(
    message = "exist edge target `{Self}` must be an existing node",
    label = "not an existing node — use `x(id)` or `X(id)` here",
    note = "`e()` / `!e()` operate on existing graph edges; their target must already exist",
    note = "to connect a new edge to a new node, use `E()` instead of `e()`"
)]
pub trait IntoExistNode<NV, ER: graph::Edge> {
    fn into_exist_node(self) -> Node<NV, ER>;
}

#[diagnostic::on_unimplemented(
    message = "only existing nodes can source an existing-edge operation",
    label = "this is a new node — use `X(id)` or `x(id)` here",
    note = "`e()` / `!e()` operate on already-present graph edges; the source must exist",
    note = "to attach a new edge to a new node, use `E()` instead of `e()`"
)]
pub(crate) trait ExistEdgeSource {}

impl<NV, V, ER: graph::Edge> ExistEdgeSource for node::exist::Node<NV, V, ER> {}
impl<NV, ER: graph::Edge> ExistEdgeSource for node::exist::Ref<NV, ER> {}
impl<NV, V, ER: graph::Edge> ExistEdgeSource for node::translated::Node<NV, V, ER> {}
impl<NV, ER: graph::Edge> ExistEdgeSource for node::translated::Ref<NV, ER> {}

impl<NV, V: IntoVal<NV>, ER: graph::Edge> IntoNode<NV, ER> for node::new::Node<NV, V, ER> {
    fn into_node(self) -> Node<NV, ER> {
        self.into()
    }
}
impl<NV, V: IntoOptional<NV>, ER: graph::Edge> IntoNode<NV, ER> for node::exist::Node<NV, V, ER> {
    fn into_node(self) -> Node<NV, ER> {
        self.into()
    }
}
impl<NV, ER: graph::Edge> IntoNode<NV, ER> for node::new::Ref<NV, ER> {
    fn into_node(self) -> Node<NV, ER> {
        self.into()
    }
}
impl<NV, ER: graph::Edge> IntoNode<NV, ER> for node::exist::Ref<NV, ER> {
    fn into_node(self) -> Node<NV, ER> {
        self.into()
    }
}
impl<NV, V: IntoOptional<NV>, ER: graph::Edge> IntoExistNode<NV, ER>
    for node::exist::Node<NV, V, ER>
{
    fn into_exist_node(self) -> Node<NV, ER> {
        self.into()
    }
}
impl<NV, ER: graph::Edge> IntoExistNode<NV, ER> for node::exist::Ref<NV, ER> {
    fn into_exist_node(self) -> Node<NV, ER> {
        self.into()
    }
}
impl<NV, V: IntoOptional<NV>, ER: graph::Edge> IntoNode<NV, ER>
    for node::translated::Node<NV, V, ER>
{
    fn into_node(self) -> Node<NV, ER> {
        self.into()
    }
}
impl<NV, ER: graph::Edge> IntoNode<NV, ER> for node::translated::Ref<NV, ER> {
    fn into_node(self) -> Node<NV, ER> {
        self.into()
    }
}
impl<NV, V: IntoOptional<NV>, ER: graph::Edge> IntoExistNode<NV, ER>
    for node::translated::Node<NV, V, ER>
{
    fn into_exist_node(self) -> Node<NV, ER> {
        self.into()
    }
}
impl<NV, ER: graph::Edge> IntoExistNode<NV, ER> for node::translated::Ref<NV, ER> {
    fn into_exist_node(self) -> Node<NV, ER> {
        self.into()
    }
}
