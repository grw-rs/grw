# Modify DSL Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a typestate-driven DSL for batch graph modification (add/remove/change nodes and edges) with two-phase validation.

**Architecture:** Constructor functions (`N`, `n`, `X`, `x`, `E`, `e`) return typestate structs. Operator overloading (`>>`, `<<`, `^`, `&`, `!`) builds expression trees. `Fragment` collects expressions, validates in two phases (fragment-only, then graph-dependent), and applies mutations atomically.

**Tech Stack:** Pure Rust, no new dependencies. Uses existing `graph::edge::{Src, Tgt, Und}` traits for direction constraints. Follows search DSL patterns from `src/search/dsl/`.

**Design doc:** `docs/plans/2026-02-20-modify-dsl-design.md`

---

### Task 1: Module scaffold + core types

**Files:**
- Create: `src/modify/mod.rs` (overwrite existing parked stub)
- Create: `src/modify/dsl/mod.rs`
- Create: `src/modify/dsl/node.rs`
- Create: `src/modify/dsl/edge.rs`
- Modify: `src/lib.rs:127` (add `pub mod modify;`)

**Step 1: Write a compile-check test**

Add to `src/modify/mod.rs` a `#[cfg(test)] mod tests` with a basic test that constructs the core types:

```rust
#[cfg(test)]
mod tests {
    use super::dsl::*;
    use crate::graph;

    #[test]
    fn local_id_equality() {
        assert_eq!(Local(1), Local(1));
        assert_ne!(Local(1), Local(2));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo check --lib`
Expected: FAIL — module `modify::dsl` and type `Local` don't exist yet.

**Step 3: Write minimal implementation**

`src/modify/mod.rs`:
```rust
pub mod dsl;
```

`src/modify/dsl/mod.rs`:
```rust
pub mod node;
pub mod edge;

use crate::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Local(pub Id);

impl From<Id> for Local {
    fn from(id: Id) -> Self {
        Local(id)
    }
}

pub use node::{N, n, X, x};
pub use edge::{E, e};
```

`src/modify/dsl/node.rs`:
```rust
use std::marker::PhantomData;
use crate::{Id, id};
use super::Local;

pub trait Kind {}
pub struct New;
pub struct Exist;
pub struct Remove;
impl Kind for New {}
impl Kind for Exist {}
impl Kind for Remove {}

pub struct Node<K: Kind, State>(pub(crate) State, pub(crate) PhantomData<K>);

pub struct NodeRef<K: Kind, I>(pub(crate) I, pub(crate) PhantomData<K>);

pub fn N(local: impl Into<Local>) -> Node<New, Init<Local, ()>> {
    Node(Init { id: local.into(), edges: vec![] }, PhantomData)
}

pub fn n(local: impl Into<Local>) -> NodeRef<New, Local> {
    NodeRef(local.into(), PhantomData)
}

pub fn X(id: impl Into<id::N>) -> Node<Exist, Init<id::N, ()>> {
    Node(Init { id: id.into(), edges: vec![] }, PhantomData)
}

pub fn x(id: impl Into<id::N>) -> NodeRef<Exist, id::N> {
    NodeRef(id.into(), PhantomData)
}

pub struct Init<I, E> {
    pub(crate) id: I,
    pub(crate) edges: Vec<E>,
}

pub struct WithVal<I, NV, E> {
    pub(crate) id: I,
    pub(crate) val: NV,
    pub(crate) edges: Vec<E>,
}
```

`src/modify/dsl/edge.rs`:
```rust
use std::marker::PhantomData;

pub struct NewEdge;
pub struct ExistEdge;
pub struct RemoveEdge;

pub struct Edge<K, State>(pub(crate) State, pub(crate) PhantomData<K>);

pub struct Disconnected;
pub struct DisconnectedVal<EV>(pub(crate) EV);

pub fn E() -> Edge<NewEdge, Disconnected> {
    Edge(Disconnected, PhantomData)
}

pub fn e() -> Edge<ExistEdge, Disconnected> {
    Edge(Disconnected, PhantomData)
}
```

Modify `src/lib.rs:127` — add after `mod shape;`:
```rust
pub mod modify;
```

**Step 4: Run test to verify it passes**

Run: `cargo test --lib modify::tests::local_id_equality`
Expected: PASS

**Step 5: Commit**

```bash
git add src/modify/ src/lib.rs
git commit -m "scaffold modify module with core DSL types"
```

