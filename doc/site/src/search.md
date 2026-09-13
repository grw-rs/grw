# `search!` — Pattern Matching

`search!` compiles a pattern into a query and iterates over all morphism-valid mappings from pattern nodes to target graph nodes. Patterns are organized into **clusters** — `get` (required) and `ban` (forbidden substructures).

<svg viewBox="0 0 334 132" style="max-width:334px;display:block;margin:0.8em auto" role="img" aria-label="pattern and target"><line x1="90" y1="58" x2="147" y2="58" stroke="#7ee0a3" stroke-width="2.4"/><line x1="187" y1="58" x2="244" y2="58" stroke="#7ee0a3" stroke-width="2.4"/><circle cx="70" cy="58" r="21" fill="#7ee0a326"/><circle cx="70" cy="58" r="15" fill="#7ee0a3"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">A</text><circle cx="167" cy="58" r="21" fill="#7ee0a326"/><circle cx="167" cy="58" r="15" fill="#7ee0a3"/><text x="167" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">B</text><circle cx="264" cy="58" r="21" fill="#7ee0a326"/><circle cx="264" cy="58" r="15" fill="#7ee0a3"/><text x="264" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">C</text></svg> <svg viewBox="0 0 334 171" style="max-width:334px;display:block;margin:0.8em auto" role="img" aria-label="target"><line x1="87" y1="101" x2="150" y2="62" stroke="#46c6d6" stroke-width="2.4"/><line x1="184" y1="62" x2="247" y2="101" stroke="#46c6d6" stroke-width="2.4"/><line x1="244" y1="111" x2="90" y2="111" stroke="#46c6d6" stroke-width="2.4"/><circle cx="70" cy="111" r="21" fill="#f0a63f26"/><circle cx="70" cy="111" r="15" fill="#f0a63f"/><text x="70" y="116" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><circle cx="167" cy="52" r="21" fill="#f0a63f26"/><circle cx="167" cy="52" r="15" fill="#f0a63f"/><text x="167" y="57" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><circle cx="264" cy="111" r="21" fill="#f0a63f26"/><circle cx="264" cy="111" r="15" fill="#f0a63f"/><text x="264" y="116" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text></svg>

## DSL Primitives

| Symbol | Meaning |
|--------|---------|
| `get(morphism) { ... }` | Required pattern cluster — must be found |
| `ban(morphism) { ... }` | Forbidden pattern cluster — matches are rejected |
| `N(id)` | Pattern node |
| `n(id)` | Reference to pattern node |
| `^` `>>` `<<` | Edge operators (same as `mgraph!`) |
| `!N(id)` | Negated node — the edge must NOT exist |
| `N(id).val(v)` | Node value — exact match |
| `N(id).test(\|v\| ...)` | Node value predicate |
| `E().val(v)` | Edge value — exact match |
| `X(id)` | Context node — pinned to a specific graph node |
| `N(name)` / `n(name)` | Named pattern node / reference — names lower to ids; `pattern.lid("name")` |
| `N(id: pattern)` | Node value must match a Rust pattern (`Some(1)`, `Kind::A { .. }`, `1..=5`, `A \| B`) |
| `E(pattern)` | Edge value must match a Rust pattern |
| `X(name = node_id)` / `X(name = node_id : pattern)` | Context node pinned to a graph node, optionally checked |
| `pattern![..]` | Graph-free pattern; rejects `X`; returns a `Pattern` (query + names, via `.query()`/`.lid()`) |
| `search![&g, p]` / `search![&g, p with X(a = id), ..]` | Run a stored pattern, optionally pinning named nodes |

## Basic Usage

When `search!` receives a graph reference, it creates a `Session` that you iterate with a `for` loop:

```rust
use grw::*;
use grw::graph::edge;

// target graph: triangle
let g: graph::MUndir0 = mgraph![
    N(0) ^ (N(1) ^ (N(2) ^ n(0)))
].unwrap();

// search for edges
let session = search![&g,
    get(Mono) {
        N(0) ^ N(1)
    }
].unwrap();

for m in &session {
    let a = m.get(0).unwrap();
    let b = m.get(1).unwrap();
    println!("{:?} — {:?}", a, b);
}
```

