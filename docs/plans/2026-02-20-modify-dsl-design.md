# Modify DSL Design

## Overview

A transaction-based DSL for batch graph modification: adding/removing nodes and edges, changing values. Follows the same function/struct + operator overloading pattern as the search DSL, with typestate enforcement of correctness.

## Vocabulary

| Symbol | Constructor | Meaning |
|--------|-------------|---------|
| `N(id)` | New node definition | Creates node with local transaction id |
| `n(id)` | New node reference | References an `N` with same local id |
| `X(id)` | Existing node definition | Pins to graph node, optionally changes value |
| `x(id)` | Existing node reference | References an `X` with same graph id |
| `!X(id)` | Remove existing node | Cascade-deletes edges (unless conflict) |
| `E()` | New edge | Created via operator or `& E().val(v)` |
| `e()` | Existing edge | References existing edge (only from X/x) |
| `!e()` | Remove existing edge | Removes edge (only from X/x) |
| `.val(v)` | Set/change value | On N/E: sets value. On X/e: changes value |
| `>>` | Directed out | Creates/references edge src→tgt |
| `<<` | Directed in | Creates/references edge tgt→src |
| `^` | Undirected | Creates/references undirected edge |
| `&` | Attach edge | Attach explicit edge to node: `node & edge OP target` |

## Constraints

Operator availability depends on graph edge type:
- `Dir<V>`: only `>>` and `<<`
- `Undir<V>`: only `^`
- `Anydir<V>`: all three

Enforced at compile time via `kind::DirOut`, `kind::DirIn`, `kind::Undir` trait bounds.

## Type System

### Identifiers

- `N(id)` / `n(id)`: accept `Id` (u32/u64), wrapped into `Local` internally
- `X(id)` / `x(id)`: accept `Id` or `id::N`, wrapped into `id::N` internally

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Local(pub Id);
```

### Node Kinds

```rust
pub trait Kind {}
pub struct New;      // N - creating new node
pub struct Exist;    // X - referencing existing node
pub struct Remove;   // !X - removing existing node
```

### Node States

Edges live in state, not in Node struct.

```rust
pub struct Node<K: Kind, State>(State, PhantomData<K>);

pub struct Init<Id, E> {
    id: Id,
    edges: Vec<E>,
}

pub struct WithVal<Id, NV, E> {
    id: Id,
    val: NV,
    edges: Vec<E>,
}
```

State transitions:
- `N(1)` -> `Node<New, Init<Local, E>>` (edges empty)
- `N(1).val(v)` -> `Node<New, WithVal<Local, NV, E>>` (can't call .val() again)
- `X(100)` -> `Node<Exist, Init<id::N, E>>`
- `X(100).val(v)` -> `Node<Exist, WithVal<id::N, NV, E>>`
- `!X(100)` -> `Node<Remove, id::N>` (no operators, no .val())

### Node References

```rust
pub struct NodeRef<K: Kind, Id>(Id, PhantomData<K>);
// n(1)   -> NodeRef<New, Local>
// x(100) -> NodeRef<Exist, id::N>
```

### Operator Availability Matrix

|                        | `.val()` | `>> << ^` | `& E()` | `& e()` | `& !e()` | `!` (Not) |
|------------------------|----------|-----------|---------|---------|----------|-----------|
| `Node<New, Init>`      | yes      | yes       | yes     | no      | no       | no        |
| `Node<New, WithVal>`   | no       | yes       | yes     | no      | no       | no        |
| `Node<Exist, Init>`    | yes      | yes       | yes     | yes     | yes      | yes       |
| `Node<Exist, WithVal>` | no       | yes       | yes     | yes     | yes      | no        |
| `Node<Remove, _>`      | no       | no        | no      | no      | no       | no        |

### Edge Types

```rust
pub struct NewEdge;      // E()
pub struct ExistEdge;    // e()
pub struct RemoveEdge;   // !e()