---

### Task 2: Node .val() typestate transition

**Files:**
- Modify: `src/modify/dsl/node.rs`

**Step 1: Write compile-check test**

Add to `src/modify/mod.rs` tests:
```rust
#[test]
fn node_val_typestate() {
    let _bare: node::Node<node::New, node::Init<Local, ()>> = N(1);
    let _valued: node::Node<node::New, node::WithVal<Local, &str, ()>> = N(2).val("hello");
    let _xbare: node::Node<node::Exist, node::Init<id::N, ()>> = X(10);
    let _xval: node::Node<node::Exist, node::WithVal<id::N, &str, ()>> = X(10).val("hi");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo check --lib`
Expected: FAIL — `.val()` not defined.

**Step 3: Write implementation**

In `src/modify/dsl/node.rs`, add `.val()` impls:
```rust
impl<I, E> Node<New, Init<I, E>> {
    pub fn val<NV>(self, val: NV) -> Node<New, WithVal<I, NV, E>> {
        Node(WithVal { id: self.0.id, val, edges: vec![] }, PhantomData)
    }
}

impl<E> Node<Exist, Init<id::N, E>> {
    pub fn val<NV>(self, val: NV) -> Node<Exist, WithVal<id::N, NV, E>> {
        Node(WithVal { id: self.0.id, val, edges: vec![] }, PhantomData)
    }
}
```

Note: `Node<New, WithVal>` and `Node<Exist, WithVal>` do NOT have `.val()` — calling `.val()` twice is a compile error.

**Step 4: Run test**

Run: `cargo test --lib modify::tests::node_val_typestate`
Expected: PASS

**Step 5: Commit**

```bash
git add src/modify/
git commit -m "add .val() typestate transition for nodes"
```

---

### Task 3: Edge .val() typestate + Not operator

**Files:**
- Modify: `src/modify/dsl/edge.rs`
- Modify: `src/modify/dsl/node.rs`

**Step 1: Write compile-check test**

Add to `src/modify/mod.rs` tests:
```rust
#[test]
fn edge_val_typestate() {
    let _new_edge: edge::Edge<edge::NewEdge, edge::Disconnected> = E();
    let _new_val: edge::Edge<edge::NewEdge, edge::DisconnectedVal<u32>> = E().val(42u32);
    let _exist_edge: edge::Edge<edge::ExistEdge, edge::Disconnected> = e();
    let _exist_val: edge::Edge<edge::ExistEdge, edge::DisconnectedVal<u32>> = e().val(42u32);
    let _remove_edge: edge::Edge<edge::RemoveEdge, edge::Disconnected> = !e();
    let _remove_node: node::Node<node::Remove, id::N> = !X(10);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo check --lib`
Expected: FAIL — `.val()` on edges and `!` operator not defined.

**Step 3: Write implementation**

In `src/modify/dsl/edge.rs`, add `.val()`:
```rust
impl Edge<NewEdge, Disconnected> {
    pub fn val<EV>(self, val: EV) -> Edge<NewEdge, DisconnectedVal<EV>> {
        Edge(DisconnectedVal(val), PhantomData)
    }
}

impl Edge<ExistEdge, Disconnected> {
    pub fn val<EV>(self, val: EV) -> Edge<ExistEdge, DisconnectedVal<EV>> {
        Edge(DisconnectedVal(val), PhantomData)
    }
}
```

Add `Not` impl for `!e()`:
```rust
use std::ops::Not;

impl Not for Edge<ExistEdge, Disconnected> {
    type Output = Edge<RemoveEdge, Disconnected>;
    fn not(self) -> Self::Output {
        Edge(Disconnected, PhantomData)
    }
}
```

In `src/modify/dsl/node.rs`, add `Not` impl for `!X()`:
```rust
use std::ops::Not;

impl<E> Not for Node<Exist, Init<id::N, E>> {
    type Output = Node<Remove, id::N>;
    fn not(self) -> Self::Output {
        Node(self.0.id, PhantomData)
    }
}
```

**Step 4: Run test**

Run: `cargo test --lib modify::tests::edge_val_typestate`
Expected: PASS

**Step 5: Commit**

```bash
git add src/modify/
git commit -m "add edge .val() and Not operators for removal"
```

---

### Task 4: TopNode enum + EdgeEntry enum

**Files:**
- Modify: `src/modify/dsl/mod.rs`
- Modify: `src/modify/dsl/node.rs`
- Modify: `src/modify/dsl/edge.rs`

