# Morphism Cheat Sheet

## Visual Examples

Pattern: path **A — B — C** (3 nodes, 2 edges). Target: triangle **0 — 1 — 2 — 0** (3 nodes, 3 edges).

### Mapping: A→0, B→1, C→2

The required edges (A—B, B—C) are present in the target. But there's an **extra edge 0—2** that the pattern doesn't mention.

<svg viewBox="0 0 347 193" style="max-width:347px;display:block;margin:0.8em auto" role="img" aria-label="mono accept"><line x1="86" y1="120" x2="158" y2="64" stroke="#46c6d6" stroke-width="2.4"/><line x1="90" y1="133" x2="257" y2="133" stroke="#46c6d6" stroke-width="2.4" stroke-dasharray="5 5" opacity="0.4"/><line x1="189" y1="64" x2="261" y2="120" stroke="#46c6d6" stroke-width="2.4"/><circle cx="70" cy="133" r="21" fill="#f0a63f26"/><circle cx="70" cy="133" r="15" fill="#f0a63f"/><text x="70" y="138" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><text x="70" y="104" text-anchor="middle" font-family="monospace" font-size="12" fill="#46c6d6">A</text><circle cx="173" cy="52" r="21" fill="#f0a63f26"/><circle cx="173" cy="52" r="15" fill="#f0a63f"/><text x="173" y="57" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><text x="173" y="23" text-anchor="middle" font-family="monospace" font-size="12" fill="#46c6d6">B</text><circle cx="277" cy="133" r="21" fill="#f0a63f26"/><circle cx="277" cy="133" r="15" fill="#f0a63f"/><text x="277" y="138" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">2</text><text x="277" y="104" text-anchor="middle" font-family="monospace" font-size="12" fill="#46c6d6">C</text><text x="173" y="145" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">extra</text></svg>
*Mono: **accept** — extra edge is fine*

<svg viewBox="0 0 347 193" style="max-width:347px;display:block;margin:0.8em auto" role="img" aria-label="subiso reject"><line x1="86" y1="120" x2="158" y2="64" stroke="#46c6d6" stroke-width="2.4"/><line x1="90" y1="133" x2="257" y2="133" stroke="#e2596e" stroke-width="2.4" stroke-dasharray="6 5"/><line x1="166" y1="126" x2="180" y2="140" stroke="#e2596e" stroke-width="2.4"/><line x1="166" y1="140" x2="180" y2="126" stroke="#e2596e" stroke-width="2.4"/><line x1="189" y1="64" x2="261" y2="120" stroke="#46c6d6" stroke-width="2.4"/><circle cx="70" cy="133" r="19" fill="#e2596e1f"/><circle cx="70" cy="133" r="15" fill="none" stroke="#e2596e" stroke-width="2.2" stroke-dasharray="4 4"/><text x="70" y="138" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#e2596e">0</text><text x="70" y="104" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">A</text><circle cx="173" cy="52" r="19" fill="#e2596e1f"/><circle cx="173" cy="52" r="15" fill="none" stroke="#e2596e" stroke-width="2.2" stroke-dasharray="4 4"/><text x="173" y="57" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#e2596e">1</text><text x="173" y="23" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">B</text><circle cx="277" cy="133" r="19" fill="#e2596e1f"/><circle cx="277" cy="133" r="15" fill="none" stroke="#e2596e" stroke-width="2.2" stroke-dasharray="4 4"/><text x="277" y="138" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#e2596e">2</text><text x="277" y="104" text-anchor="middle" font-family="monospace" font-size="12" fill="#8b95a9">C</text><text x="173" y="145" text-anchor="middle" font-family="monospace" font-size="12" fill="#e2596e">reject</text></svg>
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

<svg viewBox="0 0 242 132" style="max-width:242px;display:block;margin:0.8em auto" role="img" aria-label="homo collapse"><line x1="90" y1="58" x2="152" y2="58" stroke="#46c6d6" stroke-width="2.4"/><circle cx="70" cy="58" r="21" fill="#f0a63f26"/><circle cx="70" cy="58" r="15" fill="#f0a63f"/><text x="70" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">0</text><text x="70" y="29" text-anchor="middle" font-family="monospace" font-size="12" fill="#46c6d6">A</text><circle cx="172" cy="58" r="21" fill="#f0a63f26"/><circle cx="172" cy="58" r="15" fill="#f0a63f"/><text x="172" y="63" text-anchor="middle" font-family="monospace" font-size="15" font-weight="700" fill="#0d0a03">1</text><text x="172" y="29" text-anchor="middle" font-family="monospace" font-size="12" fill="#46c6d6">B,C</text></svg>
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
