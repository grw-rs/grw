//! Variable-length path search with lazy iterators.
//!
//! ```ignore
//! ..n(1).dfs()
//! ..n(1).bfs().len(3..10)
//! ..n(1).navigate(Dijkstra::counted())
//! ..n(1).navigate(Dijkstra::weighted(|ev: &EV| ev.cost as f64))
//! ..n(1).navigate(AStar::new(|ev: &EV| ev.cost, |node| heuristic(node)))
//! ```

use std::collections::VecDeque;
use std::sync::Arc;
use ordered_float::NotNan;
use smallvec::SmallVec;
use crate::id;
use crate::graph::collections::IdSet;
use rustc_hash::FxHashMap;

// ── Explore ─────────────────────────────────────────────────────────

/// A `.drive()` closure's decision at one expansion step. The closure receives
/// the number of candidate continuations `n` at the current path tip and picks
/// which of the indices `0..n` to explore; returning `None` prunes the tip.
pub enum Explore {
    /// Explore only the `i`-th candidate.
    One(u8),
    /// Explore the first `min(l, n)` candidates.
    Len(u8),
    /// Explore every candidate.
    All,
    /// Explore exactly the listed candidate indices.
    Steps(SmallVec<[u8; 4]>),
}

// ── Len trait ───────────────────────────────────────────────────────

/// A path-length bound. Implemented for `usize` (exact) and every `usize`
/// range, so `.len(3)`, `.len(2..5)`, `.len(2..=4)`, `.len(3..)` and `.len(..)`
/// all work. Returns `(min, exclusive_max)`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid path length — expected a `usize` or a range of `usize`",
    label = "not a path-length bound",
    note = "use an exact length like `.len(3)` or a range like `.len(2..5)`, `.len(2..=4)`, `.len(3..)`"
)]
pub trait Len {
    /// The `(min, exclusive_max)` edge-count bounds; `None` max means unbounded.
    fn bounds(&self) -> (usize, Option<usize>);
}

impl Len for usize {
    fn bounds(&self) -> (usize, Option<usize>) { (*self, Some(*self + 1)) }
}
impl Len for std::ops::Range<usize> {
    fn bounds(&self) -> (usize, Option<usize>) { (self.start, Some(self.end)) }
}
impl Len for std::ops::RangeInclusive<usize> {
    fn bounds(&self) -> (usize, Option<usize>) { (*self.start(), Some(*self.end() + 1)) }
}
impl Len for std::ops::RangeFrom<usize> {
    fn bounds(&self) -> (usize, Option<usize>) { (self.start, None) }
}
impl Len for std::ops::RangeTo<usize> {
    fn bounds(&self) -> (usize, Option<usize>) { (0, Some(self.end)) }
}
impl Len for std::ops::RangeToInclusive<usize> {
    fn bounds(&self) -> (usize, Option<usize>) { (0, Some(self.end + 1)) }
}
impl Len for std::ops::RangeFull {
    fn bounds(&self) -> (usize, Option<usize>) { (0, None) }
}

// ── Navigator trait ─────────────────────────────────────────────────
//
// A Navigator assigns a non-negative cost to each edge, plus (for A*) an
// admissible per-node heuristic. Navigated search enumerates trails from
// `from` to `to` with cost carried *along the path* (see CostPathIter), so
// cycles (from == to) and the trivial zero-length path report correct costs
// and are governed by the `.len` constraint like any other trail.

/// A cost model for navigated (Dijkstra / A\*) path search: a non-negative
/// per-edge cost, plus an optional admissible heuristic for A\*. Implemented by
/// [`Dijkstra`] and [`AStar`]; pass one to [`Config::navigate`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a path navigator",
    label = "not a `Navigator`",
    note = "pass `Dijkstra::counted()`, `Dijkstra::weighted(|ev| ..)`, or `AStar::new(weight, heuristic)`"
)]
pub trait Navigator<EV>: 'static + Send + Sync {
    /// Non-negative cost of traversing an edge carrying `ev`.
    fn edge_cost(&self, ev: &EV) -> f64;
    /// Admissible (never-overestimating) heuristic from `node` to the target.
    /// Defaults to 0 — i.e. a uniform-cost (Dijkstra) sweep.
    fn heuristic(&self, _node: id::N) -> f64 { 0.0 }
}

pub(crate) trait NavigatorErased<EV>: Send + Sync {
    fn edge_cost(&self, ev: &EV) -> f64;
    fn heuristic(&self, node: id::N) -> f64;
}

impl<EV, T: Navigator<EV>> NavigatorErased<EV> for T {
    fn edge_cost(&self, ev: &EV) -> f64 { Navigator::edge_cost(self, ev) }
    fn heuristic(&self, node: id::N) -> f64 { Navigator::heuristic(self, node) }
}

/// A navigator the cost iterator can hold either way: **owned** for the direct
/// `path_navigate` API (the iterator outlives the caller's config), or
/// **borrowed** for the `search!` engine (the navigator lives in the `Query`).
pub(crate) enum NavRef<'g, EV> {
    Owned(Box<dyn NavigatorErased<EV>>),
    Borrowed(&'g dyn NavigatorErased<EV>),
}

impl<'g, EV> NavRef<'g, EV> {
    #[inline]
    fn edge_cost(&self, ev: &EV) -> f64 {
        match self { NavRef::Owned(b) => b.edge_cost(ev), NavRef::Borrowed(r) => r.edge_cost(ev) }
    }
    #[inline]
    fn heuristic(&self, node: id::N) -> f64 {
        match self { NavRef::Owned(b) => b.heuristic(node), NavRef::Borrowed(r) => r.heuristic(node) }
    }
}

// ── Dijkstra ────────────────────────────────────────────────────────

/// Uniform-cost navigator: orders paths by summed edge weight, no heuristic.
/// Build with [`Dijkstra::counted`] (hop count) or [`Dijkstra::weighted`].
pub struct Dijkstra<F> {
    weight_fn: F,
}

impl Dijkstra<()> {
    /// Unit cost per edge — total cost equals the number of edges (path length).
    pub fn counted<EV: 'static>() -> Dijkstra<fn(&EV) -> f64> {
        fn unit<EV>(_: &EV) -> f64 { 1.0 }
        Dijkstra { weight_fn: unit::<EV> }
    }
}

impl<F> Dijkstra<F> {
    /// Per-edge cost from `f(&EV)`; total cost is the sum along the path.
    pub fn weighted(f: F) -> Self { Dijkstra { weight_fn: f } }
}

impl<EV, F: Fn(&EV) -> f64 + Send + Sync + 'static> Navigator<EV> for Dijkstra<F> {
    fn edge_cost(&self, ev: &EV) -> f64 { (self.weight_fn)(ev) }
}

// ── A* ──────────────────────────────────────────────────────────────

/// A\* navigator: like [`Dijkstra`] plus an admissible per-node heuristic that
/// steers exploration toward the target without changing which path is cheapest.
pub struct AStar<W, H> {
    weight_fn: W,
    heuristic: H,
}

impl<W, H> AStar<W, H> {
    /// `weight_fn(&EV)` is the per-edge cost; `heuristic(node)` must never
    /// overestimate the remaining cost to the target (admissibility).
    pub fn new(weight_fn: W, heuristic: H) -> Self { AStar { weight_fn, heuristic } }
}

