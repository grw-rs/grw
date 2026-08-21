# Search Module Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a graph morphism search engine with composed graph patterns (per-cluster morphism types), shape-aware candidate pruning, and a compiled-query model.

**Architecture:** DSL layer (`search!` macro with `get`/`ban` clusters, `N`/`n`/`X`/`x`/`E` constructors) compiles into a `Query` struct with precomputed search plan. At search time, shape-aware domains are computed per pattern node, and a stack-based backtracking engine yields `Match` results via an iterator.

**Tech Stack:** Rust, shagra's existing `Graph`/`Shapes`/`Edge` types, `FxHashMap`/`IdSet` (roaring bitmaps) from `collections`.

**Design doc:** `docs/plans/2026-03-02-search-module-design.md`

---

## Phase 1: Foundation — Types, DSL, and Validation

Wire the `search` module into `lib.rs`, define core enums/structs, build the DSL with cluster syntax, and validate patterns at compile time. No search engine yet — just pattern construction and validation.

### Task 1: Wire search module into lib.rs

**Files:**
- Create: `src/search/mod.rs`
- Modify: `src/lib.rs`

**Step 1: Create empty module**

Create `src/search/mod.rs`:

```rust
pub mod error;
```

Create `src/search/error.rs`:

```rust
use crate::Id;

#[derive(Debug)]
pub enum Pattern {
    DuplicateLocalId(Id),
    UndefinedRef(Id),
    ContextOnlyInBan(Id),
    NodeNotInAnyCluster(Id),
    InvalidClusterConflict { ban_cluster: usize, get_cluster: usize },
}
```

**Step 2: Wire into lib.rs**

Add `pub mod search;` to `src/lib.rs` after `pub mod modify;`.

**Step 3: Verify**

Run: `cargo check`
Expected: compiles with no errors.

**Step 4: Commit**

```
search: wire empty search module into lib.rs
```

---

### Task 2: Core enums — Morphism, Decision, Cluster

**Files:**
- Modify: `src/search/mod.rs`

**Step 1: Write test for morphism ordering**

Add to `src/search/mod.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Morphism {
    Iso,
    SubIso,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Get,
    Ban,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morphism_ordering_iso_most_restrictive() {
        assert!(Morphism::Iso < Morphism::SubIso);
        assert!(Morphism::SubIso < Morphism::Mono);
    }
}
```

The ordering encodes matching power: Iso < SubIso < Mono. This ordering is used in invalid pattern detection — a ban cluster invalidates a get cluster when `ban.morphism >= get.morphism`.

**Step 2: Verify**

Run: `cargo test --lib search::tests`

**Step 3: Commit**

```
search: add Morphism and Decision enums
```

---

### Task 3: DSL module — cluster IR and node/edge types

**Files:**
- Create: `src/search/dsl/mod.rs`
- Create: `src/search/dsl/node.rs`
- Create: `src/search/dsl/edge.rs`
- Modify: `src/search/mod.rs`

**Step 1: Create the DSL IR types**

The search DSL follows the same pattern as `build/dsl` and `modify/dsl`: constructor functions produce IR types, operator overloading (`>>`, `<<`, `^`) chains edges onto nodes, and a top-level function collects everything.

Create `src/search/dsl/mod.rs`:

```rust
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

pub use crate::graph::dsl::{HasVal, LocalId};
pub(crate) use crate::graph::dsl::{IntoOptional, IntoVal};

use crate::graph;
use crate::id;
use crate::search::{Decision, Morphism};
use std::marker::PhantomData;

pub struct UndirPending<N, E>(pub N, pub E);

pub enum Op<NV, ER: graph::Edge> {
    Free {
        id: Option<LocalId>,
        val: NV,
        edges: Vec<EdgeOp<NV, ER>>,
    },
    FreeRef {
        id: LocalId,
        edges: Vec<EdgeOp<NV, ER>>,
    },
    Context {
        id: LocalId,
        edges: Vec<EdgeOp<NV, ER>>,
    },
    ContextRef {
        id: LocalId,
        edges: Vec<EdgeOp<NV, ER>>,
    },
}

pub struct EdgeOp<NV, ER: graph::Edge> {
    pub(crate) slot: ER::Slot,
    pub(crate) val: ER::Val,
    pub(crate) target: Op<NV, ER>,
}

pub(crate) trait IntoOp<NV, ER: graph::Edge> {
    fn into_op(self) -> Op<NV, ER>;
}

pub struct ClusterOps<NV, ER: graph::Edge> {
    pub(crate) morphism: Morphism,
    pub(crate) decision: Decision,
    pub(crate) ops: Vec<Op<NV, ER>>,
}

#[allow(non_snake_case)]
pub fn N<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::Free<NV, (), ER> {
    node::Free {
        id: Some(local.into()),
        v: (),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn N_<NV, ER: graph::Edge>() -> node::Free<NV, (), ER> {
    node::Free {
        id: None,
        v: (),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn n<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::FreeRef<NV, ER> {
    node::FreeRef {
        id: local.into(),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn X<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::Context<NV, ER> {
    node::Context {
        id: local.into(),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn x<NV, ER: graph::Edge>(local: impl Into<LocalId>) -> node::ContextRef<NV, ER> {
    node::ContextRef {
        id: local.into(),
        edges: Vec::new(),
    }
}

#[allow(non_snake_case)]
pub fn E<NV, ER: graph::Edge>() -> edge::Edge<(), NV, ER> {
    edge::Edge((), PhantomData)
}

pub fn get<NV, ER: graph::Edge>(morphism: Morphism, ops: Vec<Op<NV, ER>>) -> ClusterOps<NV, ER> {
    ClusterOps { morphism, decision: Decision::Get, ops }
}

pub fn ban<NV, ER: graph::Edge>(morphism: Morphism, ops: Vec<Op<NV, ER>>) -> ClusterOps<NV, ER> {
    ClusterOps { morphism, decision: Decision::Ban, ops }
}
```