## The Match Struct

Each iteration yields a `Match` — a mapping from pattern node local ids to graph node ids.

```rust
for m in &session {
    // get a specific pattern node's graph mapping
    let node_id: id::N = m.get(0).unwrap();

    // iterate all (pattern_local_id, graph_node_id) pairs
    for &(lid, nid) in m.iter() {
        println!("pattern {} → graph {:?}", lid.0, nid);
    }

    // just the graph node ids
    let graph_nodes: Vec<id::N> = m.values().collect();
}
```

### TranslatedMatch

For richer access to node values and adjacencies, use `translate()`:

```rust
for m in &session {
    let tm = session.translate(&m);

    // node with its value
    if let Some((nid, val)) = tm.node(0) {
        println!("pattern 0 → graph {:?} val={:?}", nid, val);
    }

    // all matched nodes with values
    for (lid, nid, val) in tm.nodes() {
        println!("{} → {:?} = {:?}", lid.0, nid, val);
    }
}
```

## Ban Clusters

Ban clusters define **forbidden substructures**. If the ban pattern matches, the overall match is rejected.

```rust
// find edges whose endpoints do NOT share a common neighbor
let session = search![&g,
    get(Mono) {
        N(0) ^ N(1)
    },
    ban(Mono) {
        n(0) ^ N(2),
        n(1) ^ n(2),
    }
].unwrap();
```

The `n(0)` and `n(1)` in the `ban` cluster refer back to the nodes defined in the `get` cluster. The ban says: "reject this match if nodes 0 and 1 share a common neighbor (node 2)."

## Sequential vs Parallel

The `Session` supports two iteration modes:

```rust
// sequential — lazy iterator, one match at a time
for m in session.iter() { /* ... */ }

// also works via IntoIterator
for m in &session { /* ... */ }

// parallel — uses rayon, collects all matches
let all: Vec<Match> = session.par_iter().collect();
```

**`session.iter()`** is single-threaded lazy backtracking — yields one match at a time. Use this when you want to process matches as a stream or stop early.

**`session.par_iter()`** partitions the search space across threads via rayon. Use this for large graphs where you need all matches and have cores to spare.

## Value Predicates

Filter matches based on node or edge values:

```rust
// find nodes where x > 10.0
let session = search![&g,
    get(Mono) {
        N(0).test(|p: &Point| p.x > 10.0) ^ N(1)
    }
].unwrap();
```

## Context Nodes

`X(name = node_id)` pins a pattern node to a specific graph node, named on the left and given its target on the right:

```rust
use grw::graph::{edge, MGraph};
use grw::{mgraph, search};

let g: MGraph<(), edge::Undir<()>> = mgraph![N(0) ^ (N(1) ^ (N(2) ^ n(0)))].unwrap();
let hub = grw::id::N(1);
let session = search![&g, get(Mono) { X(h = hub) ^ N(o) }].unwrap();
let mut neighbours: Vec<_> = session.iter().map(|m| m.get(1).unwrap()).collect();
neighbours.sort();
assert_eq!(neighbours, vec![grw::id::N(0), grw::id::N(2)]);
```

`X(h = hub)` is pinned to graph node 1 — the search only looks for nodes connected to it.

A bare `X(5)` or `X(c)` inside `search![&g, ..]` is a *translated* node with no target: the macro compiles it to an unresolved search, and building the session fails at runtime with `error::Search::BoundPatternInSession`. Use the pinned form above, or drop the graph and do the binding by hand — `search![<NV, ER>; get(Mono) { X(5) ^ N(0) }]` yields a `Search::Unresolved` whose `bind(..)` supplies the targets before the search runs.

## Negation, Bans And Pins

`!E()` bans one edge: the match is rejected only if that specific edge exists. `!X(name = id)` bans a node together with every edge attached to it in that term — node and edges are one ban, checked as a unit.

