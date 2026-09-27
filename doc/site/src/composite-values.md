# Composite Values

Between two nodes a graph holds at most **one link per slot** (see
[Graph Model](./graph-model.md)). When one link must say several things at
once — two nodes that are related in more than one way — its value is a
**composite**: a set of typed **parts**, at most one part per **kind**. The
link stays one link; its value grows and shrinks part by part.

The `Composite` trait is the contract: build a value from one part, list its
parts, look one up by kind, include a part, exclude a kind. Each refusal is
the combinator's own typed error. grw ships one combinator, `TypeSet<P>`, and
a downstream crate can write its own.

The snippets in this chapter share these imports and declarations:

```rust
use grw::prelude::*;
use grw::composite::{Included, KindOf, Kinded, Removed, TypeSet};
use grw::edge::{Anydir, Dir};
use grw::graph::{Graph, MGraph};
use grw::modify::error::{apply, Apply, Modify};
use grw::id;
```

## Declaring the parts: `#[derive(Part)]`

Each variant of a part enum is one kind, and holds exactly one payload type:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Signs {
    pub since: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Uses {
    pub service: u32,
}

#[derive(Debug, Clone, PartialEq, Part)]
pub enum Rel {
    Signs(Signs),
    Uses(Uses),
}

pub type Link = TypeSet<Rel>;
```

`#[derive(Part)]` generates a fieldless kind enum (`RelKind { Signs, Uses }`),
implements `Kinded` for `Rel`, `From<Signs>` / `From<Uses>` for `Rel`, and
`PartOf<Signs>` / `PartOf<Uses>`, which name the static kind of each payload
type. From those, every composite over `Rel` answers by type:
`get::<Signs>()`, `include(Uses { .. })`, `exclude::<Signs>()` (the `Typed`
trait) and, in patterns, `has::<Signs>()`. The derive refuses a struct, a
generic enum, a variant that is not a single-field tuple variant, and two
variants with the same payload type.

A part type does not need the derive: any type implementing `Kinded` can be
the part of a `TypeSet`, and is then used through the raw-part forms
(`include(part)`, `exclude_kind(kind)`, `has_kind(kind)`).

## `TypeSet`

`TypeSet<P>` keeps its parts in kind order, one per kind. Including an equal
part is `Included::Held` and changes nothing; including a different part of a
kind already held is refused (`KindPresent`); excluding a kind not held is
refused (`NotHeld`); excluding the last part leaves nothing, and the link is
removed.

```rust
let link = Link::from_part(Rel::from(Signs { since: 2023 }));
let Ok(Included::Added(link)) = link.include(Uses { service: 7 }) else {
    panic!("Uses is new to the link");
};

assert_eq!(link.get::<Signs>(), Some(&Signs { since: 2023 }));
assert_eq!(link.parts().count(), 2);
assert_eq!(link.include(Uses { service: 7 }).ok(), Some(Included::Held));
assert_eq!(link.include(Uses { service: 8 }).err().map(|refused| refused.kind), Some(RelKind::Uses));
assert_eq!(link.exclude::<Uses>().ok(), Some(Removed::Remains(Link::from_part(Rel::from(Signs { since: 2023 })))));
```

Part operations work wherever the value a slot holds is a composite. On a
`Dir` or `Undir` graph that is the edge value itself. On an `Anydir<U, D>`
graph the operator picks the slot, and with it the value the part belongs to:
see [Anydir graphs](#anydir-graphs-the-operator-picks-the-slot).

## Building: `mgraph!` / `vgraph!`

`E().include(p)` puts a part on the link; further `.include(q)` calls add more
parts to the same link. Naming one kind twice on one link is a typed build
error (`graph::error::Part::Repeated`), even when the two parts are equal.

```rust
let g = mgraph![<(), Dir<Link>>;
    N(0) & E().include(Signs { since: 2023 }).include(Uses { service: 7 }) >> N(1),
    n(1) & E().include(Uses { service: 9 }) >> N(2),
]
.unwrap();
assert_eq!(g.edge_count(), 2);

let twice = mgraph![<(), Dir<Link>>;
    N(0) & E().include(Signs { since: 2023 }).include(Signs { since: 2024 }) >> N(1)
];
assert!(twice.is_err());
```

`E().val(whole)` still takes a whole composite value as given.

## Changing: `modify!`

```rust
let mut g = mgraph![<(), Dir<Link>>;
    N(0) & E().include(Signs { since: 2023 }) >> N(1),
    n(1) & E().include(Uses { service: 9 }) >> N(2),
]
.unwrap();

modify!(g, [X(0) & E().include(Uses { service: 7 }) >> x(1)]).unwrap();
modify!(g, [X(0) & E().include(Uses { service: 4 }) >> x(2)]).unwrap();
modify!(g, [X(0) & E().exclude::<Signs>() >> x(1)]).unwrap();
modify!(g, [X(1) & E().exclude_kind(RelKind::Uses) >> x(2)]).unwrap();
modify!(g, [X(0) & !e() >> x(2)]).unwrap();

assert_eq!(g.edge_count(), 1);
let (_, link) = g.edges_between(id::N(0), id::N(1)).next().unwrap();
assert_eq!(link.get::<Uses>(), Some(&Uses { service: 7 }));
assert_eq!(link.get::<Signs>(), None);
```

| Form | Effect |
|------|--------|
| `E().include(p)` | the link holds `p`; on a vacant pair the link is created |
| `E().exclude::<T>()` | the link drops its `T` part |
| `E().exclude_kind(k)` | the same, with the kind chosen at runtime |
| `!e()` | the whole link is deleted, every part with it |

Part operations on one pair in one `modify!` fold in order into one change of
the link: an add, a value swap, a removal, or nothing when the result equals
the stored value.

### Refusals

| Error (`modify::error::apply::Edge`) | When |
|------|------|
| `PartRejected { refused, .. }` | the combinator refused an include |
| `PartNotHeld { refused, .. }` | the combinator refused an exclude |
| `PartConflict(a, b)` | one `modify!` changes a link both whole (`e().val`, `!e()`, a new `E()`) and part by part |

`refused` carries the combinator's own refusal, unchanged; read it back by
naming the combinator: `refused.of::<Link>()`. A refused batch applies
nothing.

```rust
let mut g = mgraph![<(), Dir<Link>>; N(0) & E().include(Signs { since: 2023 }) >> N(1)].unwrap();

match modify!(g, [X(0) & E().include(Signs { since: 1999 }) >> x(1)]).err() {
    Some(Modify::Apply(Apply::Edge(apply::Edge::PartRejected { refused, .. }))) => {
        assert_eq!(refused.of::<Link>().map(|r| r.kind), Some(RelKind::Signs));
    }
    other => panic!("expected PartRejected, got {other:?}"),
}

match modify!(g, [X(0) & E().exclude::<Uses>() >> x(1)]).err() {
    Some(Modify::Apply(Apply::Edge(apply::Edge::PartNotHeld { refused, .. }))) => {
        assert_eq!(refused.of::<Link>().map(|r| r.kind), Some(RelKind::Uses));
    }
    other => panic!("expected PartNotHeld, got {other:?}"),
}

let whole = Link::from_part(Rel::from(Uses { service: 7 }));
let both = modify!(g, [X(0) & e().val(whole) >> x(1), x(0) & E().exclude::<Signs>() >> x(1)]);
assert!(matches!(both.err(), Some(Modify::Apply(Apply::Edge(apply::Edge::PartConflict(_, _))))));
```

## A combinator of your own

What an empty composite means is the combinator's decision. `TypeSet` says the
link is gone. A combinator that keeps a hollow link says otherwise — this one
answers `Removed::Remains` with no parts left:

```rust
#[derive(Debug, thiserror::Error)]
#[error("the link already holds another {0:?}")]
pub struct OtherPartHeld<K: std::fmt::Debug>(pub K);

#[derive(Debug, thiserror::Error)]
#[error("the link holds no {0:?}")]
pub struct NothingToExclude<K: std::fmt::Debug>(pub K);

#[derive(Debug, Clone, PartialEq)]
pub struct KeepHollow<P> {
    parts: Vec<P>,
}

impl<P: Kinded> KeepHollow<P> {
    pub fn is_hollow(&self) -> bool {
        self.parts.is_empty()
    }

    fn at(&self, kind: &P::Kind) -> Result<usize, usize> {
        self.parts.binary_search_by(|held| held.kind().cmp(kind))
    }
}

impl<P: Kinded + Clone + PartialEq> Composite for KeepHollow<P> {
    type Part = P;
    type IncludeRefused = OtherPartHeld<P::Kind>;
    type ExcludeRefused = NothingToExclude<P::Kind>;

    fn from_part(part: P) -> Self {
        KeepHollow { parts: vec![part] }
    }

    fn parts(&self) -> impl Iterator<Item = &P> {
        self.parts.iter()
    }

    fn part(&self, kind: &KindOf<Self>) -> Option<&P> {
        self.at(kind).ok().map(|i| &self.parts[i])
    }

    fn with_part(&self, part: P) -> Result<Included<Self>, OtherPartHeld<P::Kind>> {
        match self.at(&part.kind()) {
            Ok(i) if self.parts[i] == part => Ok(Included::Held),
            Ok(_) => Err(OtherPartHeld(part.kind())),
            Err(i) => {
                let mut parts = self.parts.clone();
                parts.insert(i, part);
                Ok(Included::Added(KeepHollow { parts }))
            }
        }
    }

    fn without_part(&self, kind: &KindOf<Self>) -> Result<Removed<Self>, NothingToExclude<P::Kind>> {
        match self.at(kind) {
            Ok(i) => {
                let mut parts = self.parts.clone();
                parts.remove(i);
                Ok(Removed::Remains(KeepHollow { parts }))
            }
            Err(_) => Err(NothingToExclude(kind.clone())),
        }
    }
}
```

The graph DSLs and the engine work with any `Composite`:

```rust
let mut g = mgraph![<(), Dir<KeepHollow<Rel>>>;
    N(0) & E().include(Signs { since: 2023 }) >> N(1)
]
.unwrap();

modify!(g, [X(0) & E().exclude::<Signs>() >> x(1)]).unwrap();

assert_eq!(g.edge_count(), 1);
assert!(g.edges_between(id::N(0), id::N(1)).all(|(_, link)| link.is_hollow()));
```

## Searching: `has`

The examples search this graph:

```rust
fn hub() -> MGraph<(), Dir<Link>> {
    mgraph![<(), Dir<Link>>;
        N(0) & E().include(Signs { since: 2021 }) >> N(1),
        n(0) & E().include(Signs { since: 2022 }) >> N(2),
        n(0) & E().include(Signs { since: 2023 }) >> N(3),
        n(1) & E().include(Signs { since: 2024 }) >> n(2),
        n(2) & E().include(Uses { service: 7 }) >> n(3),
    ]
    .unwrap()
}
```

```rust
let g = hub();

let signed = search![&g, get(Mono) { N(0) & E().has::<Signs>() >> N(1) }].unwrap();
assert_eq!(signed.iter().count(), 4);

let recent = search![&g, get(Mono) {
    N(0) & E().has::<Signs>().test(|s: &Signs| s.since >= 2023) >> N(1)
}]
.unwrap();
assert_eq!(recent.iter().count(), 2);

let raw = search![&g, get(Mono) {
    N(0) & E().has_kind(RelKind::Uses).test(|p: &Rel| matches!(p, Rel::Uses(u) if u.service == 7)) >> N(1)
}]
.unwrap();
assert_eq!(raw.iter().count(), 1);

let whole = search![&g, get(Mono) { N(0) & E().test(|v: &Link| v.parts().count() == 1) >> N(1) }].unwrap();
assert_eq!(whole.iter().count(), 5);
```

| Form | Matches |
|------|---------|
| `E().has::<T>()` | a link holding a `T` part, whatever else it holds |
| `E().has::<A>().has::<B>()` | a link holding both |
| `E().has::<T>().test(\|t: &T\| ..)` | the predicate reads the part named just before it |
| `E().has_kind(k)` | the same with a runtime kind; `.test` then reads the raw part |
| `E().test(\|v: &Link\| ..)` | with no `has` before it, the predicate reads the whole value |
| `!E().has::<T>()` | no link holding a `T` part — another part may still link the pair |
| `!E()` | no link at all |

`has::<T>()` compiles only where the link's value has a `T` part, and a
`.test` closure must take the part its `has` named.

`!E().has::<T>()` and `!E()` differ exactly where a pair is linked by some
other part:

```rust
let g = hub();

let no_uses = search![&g, get(Mono) {
    N(0) & E().has::<Signs>() >> N(1),
    n(0) & E().has::<Signs>() >> N(2),
    n(1) & !E().has::<Uses>() >> n(2)
}]
.unwrap();
assert_eq!(no_uses.iter().count(), 5);

let no_link = search![&g, get(Mono) {
    N(0) & E().has::<Signs>() >> N(1),
    n(0) & E().has::<Signs>() >> N(2),
    n(1) & !E() >> n(2)
}]
.unwrap();
assert_eq!(no_link.iter().count(), 4);
```

### Terms on one pair merge

All terms on one pair and slot within a cluster become one constraint on the
link — a conjunction. `E().has::<A>()` beside `!E().has::<B>()` means "holds A
and not B", as one pattern edge:

```rust
let g = hub();

let merged = search![&g, get(Mono) {
    N(0) & E().has::<Signs>() >> N(1),
    n(0) & !E().has::<Uses>() >> n(1)
}]
.unwrap();
assert_eq!(merged.query().edge_count(), 1);
assert_eq!(merged.iter().count(), 4);

let contradiction = search![&g, get(Mono) {
    N(0) & E().has::<Signs>() >> N(1),
    n(0) & !E().has::<Signs>() >> n(1)
}];
assert!(contradiction.is_err());
```

The query refuses what could never match, with typed errors of
`search::error::Edge`:

- `PartRepeated` — one kind required twice on one link;
- `Contradictory` — what one term requires another forbids: `!E()` beside
  a positive term, or an untested `!E().has::<A>()…` beside a term that
  requires every part it names;
- `ConflictingPred` — a `ban` cluster putting a part term on a pattern edge
  its `get` cluster already declared.

## Anydir graphs: the operator picks the slot

An `Anydir<U, D>` graph holds an undirected value `U` and a directed value `D`
between the same two nodes, each in its own slot (its edge value is
`AnyVal<U, D>`). `include`, `exclude`, `exclude_kind`, `has` and `has_kind`
wait for the operator: on `^` they work on the `U` held in the undirected
slot, on `>>` / `<<` on the `D` held in the directed slot. The graph literal's
`include` waits the same way. The two values may be different composites:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Knows {
    pub since: u32,
}

#[derive(Debug, Clone, PartialEq, Part)]
pub enum Tie {
    Knows(Knows),
}

pub type Bond = TypeSet<Tie>;
```

```rust
let mut g = mgraph![<(), Anydir<Bond, Link>>;
    N(0) & E().include(Knows { since: 2019 }) ^ N(1),
    n(0) & E().include(Signs { since: 2023 }) >> n(1),
]
.unwrap();
assert_eq!(g.edge_count(), 2);

modify!(g, [X(0) & E().include(Uses { service: 7 }) >> x(1), x(1) & E().exclude::<Knows>() ^ x(0)]).unwrap();
assert_eq!(g.edge_count(), 1);

let known = search![&g, get(Mono) { N(0) & E().has::<Knows>() ^ N(1) }].unwrap();
assert_eq!(known.iter().count(), 0);
let used = search![&g, get(Mono) { N(0) & E().has::<Signs>().has::<Uses>() >> N(1) }].unwrap();
assert_eq!(used.iter().count(), 1);
```

A part the slot's value cannot hold does not compile: `E().include(Signs { .. })`
on `^` names a part of `Rel`, not of `Tie`, and so does `E().has::<Signs>()`
on `^`. Everything else is as on `Dir`: an equal part changes nothing, a batch
that ends where it started writes nothing, and the refusals are the slot
value's own, read back with `refused.of::<Bond>()` on `^` and
`refused.of::<Link>()` on `>>` / `<<`. A value of the other class held in a
slot (only a whole-value op can put one there) is refused with
`apply::Edge::SlotClass`.

Outside `search!`, `has` and `has_kind` on a bare `E()` come from the
`grw::search::dsl::Seek` trait, which has to be in scope; `use grw::prelude::*;`
brings it in together with `mgraph!`, `vgraph!`, `modify!`, `search!`,
`pattern!`, `#[derive(Val)]`, `#[derive(Part)]`, `Typed` and `Composite`.

## Persistence

A composite value is saved as its parts in kind order; the
[snapshot format](./persistence.md) is unchanged (still v3), and the file
header's edge layout hash tells a composite file from any other. `load` /
`load_with` remain strict: a non-canonical part order is refused, never
re-sorted.

A snapshot written while each link held one plain value `P` loads into a
`TypeSet<P>` graph through the explicit promoting path only — each old link
becomes a one-part link. The part type here implements `Kinded` by hand:

```rust
use grw::prelude::*;
use grw::composite::Kinded;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Val, serde::Serialize, serde::Deserialize)]
pub enum Role {
    Owner,
    Reader,
}

#[derive(Debug, Clone, PartialEq, Val, serde::Serialize, serde::Deserialize)]
pub struct Grant {
    pub role: Role,
    pub since: u32,
}

impl Kinded for Grant {
    type Kind = Role;

    fn kind(&self) -> Role {
        self.role
    }
}
```

```rust
use grw::graph::VGraph;
use promoted::{Grant, Role};

let old: MGraph<(), Dir<Grant>> = mgraph![<(), Dir<Grant>>;
    N(0) & E().val(Grant { role: Role::Owner, since: 2020 }) >> N(1)
]
.unwrap();
let path = std::env::temp_dir().join(format!("grw_book_promoted_{}.grw", std::process::id()));
old.save(&path).unwrap();

assert!(MGraph::<(), Dir<TypeSet<Grant>>>::load(&path).is_err());
let g: MGraph<(), Dir<TypeSet<Grant>>> = MGraph::load_promoting(&path).unwrap();
let (_, link) = g.edges_between(id::N(0), id::N(1)).next().unwrap();
assert_eq!(link.parts().count(), 1);
let v: VGraph<(), Dir<TypeSet<Grant>>> = VGraph::load_promoting(&path).unwrap();
assert_eq!(v.edge_count(), 1);

std::fs::remove_file(&path).unwrap();
```

`load_promoting_with(path, decls)` restores indices as `load_with` does. A file
whose edge values are neither the composite nor its part is refused.
