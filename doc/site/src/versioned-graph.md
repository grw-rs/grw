# Versioned Graphs

`MGraph` mutates in place: `modify!` takes `&mut self` and the graph you had
is the graph you now have. `VGraph` is the same graph as a **value**. Every
edit returns a *new* graph and leaves the old one exactly as it was —
readable, searchable, and still yours.

```rust
let (g2, _) = g1.modify(modify![X(0).val(99u32)]).unwrap();

assert_eq!(g2.node_val(id::N(0)), Some(&99));
assert_eq!(g1.node_val(id::N(0)), Some(&1));   // g1 never moved
```

Nothing is copied to make that true. Nodes and edges live in a persistent
32-way trie; an edit rebuilds only the path from the root to what changed and
shares every untouched subtree with its parent version. Cloning a `VGraph`
copies a handful of trie roots and bumps some refcounts — it does not walk the
graph.

<svg viewBox="0 0 470 190" style="max-width:470px;display:block;margin:1em auto" role="img" aria-label="Three versions sharing untouched subtrees; only the path to the edited node is rebuilt">
  <text x="60" y="26" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">v1</text>
  <text x="215" y="26" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">v2</text>
  <text x="370" y="26" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">v3</text>
  <circle cx="60" cy="52" r="12" fill="#46c6d6"/>
  <circle cx="215" cy="52" r="12" fill="#46c6d6"/>
  <circle cx="370" cy="52" r="12" fill="#46c6d6"/>
  <path d="M60 64 L28 108" stroke="#46c6d6" stroke-width="2"/>
  <path d="M60 64 L120 108" stroke="#46c6d6" stroke-width="2"/>
  <path d="M215 64 L28 108" stroke="#8b95a9" stroke-width="1.6" stroke-dasharray="4 4"/>
  <path d="M215 64 L215 108" stroke="#f0a63f" stroke-width="2.4"/>
  <path d="M370 64 L28 108" stroke="#8b95a9" stroke-width="1.6" stroke-dasharray="4 4"/>
  <path d="M370 64 L370 108" stroke="#f0a63f" stroke-width="2.4"/>
  <circle cx="28" cy="120" r="12" fill="#46c6d6"/>
  <circle cx="120" cy="120" r="12" fill="#46c6d6"/>
  <circle cx="215" cy="120" r="12" fill="#f0a63f"/>
  <circle cx="370" cy="120" r="12" fill="#f0a63f"/>
  <text x="28" y="152" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">shared</text>
  <text x="248" y="152" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">rebuilt on the edited path only</text>
</svg>

That buys three things at once: **snapshots** you can hand to another thread
without a lock, **rollback** that is just keeping the older binding around,
and **history** — a version chain is nothing more than the versions you chose
not to drop.

```rust
let mut history = vec![g1.clone()];
let mut g = g1;
for i in 2..6 as grw::Id {
    let (next, _) = g.modify(modify![N(i) ^ x(0)]).unwrap();
    history.push(next.clone());
    g = next;
}

let counts: Vec<usize> = history.iter().map(|h| h.node_count()).collect();
assert_eq!(counts, vec![2, 3, 4, 5, 6]);
```

## Quick start

`vgraph!` is `mgraph!` with a versioned result — same DSL, same aliases with a
`V` in front of them:

```rust
let g: VUndir0 = vgraph![N(0) ^ (N(1) ^ (N(2) ^ n(0)))].unwrap();
assert_eq!(g.node_count(), 3);
assert_eq!(g.edge_count(), 3);
assert_eq!(g.version(), 0);
```

| mutable | versioned |
|---|---|
| `MUndir0`, `MUndir<NV, EV>` | `VUndir0`, `VUndir<NV, EV>` |
| `MDir0`, `MDir<NV, EV>` | `VDir0`, `VDir<NV, EV>` |
| `MAnydir0`, `MAnydir<NV, EV>` | `VAnydir0`, `VAnydir<NV, EV>` |

`VGraph::new()` gives an empty graph at version `0`.

## Modifying: `&self` in, a new graph out