Before implementing operators, we need the enum types that edges and top-level nodes collect into.

**Step 1: Write compile-check test**

```rust
#[test]
fn top_node_enum_collects() {
    use crate::graph::edge;
    let nodes: Vec<dsl::TopNode<(), edge::Undir<()>>> = vec![];
    assert!(nodes.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo check --lib`
Expected: FAIL — `TopNode` doesn't exist.

**Step 3: Write implementation**

In `src/modify/dsl/mod.rs`, define `TopNode` and `EdgeEntry`:
```rust
use crate::{id, Edge};

pub enum TopNode<NV, ER: Edge> {
    NewInit(node::Node<node::New, node::Init<Local, EdgeEntry<NV, ER>>>),
    NewVal(node::Node<node::New, node::WithVal<Local, NV, EdgeEntry<NV, ER>>>),
    ExistInit(node::Node<node::Exist, node::Init<id::N, EdgeEntry<NV, ER>>>),
    ExistVal(node::Node<node::Exist, node::WithVal<id::N, NV, EdgeEntry<NV, ER>>>),
    Remove(node::Node<node::Remove, id::N>),
}

pub enum TargetNode<NV, ER: Edge> {
    NewInit(node::Node<node::New, node::Init<Local, EdgeEntry<NV, ER>>>),
    NewVal(node::Node<node::New, node::WithVal<Local, NV, EdgeEntry<NV, ER>>>),
    NewRef(node::NodeRef<node::New, Local>),
    ExistInit(node::Node<node::Exist, node::Init<id::N, EdgeEntry<NV, ER>>>),
    ExistVal(node::Node<node::Exist, node::WithVal<id::N, NV, EdgeEntry<NV, ER>>>),
    ExistRef(node::NodeRef<node::Exist, id::N>),
}

pub enum ExistTarget<NV, ER: Edge> {
    ExistInit(node::Node<node::Exist, node::Init<id::N, EdgeEntry<NV, ER>>>),
    ExistVal(node::Node<node::Exist, node::WithVal<id::N, NV, EdgeEntry<NV, ER>>>),
    ExistRef(node::NodeRef<node::Exist, id::N>),
}

pub enum EdgeEntry<NV, ER: Edge> {
    New(edge::Connected<NewTarget<NV, ER>, ER>),
    Exist(edge::Connected<ExistTarget<NV, ER>, ER>),
    Remove(edge::RemoveConnected<ExistTarget<NV, ER>>),
    AnonNew(ER::Slot, TargetNode<NV, ER>),
}

pub type NewTarget<NV, ER> = TargetNode<NV, ER>;
```

In `src/modify/dsl/edge.rs`, add `Connected` and `RemoveConnected`:
```rust
pub struct Connected<TargetNode, ER: crate::Edge> {
    pub(crate) slot: ER::Slot,
    pub(crate) val: Option<ER::Val>,
    pub(crate) target: TargetNode,
}

pub struct RemoveConnected<TargetNode> {
    pub(crate) slot_placeholder: (),
    pub(crate) target: TargetNode,
}
```

Add `From` impls to convert concrete node types into `TopNode`, `TargetNode`, and `ExistTarget` variants. Use a macro like the search DSL:
```rust
macro_rules! impl_into_top_node {
    ($variant:ident, $node_ty:ty) => {
        impl<NV, ER: Edge> From<$node_ty> for TopNode<NV, ER> {
            fn from(node: $node_ty) -> Self {
                TopNode::$variant(node)
            }
        }
    };
}
```

**Step 4: Run test**

Run: `cargo test --lib modify::tests::top_node_enum_collects`
Expected: PASS

**Step 5: Commit**

```bash
git add src/modify/
git commit -m "add TopNode, EdgeEntry, TargetNode enums"
```

---

### Task 5: Direction operators on nodes (anonymous new edges)

**Files:**
- Modify: `src/modify/dsl/node.rs`

Implement `>>` (`Shr`), `<<` (`Shl`), `^` (`BitXor`) on `Node<New, Init>`, `Node<New, WithVal>`, `Node<Exist, Init>`, `Node<Exist, WithVal>`. These create anonymous new edges (requires `ER::Val = ()` or some way to default). Bounded on `edge::Src`/`edge::Tgt`/`edge::Und` traits.

**Step 1: Write compile-check test**