impl<EV, W, H> Navigator<EV> for AStar<W, H>
where
    W: Fn(&EV) -> f64 + Send + Sync + 'static,
    H: Fn(id::N) -> f64 + Send + Sync + 'static,
{
    fn edge_cost(&self, ev: &EV) -> f64 { (self.weight_fn)(ev) }
    fn heuristic(&self, node: id::N) -> f64 { (self.heuristic)(node) }
}

// ── Driver ──────────────────────────────────────────────────────────

/// Result cardinality for navigated (Dijkstra/A*) search.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavMode {
    /// Yield only the single cheapest path. Uses per-node cost dominance
    /// (textbook Dijkstra/A*), so it runs in `O((V+E) log V)`. Cannot find a
    /// cycle through the source (use `All` and take the first for that).
    One,
    /// Enumerate every trail in non-decreasing cost. No node dominance, so the
    /// frontier can grow exponentially — the caller owns that trade-off.
    All,
}

pub(crate) enum Driver<EV = ()> {
    Stack(Option<Arc<dyn Fn(u8) -> Option<Explore> + Send + Sync>>),
    Queue(Option<Arc<dyn Fn(u8) -> Option<Explore> + Send + Sync>>),
    Navigate(Box<dyn NavigatorErased<EV>>, NavMode),
}

impl<EV> Default for Driver<EV> {
    fn default() -> Self { Driver::Stack(None) }
}

impl Clone for Driver<()> {
    fn clone(&self) -> Self {
        match self {
            Driver::Stack(d) => Driver::Stack(d.clone()),
            Driver::Queue(d) => Driver::Queue(d.clone()),
            Driver::Navigate(..) => unreachable!("Driver<()> never holds Navigate"),
        }
    }
}

// ── Mode type-states ────────────────────────────────────────────────

/// Type-state of a fresh path: no traversal or navigation chosen yet.
pub struct Unset;
/// Type-state after `.dfs()`, `.bfs()` or `.drive()`.
pub struct Traversal;
/// Type-state after `.navigate(..)`.
pub struct Navigated;

mod mode_seal {
    pub trait Sealed {}
    impl Sealed for super::Unset {}
    impl Sealed for super::Traversal {}
    impl Sealed for super::Navigated {}
}

/// Marker satisfied only by the [`Navigated`] type-state, so that calling a
/// navigated-only method in the wrong state produces a trait-bound error with
/// this explanation instead of a bare "method not found".
#[diagnostic::on_unimplemented(
    message = "`.one()` / `.all()` apply only to a navigated path",
    label = "this path is not navigated",
    note = "call `.navigate(Dijkstra::counted() / Dijkstra::weighted(..) / AStar::new(..))` first — traversal (`.dfs()`/`.bfs()`) paths have no cost ordering"
)]
pub trait IsNavigated: mode_seal::Sealed {}
impl IsNavigated for Navigated {}

/// Marker satisfied only by the [`Unset`] type-state, so that re-choosing a
/// mode on an already-configured path produces a trait-bound error naming the
/// conflict instead of a bare "method not found".
#[diagnostic::on_unimplemented(
    message = "a path mode was already chosen — `.dfs()`, `.bfs()`, `.drive()` and `.navigate()` are mutually exclusive",
    label = "this path already has a mode",
    note = "choose exactly one traversal or navigation on a fresh `..node` path"
)]
pub trait IsUnset: mode_seal::Sealed {}
impl IsUnset for Unset {}

// ── Config ──────────────────────────────────────────────────────────

/// A path-search configuration: target `N`, type-state `Mode`
/// ([`Unset`] → [`Traversal`] or [`Navigated`]), and edge-value type `EV`.
/// Built from a node via the `..node.dfs()`-style DSL or [`Config::new`].
pub struct Config<N, Mode = Unset, EV = ()> {
    pub(crate) target: N,
    pub(crate) min_len: usize,
    pub(crate) max_len: Option<usize>,
    pub(crate) guard: Option<Arc<GuardFn>>,
    pub(crate) driver: Driver<EV>,
    pub(crate) _mode: std::marker::PhantomData<Mode>,
}

impl Clone for Config<()> {
    fn clone(&self) -> Self {
        Config {
            target: (),
            min_len: self.min_len,
            max_len: self.max_len,
            guard: self.guard.clone(),
            driver: self.driver.clone(),
            _mode: std::marker::PhantomData,
        }
    }
}

impl<N, Mode, EV> Config<N, Mode, EV> {
    /// Constrain path length (edge count) to `bounds` — exact or any range.
    pub fn len(mut self, bounds: impl Len) -> Self {
        let (min, max) = bounds.bounds();
        self.min_len = min;
        self.max_len = max;
        self
    }

    /// Reject (partial) paths. The predicate receives the path **tip-first**
    /// (current node back toward the start) as a lazy iterator — decide from a
    /// short suffix and return early. An escape hatch for constraints grw can't
    /// express directly; prefer `edge_pred`/`.len` where possible.
    pub fn guard(mut self, pred: impl Fn(&PathView) -> bool + Send + Sync + 'static) -> Self {
        self.guard = Some(Arc::new(pred));
        self
    }

    fn into_mode<M2, EV2>(self, driver: Driver<EV2>) -> Config<N, M2, EV2> {
        Config {
            target: self.target, min_len: self.min_len, max_len: self.max_len,
            guard: self.guard, driver, _mode: std::marker::PhantomData,
        }
    }
}

// Builders are generic over `EV` so that `.dfs()/.bfs()` configs (which ignore
// edge values) unify their `EV` with the edge type at the path operator — letting
// a single operator arm accept both traversal and navigated paths.
impl<N, EV> Config<N, Unset, EV> {
    /// A fresh path config targeting `target`, in the [`Unset`] state.
    pub fn new(target: N) -> Self {
        Config {
            target, min_len: 1, max_len: None, guard: None,
            driver: Driver::default(), _mode: std::marker::PhantomData,
        }
    }
}

impl<N, Mode: IsUnset, EV> Config<N, Mode, EV> {
    /// Depth-first traversal: deepest paths yielded first.
    pub fn dfs(self) -> Config<N, Traversal, EV> {
        self.into_mode(Driver::Stack(None))
    }

    /// Breadth-first traversal: shortest paths yielded first.
    pub fn bfs(self) -> Config<N, Traversal, EV> {
        self.into_mode(Driver::Queue(None))
    }

    /// DFS traversal steered by a per-depth [`Explore`] decision.
    pub fn drive(self, f: impl Fn(u8) -> Option<Explore> + Send + Sync + 'static) -> Config<N, Traversal, EV> {
        self.into_mode(Driver::Stack(Some(Arc::new(f))))
    }
}

// `navigate` lives on the `()` config so its receiver `EV` is pinned (the
// navigator brings its own `EV`), avoiding inference ambiguity in chains.
impl<N, Mode: IsUnset> Config<N, Mode, ()> {
    /// Cost-ordered (Dijkstra / A\*) search using `nav` as the cost model.
    /// Defaults to [`NavMode::One`]; switch with [`Config::all`].
    pub fn navigate<EV: 'static, Nav: Navigator<EV>>(self, nav: Nav) -> Config<N, Navigated, EV> {
        // Default to `One` — a single shortest path, the cheap textbook case.
        self.into_mode(Driver::Navigate(Box::new(nav), NavMode::One))
    }
}