That grouping is what separates "none of these" from "not all of these together". Several independent `!E`s add up: each is checked on its own, so the match survives only when *every one* of them is absent — "none of these edges may exist". A single `!X` (or an explicit `ban { }` block) instead bans the *whole configuration* it names: the ban only fires when everything inside it holds at once, so it's satisfied — and the match survives — as soon as just one piece is missing. Two nodes `p` and `q`, both possibly connected to a node pinned to id `5`, but only `p` actually is, show the difference:

```rust
use grw::graph::{edge, MGraph};
use grw::search::{Pattern, Session};
use grw::{mgraph, pattern};

let g: MGraph<(), edge::Undir<()>> = mgraph![N(0), N(1), N(2), n(0) ^ n(2)].unwrap();
let (p_id, q_id, five) = (grw::id::N(0), grw::id::N(1), grw::id::N(2));

// "not all of these together": one ban_only node shared by both edges.
let and_form: Pattern<(), edge::Undir<()>> = pattern![
    get(Mono) { N(p), N(q) },
    ban(Mono) { n(p) ^ N(c), n(q) ^ n(c) }
].unwrap();
let and_session =
    Session::from_pattern(and_form, &g, &[("p", p_id), ("q", q_id), ("c", five)]).unwrap();
assert_eq!(and_session.iter().count(), 1); // only p—5 holds, so the ban can't fire

// "none of these": two separate bans, each pinned to the same node 5.
let or_form: Pattern<(), edge::Undir<()>> = pattern![
    get(Mono) { N(p), N(q) },
    ban(Mono) { n(p) ^ N(c1) },
    ban(Mono) { n(q) ^ N(c2) }
].unwrap();
let or_session =
    Session::from_pattern(or_form, &g, &[("p", p_id), ("q", q_id), ("c1", five), ("c2", five)]).unwrap();
assert_eq!(or_session.iter().count(), 0); // p—5 alone is enough to fire the first ban
```