```rust
#[test]
fn anonymous_edge_operators_undir() {
    use crate::graph::edge;
    let _: TopNode<(), edge::Undir<()>> = (N(1) ^ N(2)).into();
}

#[test]
fn anonymous_edge_operators_dir() {
    use crate::graph::edge;
    let _: TopNode<(), edge::Dir<()>> = (N(1) >> N(2)).into();
    let _: TopNode<(), edge::Dir<()>> = (N(1) << N(2)).into();
}
```

**Step 2: Run test to verify it fails**

Run: `cargo check --lib`
Expected: FAIL — no `Shr`, `Shl`, `BitXor` impls.

**Step 3: Write implementation**

Use a macro similar to `def_node_connector` from `src/search/dsl/node.rs:334`. The key difference: we push `EdgeEntry::AnonNew(slot, target.into())` into `self.0.edges`.

For each node state (`Init`, `WithVal`) × each kind (`New`, `Exist`):
- Bound `Shr` on `ER: edge::Src` (edge type supports directed-out)
- Bound `Shl` on `ER: edge::Tgt` (edge type supports directed-in)
- Bound `BitXor` on `ER: edge::Und` (edge type supports undirected)

RHS can be any type that converts `Into<TargetNode<NV, ER>>`:
- `Node<New, Init<Local, EdgeEntry<NV, ER>>>` — another new node
- `Node<New, WithVal<Local, NV, EdgeEntry<NV, ER>>>` — new node with val
- `NodeRef<New, Local>` — n(id)
- `Node<Exist, Init<id::N, EdgeEntry<NV, ER>>>` — existing node
- `Node<Exist, WithVal<id::N, NV, EdgeEntry<NV, ER>>>` — existing with val
- `NodeRef<Exist, id::N>` — x(id)

The operator returns `Self` (same type, edges vec has one more entry), but now the edge generic `E` is `EdgeEntry<NV, ER>`. This means `N(1)` initially has `E = ()` but needs to become `E = EdgeEntry<NV, ER>` when first operator is used.

**Key insight:** The node constructors `N(1)` should return `E = EdgeEntry<NV, ER>` from the start (even though the Vec is empty). This means the constructor needs the `NV` and `ER` type parameters resolved. In practice, type inference resolves them when the expression is used in a `Vec<TopNode<NV, ER>>`.

Revise constructors to be generic over `E`:
```rust
pub fn N<NV, ER: Edge>(local: impl Into<Local>) -> Node<New, Init<Local, EdgeEntry<NV, ER>>> {
    Node(Init { id: local.into(), edges: vec![] }, PhantomData)
}
```

Implement operators via macro:
```rust
macro_rules! impl_anon_edge_ops {
    ($kind:ident, $state:ident < $($gen:ident),+ >, [$($rhs:ty),+]) => {
        $(
            impl<NV, ER: Edge + crate::edge::Src> std::ops::Shr<$rhs>
                for Node<$kind, $state<$($gen),+, EdgeEntry<NV, ER>>>
            where ER::Val: Default
            {
                type Output = Self;
                fn shr(mut self, rhs: $rhs) -> Self::Output {
                    let slot = <ER as crate::edge::Src>::SLOT;
                    self.0.edges.push(EdgeEntry::AnonNew(slot, rhs.into()));
                    self
                }
            }
            // similar for Shl (Tgt), BitXor (Und)
        )+
    };
}
```

Note: anonymous edges only work when `ER::Val: Default` (so `()` works automatically). For non-`()` edge values, the user MUST use `& E().val(v)`.

**Step 4: Run test**

Run: `cargo test --lib modify::tests::anonymous_edge_operators_undir`
Expected: PASS

**Step 5: Commit**

```bash
git add src/modify/
git commit -m "add direction operators for anonymous new edges"
```

---

### Task 6: BitAnd (&) operator for explicit edges

**Files:**
- Modify: `src/modify/dsl/node.rs`
- Modify: `src/modify/dsl/edge.rs`

Implement `&` (`BitAnd`) to attach explicit edges to nodes. Edge must be connected first (via `>>`, `<<`, `^` on the edge itself), which transitions `Disconnected` → `Connected`.

**Step 1: Write compile-check test**