impl<N, EV> Config<N, Traversal, EV> {
    /// Steer an already-chosen traversal with a per-depth [`Explore`] decision.
    pub fn drive(mut self, f: impl Fn(u8) -> Option<Explore> + Send + Sync + 'static) -> Self {
        self.driver = match self.driver {
            Driver::Queue(_) => Driver::Queue(Some(Arc::new(f))),
            _ => Driver::Stack(Some(Arc::new(f))),
        };
        self
    }
}

impl<N, Mode: IsNavigated, EV> Config<N, Mode, EV> {
    /// Find only the single cheapest path (default). Efficient — node dominance.
    pub fn one(mut self) -> Self {
        if let Driver::Navigate(_, mode) = &mut self.driver { *mode = NavMode::One; }
        self
    }

    /// Enumerate every trail in non-decreasing cost. May explode on large graphs.
    pub fn all(mut self) -> Self {
        if let Driver::Navigate(_, mode) = &mut self.driver { *mode = NavMode::All; }
        self
    }
}

// ── PathConstraint ──────────────────────────────────────────────────

/// Morphism-derived constraints on path intermediates, enforced during the walk.
pub struct PathConstraint {
    /// Intermediates may not reuse nodes already bound elsewhere in the match.
    pub injective: bool,
    /// Intermediates may not have neighbors outside the path (exact-neighborhood morphisms).
    pub induced: bool,
    pub(crate) excluded: IdSet<id::N>,
}

impl Default for PathConstraint {
    fn default() -> Self { Self::unconstrained() }
}

impl PathConstraint {
    /// Derive the constraint set implied by `morphism`, excluding `bindings`
    /// (nodes already matched) from reuse where injectivity demands it.
    pub fn from_morphism(morphism: crate::search::Morphism, bindings: &[id::N]) -> Self {
        use crate::search::Morphism::*;
        PathConstraint {
            injective: matches!(morphism, Mono | EpiMono | SubIso | Iso),
            induced: matches!(morphism, SubIso | Iso),
            excluded: IdSet::from_raw_ids(bindings.iter().map(|n| crate::Id::from(*n))),
        }
    }

    /// No constraints: any intermediate is acceptable.
    pub fn unconstrained() -> Self {
        PathConstraint {
            injective: false,
            induced: false,
            excluded: IdSet::default(),
        }
    }

    /// May `node` appear as a path intermediate under the injectivity rule?
    pub fn accept_node(&self, node: id::N) -> bool {
        if self.injective && self.excluded.contains_key(&node) { return false; }
        true
    }