pub struct Disconnected;
pub struct DisconnectedVal<EV>(EV);
pub struct Connected<TargetNode, EV>(Option<EV>, TargetNode);
```

Edge target constraints:

| Edge kind | Allowed targets |
|-----------|----------------|
| NewEdge   | N, n, X, x (any node) |
| ExistEdge | X, x only |
| RemoveEdge| X, x only |

## Two-Phase Validation

### Phase 1: Fragment validation (no graph needed)

Checks DSL expressions in isolation:
- Every `n(local)` has corresponding `N(local)`
- Every `x(id)` has corresponding `X(id)`
- No duplicate `N(local)` definitions
- No duplicate `X(id)` definitions
- `X(id)` and `!X(id)` don't coexist for same id

### Phase 2: Graph-dependent validation

Checks against actual graph:
- `X(id)` node exists in graph
- `e()` edge exists between specified nodes
- `!X` cascade doesn't conflict with edges referenced elsewhere
- New edges don't duplicate existing graph edges

### Fragment Typestate

```rust
pub struct Unchecked;
pub struct Checked;

pub struct Fragment<NV, E, Phase> {
    ops: Vec<TopNode<NV, E>>,
    _phase: PhantomData<Phase>,
}

impl<NV, E> Fragment<NV, E, Unchecked> {
    pub fn new(ops: Vec<TopNode<NV, E>>) -> Self;
    pub fn validate(self) -> Result<Fragment<NV, E, Checked>, FragmentError>;
}
```

## Application

```rust
impl<NV: Sync, E: Edge> Graph<NV, E> {
    // Two-phase explicit
    pub fn apply(
        &mut self,
        fragment: Fragment<NV, E, Checked>,
    ) -> Result<ModifyResult<NV, E::Val>, ApplyError>;

    // Convenience: both phases
    pub fn modify(
        &mut self,
        ops: Vec<TopNode<NV, E>>,
    ) -> Result<ModifyResult<NV, E::Val>, ModifyError>;
}
```

## Result & Error Types

```rust
pub struct ModifyResult<NV: Sync, EV: Sync> {
    pub new_node_ids: HashMap<Local, id::N>,
    pub removed_nodes: Vec<(id::N, NV)>,
    pub removed_edges: Vec<(NR<id::N>, Slot, EV)>,
    pub swapped_node_vals: Vec<(id::N, NV)>,
    pub swapped_edge_vals: Vec<(NR<id::N>, Slot, EV)>,
}

pub enum FragmentError {
    DuplicateNewNode(Local),
    DuplicateExistNode(id::N),
    UndefinedNewRef(Local),
    UndefinedExistRef(id::N),
    ExistRemoveConflict(id::N),
}

pub enum ApplyError {
    NodeNotFound(id::N),
    EdgeNotFound(id::N, id::N),
    CascadeConflict(id::N),
    DuplicateEdge(id::N, id::N),
}

pub enum ModifyError {
    Fragment(FragmentError),
    Apply(ApplyError),
}
```

## Cascade Behavior

`!X(id)` removes the node and all its edges, UNLESS another expression in the same transaction references one of those edges. In that case, validation fails with `CascadeConflict`.

## Usage Examples

```rust
use shagra::modify::dsl::{N, n, X, x, E, e};

// Add new nodes with valued directed edges
let result = graph.modify(vec![
    N(1).val("alice") & E().val(10) >> N(2).val("bob"),
])?;

// Change existing node value + add new edge to new node
let result = graph.modify(vec![
    X(57).val("updated") & E().val(20) >> N(1).val("carol"),
])?;

// Cycle of new nodes
let result = graph.modify(vec![
    N(1).val("x") & E().val(1) >> N(2).val("y") & E().val(2) >> n(1),
])?;

// Remove specific edge + remove node with cascade
let result = graph.modify(vec![
    X(57) & !e() >> X(58),
    !X(59),
])?;

// Change existing edge value
let result = graph.modify(vec![
    X(57) & e().val(99) >> X(58),
])?;

// Undir0 graph (no values)
let result = g.modify(vec![
    N(1) ^ N(2) ^ n(1),
    X(5) ^ N(3),
])?;

// Two-phase validation
let fragment = Fragment::new(vec![
    N(1).val("test") >> N(2).val("test2"),
]).validate()?;
graph.apply(fragment)?;
```

## Module Structure

```
src/modify/
  mod.rs          - Fragment, ModifyResult, error types, Graph::modify/apply
  dsl/
    mod.rs        - TopNode enum, Local type, re-exports
    node.rs       - Node<K, S>, NodeRef, constructors N/n/X/x, operators
    edge.rs       - Edge<K, S>, constructors E/e, operators
  validate.rs     - Phase 1: fragment-only validation
  apply.rs        - Phase 2: graph validation + mutation
```