Note: `X(id)` and `x(id)` take `LocalId` (not `id::N`) — the context node is identified by a pattern-local ID. The actual target node mapping is provided at bind time via `Context`.

**Step 2: Create node types**

Create `src/search/dsl/node.rs` — mirrors `build/dsl/node.rs` but with four node variants (Free, FreeRef, Context, ContextRef):

```rust
use super::edge::{self, Connected};
use super::{EdgeOp, IntoOp, Op, UndirPending};
use crate::graph::dsl::{HasVal, IntoVal, LocalId};
use crate::graph;
use crate::graph::edge::{Src, Tgt, Und};
use std::ops::{BitAnd, BitXor, Shl, Shr};

pub struct Free<NV, V, ER: graph::Edge> {
    pub(crate) id: Option<LocalId>,
    pub(crate) v: V,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

pub struct FreeRef<NV, ER: graph::Edge> {
    pub(crate) id: LocalId,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

pub struct Context<NV, ER: graph::Edge> {
    pub(crate) id: LocalId,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

pub struct ContextRef<NV, ER: graph::Edge> {
    pub(crate) id: LocalId,
    pub(crate) edges: Vec<EdgeOp<NV, ER>>,
}

impl<NV, ER: graph::Edge> Free<NV, (), ER> {
    pub fn val(self, v: NV) -> Free<NV, HasVal<NV>, ER> {
        Free {
            id: self.id,
            v: HasVal(v),
            edges: self.edges,
        }
    }
}

impl<NV, V: IntoVal<NV>, ER: graph::Edge> From<Free<NV, V, ER>> for Op<NV, ER> {
    fn from(n: Free<NV, V, ER>) -> Self {
        Op::Free {
            id: n.id,
            val: n.v.into_val(),
            edges: n.edges,
        }
    }
}

impl<NV, V: IntoVal<NV>, ER: graph::Edge> IntoOp<NV, ER> for Free<NV, V, ER> {
    fn into_op(self) -> Op<NV, ER> { self.into() }
}

impl<NV, ER: graph::Edge> From<FreeRef<NV, ER>> for Op<NV, ER> {
    fn from(n: FreeRef<NV, ER>) -> Self {
        Op::FreeRef { id: n.id, edges: n.edges }
    }
}

impl<NV, ER: graph::Edge> IntoOp<NV, ER> for FreeRef<NV, ER> {
    fn into_op(self) -> Op<NV, ER> { self.into() }
}

impl<NV, ER: graph::Edge> From<Context<NV, ER>> for Op<NV, ER> {
    fn from(n: Context<NV, ER>) -> Self {
        Op::Context { id: n.id, edges: n.edges }
    }
}

impl<NV, ER: graph::Edge> IntoOp<NV, ER> for Context<NV, ER> {
    fn into_op(self) -> Op<NV, ER> { self.into() }
}

impl<NV, ER: graph::Edge> From<ContextRef<NV, ER>> for Op<NV, ER> {
    fn from(n: ContextRef<NV, ER>) -> Self {
        Op::ContextRef { id: n.id, edges: n.edges }
    }
}

impl<NV, ER: graph::Edge> IntoOp<NV, ER> for ContextRef<NV, ER> {
    fn into_op(self) -> Op<NV, ER> { self.into() }
}

macro_rules! impl_anon_edge_op {
    ($Self:ty, $V:ident, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, $V, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>> $Op<RHS> for $Self
        where ER::Val: Default,
        {
            type Output = Self;
            fn $op(mut self, rhs: RHS) -> Self {
                self.edges.push(EdgeOp {
                    slot: <ER as $Dir>::SLOT,
                    val: ER::Val::default(),
                    target: rhs.into_op(),
                });
                self
            }
        }
    };
    ($Self:ty, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>> $Op<RHS> for $Self
        where ER::Val: Default,
        {
            type Output = Self;
            fn $op(mut self, rhs: RHS) -> Self {
                self.edges.push(EdgeOp {
                    slot: <ER as $Dir>::SLOT,
                    val: ER::Val::default(),
                    target: rhs.into_op(),
                });
                self
            }
        }
    };
}

for_each_dir!(impl_anon_edge_op!(Free<NV, V, ER>, V));
for_each_dir!(impl_anon_edge_op!(FreeRef<NV, ER>));
for_each_dir!(impl_anon_edge_op!(Context<NV, ER>));
for_each_dir!(impl_anon_edge_op!(ContextRef<NV, ER>));

macro_rules! impl_undir_op {
    ($NodeTy:ty, $V:ident, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, $V, EV: IntoVal<ER::Val>, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>>
            $Op<RHS> for UndirPending<$NodeTy, edge::Edge<EV, NV, ER>>
        {
            type Output = $NodeTy;
            fn $op(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(EdgeOp {
                    slot: <ER as $Dir>::SLOT,
                    val: self.1 .0.into_val(),
                    target: rhs.into_op(),
                });
                node
            }
        }
    };
    ($NodeTy:ty, $Dir:ident, $Op:ident, $op:ident) => {
        impl<NV, EV: IntoVal<ER::Val>, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>>
            $Op<RHS> for UndirPending<$NodeTy, edge::Edge<EV, NV, ER>>
        {
            type Output = $NodeTy;
            fn $op(self, rhs: RHS) -> $NodeTy {
                let mut node = self.0;
                node.edges.push(EdgeOp {
                    slot: <ER as $Dir>::SLOT,
                    val: self.1 .0.into_val(),
                    target: rhs.into_op(),
                });
                node
            }
        }
    };
}

for_each_dir!(impl_undir_op!(Free<NV, V, ER>, V));
for_each_dir!(impl_undir_op!(FreeRef<NV, ER>));
for_each_dir!(impl_undir_op!(Context<NV, ER>));
for_each_dir!(impl_undir_op!(ContextRef<NV, ER>));

macro_rules! impl_bitand_connected {
    ($Self:ty, $V:ident) => {
        impl<NV, $V, ER: graph::Edge> BitAnd<edge::Edge<Connected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: edge::Edge<Connected<NV, ER>, NV, ER>) -> Self {
                self.edges.push(EdgeOp {
                    slot: arm.0.slot,
                    val: arm.0.val,
                    target: arm.0.target,
                });
                self
            }
        }
    };
    ($Self:ty) => {
        impl<NV, ER: graph::Edge> BitAnd<edge::Edge<Connected<NV, ER>, NV, ER>> for $Self {
            type Output = Self;
            fn bitand(mut self, arm: edge::Edge<Connected<NV, ER>, NV, ER>) -> Self {
                self.edges.push(EdgeOp {
                    slot: arm.0.slot,
                    val: arm.0.val,
                    target: arm.0.target,
                });
                self
            }
        }
    };
}

impl_bitand_connected!(Free<NV, V, ER>, V);
impl_bitand_connected!(FreeRef<NV, ER>);
impl_bitand_connected!(Context<NV, ER>);
impl_bitand_connected!(ContextRef<NV, ER>);

macro_rules! impl_bitand_undir_pending {
    ($Self:ty, $EdgeTy:ty, $V:ident) => {
        impl<NV, $V, ER: graph::Edge> BitAnd<$EdgeTy> for $Self {
            type Output = UndirPending<Self, $EdgeTy>;
            fn bitand(self, edge: $EdgeTy) -> Self::Output {
                UndirPending(self, edge)
            }
        }
    };
    ($Self:ty, $EdgeTy:ty) => {
        impl<NV, ER: graph::Edge> BitAnd<$EdgeTy> for $Self {
            type Output = UndirPending<Self, $EdgeTy>;
            fn bitand(self, edge: $EdgeTy) -> Self::Output {
                UndirPending(self, edge)
            }
        }
    };
}

impl_bitand_undir_pending!(Free<NV, V, ER>, edge::Edge<(), NV, ER>, V);
impl_bitand_undir_pending!(Free<NV, V, ER>, edge::Edge<HasVal<ER::Val>, NV, ER>, V);
impl_bitand_undir_pending!(FreeRef<NV, ER>, edge::Edge<(), NV, ER>);
impl_bitand_undir_pending!(FreeRef<NV, ER>, edge::Edge<HasVal<ER::Val>, NV, ER>);
impl_bitand_undir_pending!(Context<NV, ER>, edge::Edge<(), NV, ER>);
impl_bitand_undir_pending!(Context<NV, ER>, edge::Edge<HasVal<ER::Val>, NV, ER>);
impl_bitand_undir_pending!(ContextRef<NV, ER>, edge::Edge<(), NV, ER>);
impl_bitand_undir_pending!(ContextRef<NV, ER>, edge::Edge<HasVal<ER::Val>, NV, ER>);
```