    /// Does the completed `path` satisfy the induced (exact-neighborhood) rule?
    pub fn accept_path_induced<NV, E: crate::graph::Edge, G: crate::graph::Graph<NV, E>>(
        &self, path: &[id::N], graph: &G,
    ) -> bool {
        if !self.induced { return true; }
        for &node in &path[1..path.len().saturating_sub(1)] {
            if let Some(nbs) = graph.neighbors(node) {
                for (nb, _, _) in nbs {
                    if !path.contains(&nb) && !self.excluded.contains_key(&nb) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Record `path`'s intermediates as used, excluding them from later paths
    /// of the same match (cross-path injectivity).
    pub fn commit(&mut self, path: &[id::N]) {
        if path.len() > 2 {
            for &n in &path[1..path.len() - 1] {
                self.excluded.insert(&n);
            }
        }
    }
}

// ── PathView (guard argument) ───────────────────────────────────────

/// Read-only view of the (partial) path handed to a `.guard`. Ordered
/// **tip-first**: the current node, then its predecessor, … back to the start.
/// All queries are lazy and allocation-free — `contains`/`iter` short-circuit,
/// and navigated `One` search backs this with a predecessor walk (no
/// materialisation), so a guard that decides from a short suffix stays cheap.
pub struct PathView<'a>(PathSrc<'a>);

enum PathSrc<'a> {
    /// A materialised path (dfs/bfs, navigated `All`), stored start→tip.
    Slice(&'a [id::N]),
    /// A predecessor chain (navigated `One`): walk `node` back to the source.
    Walk { node: id::N, prev: &'a FxHashMap<id::N, id::N> },
}

impl<'a> PathView<'a> {
    fn slice(path: &'a [id::N]) -> Self { PathView(PathSrc::Slice(path)) }
    fn walk(node: id::N, prev: &'a FxHashMap<id::N, id::N>) -> Self {
        PathView(PathSrc::Walk { node, prev })
    }

    /// Nodes from the current tip back toward the start.
    pub fn iter(&self) -> impl Iterator<Item = id::N> + '_ {
        let it: Box<dyn Iterator<Item = id::N> + '_> = match &self.0 {
            PathSrc::Slice(p) => Box::new(p.iter().rev().copied()),
            PathSrc::Walk { node, prev } => Box::new(PrevWalk { node: Some(*node), prev }),
        };
        it
    }

    /// The current tip (most recent node), or `None` if the path is empty.
    pub fn tip(&self) -> Option<id::N> { self.iter().next() }

    /// Whether `node` appears anywhere on the path (tip-first, short-circuits).
    pub fn contains(&self, node: id::N) -> bool { self.iter().any(|n| n == node) }

    /// Number of nodes on the path. O(1) for dfs/bfs/`All`; O(len) for `One`.
    pub fn len(&self) -> usize {
        match &self.0 {
            PathSrc::Slice(p) => p.len(),
            PathSrc::Walk { .. } => self.iter().count(),
        }
    }

    /// `true` when the path has no nodes yet.
    pub fn is_empty(&self) -> bool { self.tip().is_none() }
}

// ── Executor helpers ───────────────────────────────────────────────

/// A path guard, called with a tip-first [`PathView`]. Return `false` to reject
/// the (partial) path. An escape hatch for constraints grw can't express
/// directly; prefer `edge_pred`/`.len` where possible.
pub(crate) type GuardFn = dyn Fn(&PathView) -> bool + Send + Sync;

/// Run a guard over a materialised path slice (dfs/bfs and navigated `All`,
/// which carry the path anyway).
fn guard_ok_slice(guard: Option<&GuardFn>, path: &[id::N]) -> bool {
    match guard {
        None => true,
        Some(g) => g(&PathView::slice(path)),
    }
}

fn can_extend(
    path_edges: usize, max_len: usize,
    guard: Option<&GuardFn>,
    path: &[id::N],
) -> bool {
    if path_edges >= max_len { return false; }
    guard_ok_slice(guard, path)
}

fn is_complete(
    path: &[id::N], min_len: usize, max_len: usize,
    guard: Option<&GuardFn>,
) -> bool {
    let edges = path.len().saturating_sub(1);
    edges >= min_len && edges < max_len && guard_ok_slice(guard, path)
}

fn for_each_explore(explore: Explore, n: u8, mut f: impl FnMut(u8)) {
    match explore {
        Explore::All => (0..n).for_each(&mut f),
        Explore::One(i) => f(i),
        Explore::Len(l) => (0..l.min(n)).for_each(&mut f),
        Explore::Steps(s) => s.iter().copied().for_each(&mut f),
    }
}

// ── DFS iterator state ─────────────────────────────────────────────

enum DfsOp {
    Visit(id::N),
    Unvisit,
}

struct DfsState {
    stack: Vec<DfsOp>,
    visited: IdSet<id::N>,
    path: Vec<id::N>,
}

// ── BFS iterator state (parent-pointer tree) ────────────────────────

struct BfsState {
    queue: VecDeque<(id::N, usize, usize)>, // (node, tree_idx, depth)
    path_tree: Vec<(id::N, Option<usize>)>,  // (node, parent_idx)
}

impl BfsState {
    fn reconstruct(&self, leaf: usize) -> Vec<id::N> {
        let mut path = Vec::new();
        let mut idx = Some(leaf);
        while let Some(i) = idx {
            path.push(self.path_tree[i].0);
            idx = self.path_tree[i].1;
        }
        path.reverse();
        path
    }

    fn is_on_path(&self, leaf: usize, node: id::N) -> bool {
        let mut idx = Some(leaf);
        while let Some(i) = idx {
            if self.path_tree[i].0 == node { return true; }
            idx = self.path_tree[i].1;
        }
        false
    }
}

// ── PathIter: lazy DFS/BFS iterator ─────────────────────────────────

enum AlgoState {
    Dfs(DfsState),
    Bfs(BfsState),
}

/// Lazy traversal iterator (`.dfs()`/`.bfs()`/`.drive()`): yields one complete
/// path per `next()`, backtracking on demand.
pub struct PathIter<'g, NV, E: crate::graph::Edge, G, F> {
    graph: &'g G,
    edge_pred: F,
    to: id::N,
    min_len: usize,
    max_len: usize,
    guard: Option<Arc<GuardFn>>,
    drive: Option<Arc<dyn Fn(u8) -> Option<Explore> + Send + Sync>>,
    constraint: &'g PathConstraint,
    algo: AlgoState,
    pending: Vec<Vec<id::N>>,
    _marker: std::marker::PhantomData<fn() -> (NV, E)>,
}

impl<'g, NV, E: crate::graph::Edge, G: crate::graph::Graph<NV, E>, F: Fn(E::Slot, &E::Val) -> bool> PathIter<'g, NV, E, G, F> {
    fn advance_dfs(&mut self) -> Option<Vec<id::N>> {
        let Self { graph, edge_pred, to, min_len, max_len, guard, drive, constraint, algo, pending, .. } = self;
        let guard_fn = guard.as_deref();
        let drive_fn = drive.as_deref();
        let AlgoState::Dfs(st) = algo else { unreachable!() };
        let mut candidates: Vec<(id::N, E::Slot)> = Vec::new();

        while let Some(op) = st.stack.pop() {
            match op {
                DfsOp::Unvisit => {
                    if let Some(n) = st.path.pop() {
                        st.visited.remove(&n);
                    }
                }
                DfsOp::Visit(cur) => {
                    if st.visited.contains_key(&cur) { continue; }
                    st.visited.insert(&cur);
                    st.path.push(cur);

                    let path_edges = st.path.len().saturating_sub(1);
                    if !can_extend(path_edges, *max_len, guard_fn, &st.path) {
                        st.path.pop();
                        st.visited.remove(&cur);
                        continue;
                    }

                    candidates.clear();
                    if let Some(nbs) = graph.neighbors(cur) {
                        candidates.extend(
                            nbs.filter(|(_, slot, ev)| edge_pred(*slot, ev))
                               .map(|(nb, slot, _)| (nb, slot))
                        );
                    }
                    let n = candidates.len().min(255) as u8;

                    if n == 0 {
                        st.stack.push(DfsOp::Unvisit);
                        continue;
                    }

                    // Previous node on the path (for U-turn detection at closure).
                    let prev = (st.path.len() >= 2).then(|| st.path[st.path.len() - 2]);

                    let explore = match drive_fn {
                        Some(d) => match d(n) {
                            Some(e) => e,
                            None => { st.stack.push(DfsOp::Unvisit); continue; }
                        },
                        None => Explore::All,
                    };

                    st.stack.push(DfsOp::Unvisit);

                    let mut indices = SmallVec::<[u8; 8]>::new();
                    for_each_explore(explore, n, |idx| indices.push(idx));

                    let mut first_result: Option<Vec<id::N>> = None;
                    let mut pushed = SmallVec::<[id::N; 8]>::new();
                    for &idx in indices.iter().rev() {
                        let (nb, slot) = candidates[idx as usize];
                        if nb == *to {
                            // Reject degenerate cycles that reuse the edge we just
                            // arrived on. With node-injective walks the only possible
                            // edge reuse is an immediate U-turn back to `to` along the
                            // same undirected edge (reverse_slot(slot) == slot holds for
                            // undirected slots, but not for directed ones — so genuine
                            // directed 2-cycles via distinct edge records are preserved).
                            if prev == Some(nb) && E::reverse_slot(slot) == slot {
                                continue;
                            }
                            let mut p = st.path.clone();
                            p.push(nb);
                            if is_complete(&p, *min_len, *max_len, guard_fn)
                                && constraint.accept_path_induced(&p, *graph) {
                                if first_result.is_none() {
                                    first_result = Some(p);
                                } else {
                                    pending.push(p);
                                }
                            }
                        } else if !st.visited.contains_key(&nb)
                               && !pushed.contains(&nb)
                               && constraint.accept_node(nb) {
                            pushed.push(nb);
                            st.stack.push(DfsOp::Visit(nb));
                        }
                    }
                    if let Some(r) = first_result {
                        return Some(r);
                    }
                }
            }
        }
        None
    }

    fn advance_bfs(&mut self) -> Option<Vec<id::N>> {
        let Self { graph, edge_pred, to, min_len, max_len, guard, drive, constraint, algo, pending, .. } = self;
        let guard_fn = guard.as_deref();
        let drive_fn = drive.as_deref();
        let AlgoState::Bfs(st) = algo else { unreachable!() };
        let mut candidates: Vec<(id::N, E::Slot)> = Vec::new();

        while let Some((cur, tree_idx, depth)) = st.queue.pop_front() {
            if depth >= *max_len { continue; }

            if let Some(g) = guard_fn {
                let path = st.reconstruct(tree_idx);
                if !g(&PathView::slice(&path)) { continue; }
            }

            candidates.clear();
            if let Some(nbs) = graph.neighbors(cur) {
                candidates.extend(
                    nbs.filter(|(_, slot, ev)| edge_pred(*slot, ev))
                       .map(|(nb, slot, _)| (nb, slot))
                );
            }
            let n = candidates.len().min(255) as u8;
            if n == 0 { continue; }

            // Parent of `cur` in the BFS tree (for U-turn detection at closure).
            let prev = st.path_tree[tree_idx].1.map(|pi| st.path_tree[pi].0);

            let explore = match drive_fn {
                Some(d) => match d(n) { Some(e) => e, None => continue },
                None => Explore::All,
            };

            let mut first_result: Option<Vec<id::N>> = None;
            let mut indices = SmallVec::<[u8; 8]>::new();
            for_each_explore(explore, n, |idx| indices.push(idx));

            for idx in indices {
                let (nb, slot) = candidates[idx as usize];
                if nb != *to && !constraint.accept_node(nb) { continue; }

                if nb == *to {
                    // Skip degenerate U-turn cycles along the same undirected edge
                    // (see advance_dfs for the full rationale).
                    if prev == Some(nb) && E::reverse_slot(slot) == slot {
                        continue;
                    }
                    let mut p = st.reconstruct(tree_idx);
                    p.push(nb);
                    if is_complete(&p, *min_len, *max_len, guard_fn)
                        && constraint.accept_path_induced(&p, *graph) {
                        if first_result.is_none() {
                            first_result = Some(p);
                        } else {
                            pending.push(p);
                        }
                    }
                } else if !st.is_on_path(tree_idx, nb) {
                    let new_depth = depth + 1;
                    if new_depth < *max_len {
                        let new_idx = st.path_tree.len();
                        st.path_tree.push((nb, Some(tree_idx)));
                        st.queue.push_back((nb, new_idx, new_depth));
                    }
                }
            }
            if let Some(r) = first_result {
                return Some(r);
            }
        }
        None
    }
}

impl<'g, NV, E: crate::graph::Edge, G: crate::graph::Graph<NV, E>, F: Fn(E::Slot, &E::Val) -> bool> Iterator for PathIter<'g, NV, E, G, F> {
    type Item = Vec<id::N>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(p) = self.pending.pop() {
            return Some(p);
        }
        match self.algo {
            AlgoState::Dfs(_) => self.advance_dfs(),
            AlgoState::Bfs(_) => self.advance_bfs(),
        }
    }
}

// ── CostPathIter: cost-ordered trail enumerator ─────────────────────
//
// Enumerates trails from `from` to `to` in increasing cost order (Dijkstra;
// shortest-first for A*). A trail is a simple path (no repeated node),
// optionally closed back to its start when the expression targets the start
// (from == to). Cost is accumulated *on the path*, so the start node playing
// both source and sink (a cycle) and the trivial zero-length path are handled
// uniformly. Edge reuse is forbidden via the same U-turn guard as dfs/bfs;
// node-injectivity then bounds path length, so enumeration terminates.

struct CostEntry {
    priority: NotNan<f64>, // ordering key: g + heuristic
    g: NotNan<f64>,        // accumulated path cost
    seq: u64,              // insertion order: total, deterministic tie-break
    node: id::N,
    depth: usize,          // edges from source (used by `One`; `All` uses `path`)
    path: Vec<id::N>,      // full path (used by `All`; empty in `One`)
}

impl PartialEq for CostEntry {
    fn eq(&self, o: &Self) -> bool {
        self.priority == o.priority && self.g == o.g && self.seq == o.seq
    }
}
impl Eq for CostEntry {}
impl Ord for CostEntry {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&o.priority)
            .then(self.g.cmp(&o.g))
            .then(self.seq.cmp(&o.seq))
    }
}
impl PartialOrd for CostEntry {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(o)) }
}