```rust
#[test]
fn explicit_edge_bitand() {
    use crate::graph::edge;
    let _: TopNode<(), edge::Undir<u32>> =
        (N(1) & E().val(42u32) ^ N(2)).into();
}

#[test]
fn exist_edge_from_exist_node() {
    use crate::graph::edge;
    let _: TopNode<(), edge::Dir<()>> =
        (X(1) & e() >> X(2)).into();
}

#[test]
fn remove_edge_from_exist_node() {
    use crate::graph::edge;
    let _: TopNode<(), edge::Dir<()>> =
        (X(1) & !e() >> X(2)).into();
}
```

**Step 2: Run test to verify it fails**

Run: `cargo check --lib`

**Step 3: Write implementation**

First, implement direction operators on `Edge` (disconnected → connected). These are on the edge itself, not the node:

```rust
// Edge<NewEdge, Disconnected> >> TargetNode → connected new edge
// Edge<NewEdge, DisconnectedVal<EV>> >> TargetNode → connected new edge with val
// Edge<ExistEdge, Disconnected> >> ExistTarget → connected exist edge
// Edge<ExistEdge, DisconnectedVal<EV>> >> ExistTarget → connected exist edge with val
// Edge<RemoveEdge, Disconnected> >> ExistTarget → connected remove edge
```

Then implement `BitAnd` on nodes:
- `Node<New, _>` can `& ConnectedNewEdge` only
- `Node<Exist, _>` can `& ConnectedNewEdge`, `& ConnectedExistEdge`, `& ConnectedRemoveEdge`

The `&` operator takes a connected edge and pushes it into the node's edges vec as an `EdgeEntry`.

However, the `&` operator has a subtlety: `N(1) & E().val(42) ^ N(2)` — the `&` must return something that the `^` can operate on to complete the edge connection. This means `&` doesn't push a completed edge; instead it returns an intermediate type that the direction operator finalizes.

**Revised approach:** `&` returns a `NodeWithPendingEdge` wrapper:

```rust
pub struct NodeWithPendingEdge<K: Kind, S, EK, ES> {
    node: Node<K, S>,
    edge: Edge<EK, ES>,
}
```

Then `>>` on `NodeWithPendingEdge` connects the edge to the target, pushes it into the node's edges, and returns the `Node`:

```rust
impl Shr<TargetNode> for NodeWithPendingEdge<K, S, NewEdge, DisconnectedVal<EV>> {
    type Output = Node<K, S>;
    fn shr(mut self, rhs: TargetNode) -> Self::Output {
        self.node.0.edges.push(EdgeEntry::New(Connected {
            slot: <ER as Src>::SLOT,
            val: Some(self.edge.0.0),
            target: rhs.into(),
        }));
        self.node
    }
}
```

This means the chain `N(1) & E().val(42) ^ N(2)` parses as:
1. `N(1)` — Node
2. `& E().val(42)` — BitAnd returns NodeWithPendingEdge
3. `^ N(2)` — BitXor on NodeWithPendingEdge, connects edge and returns Node

**Step 4: Run test**

Run: `cargo test --lib modify::tests::explicit_edge_bitand`
Expected: PASS

**Step 5: Commit**

```bash
git add src/modify/
git commit -m "add BitAnd operator and NodeWithPendingEdge for explicit edges"
```

---

### Task 7: From impls + TopNode conversions

**Files:**
- Modify: `src/modify/dsl/mod.rs`
- Modify: `src/modify/dsl/node.rs`

Implement all the `From` conversions so that node types can be collected into `Vec<TopNode<NV, ER>>` and used as operator targets.

**Step 1: Write compile-check test**

```rust
#[test]
fn full_expression_in_vec() {
    use crate::graph::edge;
    let ops: Vec<TopNode<(), edge::Undir<()>>> = vec![
        (N(1) ^ N(2) ^ n(1)).into(),
        (X(5) ^ N(3)).into(),
    ];
    assert_eq!(ops.len(), 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo check --lib`

**Step 3: Write From impls**

Use macros to generate `From` impls for all node/ref variants into `TopNode`, `TargetNode`, and `ExistTarget`:

```rust
macro_rules! impl_from {
    ($source:ty => $target:ident :: $variant:ident [$($gen:tt)*]) => {
        impl<$($gen)*> From<$source> for $target<$($gen)*> {
            fn from(v: $source) -> Self {
                $target::$variant(v)
            }
        }
    };
}
```

**Step 4: Run test**

Run: `cargo test --lib modify::tests::full_expression_in_vec`
Expected: PASS

**Step 5: Commit**

```bash
git add src/modify/
git commit -m "add From impls for TopNode/TargetNode conversions"
```