**Step 3: Create edge types**

Create `src/search/dsl/edge.rs` — identical to `build/dsl/edge.rs`:

```rust
use super::{IntoOp, Op};
use crate::graph::dsl::{HasVal, IntoVal};
use crate::graph;
use crate::graph::edge::{Src, Tgt, Und};
use std::marker::PhantomData;
use std::ops::{BitXor, Shl, Shr};

pub struct Edge<EV, NV, ER: graph::Edge>(pub(crate) EV, pub(crate) PhantomData<(NV, ER)>);

pub struct Connected<NV, ER: graph::Edge> {
    pub(crate) slot: ER::Slot,
    pub(crate) val: ER::Val,
    pub(crate) target: Op<NV, ER>,
}

impl<NV, ER: graph::Edge> Edge<(), NV, ER> {
    pub fn val(self, v: impl Into<ER::Val>) -> Edge<HasVal<ER::Val>, NV, ER> {
        Edge(HasVal(v.into()), PhantomData)
    }
}

macro_rules! impl_connect_op {
    ($Dir:ident, $Op:ident, $op:ident) => {
        impl<EV: IntoVal<ER::Val>, NV, ER: graph::Edge + $Dir, RHS: IntoOp<NV, ER>>
            $Op<RHS> for Edge<EV, NV, ER>
        {
            type Output = Edge<Connected<NV, ER>, NV, ER>;
            fn $op(self, rhs: RHS) -> Self::Output {
                Edge(
                    Connected {
                        slot: <ER as $Dir>::SLOT,
                        val: self.0.into_val(),
                        target: rhs.into_op(),
                    },
                    PhantomData,
                )
            }
        }
    };
}

for_each_dir!(impl_connect_op!());
```