`pattern!` rejects `X(..)` outright (see [Stored Patterns And Pinning](#stored-patterns-and-pinning)), so a ban-only position pinned by external id — rather than by `.val()`/`.test()`/`.key()` — is spelled with a plain, named `N(c)` and bound afterwards through `Session::from_pattern`'s pins, exactly as `and_form`/`or_form` do above.

A pin on a banned position aims the ban at *one specific* node instead of the whole graph: "`p` must not be connected like this to Alice" is not the same claim as "`p` must not be connected like this to anyone". Say `p1` blocks Alice, `p2` blocks only Bob, and `p3` blocks nobody:

```rust
use grw::graph::{edge, MGraph};
use grw::{mgraph, search};

let g: MGraph<bool, edge::Dir<()>> = mgraph![
    N(0).val(true), N(1).val(true), N(2).val(true),
    N(3).val(false), N(4).val(false),
    n(0) >> n(3), // p1 -> alice
    n(1) >> n(4)  // p2 -> bob
].unwrap();
let alice = grw::id::N(3);

let session = search![&g,
    get(Mono) { N(p).val(true) >> (!X(a = alice)).test(|_: &bool| true) }
].unwrap();
let mut survivors: Vec<_> = session.iter().map(|m| m.get(0).unwrap()).collect();
survivors.sort();
assert_eq!(survivors, vec![grw::id::N(1), grw::id::N(2)]); // p2 and p3 — only p1 blocks Alice
```

Only `p1` is dropped: the ban is checked against node `alice` specifically, not against every node `p` happens to reach. The pinned node itself, `a`, is never part of the match — it has no place in the `get` cluster's mapping, so `m.get(..)` for it returns nothing; only the surviving `p` shows up in `survivors`.

## Named Nodes

`N(name)` defines a pattern node by name instead of by integer id; `n(name)` refers back to it. Names lower to integer local ids at macro expansion: indices are assigned starting one past the largest integer id used anywhere in the pattern (or from `0` if no integer ids are used). Every stored `Pattern` carries a `Names` table pairing each name with its `LocalId`; look one up with `pattern.lid("name")`.

```rust
use grw::graph::{edge, Graph, MGraph};
use grw::search::{Pattern, RevCsr, Seq};
use grw::{mgraph, pattern};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Role { Signer, Provider }

let g: MGraph<Role, edge::Dir<()>> = mgraph![N(0).val(Role::Signer) >> N(1).val(Role::Provider)].unwrap();
let p: Pattern<Role, edge::Dir<()>> = pattern![get(Mono) { N(s: Role::Signer) >> N(p: Role::Provider) }].unwrap();
let indexed = g.index(RevCsr);
let matches: Vec<_> = Seq::search(p.query(), &indexed).unwrap().collect();
assert_eq!(matches.len(), 1);
for m in &matches {
    assert_eq!(m[p.lid("s").unwrap()], grw::id::N(0));
    assert_eq!(m[p.lid("p").unwrap()], grw::id::N(1));
}
```

A duplicate definition in a cluster, an undefined reference, or `X(..)` used inside `pattern!` are all compile errors — caught while expanding the macro, before the pattern ever runs. Looking up a name that isn't in the pattern (`pattern.lid("nope")`) is a runtime error instead: `error::Search::UnknownName`.

## Value Patterns

`N(id: pattern)` and `N(name: pattern)` require the node's value to match a Rust pattern — anything valid on the right of a `match` arm: `Some(1)`, `Kind::A { .. }`, `1..=5`, `A | B`. `E(pattern)` does the same for an edge's value.

```rust
use grw::graph::{edge, Graph, MGraph};
use grw::search::{Pattern, RevCsr, Seq};
use grw::{mgraph, pattern};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape { Circle(u8), Square(u8) }

let g: MGraph<Shape, edge::Undir<u8>> =
    mgraph![N(0).val(Shape::Circle(1)) & E().val(7u8) ^ N(1).val(Shape::Square(2))].unwrap();
let nodes: Pattern<Shape, edge::Undir<u8>> =
    pattern![get(Mono) { N(c: Shape::Circle(_)) ^ N(s: Shape::Square(2)) }].unwrap();
let edges: Pattern<Shape, edge::Undir<u8>> = pattern![get(Mono) { N(a) & E(7) ^ N(b) }].unwrap();
let indexed = g.index(RevCsr);
assert_eq!(Seq::search(nodes.query(), &indexed).unwrap().count(), 1);
assert_eq!(Seq::search(edges.query(), &indexed).unwrap().count(), 2);
```

`pattern![..]` builds a `Pattern<NV, ER>` without a graph: it rejects `X(..)` context nodes at compile time (`pattern!: context nodes X(..) are not allowed; use search![&g, ...]`) and returns `Result<Pattern<NV, ER>, error::Search>`, giving you `query()`, `names()`, and `lid()`.

## Key Predicates

A node with an [index](./indices.md) declared on the target graph can be
matched by key instead of by value predicate: `.key(index, value)` seeds the
match with exactly the index's hit set for that key, `.key_in(index, [values])`
unions the hits for several keys. Both exist on `N`/`X` (free, context, and
negated-context nodes) in the same typestate slot as `.test`, and `.test`
after `.key`/`.key_in` composes into a conjunction — the closure is
evaluated only on the nodes the key already selected, not on the whole
graph. A macro grammar form — `N(a: key(INDEX, expr))` / `N(a: key_in(INDEX, [exprs]))`,
and `X(name = id : key(INDEX, expr))` on a pinned context node — binds
identically to the method-call form.

```rust
use grw::graph::index::{Cardinality, IndexDecl, IndexName};
use grw::graph::{edge, MGraph};
use grw::{mgraph, search};

const BY_VAL: IndexName = IndexName("by_val");

let g: MGraph<u32, edge::Undir<()>> = mgraph![
    N(0).val(10u32) ^ (N(1).val(20u32) ^ N(2).val(30u32))
].unwrap();
let g = g.with_indices(vec![
    IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v)),
]).unwrap();

// method form and macro form bind identically
let by_key = search![&g, get(Mono) { N(a).key(BY_VAL, 20u32) ^ N(b) }].unwrap();
let macro_form = search![&g, get(Mono) { N(a: key(BY_VAL, 20u32)) ^ N(b) }].unwrap();
assert_eq!(by_key.iter().count(), 2); // 20—10 and 20—30
assert_eq!(macro_form.iter().count(), 2);

// key_in unions its hits
let in_set = search![&g, get(Mono) { N(a).key_in(BY_VAL, [10u32, 30u32]) ^ N(b) }].unwrap();
assert_eq!(in_set.iter().count(), 2); // 10—20 and 30—20

// a pinned context node can carry a key check too, narrowing the pin itself
let ten = grw::id::N(0);
let pinned = search![&g, get(Mono) { X(c = ten : key(BY_VAL, 10u32)) ^ N(o) }].unwrap();
assert_eq!(pinned.iter().count(), 1);
```

### The `Result` change

Because a key predicate is checked against the graph's catalogue *before*
any matching starts, every entry point that can begin a search now returns
`Result<_, error::Search>` instead of the bare iterator or output — even for
a pattern with no key predicate at all, since the check happens at the same
call: `Seq::search`, `Seq::search_bound`, `Seq::search_watched`, `Par::search`,
and `Par::search_bound`. `search![&g, ..]` already returned a `Result` (it
always could fail to build a `Session`), so that call site is unchanged;
only the lower-level, index-explicit calls shown in
[Index Tiers](./index-tiers.md) gained the `?`/`.unwrap()`.

Two errors come out of that check, both naming the offending index:

- `error::Search::IndexMissing { index }` — the pattern names an index the
  graph doesn't carry at all.
- `error::Search::KeyTagMismatch { index }` — the graph has that index, but
  it was declared over a different key type than the one `.key`/`.key_in`
  serialized here.

```rust
use grw::graph::index::IndexName;
use grw::graph::{edge, MGraph};
use grw::search::{error, RevCsr, Search, Seq};
use grw::{mgraph, search, Graph as _};

const BY_VAL: IndexName = IndexName("by_val");

let plain: MGraph<u32, edge::Undir<()>> = mgraph![N(0).val(10u32)].unwrap();
let Search::Resolved(r) = search![<u32, edge::Undir<()>>;
    get(Mono) { N(a).key(BY_VAL, 10u32) }
].unwrap() else { panic!() };

let indexed = plain.index(RevCsr);
let Err(error::Search::IndexMissing { index }) = Seq::search(r.query(), &indexed) else {
    panic!("expected IndexMissing")
};
assert_eq!(index, BY_VAL);
```

`Session::candidate_pool(idx)` exposes the resolved pool for pattern node
`idx` as a diagnostic accessor: the ids a key predicate narrowed that node
to, or the ids a closure predicate admits. It is `None` when the node
carries no predicate at all, or when `idx` is not a pattern node of the
session's query — it never panics on an out-of-range index.

## Stored Patterns And Pinning

A `Pattern` from `pattern![..]` is consumed by the search it is handed to: `search![&g, p]` runs it as written, and `search![&g, p with X(a = id), X(b = id), ..]` pins named nodes to concrete graph ids first. A `Pattern` holds boxed predicates, so it is not `Clone`; to run the same shape more than once, build it in a constructor function and call that per search — `search![&g, cif_incident()]`.

```rust
use grw::graph::{edge, MGraph};
use grw::search::Pattern;
use grw::{mgraph, pattern, search};

let g: MGraph<u8, edge::Undir<()>> = mgraph![N(0).val(1u8) ^ (N(1).val(2u8) ^ N(2).val(3u8))].unwrap();
let p: Pattern<u8, edge::Undir<()>> = pattern![get(Mono) { N(a) ^ N(b) }].unwrap();
let middle = grw::id::N(1);
let s = search![&g, p with X(a = middle)].unwrap();
assert_eq!(s.iter().count(), 2);
```

Pinning the same name twice, literally, in one `with` clause is a compile error (``node `a` pinned twice``). Everything downstream of that is checked at runtime once the target ids are known: an unknown name is `error::Search::UnknownName`, pinning the same name twice through the manual `Session::from_pattern` API is `error::Search::DuplicatePin`, pinning to a graph node that doesn't exist is `error::Search::TargetMissing`, and two different pins landing on the same target node under an injective morphism is `error::Search::Bind` (`BindError::Collision`).
