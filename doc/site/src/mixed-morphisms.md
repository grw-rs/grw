# Mixing Clusters & Morphisms

Every cluster carries its own morphism — and patterns may combine clusters
freely. This page defines exactly what such mixtures mean, one rule at a
time, with the corner cases drawn out.

## The distinctness rule

> **Two pattern nodes may bind the same target node iff they are not both
> injective.**

Nodes defined in an injective cluster (`Iso`, `SubIso`, `EpiMono`, `Mono`)
form one **injective population**: pairwise distinct, across *all* clusters.
Nodes defined in a free cluster (`Homo`, `Epi`) reserve nothing and respect
no reservations — in both directions:

| pair | may collide? |
|---|---|
| injective ↔ injective, same cluster | **never** |
| injective ↔ injective, different clusters | **never** |
| injective ↔ free | yes |
| free ↔ free | yes |

Distinctness is never implicit. If you need two nodes to be genuinely
different, put both in the injective population — that is the *only*
guarantee. Conversely, a `Homo` node means "some node, possibly one you
already matched."

## A node's morphism comes from its defining cluster

`N(x)` defines a node; `n(x)` references it. References never tighten:

```rust
grw::search![<NV, ER>;
    get(Homo) { N(0), N(1) },                       // 0, 1 are free
    get(Mono) { n(0) ^ ..n(1).dfs().len(1..) }      // referencing them
]                                                    // does not change that
```

Nodes `0` and `1` stay free even though a `Mono` cluster uses them — which
is exactly what lets this pattern find **cycles**: both path endpoints may
bind the *same* node.

<svg viewBox="0 0 420 130" style="max-width:420px;display:block;margin:1em auto" role="img" aria-label="Cycle: homo endpoints collapse onto one node, the mono path loops back">
  <path d="M150 70 C 100 8, 240 8, 190 70" fill="none" stroke="#46c6d6" stroke-width="2.4" stroke-dasharray="7 6"/>
  <circle cx="132" cy="30" r="6" fill="none" stroke="#f0a63f88" stroke-width="1.6" stroke-dasharray="3 4"/>
  <circle cx="205" cy="30" r="6" fill="none" stroke="#f0a63f88" stroke-width="1.6" stroke-dasharray="3 4"/>
  <circle cx="170" cy="82" r="21" fill="#f0a63f26"/><circle cx="170" cy="82" r="15" fill="#f0a63f"/>
  <text x="170" y="86" text-anchor="middle" font-family="monospace" font-size="14" font-weight="700" fill="#0d0a03">0=1</text>
  <text x="170" y="118" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">free endpoints may be the same node</text>
</svg>

## Free nodes never add multiplicity

The subtlest consequence. Consider a `Mono` edge plus a free witness:

```rust
get(Mono) { N(0) ^ N(1) },       // an edge A—B
get(Homo) { N(2) ^ n(0) }        // "and 0 has a neighbor"
```

You might read this as "node 0 has a *second* neighbor besides 1." It does
not say that: node `2` is free and not adjacent to `1`, so `m(2) = m(1)` is
always available — the target edge serving `0—1` serves `2—0` too.

<svg viewBox="0 0 440 150" style="max-width:440px;display:block;margin:1em auto" role="img" aria-label="A free witness node collapsing onto the mono neighbor it duplicates">
  <line x1="100" y1="70" x2="230" y2="70" stroke="#46c6d6" stroke-width="2.2"/>
  <circle cx="84" cy="70" r="20" fill="#f0a63f26"/><circle cx="84" cy="70" r="14" fill="#f0a63f"/>
  <text x="84" y="74" text-anchor="middle" font-family="monospace" font-size="14" font-weight="700" fill="#0d0a03">0</text>
  <circle cx="246" cy="70" r="20" fill="#f0a63f26"/><circle cx="246" cy="70" r="14" fill="#f0a63f"/>
  <text x="246" y="74" text-anchor="middle" font-family="monospace" font-size="14" font-weight="700" fill="#0d0a03">1</text>
  <line x1="110" y1="56" x2="300" y2="26" stroke="#46c6d6" stroke-width="2" stroke-dasharray="5 5"/>
  <circle cx="318" cy="24" r="14" fill="none" stroke="#f0a63f" stroke-width="2.2" stroke-dasharray="4 4"/>
  <text x="318" y="28" text-anchor="middle" font-family="monospace" font-size="14" font-weight="700" fill="#f0a63f">2</text>
  <path d="M310 38 C 290 58, 272 62, 262 64" fill="none" stroke="#8b95a9" stroke-width="1.6" stroke-dasharray="3 4" marker-end="url(#collapse-arrow)"/>
  <defs><marker id="collapse-arrow" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7 z" fill="#8b95a9"/></marker></defs>
  <text x="220" y="128" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">2 may alias 1 — the pattern asserts no second neighbor</text>
</svg>

The pattern's true meaning: "edge 0—1 exists, and 0 has *some* neighbor" —
trivially satisfied by node 1 itself. Want a genuinely second neighbor?
Define node 2 in a `Mono` cluster.

**Counting inherits this.** Free-node matches include every aliased
configuration. If you are counting distinct structures — fan-outs, distinct
witnesses — use injective clusters.

## Collapsing onto a neighbor needs a self-loop

A free node can never collapse onto a node it is *adjacent to* in the
pattern, unless the target has a self-loop — its own edge would have to map
to `x—x`:

<svg viewBox="0 0 460 120" style="max-width:460px;display:block;margin:1em auto" role="img" aria-label="Adjacent collapse requires a self-loop on the target">
  <line x1="70" y1="60" x2="160" y2="60" stroke="#46c6d6" stroke-width="2.2"/>
  <circle cx="55" cy="60" r="14" fill="#f0a63f"/><text x="55" y="64" text-anchor="middle" font-family="monospace" font-size="14" font-weight="700" fill="#0d0a03">0</text>
  <circle cx="175" cy="60" r="14" fill="#f0a63f"/><text x="175" y="64" text-anchor="middle" font-family="monospace" font-size="14" font-weight="700" fill="#0d0a03">1</text>
  <text x="115" y="95" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">Homo: 0 ^ 1</text>
  <text x="240" y="64" text-anchor="middle" font-family="monospace" font-size="14" fill="#8b95a9">⇒</text>
  <circle cx="320" cy="60" r="14" fill="#f0a63f"/><text x="320" y="64" text-anchor="middle" font-family="monospace" font-size="14" font-weight="700" fill="#0d0a03">0=1</text>
  <path d="M328 50 C 352 28, 352 92, 328 70" fill="none" stroke="#46c6d6" stroke-width="2.2"/>
  <text x="382" y="46" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">needs x—x</text>
  <text x="330" y="102" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">merge legal only if the loop exists</text>
</svg>

Self-loops are first-class in the graph model (`N(0) ^ n(0)`), so this is a
real capability — but on loop-free targets, adjacent free nodes are
effectively distinct.

## Paths under mixed morphisms

Variable-length paths interact with the population rule in three ways:

- **Interiors are always node-simple.** A path never revisits a node,
  regardless of morphism — `Homo` paths are simple paths, not walks.
  Endpoints may still close into a cycle (above).
- **Interiors avoid the injective population only.** A path may pass
  *through* a target that a free node happens to occupy; it may never pass
  through an injective binding.
- **Cross-path interior sharing is morphism-driven.** Two path edges in a
  `Mono` cluster must use disjoint interiors; in a `Homo` cluster they may
  share.

<svg viewBox="0 0 440 150" style="max-width:440px;display:block;margin:1em auto" role="img" aria-label="Two paths forced through one interior: rejected under Mono, accepted under Homo">
  <circle cx="60" cy="75" r="14" fill="#f0a63f"/><text x="60" y="79" text-anchor="middle" font-family="monospace" font-size="14" font-weight="700" fill="#0d0a03">0</text>
  <circle cx="220" cy="40" r="11" fill="#46c6d6"/><text x="220" y="22" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">x</text>
  <circle cx="380" cy="75" r="14" fill="#f0a63f"/><text x="380" y="79" text-anchor="middle" font-family="monospace" font-size="14" font-weight="700" fill="#0d0a03">2</text>
  <circle cx="220" cy="110" r="14" fill="#f0a63f"/><text x="220" y="114" text-anchor="middle" font-family="monospace" font-size="14" font-weight="700" fill="#0d0a03">1</text>
  <path d="M72 68 C 120 45, 170 40, 209 40" fill="none" stroke="#46c6d6" stroke-width="2.2" stroke-dasharray="6 5"/>
  <path d="M231 40 C 280 40, 330 48, 368 68" fill="none" stroke="#46c6d6" stroke-width="2.2" stroke-dasharray="6 5"/>
  <path d="M215 100 C 205 80, 210 60, 217 50" fill="none" stroke="#e2596e" stroke-width="2" stroke-dasharray="4 4"/>
  <text x="220" y="140" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">both paths need x: Mono cluster → no match · Homo cluster → match</text>
</svg>

## Surjective clusters (`Epi`, `EpiMono`)

Surjectivity is checked against the **whole query's** bindings: every target
node must be the image of some pattern node. Two nuances:

- Free clusters' bindings count toward coverage.
- **Path interiors do not count.** A path edge asserts "a path exists" — its
  interior nodes are witnesses, not part of the pattern's image. An `Epi`
  pattern whose explicit nodes cannot cover the target has no matches, even
  if its paths sweep every node.

## Advanced: induced morphisms in mixtures

`SubIso` and `Iso` add the *induced* axis: extra target edges between
matched nodes are forbidden. In a mixed pattern, the axis it is measured
against is the injective population — the same population the distinctness
rule builds:

> **Induced-ness holds between a `SubIso`/`Iso` node and every injective
> binding.** For a node `i` in an induced cluster and any node `j` bound
> injectively — in *any* injective cluster, not just `i`'s own — a target
> edge `m(i)—m(j)` is legal only if the pattern has `i—j`. Bindings from
> free clusters (`Homo`, `Epi`) are invisible to the check: they neither
> trigger it nor are protected by it.

So an induced node's neighbourhood is pinned exactly against everything the
pattern promised to keep distinct, and left unconstrained against everything
it did not. A free node may sit on a target neighbour of an induced node
without rejecting the match — that target edge simply is not part of what
induced-ness reads. Mixing induced clusters with free clusters is rarely
what you want; prefer `Mono` + `Homo` mixtures unless you specifically need
exact neighborhoods.

## Rules of thumb

1. **Distinct entities → one injective population.** Any number of clusters;
   injectivity composes across them.
2. **"Possibly the same node" → `Homo`.** That is its entire purpose.
3. **Counting distinct structures → no free nodes** in the counted part.
4. **Cycles → free endpoints + an injective path cluster** referencing them.