**Step 4: Wire DSL module**

Add to `src/search/mod.rs`:

```rust
pub mod dsl;
pub mod error;

// re-exports
pub use dsl::{Morphism, Decision};
```

Move `Morphism` and `Decision` from `mod.rs` into `dsl/mod.rs` and re-export from `search/mod.rs`.

**Step 5: Verify**

Run: `cargo check`

**Step 6: Commit**

```
search: add DSL module with node/edge types and operator overloading
```

---

### Task 4: The `search!` macro

**Files:**
- Modify: `src/search/mod.rs`

**Step 1: Define the macro**

The `search!` macro needs to handle cluster syntax: `get(Morphism) { exprs }` and `ban(Morphism) { exprs }`. Each cluster body is a comma-separated list of node/edge expressions (same as `graph!`).

```rust
#[macro_export]
macro_rules! search {
    [<$nv:ty, $er:ty> $($body:tt)*] => {{
        #[allow(unused_imports)]
        use $crate::search::dsl::*;
        $crate::search::dsl::compile::<$nv, $er>(
            $crate::search_clusters!($($body)*)
        )
    }};
    [$($body:tt)*] => {{
        #[allow(unused_imports)]
        use $crate::search::dsl::*;
        $crate::search::dsl::compile(
            $crate::search_clusters!($($body)*)
        )
    }};
}

#[macro_export]
macro_rules! search_clusters {
    (@cluster get($m:expr) { $($expr:expr),* $(,)? } $($rest:tt)*) => {{
        let mut clusters = vec![
            $crate::search::dsl::get($m, vec![$($expr.into()),*])
        ];
        clusters.extend($crate::search_clusters!($($rest)*));
        clusters
    }};
    (@cluster ban($m:expr) { $($expr:expr),* $(,)? } $($rest:tt)*) => {{
        let mut clusters = vec![
            $crate::search::dsl::ban($m, vec![$($expr.into()),*])
        ];
        clusters.extend($crate::search_clusters!($($rest)*));
        clusters
    }};
    (get $($rest:tt)*) => { $crate::search_clusters!(@cluster get $($rest)*) };
    (ban $($rest:tt)*) => { $crate::search_clusters!(@cluster ban $($rest)*) };
    (, get $($rest:tt)*) => { $crate::search_clusters!(@cluster get $($rest)*) };
    (, ban $($rest:tt)*) => { $crate::search_clusters!(@cluster ban $($rest)*) };
    () => { vec![] };
}
```

Note: the exact macro syntax may need iteration to get right with the Rust macro parser. The key behavior is that `search!` expands cluster blocks into `Vec<ClusterOps>`, then calls `compile()`.

**Step 2: Write test**

```rust
#[test]
fn search_macro_builds_clusters() {
    use crate::graph::edge;
    let clusters: Vec<dsl::ClusterOps<(), edge::Undir<()>>> = search_clusters!(
        get(Morphism::SubIso) {
            N(0) >> E() >> N(1)
        },
        ban(Morphism::Mono) {
            n(0) >> E() >> N(2)
        }
    );
    assert_eq!(clusters.len(), 2);
    assert_eq!(clusters[0].decision, Decision::Get);
    assert_eq!(clusters[0].morphism, Morphism::SubIso);
    assert_eq!(clusters[1].decision, Decision::Ban);
    assert_eq!(clusters[1].morphism, Morphism::Mono);
}
```

**Step 3: Verify**

Run: `cargo test --lib search::tests`

**Step 4: Commit**

```
search: add search! macro with cluster syntax
```

---

### Task 5: Pattern validation

**Files:**
- Create: `src/search/dsl/validate.rs`
- Modify: `src/search/dsl/mod.rs`

