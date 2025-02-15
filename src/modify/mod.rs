pub mod dsl;
pub mod error;
mod apply;

pub use apply::Modification;

pub use dsl::{Node, edge, node};
pub use dsl::{E, N, N_, T, X, e, n, t, x};
pub use dsl::{IntoExistNode, IntoNode, UndirPending};
pub use crate::graph::dsl::{HasVal, LocalId};

use crate::graph;
use std::marker::PhantomData;

pub struct Unchecked;
pub struct Checked;

pub struct Fragment<NV, ER: graph::Edge, Phase> {
    pub(crate) ops: Vec<Node<NV, ER>>,
    _phase: PhantomData<Phase>,
}

impl<NV, ER: graph::Edge> Fragment<NV, ER, Unchecked> {
    pub fn new(ops: Vec<Node<NV, ER>>) -> Self {
        Fragment {
            ops,
            _phase: PhantomData,
        }
    }

    pub fn validate(self) -> Result<Fragment<NV, ER, Checked>, error::Fragment> {
        dsl::check_fragment(&self.ops)?;
        Ok(Fragment {
            ops: self.ops,
            _phase: PhantomData,
        })
    }
}

impl<NV: Sync, ER: graph::Edge> graph::Graph<NV, ER> {
    pub fn apply(
        &mut self,
        fragment: Fragment<NV, ER, Checked>,
    ) -> Result<Modification<NV, ER>, error::Apply> {
        self.apply_ops(fragment.ops)
    }

    pub fn modify(
        &mut self,
        ops: Vec<Node<NV, ER>>,
    ) -> Result<Modification<NV, ER>, error::Modify> {
        let fragment = Fragment::new(ops).validate()?;
        Ok(self.apply(fragment)?)
    }
}

pub fn resolve_with_bindings<NV, ER: graph::Edge>(
    ops: Vec<Node<NV, ER>>,
    bindings: &[(u32, u32)],
) -> Vec<Node<NV, ER>> {
    ops.into_iter().map(|op| resolve_node(op, bindings)).collect()
}

fn resolve_node<NV, ER: graph::Edge>(
    node: Node<NV, ER>,
    bindings: &[(u32, u32)],
) -> Node<NV, ER> {
    match node {
        Node::Translated(t) => {
            match t {
                node::Translated::Bind { id, op, edges } => {
                    let target = lookup_binding(bindings, id);
                    let edges = edges.into_iter()
                        .map(|e| resolve_edge(e, bindings))
                        .collect();
                    Node::Exist(node::Exist::Bind { id: target, op, edges })
                }
                node::Translated::Rem { id } => {
                    let target = lookup_binding(bindings, id);
                    Node::Exist(node::Exist::Rem { id: target })
                }
            }
        }
        Node::New(n, edges) => {
            let edges = edges.into_iter()
                .map(|e| resolve_edge(e, bindings))
                .collect();
            Node::New(n, edges)
        }
        Node::Exist(e) => {
            let e = match e {
                node::Exist::Bind { id, op, edges } => {
                    let edges = edges.into_iter()
                        .map(|e| resolve_edge(e, bindings))
                        .collect();
                    node::Exist::Bind { id, op, edges }
                }
                other => other,
            };
            Node::Exist(e)
        }
    }
}

fn resolve_edge<NV, ER: graph::Edge>(
    edge: edge::Edge<NV, ER>,
    bindings: &[(u32, u32)],
) -> edge::Edge<NV, ER> {
    match edge {
        edge::Edge::New { slot, val, target } => {
            edge::Edge::New { slot, val, target: resolve_node(target, bindings) }
        }
        edge::Edge::Exist { slot, op, target } => {
            edge::Edge::Exist { slot, op, target: resolve_node(target, bindings) }
        }
    }
}

fn lookup_binding(bindings: &[(u32, u32)], id: LocalId) -> crate::id::N {
    let local = id.0;
    for &(from, to) in bindings {
        if from == local {
            return crate::id::N(to);
        }
    }
    panic!("no binding for translated node T({local})")
}


#[cfg(test)]
mod tests;