fn not_nan(v: f64, what: &str) -> NotNan<f64> {
    NotNan::new(v).unwrap_or_else(|_| panic!("{what} produced NaN"))
}

/// Tip-first walk over a predecessor chain: `node`, prev[node], … to the source
/// (which has no predecessor). Lazy — `One`-mode guards consume only as much as
/// they need, with no path materialisation.
struct PrevWalk<'a> {
    node: Option<id::N>,
    prev: &'a FxHashMap<id::N, id::N>,
}
impl<'a> Iterator for PrevWalk<'a> {
    type Item = id::N;
    fn next(&mut self) -> Option<id::N> {
        let n = self.node?;
        self.node = self.prev.get(&n).copied();
        Some(n)
    }
}

/// Lazy navigated iterator (`.navigate(..)`): yields `(cost, path)` pairs in
/// non-decreasing cost order.
pub struct CostPathIter<'g, NV, E: crate::graph::Edge, G, F> {
    graph: &'g G,
    edge_pred: F,
    to: id::N,
    min_len: usize,
    max_len: usize,
    guard: Option<Arc<GuardFn>>,
    constraint: &'g PathConstraint,
    nav: NavRef<'g, E::Val>,
    mode: NavMode,
    heap: std::collections::BinaryHeap<std::cmp::Reverse<CostEntry>>,
    seq: u64,
    /// Best cost reached per node — Dijkstra dominance (`One` only). Seeded with
    /// the source at 0, which also blocks cycle-through-source (use `All`).
    best: FxHashMap<id::N, NotNan<f64>>,
    /// Predecessor of each settled node, for `One`-mode path reconstruction.
    prev: FxHashMap<id::N, id::N>,
    /// `One` mode is exhausted after it yields its single shortest path.
    done: bool,
    /// The trivial zero-length path `[from]`, emitted once when `from == to`
    /// and `min_len == 0` (no edge reused, start == end).
    trivial: Option<Vec<id::N>>,
    _marker: std::marker::PhantomData<fn() -> NV>,
}

impl<'g, NV, E: crate::graph::Edge, G: crate::graph::Graph<NV, E>, F: Fn(E::Slot, &E::Val) -> bool> CostPathIter<'g, NV, E, G, F> {
    fn advance(&mut self) -> Option<(f64, Vec<id::N>)> {
        if self.done { return None; }
        if let Some(p) = self.trivial.take() {
            // cost 0 — the cheapest possible, so it's also the `One` answer.
            if self.mode == NavMode::One { self.done = true; }
            return Some((0.0, p));
        }
        match self.mode {
            NavMode::One => self.advance_one(),
            NavMode::All => self.advance_all(),
        }
    }

    /// `One`: textbook Dijkstra/A* with per-node cost dominance and predecessor
    /// pointers — `O((V+E) log V)`, no path carried. Yields the single cheapest
    /// path. A `.guard` prunes prefixes lazily (predecessor walk, tip-first) and
    /// is consulted only when set, so the common no-guard case pays nothing.
    fn advance_one(&mut self) -> Option<(f64, Vec<id::N>)> {
        let Self { graph, edge_pred, to, min_len, max_len, guard, constraint, nav, heap, seq, best, prev, done, .. } = self;
        let guard_fn = guard.as_deref();

        while let Some(std::cmp::Reverse(entry)) = heap.pop() {
            let CostEntry { g, node: cur, depth, .. } = entry;

            // Skip entries superseded by a cheaper route already settled.
            if let Some(&b) = best.get(&cur) { if g > b { continue; } }

            // Goal: `to` reached via >=1 edge. With dominance it is settled once,
            // so the first valid arrival is the single cheapest path.
            if cur == *to && depth >= 1 {
                if depth >= *min_len && depth < *max_len {
                    let mut path: Vec<id::N> =
                        PrevWalk { node: Some(*to), prev }.collect();
                    path.reverse();
                    if guard_ok_slice(guard_fn, &path)
                        && constraint.accept_path_induced(&path, *graph) {
                        *done = true;
                        return Some((g.into_inner(), path));
                    }
                }
                continue; // settled `to`, but length/guard reject it — no cheaper alt
            }

            if depth >= *max_len { continue; }
            // Prefix guard prune — lazy predecessor walk, only when a guard is set.
            if let Some(gd) = guard_fn {
                if !gd(&PathView::walk(cur, prev)) { continue; }
            }

            let Some(nbs) = graph.neighbors(cur) else { continue };
            for (nb, slot, ev) in nbs {
                if !edge_pred(slot, ev) { continue; }
                if nb != *to && !constraint.accept_node(nb) { continue; }
                let new_g = not_nan(g.into_inner() + nav.edge_cost(ev), "edge cost");
                match best.get(&nb) {
                    Some(&b) if new_g >= b => continue, // dominated — keep cheaper route
                    _ => {}
                }
                best.insert(nb, new_g);
                prev.insert(nb, cur);
                let priority = not_nan(new_g.into_inner() + nav.heuristic(nb), "heuristic");
                *seq += 1;
                heap.push(std::cmp::Reverse(CostEntry {
                    priority, g: new_g, seq: *seq, node: nb, depth: depth + 1, path: Vec::new(),
                }));
            }
        }
        None
    }