**Step 1: Write validation tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Helper: make a ClusterOps with specified node IDs
    // (actual test helpers will construct real Op trees)

    #[test]
    fn valid_single_get_cluster() { ... }

    #[test]
    fn invalid_context_only_in_ban() { ... }

    #[test]
    fn invalid_mono_ban_subsumes_sub_iso_get() { ... }

    #[test]
    fn valid_sub_iso_ban_with_mono_get() { ... }

    #[test]
    fn invalid_node_not_in_any_cluster() { ... }
}
```

**Step 2: Implement validation**

Create `src/search/dsl/validate.rs`. The validation walks all cluster ops, collects:

- All free node definitions (N/N_) and their cluster membership
- All free node references (n) and which cluster they appear in
- All context node definitions (X) and their cluster membership
- All context node references (x)

Then checks:

1. No duplicate local IDs within a cluster
2. All `n()`/`x()` refs point to defined `N()`/`X()` nodes
3. Every node belongs to at least one cluster
4. Context nodes (`X`) appear in at least one `get` cluster
5. Invalidity detection: for every pair of clusters sharing elements where one is `ban` and one is `get`, check that `ban.morphism < get.morphism` (otherwise invalid)

```rust
pub(crate) fn check_pattern<NV, ER: graph::Edge>(
    clusters: &[ClusterOps<NV, ER>],
) -> Result<(), crate::search::error::Pattern> { ... }
```

**Step 3: Wire into dsl/mod.rs**

```rust
mod validate;
pub(crate) use validate::check_pattern;
```

**Step 4: Verify**

Run: `cargo test --lib search`

**Step 5: Commit**

```
search: add pattern validation with invalidity detection
```

---

### Task 6: Compile function — ClusterOps IR to compiled Query

**Files:**
- Create: `src/search/query/mod.rs`
- Create: `src/search/query/compile.rs`
- Modify: `src/search/mod.rs`

**Step 1: Define Query type**

In `src/search/query/mod.rs`:

```rust
pub(crate) mod compile;

use crate::graph;
use crate::collections::FxHashMap;
use crate::{Id, IdSet, id};
use crate::search::{Morphism, Decision};
use std::marker::PhantomData;

pub struct Unbound;
pub struct Ready;

pub struct Cluster {
    pub(crate) nodes: IdSet<id::N>,
    pub(crate) edges: Vec<crate::NR<id::N>>,
    pub(crate) morphism: Morphism,
    pub(crate) decision: Decision,
}

pub struct Query<NV: Sync, ER: graph::Edge, State> {
    pub(crate) pattern: graph::Graph<NV, ER>,
    pub(crate) clusters: Vec<Cluster>,
    pub(crate) context_nodes: IdSet<id::N>,
    pub(crate) _state: PhantomData<State>,
}
```

**Step 2: Implement compile**

In `src/search/query/compile.rs`:

The compile function:
1. Calls `check_pattern()` for validation
2. Walks all cluster ops, collects IDs (same flattening logic as `build/dsl/mod.rs:from_fragment`)
3. Builds a `Graph<NV, ER>` from the combined nodes+edges (calls `nodes.compute_shapes()`)
4. Constructs `Cluster` structs with `IdSet` membership
5. Determines whether any context nodes exist → returns `Query<.., Unbound>` or `Query<.., Ready>`

```rust
pub fn compile<NV: Sync, ER: graph::Edge>(
    cluster_ops: Vec<ClusterOps<NV, ER>>,
) -> Result<Query<NV, ER, ???>, crate::search::error::Pattern> { ... }
```

The typestate selection (Unbound vs Ready) is determined by whether any `X()` nodes are present. This requires two separate return paths or a builder that resolves the type.

**Step 3: Write test**

```rust
#[test]
fn compile_single_get_cluster_ready() {
    let q = search![
        get(Morphism::SubIso) {
            N(0) >> E() >> N(1)
        }
    ].unwrap();
    assert_eq!(q.pattern.node_count(), 2);
    assert_eq!(q.pattern.edge_count(), 1);
    assert_eq!(q.clusters.len(), 1);
}
```

**Step 4: Verify**

Run: `cargo test --lib search`

**Step 5: Commit**

```
search: compile cluster IR into Query with pattern graph
```

---

## Phase 2: Backtracking Engine — Core Search

Build the search engine incrementally: first graph isomorphism (simplest), then subgraph isomorphism, then monomorphism. Single `get` cluster only — no `ban` clusters yet.

### Task 7: Match type and Matches iterator skeleton

**Files:**
- Create: `src/search/engine/mod.rs`
- Create: `src/search/engine/backtrack.rs`
- Modify: `src/search/mod.rs`

**Step 1: Define Match and Matches**

```rust
// engine/mod.rs
pub mod backtrack;

use crate::collections::FxHashMap;
use crate::{Id, id};

pub struct Match {
    pub(crate) node_map: FxHashMap<Id, id::N>,
}

impl Match {
    pub fn node(&self, pattern_local: Id) -> id::N {
        *self.node_map.get(&pattern_local).unwrap()
    }
}
```

```rust
// engine/backtrack.rs
use crate::graph;
use crate::search::query::Query;
use super::Match;

pub struct Matches<'g, NV: Sync, ER: graph::Edge> {
    query: &'g Query<NV, ER, crate::search::query::Ready>,
    target: &'g graph::Graph<NV, ER>,
    stack: Vec<StackFrame>,
    mapping: crate::collections::FxHashMap<crate::Id, crate::id::N>,
    reverse: crate::collections::FxHashMap<crate::id::N, crate::Id>,
    exhausted: bool,
}

