# Morphism Cheat Sheet

## Visual Examples

Pattern: path **A — B — C** (3 nodes, 2 edges). Target: triangle **0 — 1 — 2 — 0** (3 nodes, 3 edges).

### Mapping: A→0, B→1, C→2

The required edges (A—B, B—C) are present in the target. But there's an **extra edge 0—2** that the pattern doesn't mention.

<svg viewBox="0 0 296 165" style="max-width:296px;display:block;margin:0.8em auto" role="img" aria-label="mono accept"><line x1="71" y1="103" x2="135" y2="54" stroke="#46c6d6" stroke-width="2.2"/><line x1="74" y1="113" x2="222" y2="113" stroke="#46c6d6" stroke-width="2.2" stroke-dasharray="5 5" opacity="0.4"/><line x1="161" y1="54" x2="225" y2="103" stroke="#46c6d6" stroke-width="2.2"/><circle cx="58" cy="113" r="16" fill="#f0a63f22"/><circle cx="58" cy="113" r="11" fill="#f0a63f"/><text x="58" y="117" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><text x="58" y="91" text-anchor="middle" font-family="monospace" font-size="11" fill="#46c6d6">A</text><circle cx="148" cy="44" r="16" fill="#f0a63f22"/><circle cx="148" cy="44" r="11" fill="#f0a63f"/><text x="148" y="48" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><text x="148" y="22" text-anchor="middle" font-family="monospace" font-size="11" fill="#46c6d6">B</text><circle cx="238" cy="113" r="16" fill="#f0a63f22"/><circle cx="238" cy="113" r="11" fill="#f0a63f"/><text x="238" y="117" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">2</text><text x="238" y="91" text-anchor="middle" font-family="monospace" font-size="11" fill="#46c6d6">C</text><text x="148" y="124" text-anchor="middle" font-family="monospace" font-size="10.5" fill="#8b95a9">extra</text></svg>
*Mono: **accept** — extra edge is fine*

<svg viewBox="0 0 296 165" style="max-width:296px;display:block;margin:0.8em auto" role="img" aria-label="subiso reject"><line x1="71" y1="103" x2="135" y2="54" stroke="#46c6d6" stroke-width="2.2"/><line x1="74" y1="113" x2="222" y2="113" stroke="#e2596e" stroke-width="2.2" stroke-dasharray="6 5"/><line x1="142" y1="107" x2="154" y2="119" stroke="#e2596e" stroke-width="2"/><line x1="142" y1="119" x2="154" y2="107" stroke="#e2596e" stroke-width="2"/><line x1="161" y1="54" x2="225" y2="103" stroke="#46c6d6" stroke-width="2.2"/><circle cx="58" cy="113" r="14" fill="#e2596e1f"/><circle cx="58" cy="113" r="11" fill="none" stroke="#e2596e" stroke-width="2" stroke-dasharray="4 4"/><text x="58" y="117" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#e2596e">0</text><text x="58" y="91" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">A</text><circle cx="148" cy="44" r="14" fill="#e2596e1f"/><circle cx="148" cy="44" r="11" fill="none" stroke="#e2596e" stroke-width="2" stroke-dasharray="4 4"/><text x="148" y="48" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#e2596e">1</text><text x="148" y="22" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">B</text><circle cx="238" cy="113" r="14" fill="#e2596e1f"/><circle cx="238" cy="113" r="11" fill="none" stroke="#e2596e" stroke-width="2" stroke-dasharray="4 4"/><text x="238" y="117" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#e2596e">2</text><text x="238" y="91" text-anchor="middle" font-family="monospace" font-size="11" fill="#8b95a9">C</text><text x="148" y="124" text-anchor="middle" font-family="monospace" font-size="10.5" fill="#e2596e">reject</text></svg>
*SubIso: **reject** — extra edge violates induced constraint*

| Morphism | Verdict | Why |
|----------|---------|-----|
| **Iso** | reject | extra edge 0—2 not in pattern (induced violation) |
| **SubIso** | reject | same — induced: extra edge 0—2 |
| **EpiMono** | accept | bijective, edges preserved, extra OK |
| **Mono** | accept | injective, edges preserved, extra OK |
| **Epi** | accept | surjective, edges preserved |
| **Homo** | accept | edges preserved, no other constraints |

### Collapsing: A→0, B→1, C→1

C maps to the same target node as B — a **collapse**.

<svg viewBox="0 0 207 120" style="max-width:207px;display:block;margin:0.8em auto" role="img" aria-label="homo collapse"><line x1="74" y1="50" x2="133" y2="50" stroke="#46c6d6" stroke-width="2.2"/><circle cx="58" cy="50" r="16" fill="#f0a63f22"/><circle cx="58" cy="50" r="11" fill="#f0a63f"/><text x="58" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">0</text><text x="58" y="28" text-anchor="middle" font-family="monospace" font-size="11" fill="#46c6d6">A</text><circle cx="149" cy="50" r="16" fill="#f0a63f22"/><circle cx="149" cy="50" r="11" fill="#f0a63f"/><text x="149" y="54" text-anchor="middle" font-family="monospace" font-size="12" font-weight="600" fill="#1a1205">1</text><text x="149" y="28" text-anchor="middle" font-family="monospace" font-size="11" fill="#46c6d6">B,C</text></svg>
*Homo: **accept** — collapse allowed*

| Morphism | Verdict | Why |
|----------|---------|-----|
| **Mono** | reject | B and C map to same node (not injective) |
| **Homo** | accept | collapse allowed, edge A—B preserved |

### Surjectivity: pattern A—B on target 0—1—2

A two-node pattern on a three-node target. Node 2 is left uncovered.

| Morphism | Verdict | Why |
|----------|---------|-----|
| **Epi** | reject | node 2 not covered (surjectivity violated) |
| **Mono** | accept | uncovered nodes are fine |

## Quick Decision Guide

**Do I need each pattern node to match a different target node?**
- **Yes** → injective (Iso, SubIso, EpiMono, Mono)
- **No** → non-injective (Epi, Homo)

**Do I need every target node to be matched?**
- **Yes** → surjective (Iso, EpiMono, Epi)
- **No** → non-surjective (SubIso, Mono, Homo)

**Do I care about extra edges between matched nodes?**
- **Yes, reject them** → induced (Iso, SubIso)
- **No, allow them** → non-induced (EpiMono, Mono, Epi, Homo)

## Complexity

| Morphism | Complexity | Why |
|----------|-----------|-----|
| Iso | GI-complete | Own complexity class, believed sub-exponential |
| SubIso | NP-complete | Generalizes clique, Hamiltonian path |
| EpiMono | GI-complete | Bijection check + edge preservation |
| Mono | NP-complete | Generalizes clique |
| Epi | NP-complete | Surjectivity + edge preservation |
| Homo | NP-complete | Generalizes graph coloring |

In practice, real-world graphs have structure (bounded degree, sparsity, value predicates) that makes these tractable with backtracking and pruning. Small patterns on large graphs are fast.