    /// `All`: enumerate trails in non-decreasing cost. No dominance — each frontier
    /// entry carries its own path; node-injectivity (+ the U-turn guard) keeps
    /// paths simple and bounds termination. Can grow exponentially by design.
    fn advance_all(&mut self) -> Option<(f64, Vec<id::N>)> {
        let Self { graph, edge_pred, to, min_len, max_len, guard, constraint, nav, heap, seq, .. } = self;
        let guard_fn = guard.as_deref();

        while let Some(std::cmp::Reverse(entry)) = heap.pop() {
            let CostEntry { g, node: cur, path, .. } = entry;

            // Closure: a non-seed path that has arrived back at `to`.
            if path.len() >= 2 && cur == *to {
                if is_complete(&path, *min_len, *max_len, guard_fn)
                    && constraint.accept_path_induced(&path, *graph) {
                    return Some((g.into_inner(), path));
                }
                continue; // reached the target but too short — a terminal dead end
            }

            let path_edges = path.len().saturating_sub(1);
            if !can_extend(path_edges, *max_len, guard_fn, &path) { continue; }

            let prev = (path.len() >= 2).then(|| path[path.len() - 2]);
            let Some(nbs) = graph.neighbors(cur) else { continue };
            for (nb, slot, ev) in nbs {
                if !edge_pred(slot, ev) { continue; }
                if nb == *to {
                    // Reject the degenerate U-turn that reuses the arrival edge.
                    if prev == Some(nb) && E::reverse_slot(slot) == slot { continue; }
                } else {
                    if path.contains(&nb) { continue; }        // simple path (node-injective)
                    if !constraint.accept_node(nb) { continue; }
                }
                let new_g = not_nan(g.into_inner() + nav.edge_cost(ev), "edge cost");
                let priority = not_nan(new_g.into_inner() + nav.heuristic(nb), "heuristic");
                *seq += 1;
                let mut p = path.clone();
                p.push(nb);
                heap.push(std::cmp::Reverse(CostEntry {
                    priority, g: new_g, seq: *seq, node: nb, depth: p.len() - 1, path: p,
                }));
            }
        }
        None
    }
}

impl<'g, NV, E: crate::graph::Edge, G: crate::graph::Graph<NV, E>, F: Fn(E::Slot, &E::Val) -> bool> Iterator for CostPathIter<'g, NV, E, G, F> {
    type Item = (f64, Vec<id::N>);

    fn next(&mut self) -> Option<Self::Item> {
        self.advance()
    }
}

// ── Construction helpers ───────────────────────────────────────────

fn build_path_iter<'a, NV, E: crate::graph::Edge, G: crate::graph::Graph<NV, E>, F: Fn(E::Slot, &E::Val) -> bool>(
    graph: &'a G,
    from: id::N, to: id::N,
    edge_pred: F,
    min_len: usize, max_len: usize,
    guard: Option<Arc<GuardFn>>,
    drive: Option<Arc<dyn Fn(u8) -> Option<Explore> + Send + Sync>>,
    is_bfs: bool,
    constraint: &'a PathConstraint,
) -> PathIter<'a, NV, E, G, F> {
    let algo = if is_bfs {
        AlgoState::Bfs(BfsState {
            queue: VecDeque::from([(from, 0, 0)]),
            path_tree: vec![(from, None)],
        })
    } else {
        AlgoState::Dfs(DfsState {
            stack: vec![DfsOp::Visit(from)],
            visited: IdSet::default(),
            path: Vec::new(),
        })
    };
    let mut pending = Vec::new();
    if from == to && min_len == 0 {
        pending.push(vec![from]);
    }
    PathIter {
        graph, edge_pred, to, min_len, max_len,
        guard, drive, constraint, algo,
        pending,
        _marker: std::marker::PhantomData,
    }
}

fn build_cost_path_iter<'a, NV, E: crate::graph::Edge, G: crate::graph::Graph<NV, E>, F: Fn(E::Slot, &E::Val) -> bool>(
    graph: &'a G,
    from: id::N, to: id::N,
    edge_pred: F,
    min_len: usize, max_len: usize,
    guard: Option<Arc<GuardFn>>,
    nav: NavRef<'a, E::Val>,
    mode: NavMode,
    constraint: &'a PathConstraint,
) -> CostPathIter<'a, NV, E, G, F> {
    let mut heap = std::collections::BinaryHeap::new();
    let zero = NotNan::new(0.0).unwrap();
    heap.push(std::cmp::Reverse(CostEntry {
        priority: zero, g: zero, seq: 0, node: from, depth: 0, path: vec![from],
    }));
    let trivial = if from == to && min_len == 0 { Some(vec![from]) } else { None };
    // Seed the source as settled at 0 (One-mode dominance): blocks re-entry of
    // the source, hence cycle-through-source — that's an All-mode concern.
    let mut best = FxHashMap::default();
    best.insert(from, zero);
    CostPathIter {
        graph, edge_pred, to, min_len, max_len,
        guard, constraint, nav, mode, heap, seq: 0, trivial, best,
        prev: FxHashMap::default(), done: false,
        _marker: std::marker::PhantomData,
    }
}

// ── Public dispatch (called from MGraph methods) ─────────────────────

pub(crate) fn execute<'a, NV, E: crate::graph::Edge, G: crate::graph::Graph<NV, E>>(
    graph: &'a G,
    from: id::N, to: id::N,
    edge_pred: &'a dyn Fn(E::Slot, &E::Val) -> bool,
    config: &Config<()>,
    constraint: &'a PathConstraint,
) -> PathIter<'a, NV, E, G, &'a dyn Fn(E::Slot, &E::Val) -> bool> {
    let max_len = config.max_len.unwrap_or(graph.node_count() + 1);
    let (is_bfs, drive) = match &config.driver {
        Driver::Stack(d) => (false, d.clone()),
        Driver::Queue(d) => (true, d.clone()),
        Driver::Navigate(..) => panic!("Navigate driver in traversal execute — use execute_nav"),
    };
    build_path_iter(
        graph, from, to, edge_pred,
        config.min_len, max_len,
        config.guard.clone(), drive,
        is_bfs, constraint,
    )
}


pub(crate) fn execute_owned<'a, NV, E: crate::graph::Edge, G: crate::graph::Graph<NV, E>, F: Fn(E::Slot, &E::Val) -> bool + 'a>(
    graph: &'a G,
    from: id::N, to: id::N,
    edge_pred: F,
    config: Config<(), Traversal>,
    constraint: &'a PathConstraint,
) -> PathIter<'a, NV, E, G, F> {
    let max_len = config.max_len.unwrap_or(graph.node_count() + 1);
    let (is_bfs, drive) = match config.driver {
        Driver::Stack(d) => (false, d),
        Driver::Queue(d) => (true, d),
        Driver::Navigate(..) => unreachable!(),
    };
    build_path_iter(
        graph, from, to, edge_pred,
        config.min_len, max_len,
        config.guard, drive,
        is_bfs, constraint,
    )
}