`modify` takes `&self` and returns `(next_graph, Modification)` — the same
[`Modification`](./modify.md#modification-result) record the mutable side
returns, reporting new ids, added edges, removed nodes and swapped values.

```rust
let (g2, m) = g1.modify(modify![X(0).val(99u32)]).unwrap();
assert_eq!(m.swapped_node_vals.len(), 1);
assert_eq!(g1.version(), 0);
assert_eq!(g2.version(), 1);
```

Freshly added nodes report their real ids exactly as they do for `MGraph`:

```rust
let (g1, m) = g0.modify(modify![N(7)]).unwrap();
assert_eq!(*m.new_node_ids[&modify::LocalId(7)], 0);
```

The whole `modify!` DSL carries over unchanged — `N`/`X`/`!X`, `E`/`e`/`!e`,
`.val(..)`, back-references, all three edge kinds. The only difference is
where the result lands.

## Versions are yours to choose

`version()` reads the graph's current version. Plain `modify` advances it by
one. When the version number means something in *your* system — a transaction
id, a logical clock, a commit counter — `modify_versioned` lets you stamp it
directly:

```rust
let (g7, _) = g1.modify_versioned(modify![N(2) ^ x(0)], 7).unwrap();
assert_eq!(g7.version(), 7);
assert_eq!(g7.node_gen(id::N(2)), Some(7));   // born at 7
assert_eq!(g7.node_gen(id::N(0)), Some(1));   // untouched since 1
```

Versions only ever go **up**. A requested version that does not strictly
advance the current one is refused, and no graph is produced:

```rust
let Err(err) = g5.modify_versioned(modify![N(2)], 4) else {
    panic!("a version that does not advance must be refused")
};
assert!(matches!(
    err,
    error::Modify::Version(error::Version::NotMonotonic { current: 5, requested: 4 })
));
assert_eq!(g5.version(), 5);
```

Monotonicity is what makes generations mean anything: a version stamped on a
node is a point on a line, not a label that might come round again.

## Stable identity: slot + generation

Node ids are **slots**, and slots are recycled — remove a node and the next
insertion may land on the same id. That is fine for a graph you hold once, and
a trap for anything that remembers ids across edits: a stale `id::N(1)` will
happily resolve to a stranger.

`stable_id` closes it. It returns `(slot, generation)`, where the generation is
the version at which the occupant of that slot was **born**:

```rust
let before = g1.stable_id(id::N(1)).unwrap();
let (g2, _) = g1.modify(modify![!X(1)]).unwrap();
let (g3, _) = g2.modify(modify![N(9)]).unwrap();
let after = g3.stable_id(id::N(1)).unwrap();

assert_eq!(before.0, after.0);   // same slot, recycled
assert_ne!(before.1, after.1);   // different birth version
assert_eq!(after.1, 3);
```

<svg viewBox="0 0 460 170" style="max-width:460px;display:block;margin:1em auto" role="img" aria-label="Slot 1 emptied then refilled; the generation stamp separates the two occupants">
  <text x="70" y="28" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">v1</text>
  <text x="230" y="28" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">v2 — !X(1)</text>
  <text x="390" y="28" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">v3 — N(9)</text>
  <circle cx="70" cy="76" r="21" fill="#f0a63f26"/><circle cx="70" cy="76" r="15" fill="#f0a63f"/>
  <text x="70" y="81" text-anchor="middle" font-family="monospace" font-size="14" font-weight="700" fill="#0d0a03">1</text>
  <circle cx="230" cy="76" r="19" fill="#e2596e1f"/>
  <circle cx="230" cy="76" r="15" fill="none" stroke="#e2596e" stroke-width="2.2" stroke-dasharray="4 4"/>
  <circle cx="390" cy="76" r="21" fill="#f0a63f26"/><circle cx="390" cy="76" r="15" fill="#f0a63f"/>
  <text x="390" y="81" text-anchor="middle" font-family="monospace" font-size="14" font-weight="700" fill="#0d0a03">1</text>
  <path d="M96 76 L206 76" stroke="#8b95a9" stroke-width="1.6" stroke-dasharray="4 4"/>
  <path d="M254 76 L364 76" stroke="#8b95a9" stroke-width="1.6" stroke-dasharray="4 4"/>
  <text x="70" y="120" text-anchor="middle" font-family="monospace" font-size="12" fill="#46c6d6">(1, gen 1)</text>
  <text x="230" y="120" text-anchor="middle" font-family="monospace" font-size="12" fill="#e2596e">slot free</text>
  <text x="390" y="120" text-anchor="middle" font-family="monospace" font-size="12" fill="#46c6d6">(1, gen 3)</text>
  <text x="230" y="152" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">same slot, different node — the generation says so</text>
</svg>

Hold `(slot, generation)` rather than a bare id and a recycled slot can never
impersonate the node you meant. `node_gen` and `edge_gen` read the stamp on
its own when you already know the id is live.

## Atomicity: an `Err` produces no version at all

A rejected batch is not a partial edit and not an empty one — it is *no*
edit. There is no new graph to look at, the original is untouched, and the
id space is not disturbed: a slot the failed batch would have consumed is
still there for the next one.

```rust
assert!(v.modify(modify![N(9) ^ x(0), x(0) ^ x(1)]).is_err());

let (next, m) = v.modify(modify![N(7)]).unwrap();
assert_eq!(*m.new_node_ids[&modify::LocalId(7)], 2);   // slot 2, not 3
assert_eq!(next.version(), 1);
assert_eq!(v.version(), 0);
```

This falls out of the shape of the API rather than being defended by
bookkeeping. `modify` builds the next graph off to the side and only hands it
back on success; on failure the value it was building is simply dropped, and
`v` — which was never borrowed mutably — is trivially intact.

## The two-way door

`VGraph` and `MGraph` are two representations of one graph model, and you can
cross between them at will. Convert when the workload changes: batch-build in
`MGraph` where in-place mutation is cheapest, then freeze into a `VGraph` for
snapshotting and sharing.

```rust
let m = v.to_mgraph();
assert_eq!(m.node_count(), v.node_count());

let back = VGraph::from_mgraph(&m);
assert_eq!(back.version(), 0);   // conversion is not an edit
```

Node ids survive a round trip, tombstones and all — a graph with a hole at
slot 1 still has that hole afterwards, and the next insertion still fills it.
Edge ids are reassigned, exactly as they are by `MGraph::from_graph`.

Persistence crosses the same door. There is one `.grw` format, not two: a file
written by either side loads into either side.

```rust
v.save(&path).unwrap();

let m: graph::MUndir0 = MGraph::load(&path).unwrap();
let v2: VUndir0 = VUndir0::load(&path).unwrap();
assert_eq!(v2.version(), 0);
```

Versions and generations are *runtime* identity, not file content — a loaded
graph starts again at version `0`. If your version numbers must outlive the
file, they belong in your own data, and `modify_versioned` will restamp the
graph to match.

## Search is unchanged

The engine never learned about versioning. It reads targets through the
`Graph` trait, and `VGraph` implements that trait, so indexing and searching a
versioned graph is the same call on the same tiers with the same results:

```rust
let m_hits = Seq::search(query, &m.index(RevCsr)).count();
let v_hits = Seq::search(query, &v.index(RevCsr)).count();
assert_eq!(v_hits, m_hits);
```

Every pattern in this book — clusters, morphisms, variable-length paths,
negation, watchers — applies verbatim. An index is built from a *particular*
version; keep the version alive alongside its index and both stay valid while
newer versions come and go.

## What v1 deliberately leaves out

- **No branch merge.** Versions form a chain, and forks are ordinary values
  you keep around. Reconciling two divergent forks is your policy to write,
  not the library's to guess.
- **The engine reads through the trait, and only through it.** No
  version-aware search, no "match against v3 as of v5". Searching a version
  means searching the graph you kept.
- **No garbage collection of history.** A version lives exactly as long as
  something holds it; drop the binding and its unshared trie nodes go with it.

## What versioning costs

Measured against `MGraph` on the same 1M-node / 2M-edge random graph
(avg degree 4, median of 3 runs; `cargo run --release --example
graph_perf -- 1000000 3 42` reproduces it):

| measurement | MGraph | VGraph | V / M |
|:--|---:|---:|---:|
| bulk build (1M nodes + 2M edges) | 602 ms | 10.49 s | 17.4× |
| 1000 small modifies | 1.7 ms | 23.2 ms | 13.5× |
| full edge scan (`iter_edges`) | 1.9 ms | 25.0 ms | 13.2× |
| index build (RevCsr) | 208 ms | 777 ms | 3.7× |
| search (triangle, Mono) | 1.41 s | 1.45 s | 1.03× |
| snapshot / keep a version | 29.8 ms | 20 ns | ~0 |

The pattern holds at 10k and 100k nodes too. Writes and raw trait scans pay
the trie's pointer-chasing tax — roughly an order of magnitude. Build an
index and the tax vanishes: search times are within a few percent either
way, because the engine runs against the index, not the store. And the row
that motivates the whole design: retaining a version is four `Arc` bumps —
20 ns flat at any size — where snapshotting `MGraph` is a deep copy that
grows linearly (213 µs at 10k, 3.1 ms at 100k, 29.8 ms at 1M).

`VGraph` has no bulk constructor by design — bulk load is one `modify`
carrying all the ops, which still path-copies per element. If you are
assembling a large graph from scratch, build it as `MGraph` and convert;
the two-way door (`VGraph::from_mgraph` / `to_mgraph`) preserves slots and ids.