struct StackFrame {
    depth: usize,
    candidate_idx: usize,
    candidates: Vec<crate::id::N>,
}

impl<'g, NV: Sync, ER: graph::Edge> Iterator for Matches<'g, NV, ER> {
    type Item = Match;
    fn next(&mut self) -> Option<Match> {
        if self.exhausted { return None; }
        unimplemented!("backtracking search")
    }
}
```

**Step 2: Add find_in to Query**

```rust
impl<NV: Sync, ER: graph::Edge> Query<NV, ER, Ready> {
    pub fn find_in<'g>(&'g self, graph: &'g graph::Graph<NV, ER>) -> Matches<'g, NV, ER> {
        Matches::new(self, graph)
    }
}
```

**Step 3: Commit**

```
search: add Match type and Matches iterator skeleton
```

---

### Task 8: Graph isomorphism — simplest morphism

**Files:**
- Modify: `src/search/engine/backtrack.rs`

**Step 1: Write failing test**

```rust
#[test]
fn iso_identical_triangle() {
    let target = graph![N(0) >> E() >> N(1) >> E() >> N(2), n(0) >> E() >> n(2)].unwrap();
    let pattern = graph![N(0) >> E() >> N(1) >> E() >> N(2), n(0) >> E() >> n(2)].unwrap();
    // ... construct Query with Iso morphism, find_in target
    // should find matches (6 for a triangle — 3! permutations)
}

#[test]
fn iso_no_match_different_sizes() {
    // 3-node pattern vs 4-node target → zero matches
}
```

**Step 2: Implement backtracking**

The core algorithm in `backtrack.rs`:

1. **Ordering**: Pattern nodes in order of ID (simple for now — Task 13 adds smart ordering)
2. **Candidates**: For Iso, all target nodes not yet in `reverse` mapping
3. **Feasibility**: For each candidate, check that edges to already-mapped pattern neighbors exist in the target with correct slots. For Iso, also check no extra edges exist.
4. **Yield**: When all pattern nodes mapped, yield Match. Then backtrack for next.

**Step 3: Run test**

Run: `cargo test --lib search`

**Step 4: Commit**

```
search: implement graph isomorphism via backtracking
```

---

### Task 9: Subgraph isomorphism (induced)

**Files:**
- Modify: `src/search/engine/backtrack.rs`
- Create: `src/search/engine/feasibility.rs`

**Step 1: Write failing test**

```rust
#[test]
fn sub_iso_triangle_in_k4() {
    // K4 target (4 nodes, all connected), triangle pattern
    // SubIso: should find 4*3*2 = 24 matches (pick 3 nodes from 4, all orderings)
}

#[test]
fn sub_iso_edge_in_path() {
    // Path 0-1-2, pattern is single edge 0-1
    // SubIso: node 1 in pattern matched to node 1 in target has degree 2
    //         but pattern node 1 has degree 1 → sub-iso requires no extra edges
    //         between matched nodes. Since we only matched 2 nodes and node 1
    //         has an edge to node 2 (not matched), that's fine.
    // Should find: (0→0,1→1), (0→1,1→0), (0→1,1→2), (0→2,1→1) = 4 matches
}
```

**Step 2: Extract feasibility module**

Move feasibility checks into `feasibility.rs`:

```rust
pub(crate) fn check_feasibility(
    pattern_node: id::N,
    candidate: id::N,
    morphism: Morphism,
    mapping: &FxHashMap<Id, id::N>,
    reverse: &FxHashMap<id::N, Id>,
    pattern: &Graph,
    target: &Graph,
) -> bool { ... }
```

For SubIso: pattern edges to already-mapped nodes must exist, AND no extra target edges between already-mapped nodes that don't correspond to pattern edges.

**Step 3: Verify**

Run: `cargo test --lib search`

**Step 4: Commit**

```
search: implement subgraph isomorphism with feasibility checks
```

---

### Task 10: Monomorphism (non-induced)

**Files:**
- Modify: `src/search/engine/feasibility.rs`

**Step 1: Write failing test**

```rust
#[test]
fn mono_star_matches_clique() {
    // K4 target, star pattern (center + 3 spokes, no spoke-spoke edges)
    // SubIso would reject (extra edges between spokes in K4)
    // Mono should find matches (extra edges allowed)
}