---

### Task 8: Error types + Fragment Phase 1 validation

**Files:**
- Create: `src/modify/error.rs`
- Create: `src/modify/validate.rs`
- Modify: `src/modify/mod.rs`

**Step 1: Write test**

```rust
#[test]
fn validate_rejects_duplicate_new_node() {
    use crate::graph::edge;
    let ops: Vec<TopNode<(), edge::Undir<()>>> = vec![
        (N(1) ^ N(2)).into(),
        (N(1) ^ N(3)).into(), // duplicate N(1)
    ];
    let result = Fragment::new(ops).validate();
    assert!(matches!(result, Err(FragmentError::DuplicateNewNode(Local(1)))));
}

#[test]
fn validate_rejects_undefined_ref() {
    use crate::graph::edge;
    let ops: Vec<TopNode<(), edge::Undir<()>>> = vec![
        (N(1) ^ n(2)).into(), // n(2) has no N(2)
    ];
    let result = Fragment::new(ops).validate();
    assert!(matches!(result, Err(FragmentError::UndefinedNewRef(Local(2)))));
}

#[test]
fn validate_accepts_valid_fragment() {
    use crate::graph::edge;
    let ops: Vec<TopNode<(), edge::Undir<()>>> = vec![
        (N(1) ^ N(2) ^ n(1)).into(),
    ];
    let result = Fragment::new(ops).validate();
    assert!(result.is_ok());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo check --lib`

**Step 3: Write implementation**

`src/modify/error.rs`:
```rust
use crate::id;
use super::dsl::Local;

#[derive(Debug)]
pub enum FragmentError {
    DuplicateNewNode(Local),
    DuplicateExistNode(id::N),
    UndefinedNewRef(Local),
    UndefinedExistRef(id::N),
    ExistRemoveConflict(id::N),
}

#[derive(Debug)]
pub enum ApplyError {
    NodeNotFound(id::N),
    EdgeNotFound(id::N, id::N),
    CascadeConflict(id::N),
    DuplicateEdge(id::N, id::N),
}

#[derive(Debug)]
pub enum ModifyError {
    Fragment(FragmentError),
    Apply(ApplyError),
}

impl From<FragmentError> for ModifyError {
    fn from(e: FragmentError) -> Self { ModifyError::Fragment(e) }
}
impl From<ApplyError> for ModifyError {
    fn from(e: ApplyError) -> Self { ModifyError::Apply(e) }
}
```

`src/modify/validate.rs` — walks the `TopNode` tree, collects all N/X definitions and n/x references, checks the Phase 1 invariants:
- Build `HashSet<Local>` of defined N ids
- Build `HashSet<id::N>` of defined X ids
- Build `HashSet<id::N>` of removed !X ids
- Collect all n/x references
- Check duplicates, missing refs, exist/remove conflicts

`src/modify/mod.rs` — add `Fragment` type:
```rust
pub mod dsl;
pub mod error;
mod validate;

use std::marker::PhantomData;
use crate::Edge;

pub struct Unchecked;
pub struct Checked;

pub struct Fragment<NV, ER: Edge, Phase> {
    ops: Vec<dsl::TopNode<NV, ER>>,
    _phase: PhantomData<Phase>,
}

impl<NV, ER: Edge> Fragment<NV, ER, Unchecked> {
    pub fn new(ops: Vec<dsl::TopNode<NV, ER>>) -> Self {
        Fragment { ops, _phase: PhantomData }
    }

    pub fn validate(self) -> Result<Fragment<NV, ER, Checked>, error::FragmentError> {
        validate::check_fragment(&self.ops)?;
        Ok(Fragment { ops: self.ops, _phase: PhantomData })
    }
}
```

**Step 4: Run tests**

Run: `cargo test --lib modify::tests`
Expected: PASS

**Step 5: Commit**

```bash
git add src/modify/
git commit -m "add Fragment, error types, Phase 1 validation"
```

---

### Task 9: ModifyResult + Phase 2 validation + apply

**Files:**
- Create: `src/modify/apply.rs`
- Modify: `src/modify/mod.rs`

**Step 1: Write integration test**

