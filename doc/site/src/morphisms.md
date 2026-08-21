# Morphisms

A **morphism** is a mapping from pattern nodes to target nodes that preserves edges — if two nodes are connected in the pattern, their mapped counterparts must be connected in the target.

The question is: **how strict is the mapping?** Three independent axes control this.

## The Three Axes

### Injective (one-to-one)
No two pattern nodes map to the same target node. Each match "uses up" a target node.

*Real-world: assigning people to desks — each desk holds one person.*

### Surjective (covers everything)
Every target node must be hit by at least one pattern node. No target node left unmapped.

*Real-world: every desk must have someone sitting at it.*

### Induced (exact neighborhood)
No extra edges allowed between matched nodes beyond what the pattern specifies. The pattern describes the *exact* local structure.

*Real-world: the people at these desks talk to exactly who the org chart says — no side conversations.*

## The Six Morphisms

| Morphism | Injective | Surjective | Induced | Plain English |
|----------|-----------|------------|---------|---------------|
| **Iso** | yes | yes | yes | Exact match — same shape, same size, same edges |
| **SubIso** | yes | no | yes | Find this exact shape inside the target |
| **EpiMono** | yes | yes | no | Bijection, but extra edges between matched nodes OK |
| **Mono** | yes | no | no | Each pattern node gets a unique target node, extra edges OK |
| **Epi** | no | yes | no | Must cover all target nodes, can collapse pattern nodes |
| **Homo** | no | no | no | Anything goes — just preserve edges |

## The Lattice

These form a partial order. Going up adds constraints, going down relaxes them.

```
         Iso
        /   \
    SubIso  EpiMono
       \   / \   /
        Mono   Epi
          \   /
           Homo
```

- Up = more constrained, down = more relaxed
- `SubIso` = Mono + induced
- `EpiMono` = Mono + surjective
- `Iso` = SubIso + surjective = EpiMono + induced

The `meet()` of two morphisms is the most relaxed morphism that satisfies both. This is used internally when multiple clusters share nodes.

## When to Use What

**Isomorphism** — "Are these two graphs identical?"
Graph comparison, canonical forms, symmetry detection. Both graphs must have the same number of nodes, same edges, nothing extra.

**Subgraph Isomorphism** — "Does this shape appear inside that graph?"
The workhorse of pattern matching. Find a triangle in a social network, a motif in a protein structure, a substructure in a molecule. The matched region must look *exactly* like the pattern — no extra connections between matched nodes.

**Monomorphism** — "Can I embed this pattern without node conflicts?"
Like SubIso but relaxed: extra edges between matched nodes are allowed. Good when you care about required connections but don't mind if the matched nodes have additional relationships. Often easier to compute.

**Epimorphism** — "Does the pattern cover the entire target?"
Every target node must be matched. Useful for coverage analysis, tiling, or ensuring nothing is left unmatched. Pattern can collapse multiple nodes onto one target.

**EpiMono** — "Is this a relabeling with possible extra edges?"
Bijective but non-induced. Same number of nodes, one-to-one mapping, but the target may have edges the pattern doesn't mention. Arises naturally when a node participates in both Mono and Epi constraints.

**Homomorphism** — "Can this pattern be projected onto that graph?"
Most relaxed. Multiple pattern nodes can collapse to one target node. Graph coloring is a homomorphism to a complete graph. Useful for abstract structural queries where you care about connectivity patterns, not exact node identity.