#[test]
fn mono_vs_sub_iso_count() {
    // For any pattern/target pair: mono matches >= sub_iso matches
}
```

**Step 2: Implement**

Mono feasibility: only check that required pattern edges exist in the target. Don't check for extra edges. This is the simplest feasibility check — a subset of SubIso checks.

**Step 3: Verify**

Run: `cargo test --lib search`

**Step 4: Commit**

```
search: implement monomorphism feasibility
```

---

## Phase 3: Clusters and Ban Logic

### Task 11: Multi-cluster get patterns

**Files:**
- Modify: `src/search/engine/backtrack.rs`

**Step 1: Write failing test**

```rust
#[test]
fn two_get_clusters_different_morphisms() {
    // Target: K4 with one extra node connected to one K4 member
    // Cluster 1 (SubIso): triangle
    // Cluster 2 (Mono): edge from shared node to something
    // The shared node links the clusters
}
```

**Step 2: Implement multi-cluster feasibility**

When checking a candidate for a pattern node, check feasibility against ALL clusters the node belongs to. A pattern node in both a SubIso cluster and a Mono cluster must pass both checks (SubIso is stricter, so it dominates — but both must be checked independently because they may have different edge sets).

**Step 3: Verify and commit**

```
search: support multi-cluster get patterns with per-cluster feasibility
```

---

### Task 12: Ban clusters — connected and disconnected

**Files:**
- Modify: `src/search/engine/backtrack.rs`

**Step 1: Write tests**

```rust
#[test]
fn connected_ban_rejects_extra_edge() {
    // Target: triangle 0-1-2 plus edge 0-3
    // Get(SubIso): triangle {N(0), N(1), N(2)}
    // Ban(Mono): n(0) >> E() >> N(3)  — forbid extra neighbor of node 0
    // Should find 0 matches (every triangle mapping has node 0 mapped to
    // some target node, and that target node always has the extra edge)
}

#[test]
fn disconnected_ban_precondition() {
    // Target: triangle 0-1-2 plus separate triangle 3-4-5
    // Get(SubIso): triangle {N(0), N(1), N(2)}
    // Ban(SubIso): triangle {N(6), N(7), N(8)} — forbid ANY second triangle
    // Disconnected ban runs first as precondition → finds the second triangle → 0 matches
}

#[test]
fn ban_doesnt_fire_when_structure_absent() {
    // Target: single triangle 0-1-2
    // Get(SubIso): triangle
    // Ban(SubIso): separate triangle (disconnected)
    // Only one triangle exists, ban can't find a second → matches succeed
}
```

**Step 2: Implement**

During search plan construction, classify each ban cluster as connected or disconnected. Disconnected bans run as preconditions before the main search. Connected bans are checked eagerly when all their nodes have assignments.

In the backtracking loop:
1. Before starting main search, run each disconnected ban cluster as a sub-search. If any finds a match → return empty iterator.
2. During main search, after assigning a candidate: for each connected ban cluster where all nodes are now assigned, run a local feasibility check. If the ban's structure is found → backtrack.

**Step 3: Verify and commit**

```
search: implement ban clusters with eager checking and preconditions
```

---

### Task 13: "Exactly N" pattern integration test

**Files:**
- Add tests

```rust
#[test]
fn exactly_two_triangles() {
    // Target with exactly 2 non-overlapping triangles: {0,1,2} and {3,4,5}
    // 2 get(SubIso) triangles + 1 ban(SubIso) triangle
    // Should find matches (6 * 6 * 2 = 72 — permutations of each triangle × which get maps where)
    // Verify count against manual calculation
}

#[test]
fn exactly_two_triangles_but_three_exist() {
    // Target with 3 triangles → ban fires for 3rd → 0 matches
}
```

**Commit:**

```
search: add exactly-N pattern integration tests
```

---

## Phase 4: Context Nodes and Typestate

### Task 14: Context binding and BoundQuery

**Files:**
- Create: `src/search/engine/context.rs`
- Modify: `src/search/query/mod.rs`

**Step 1: Implement Context and BoundQuery**

```rust
// engine/context.rs
pub struct Context {
    pub(crate) bindings: FxHashMap<Id, id::N>,
}

impl Context {
    pub fn from(pairs: impl IntoIterator<Item = (Id, id::N)>) -> Self {
        Context { bindings: pairs.into_iter().collect() }
    }
}

pub struct BoundQuery<'q, NV: Sync, ER: graph::Edge> {
    pub(crate) query: &'q Query<NV, ER, Unbound>,
    pub(crate) context: Context,
}
```

**Step 2: Implement bind and find_in for Unbound**

```rust
impl<NV: Sync, ER: graph::Edge> Query<NV, ER, Unbound> {
    pub fn bind(&self, ctx: Context) -> BoundQuery<NV, ER> {
        // validate all context nodes have bindings
        BoundQuery { query: self, context: ctx }
    }
}