```rust
#[test]
fn modify_add_nodes_undir0() {
    use crate::graph;
    use crate::edge::undir::E::U;

    let mut g: graph::Undir0 = vec![U(0, 1)].try_into().unwrap();
    assert_eq!(g.nodes.len(), 2);
    assert_eq!(g.edges.len(), 1);

    let result = g.modify(vec![
        (N(1) ^ N(2)).into(),
    ]).unwrap();

    assert_eq!(g.nodes.len(), 4);
    assert_eq!(g.edges.len(), 2);
    assert_eq!(result.new_node_ids.len(), 2);
}

#[test]
fn modify_pin_existing_add_edge() {
    use crate::graph;
    use crate::edge::undir::E::U;

    let mut g: graph::Undir0 = vec![U(0, 1)].try_into().unwrap();

    let result = g.modify(vec![
        (X(0) ^ N(1)).into(),
    ]).unwrap();

    assert_eq!(g.nodes.len(), 3);
    assert_eq!(g.edges.len(), 2);
}

#[test]
fn modify_remove_node_cascade() {
    use crate::graph;
    use crate::edge::undir::E::U;

    let mut g: graph::Undir0 = vec![U(0, 1), U(1, 2)].try_into().unwrap();
    assert_eq!(g.nodes.len(), 3);
    assert_eq!(g.edges.len(), 2);

    let result = g.modify(vec![
        (!X(1)).into(),
    ]).unwrap();

    assert_eq!(g.nodes.len(), 2);
    assert_eq!(g.edges.len(), 0);
}

#[test]
fn modify_change_node_value() {
    use crate::graph;

    let mut g: graph::UndirN<&str> = (
        vec![(0, "alice"), (1, "bob")],
        vec![crate::edge::undir::E::U(0, 1)],
    ).try_into().unwrap();

    let result = g.modify(vec![
        X(0).val("alice-updated").into(),
    ]).unwrap();

    assert_eq!(*g.get(id::N(0)).unwrap(), "alice-updated");
    assert_eq!(result.swapped_node_vals.len(), 1);
    assert_eq!(result.swapped_node_vals[0].1, "alice");
}

#[test]
fn modify_rejects_nonexistent_pin() {
    use crate::graph;
    use crate::edge::undir::E::U;

    let mut g: graph::Undir0 = vec![U(0, 1)].try_into().unwrap();

    let result = g.modify(vec![
        X(99).into(), // node 99 doesn't exist
    ]);

    assert!(matches!(result, Err(error::ModifyError::Apply(error::ApplyError::NodeNotFound(_)))));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo check --lib`

**Step 3: Write implementation**

`src/modify/mod.rs` — add `ModifyResult` and `Graph::modify`/`Graph::apply`:
```rust
use std::collections::HashMap;
use crate::{id, NR, Graph, Edge};

pub struct ModifyResult<NV, EV> {
    pub new_node_ids: HashMap<dsl::Local, id::N>,
    pub removed_nodes: Vec<(id::N, NV)>,
    pub removed_edges: Vec<(NR<id::N>, EV)>,
    pub swapped_node_vals: Vec<(id::N, NV)>,
    pub swapped_edge_vals: Vec<(NR<id::N>, EV)>,
}

impl<NV: Sync, E: Edge> Graph<NV, E> {
    pub fn apply(
        &mut self,
        fragment: Fragment<NV, E, Checked>,
    ) -> Result<ModifyResult<NV, E::Val>, error::ApplyError> {
        apply::apply(self, fragment.ops)
    }

    pub fn modify(
        &mut self,
        ops: Vec<dsl::TopNode<NV, E>>,
    ) -> Result<ModifyResult<NV, E::Val>, error::ModifyError> {
        let fragment = Fragment::new(ops).validate()?;
        Ok(self.apply(fragment)?)
    }
}
```

`src/modify/apply.rs` — the core mutation logic:
1. Walk expression tree, collect all operations into flat lists:
   - new nodes: `Vec<(Local, Option<NV>)>`
   - new edges: `Vec<(src_id, tgt_id, Slot, Option<EV>)>` (ids resolved to real or local)
   - pin nodes: `Vec<(id::N, Option<NV>)>` (value changes)
   - exist edges: `Vec<(id::N, id::N, Slot, Option<EV>)>` (value changes)
   - remove nodes: `Vec<id::N>`
   - remove edges: `Vec<(id::N, id::N, Slot)>`
2. Phase 2 checks against graph
3. Allocate real ids for new nodes from `graph.nodes.free_ids`
4. Apply mutations in order: add nodes, add edges, change values, remove edges, remove nodes
5. Recompute shapes for affected regions
6. Build and return `ModifyResult`