pub(crate) fn execute_nav_owned<'a, NV, E: crate::graph::Edge, G: crate::graph::Graph<NV, E>, F: Fn(E::Slot, &E::Val) -> bool + 'a>(
    graph: &'a G,
    from: id::N, to: id::N,
    edge_pred: F,
    config: Config<(), Navigated, E::Val>,
    constraint: &'a PathConstraint,
) -> CostPathIter<'a, NV, E, G, F> {
    let max_len = config.max_len.unwrap_or(graph.node_count() + 1);
    let Driver::Navigate(nav, mode) = config.driver else { unreachable!() };
    build_cost_path_iter(
        graph, from, to, edge_pred,
        config.min_len, max_len,
        config.guard, NavRef::Owned(nav), mode, constraint,
    )
}

/// Engine entry point: run a path edge by **borrowing** its config (which lives
/// in the compiled `Query`) and yield matched paths as node lists. Dispatches on
/// the driver — dfs/bfs traversal, or cost-navigation borrowing the navigator.
/// Navigated paths come out cheapest-first.
pub(crate) fn execute_paths<'a, NV: 'a, E: crate::graph::Edge + 'a, G: crate::graph::Graph<NV, E>>(
    graph: &'a G,
    from: id::N, to: id::N,
    edge_pred: &'a dyn Fn(E::Slot, &E::Val) -> bool,
    config: &'a Config<(), Unset, E::Val>,
    constraint: &'a PathConstraint,
) -> Box<dyn Iterator<Item = Vec<id::N>> + 'a> {
    let max_len = config.max_len.unwrap_or(graph.node_count() + 1);
    match &config.driver {
        Driver::Stack(d) => Box::new(build_path_iter(
            graph, from, to, edge_pred, config.min_len, max_len,
            config.guard.clone(), d.clone(), false, constraint,
        )),
        Driver::Queue(d) => Box::new(build_path_iter(
            graph, from, to, edge_pred, config.min_len, max_len,
            config.guard.clone(), d.clone(), true, constraint,
        )),
        Driver::Navigate(nav, mode) => Box::new(build_cost_path_iter(
            graph, from, to, edge_pred, config.min_len, max_len,
            config.guard.clone(), NavRef::Borrowed(&**nav), *mode, constraint,
        ).map(|(_cost, path)| path)),
    }
}

#[cfg(test)]
mod repro_tests {
    use crate::id;
    use crate::search::path::{Config, PathConstraint};

    type ER = crate::graph::edge::Undir<()>;

    /// Square 0-1-2-3-0, undirected. Search cycles from node 0 back to node 0.
    /// EXPECTED (simple cycle): the only simple cycle is [0,1,2,3,0] (and its reverse).
    /// BUG: undirected DFS U-turns along the same edge, yielding degenerate
    /// 2-node "cycles" like [0,1,0], [0,3,0].
    #[test]
    fn undirected_cycle_from_node_to_itself() {
        let g = crate::mgraph![<(), ER>;
            N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3), n(3) ^ n(0)
        ].unwrap();

        let pc = PathConstraint::unconstrained();
        let cfg = Config::new(()).dfs();
        let paths: Vec<Vec<id::N>> = g
            .path_search(id::N(0), id::N(0), |_s, _v| true, cfg, &pc)
            .collect();

        eprintln!("paths from 0 to 0:");
        for p in &paths {
            eprintln!("  {:?}", p.iter().map(|n| *n).collect::<Vec<_>>());
        }

        // A simple cycle must have at least 3 distinct nodes (length >= 4 incl. closing).
        let degenerate: Vec<_> = paths.iter().filter(|p| p.len() < 4).collect();
        assert!(
            degenerate.is_empty(),
            "degenerate (edge-reusing) cycles returned: {:?}",
            degenerate.iter().map(|p| p.iter().map(|n| *n).collect::<Vec<_>>()).collect::<Vec<_>>()
        );

