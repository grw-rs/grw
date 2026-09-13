# Indices

A graph can maintain **keyed node indices** — tables that map a value-derived
key straight to the node(s) that hold it, instead of scanning every node to
find them. `MGraph` and `VGraph` expose the identical API: `with_indices`,
`add_index`, `drop_index`, `catalogue`, plus `index_hit`/`catalogue`/
`index_decls` from the `Graph` trait.

## Declaring an index

An index is an `IndexDecl<NV>`: a name, a cardinality, and a key extractor.

```rust
use grw::graph::index::{Cardinality, IndexDecl, IndexName};
use grw::graph::{MGraph, edge};

const BY_VAL: IndexName = IndexName("by_val");
const BY_BUCKET: IndexName = IndexName("by_bucket");

let g: MGraph<u32, edge::Undir<()>> = grw::mgraph![
    N(0).val(10u32) ^ N(1).val(20u32) ^ N(2).val(30u32)
].unwrap();

let g = g.with_indices(vec![
    IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v)),
    IndexDecl::new(BY_BUCKET, Cardinality::Multi, |v: &u32| Some(*v % 20)),
]).unwrap();
```

`IndexDecl::new(name, cardinality, extractor)` accepts any
`Fn(&NV) -> Option<K>` where `K: Serialize + 'static`; returning `None`
excludes that node from the index entirely. `Cardinality::Unique` means at
most one node per key — a value collision refuses the whole call with
`graph::error::Index::NotUnique { index, nodes }`. `Cardinality::Multi` groups
every node sharing a key under it. `with_indices` builds every table with one
scan of the graph; a duplicate name among the declarations is
`graph::error::Index::DuplicateName`.

## Maintaining an index

Add or drop an index on a graph that already has data:

```rust
use grw::graph::index::{Cardinality, IndexDecl, IndexName};
use grw::graph::{MGraph, edge};

const BY_PARITY: IndexName = IndexName("by_parity");

let mut g: MGraph<u32, edge::Undir<()>> = grw::mgraph![
    N(0).val(10u32) ^ N(1).val(20u32)
].unwrap();

g.add_index(IndexDecl::new(BY_PARITY, Cardinality::Multi, |v: &u32| Some(*v % 2))).unwrap();
let dropped = g.drop_index(BY_PARITY).unwrap();
assert_eq!(dropped.name(), BY_PARITY);
```

`add_index` scans once to build the new table and fails the same way
`with_indices` does on a `Unique` collision. `drop_index` fails with
`graph::error::Index::NoSuchIndex` for a name that isn't declared.

From here on, every declared index is maintained **inside** `modify!` — as
part of the same atomic transaction as the rest of the batch, not a
follow-up step:

```rust
use grw::graph::index::{Cardinality, IndexDecl, IndexName};
use grw::graph::{MGraph, edge};
use grw::modify;

const BY_VAL: IndexName = IndexName("by_val");

let g: MGraph<u32, edge::Undir<()>> = grw::mgraph![N(0).val(10u32)].unwrap();
let mut g = g.with_indices(vec![
    IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v)),
]).unwrap();

// a batch that would give two nodes the same unique key is refused whole
let Err(err) = modify!(g, [N(1).val(10u32), N(2).val(20u32)]) else { panic!("expected Err") };
assert!(matches!(
    err,
    grw::modify::error::Modify::Apply(grw::modify::error::Apply::Index(
        grw::modify::error::apply::Index::DuplicateKey { index, .. }
    )) if index == BY_VAL
));
assert_eq!(g.node_count(), 1); // nothing was written, not even node 2
```

`DuplicateKey { index, existing }` names the offending index and the node
that already owns the key; when a batch collides on more than one index the
one reported is whichever comes first in catalogue (alphabetical-by-name)
order. This is the same all-or-nothing rule every other `modify!` failure
follows — see [`modify!` — Mutation](./modify.md).

## Hitting an index

`index_hit(name, key, tag)` is a `Graph` trait method, so it works
identically on `MGraph` and `VGraph`:

```rust
use grw::Graph as _;
use grw::graph::index::{Cardinality, IndexDecl, IndexHit, IndexName, KeyBytes, KeyTag};
use grw::graph::{MGraph, edge};

const BY_BUCKET: IndexName = IndexName("by_bucket");

let g: MGraph<u32, edge::Undir<()>> = grw::mgraph![
    N(0).val(1u32) ^ N(1).val(21u32) ^ N(2).val(2u32)
].unwrap();
let g = g.with_indices(vec![
    IndexDecl::new(BY_BUCKET, Cardinality::Multi, |v: &u32| Some(*v % 20)),
]).unwrap();

match g.index_hit(BY_BUCKET, &KeyBytes::of(&1u32), KeyTag::of::<u32>()).unwrap() {
    IndexHit::Many(set) => assert_eq!(set.len(), 2), // nodes 0 and 1 both key to 1
    _ => panic!("expected a Many hit"),
};
```

`IndexHit` is `One(id::N)` for a `Unique` hit, `Many(IdView<'_>)` for a
`Multi` hit with at least one member, or `None` for a key nothing holds.
`IdView` is a borrowed read view over the hit set — `iter()` (yielding
`id::N`), `len()`, `is_empty()`, and `contains(id::N)`; the set behind it
stays crate-private, so grw's public surface never exposes the bitmap
representation.
`index_hit` fails with `graph::error::Index::NoSuchIndex` for an unknown name
and `graph::error::Index::KeyTagMismatch { index, expected, got }` when the
key's type doesn't match the type the index was declared over — `KeyTag`
identifies a Rust type, not just a byte length, so passing a `u64` key
against a `u32`-keyed index is rejected rather than silently comparing the
wrong bytes.

`catalogue()` lists every declared index — name, cardinality, and tag — in
alphabetical-by-name order, regardless of declaration order. `index_decls()`
returns the same information as owned `IndexDecl<NV>` values (extractor
closures included), which is how `MGraph::from_graph` and
`VGraph::from_mgraph`/`to_mgraph` carry a source's indices across a
representation change without re-declaring them by hand.

## Key predicates in search

`search!`/`pattern!` reach these same tables directly from a pattern node
with `.key(index, value)` / `.key_in(index, [values])`, seeding the search
with the index's hit set instead of scanning every node — see
[Key Predicates](./search.md#key-predicates).

## Persistence

The `.grw` snapshot format carries the whole catalogue and every table
alongside the graph — see [Persistence](./persistence.md#indices). `save`
writes them, `load` refuses a file whose catalogue is non-empty (naming the
indices that need declarations — a plain load has nothing to check the file
against), and `load_with(path, decls)` verifies the file's catalogue against
`decls` and restores the tables straight from the file, with no rescan of
node values at all.