**Step 4: Run tests**

Run: `cargo test --lib modify::tests`
Expected: PASS

**Step 5: Commit**

```bash
git add src/modify/
git commit -m "add Phase 2 validation, apply logic, ModifyResult"
```

---

### Task 10: Integration tests with all graph types

**Files:**
- Create: `src/modify/tests.rs`

**Step 1: Write comprehensive tests**

```rust
#[test]
fn dir_graph_new_nodes_with_edges() {
    use crate::graph;
    use crate::edge::dir::E::D;

    let mut g: graph::Dir0 = vec![D(0, 1)].try_into().unwrap();

    let result = g.modify(vec![
        (N(1) >> N(2) >> n(1)).into(),
    ]).unwrap();

    let n1 = result.new_node_ids[&Local(1)];
    let n2 = result.new_node_ids[&Local(2)];
    assert!(g.has((n1, n2)));
    assert!(g.has((n2, n1)));
}

#[test]
fn anydir_graph_mixed_edges() {
    use crate::graph;
    use crate::edge::anydir::E::{U, D};

    let mut g: graph::Anydir0 = vec![U(0, 1)].try_into().unwrap();

    let result = g.modify(vec![
        (N(1) ^ N(2)).into(),
        (N(3) >> N(4)).into(),
    ]).unwrap();

    assert_eq!(g.nodes.len(), 6);
}

#[test]
fn valued_graph_full_workflow() {
    use crate::graph;
    use crate::edge::dir;

    let mut g: graph::Dir<String, u32> = (
        vec![(0, "a".into()), (1, "b".into())],
        vec![(dir::E::D(0, 1), 10u32)],
    ).try_into().unwrap();

    // Add new node with valued edge
    let r = g.modify(vec![
        (X(0) & E().val(20u32) >> N(1).val("c".into())).into(),
    ]).unwrap();

    let new_id = r.new_node_ids[&Local(1)];
    assert_eq!(*g.get(new_id).unwrap(), "c");

    // Change value + remove edge
    let r = g.modify(vec![
        (X(0).val("a-updated".into())).into(),
    ]).unwrap();

    assert_eq!(*g.get(id::N(0)).unwrap(), "a-updated");
    assert_eq!(r.swapped_node_vals[0].1, "a");
}

#[test]
fn fragment_reuse() {
    use crate::graph;
    use crate::edge::undir::E::U;

    let mut g1: graph::Undir0 = vec![U(0, 1)].try_into().unwrap();
    let mut g2: graph::Undir0 = vec![U(0, 1), U(1, 2)].try_into().unwrap();

    let fragment = Fragment::new(vec![
        (N(1) ^ N(2)).into(),
    ]).validate().unwrap();

    // Can't reuse fragment directly (consumed by apply), but validates once
    // This tests the Fragment<Checked> typestate
}
```

**Step 2: Run tests**

Run: `cargo test --lib modify`
Expected: PASS

**Step 3: Commit**

```bash
git add src/modify/
git commit -m "add integration tests for all graph types"
```

---

## Notes for implementer

### Key codebase patterns to follow

1. **Edge direction traits** are at `src/graph/edge/mod.rs:38-48` — `Src`, `Tgt`, `Und` with `const SLOT`. Use these for operator bounds instead of creating new traits.

2. **Node construction** pattern is at `src/build/batch.rs` — use `Node::new(id, val)` and manage `free_ids` / `by_id` / adjacencies.

3. **Edge storage** is `BTreeMap<(NR<id::N>, E::Slot), E::Val>` — insert with normalized relations.

4. **Adjacency management** — when adding edges, update `node.adjs.insert(other_n)` for both endpoints. When removing, the adjacency bitset needs updating too.

5. **Shape recomputation** — after mutation, call `nodes.compute_shapes()` to rebuild shape index. See `src/graph/mod.rs:246`.

### Things NOT in search DSL that modify DSL needs

- **Mutation of graph internals** — search only reads, modify writes. Need `pub(crate)` access to `nodes.by_id`, `edges.by_rel`, `nodes.free_ids`.
- **ID allocation** — `free_ids.pop_id()` for new nodes.
- **Value swapping** — `std::mem::replace` on node values and edge values to return old values.

### Don't

- Don't import `log::{info, warn}` directly — use `log` prefix.
- Don't use `id::*` glob imports — always `id::N`.
- Don't modify `src/search/` or any parked modules.
- Don't add comments unless asked.