        // The real square cycle must be found.
        let has_square = paths.iter().any(|p| p.len() == 5);
        assert!(has_square, "the real 4-node square cycle [0,1,2,3,0] was not found");
    }

    /// `.len` gates the trivial zero-length cycle [v] (no edge reused, start == end).
    /// len(0..) yields it; the default len(1..) rejects it.
    #[test]
    fn dfs_trivial_cycle_gated_by_len() {
        let g = crate::mgraph![<(), ER>;
            N(0) ^ N(1), n(1) ^ N(2), n(2) ^ n(0)
        ].unwrap();
        let pc = PathConstraint::unconstrained();

        // len(0..): trivial [0] is a valid result.
        let with_trivial: Vec<Vec<id::N>> = g
            .path_search(id::N(0), id::N(0), |_s, _v| true, Config::new(()).dfs().len(0..), &pc)
            .collect();
        assert!(with_trivial.iter().any(|p| p == &[id::N(0)]),
            "len(0..) should yield the trivial [0], got {:?}",
            with_trivial.iter().map(|p| p.iter().map(|n| *n).collect::<Vec<_>>()).collect::<Vec<_>>());

        // default (len 1..): trivial [0] rejected, real triangle still found, no degenerate.
        let default: Vec<Vec<id::N>> = g
            .path_search(id::N(0), id::N(0), |_s, _v| true, Config::new(()).dfs(), &pc)
            .collect();
        assert!(!default.iter().any(|p| p.len() < 4),
            "default len rejects trivial and degenerate cycles, got {:?}",
            default.iter().map(|p| p.iter().map(|n| *n).collect::<Vec<_>>()).collect::<Vec<_>>());
        assert!(default.iter().any(|p| p.len() == 4), "triangle cycle should be found");
    }

    /// A genuine *directed* 2-cycle uses two distinct edge records (0→1 and 1→0),
    /// so it must NOT be suppressed by the undirected U-turn guard.
    #[test]
    fn directed_two_cycle_preserved() {
        type DER = crate::graph::edge::Dir<()>;
        let g = crate::mgraph![<(), DER>;
            N(0) >> N(1), n(1) >> n(0)
        ].unwrap();

        let pc = PathConstraint::unconstrained();
        let cfg = Config::new(()).dfs();
        let paths: Vec<Vec<id::N>> = g
            .path_search(id::N(0), id::N(0), |_s, _v| true, cfg, &pc)
            .collect();

        let two_cycle = paths.iter().any(|p| p == &[id::N(0), id::N(1), id::N(0)]);
        assert!(two_cycle, "directed 2-cycle [0,1,0] via distinct edges must be found, got {:?}",
            paths.iter().map(|p| p.iter().map(|n| *n).collect::<Vec<_>>()).collect::<Vec<_>>());
    }

    /// Navigated (Dijkstra) cycle search from a node back to itself.
    /// Triangle 0-1-2-0: the shortest cycle through 0 is [0,1,2,0] (or reverse).
    #[test]
    fn navigated_cycle_can_close() {
        use crate::search::path::Dijkstra;
        let g = crate::mgraph![<(), ER>;
            N(0) ^ N(1), n(1) ^ N(2), n(2) ^ n(0)
        ].unwrap();

        let pc = PathConstraint::unconstrained();
        // Cycles need `.all()` — `.one()` (the default) settles the source and
        // cannot close a cycle through it.
        let cfg = Config::new(()).navigate(Dijkstra::counted::<()>()).all();
        let paths: Vec<(f64, Vec<id::N>)> = g
            .path_navigate(id::N(0), id::N(0), |_s, _v| true, cfg, &pc)
            .collect();

        eprintln!("navigated paths from 0 to 0:");
        for (c, p) in &paths {
            eprintln!("  cost={} {:?}", c, p.iter().map(|n| *n).collect::<Vec<_>>());
        }

        let degenerate: Vec<_> = paths.iter().filter(|(_, p)| p.len() < 4).collect();
        assert!(degenerate.is_empty(), "degenerate navigated cycles: {:?}",
            degenerate.iter().map(|(_, p)| p.iter().map(|n| *n).collect::<Vec<_>>()).collect::<Vec<_>>());

        let has_triangle = paths.iter().any(|(_, p)| p.len() == 4);
        assert!(has_triangle, "navigated cycle should close around the triangle");
    }

    /// `.len` gates the trivial zero-length navigated path, just like dfs/bfs.
    /// The cost-0 trivial path is valid (no edge reused, start == end), not a bug.
    #[test]
    fn navigated_trivial_path_gated_by_len() {
        use crate::search::path::Dijkstra;
        let g = crate::mgraph![<(), ER>;
            N(0) ^ N(1), n(1) ^ N(2), n(2) ^ n(0)
        ].unwrap();
        let pc = PathConstraint::unconstrained();

        // len(0..): trivial [0] at cost 0 is the first (cheapest) result.
        let with: Vec<(f64, Vec<id::N>)> = g.path_navigate(
            id::N(0), id::N(0), |_s, _v| true,
            Config::new(()).navigate(Dijkstra::counted::<()>()).all().len(0..), &pc,
        ).collect();
        assert_eq!(with.first().map(|(c, p)| (*c, p.clone())), Some((0.0, vec![id::N(0)])),
            "len(0..) should yield trivial [0] at cost 0 first, got {:?}", with);

        // default len(1..): trivial rejected, every result is a real cycle (cost >= 1).
        let def: Vec<(f64, Vec<id::N>)> = g.path_navigate(
            id::N(0), id::N(0), |_s, _v| true,
            Config::new(()).navigate(Dijkstra::counted::<()>()).all(), &pc,
        ).collect();
        assert!(!def.iter().any(|(_, p)| p.len() < 4), "trivial/degenerate rejected, got {:?}", def);
        assert!(def.iter().all(|(c, _)| *c >= 1.0));
    }

    /// Navigated results come out in non-decreasing cost order; the shortest
    /// weighted path is first.
    #[test]
    fn navigated_enumerates_in_increasing_cost() {
        use crate::search::path::Dijkstra;
        type W = crate::graph::edge::Undir<u32>;
        // 0—1(1)—3(1) => cost 2 ; 0—2(5)—3(1) => cost 6
        let g = crate::mgraph![<(), W>;
            N(0) & E().val(1u32) ^ N(1),
            n(1) & E().val(1u32) ^ N(3),
            n(0) & E().val(5u32) ^ N(2),
            n(2) & E().val(1u32) ^ n(3)
        ].unwrap();
        let pc = PathConstraint::unconstrained();
        let cfg = Config::new(()).navigate(Dijkstra::weighted(|w: &u32| *w as f64)).all();
        let paths: Vec<(f64, Vec<id::N>)> = g
            .path_navigate(id::N(0), id::N(3), |_s, _v| true, cfg, &pc)
            .collect();

        let costs: Vec<f64> = paths.iter().map(|(c, _)| *c).collect();
        assert!(costs.windows(2).all(|w| w[0] <= w[1]),
            "costs must be non-decreasing, got {:?}", costs);
        assert_eq!(paths.first().map(|(c, _)| *c), Some(2.0), "shortest path (cost 2) first");
    }

    /// `.one()` (the default) returns exactly the single cheapest path and stops.
    #[test]
    fn navigated_one_returns_single_shortest() {
        use crate::search::path::Dijkstra;
        type W = crate::graph::edge::Undir<u32>;
        // 0—1(1)—3(1) => cost 2 ; 0—2(5)—3(1) => cost 6
        let g = crate::mgraph![<(), W>;
            N(0) & E().val(1u32) ^ N(1),
            n(1) & E().val(1u32) ^ N(3),
            n(0) & E().val(5u32) ^ N(2),
            n(2) & E().val(1u32) ^ n(3)
        ].unwrap();
        let pc = PathConstraint::unconstrained();
        // default is `.one()`; spell it out for clarity.
        let cfg = Config::new(()).navigate(Dijkstra::weighted(|w: &u32| *w as f64)).one();
        let paths: Vec<(f64, Vec<id::N>)> = g
            .path_navigate(id::N(0), id::N(3), |_s, _v| true, cfg, &pc)
            .collect();

        assert_eq!(paths.len(), 1, "one() yields exactly one path, got {:?}", paths);
        assert_eq!(paths[0].0, 2.0, "and it is the shortest (cost 2)");
        assert_eq!(paths[0].1, vec![id::N(0), id::N(1), id::N(3)]);
    }

    /// A `.guard` prunes during `One`-mode search via the tip-first predecessor
    /// walk: avoiding node 1 forces the costlier direct edge.
    #[test]
    fn navigated_one_guard_prunes() {
        use crate::search::path::Dijkstra;
        type W = crate::graph::edge::Undir<u32>;
        // cheap route 0—1(1)—3(1) (cost 2) through node 1; direct 0—3(10).
        let g = crate::mgraph![<(), W>;
            N(0) & E().val(1u32) ^ N(1),
            n(1) & E().val(1u32) ^ N(3),
            n(0) & E().val(10u32) ^ n(3)
        ].unwrap();
        let pc = PathConstraint::unconstrained();
        let cfg = Config::new(())
            .navigate(Dijkstra::weighted(|w: &u32| *w as f64))
            .one()
            .guard(|p| !p.contains(id::N(1))); // reject any path through node 1
        let paths: Vec<(f64, Vec<id::N>)> = g
            .path_navigate(id::N(0), id::N(3), |_s, _v| true, cfg, &pc)
            .collect();

        assert_eq!(paths.len(), 1, "got {:?}", paths);
        assert_eq!(paths[0].0, 10.0, "guard forces the direct edge");
        assert_eq!(paths[0].1, vec![id::N(0), id::N(3)]);
    }

    /// PathView ergonomics on the slice-backed (dfs) side: a guard using
    /// `.contains()` prunes every cycle that would pass through node 2.
    #[test]
    fn dfs_guard_pathview_contains() {
        let g = crate::mgraph![<(), ER>;
            N(0) ^ N(1), n(1) ^ N(2), n(2) ^ N(3), n(3) ^ n(0)
        ].unwrap();
        let pc = PathConstraint::unconstrained();
        let cfg = Config::new(()).dfs().guard(|p| !p.contains(id::N(2)));
        let paths: Vec<Vec<id::N>> = g
            .path_search(id::N(0), id::N(0), |_s, _v| true, cfg, &pc)
            .collect();
        // every simple cycle on the square runs through node 2, so all are pruned.
        assert!(paths.is_empty(), "node-2-avoiding cycles should be pruned, got {:?}", paths);
    }
}