impl<'q, NV: Sync, ER: graph::Edge> BoundQuery<'q, NV, ER> {
    pub fn find_in<'g>(&'g self, graph: &'g graph::Graph<NV, ER>) -> Matches<'g, NV, ER> {
        // Pre-populate mapping with context bindings, then run normal search
        Matches::new_with_context(self.query, graph, &self.context)
    }
}
```

**Step 3: Test**

```rust
#[test]
fn context_node_pins_search() {
    // Target: path 0-1-2-3
    // Query: X(0) >> E() >> N(1) — find neighbor of context node
    // Bind X(0) to target node 1 → should find matches for nodes 0 and 2
}
```

**Step 4: Commit**

```
search: add Context binding and BoundQuery with typestate
```

---

## Phase 5: Shape-Aware Domains

### Task 15: Domain computation

**Files:**
- Create: `src/search/query/domain.rs`
- Modify: `src/search/engine/backtrack.rs`

**Step 1: Implement basic degree-based domains**

Before using shapes, start with degree filtering:

```rust
pub(crate) fn compute_domains<NV: Sync, ER: graph::Edge>(
    query: &Query<NV, ER, impl Any>,
    target: &graph::Graph<NV, ER>,
) -> Vec<Vec<id::N>> {
    // For each pattern node:
    // - Collect all clusters it belongs to
    // - For the most restrictive cluster (SubIso > Mono):
    //   SubIso: target node degree must == pattern node degree
    //   Mono: target node degree must >= pattern node degree
    // - Filter target nodes accordingly
}
```

**Step 2: Wire into backtracking**

Replace "all target nodes" candidate generation with domain-based candidates.

**Step 3: Test correctness is preserved**

All existing tests must still pass. Add:

```rust
#[test]
fn domains_prune_candidates() {
    // Large target (100 nodes), small pattern (3 nodes)
    // Verify domain sizes are < 100 for constrained nodes
}
```

**Step 4: Commit**

```
search: add degree-based domain computation
```

---

### Task 16: Shape-aware domain refinement

**Files:**
- Modify: `src/search/query/domain.rs`

**Step 1: Implement shape-based filtering**

Using the target graph's `shapes` field:

- Pattern node with degree N under SubIso → filter by `adj.len() == N` using shape index
- Pattern node with high degree under Mono → core members and star centers are prime candidates
- Pattern node with degree 2 under SubIso → path interiors/endpoints
- Use BTreeSet range queries on shape dimensions for fast candidate enumeration
- Roaring bitmap intersections for combining constraints

**Step 2: Test**

```rust
#[test]
fn shape_domains_narrow_candidates() {
    // Build target with known shapes: a K5 core + star(center, 4 spokes) + path(5 nodes)
    // Pattern: triangle (3 nodes, all degree 2 in pattern)
    // SubIso: candidates should be core members (they form triangles)
    //         NOT star spokes (degree 1 in star) or path interiors
}
```

**Step 3: Commit**

```
search: add shape-aware domain refinement
```

---

### Task 17: Search plan ordering

**Files:**
- Create: `src/search/query/plan.rs`

**Step 1: Implement search ordering**

```rust
pub(crate) fn compute_ordering(
    query: &Query,
    domains: &[Vec<id::N>],
) -> Vec<usize> {
    // 1. Context nodes first (domain size 1)
    // 2. Sort remaining by domain size ascending (most constrained first)
    // 3. Ban-only nodes: maximal→minimal compatibility
}
```

**Step 2: Test**

```rust
#[test]
fn ordering_context_nodes_first() { ... }

#[test]
fn ordering_smallest_domain_first() { ... }
```

**Step 3: Commit**

```
search: add search plan ordering (most-constrained-first)
```

---

## Phase 6: Cross-Validation

### Task 18: Reference solver trait and graph format serialization

**Files:**
- Create: `tests/search_refs/mod.rs`
- Create: `tests/search_refs/format.rs`

Implement DIMACS and VF graph format serialization for target and pattern graphs. Define the `ReferenceSolver` trait.

**Commit:**

```
search: add reference solver trait and DIMACS/VF format serialization
```

---

### Task 19: Glasgow solver wrapper

**Files:**
- Create: `tests/search_refs/glasgow.rs`

Subprocess wrapper that invokes `glasgow_subgraph_solver` CLI, pipes DIMACS format, parses output. Behind `cfg(feature = "cross-validate")`.

**Commit:**

```
search: add Glasgow solver subprocess wrapper
```

---

### Task 20: vf3lib wrapper

**Files:**
- Create: `tests/search_refs/vf3.rs`

Same pattern as Glasgow but for vf3lib CLI.

**Commit:**

```
search: add vf3lib subprocess wrapper
```

---

### Task 21: Cross-validation correctness tests

**Files:**
- Create: `tests/search/correctness.rs`

Property-based tests comparing shagra match counts against Glasgow and vf3lib across random graph pairs. Test the morphism hierarchy property (mono ⊇ sub-iso ⊇ iso).

**Commit:**

```
search: add cross-validation correctness tests
```

---

## Phase 7: Attribute Constraints and Wildcards

### Task 22: Attribute predicates on nodes

**Files:**
- Modify: `src/search/dsl/node.rs`
- Modify: `src/search/engine/feasibility.rs`

Add `.where_val(|v| predicate)` method to `Free` node builder. Store predicate in the compiled query. Check during feasibility.

**Commit:**

```
search: add node attribute predicates
```

---

### Task 23: Attribute predicates on edges

**Files:**
- Modify: `src/search/dsl/edge.rs`
- Modify: `src/search/engine/feasibility.rs`

Same pattern for edges.

**Commit:**

```
search: add edge attribute predicates
```

---

### Task 24: Wildcard nodes

**Files:**
- Modify: `src/search/dsl/node.rs`
- Modify: `src/search/engine/feasibility.rs`

Add wildcard flag to pattern nodes. Wildcard nodes skip attribute predicate checks — they match any target node structurally.

**Commit:**

```
search: add wildcard pattern nodes
```

---

## Verification

After each phase, run:

```bash
cargo check
cargo test --lib search
cargo test --features bench --test chaos_v2 -- v2_undir_balanced  # ensure no regressions
```

After Phase 6, additionally:

```bash
cargo test --features cross-validate --test search_correctness
```
