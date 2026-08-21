# Ban Shared Edge Semantics

## Core Principle

A ban edge between shared nodes (nodes that appear in get clusters) checks for **additional edges beyond what get requires**. It never re-checks edges that get already established.

## Edge Slot Spaces

| Graph type | Slots | Operators | SLOT_COUNT |
|------------|-------|-----------|------------|
| `Undir<V>` | {UND} | `^` | 1 |
| `Dir<V>` | {SRC, TGT} | `>>`, `<<` | 2 |
| `Anydir<V>` | {Dir(Src), Dir(Tgt), Undir} | `>>`, `<<`, `^`, `%` | 3 |

`%` = any_slot wildcard. Available on all graph types via `Rem` trait but only meaningful when SLOT_COUNT > 1.

## Coverage Rules

A ban edge's slot is **covered** by get if the get clusters collectively guarantee the ban's existence check always passes.

| Get slot(s) | Ban slot | Covered? | Reason |
|-------------|----------|----------|--------|
| Specific(S) | Specific(S) | YES | Same physical edge |
| Specific(S) | Specific(T), S!=T | NO | Different physical edge |
| Specific(S), SLOT_COUNT=1 | Any (%) | YES | Only slot covered, % has nothing to check |
| Specific(S), SLOT_COUNT>1 | Any (%) | NO | % checks uncovered slots (SLOT_COUNT-1 remain) |
| Any (%) | Specific(S) | NO | % matched some slot, not necessarily S |
| Any (%) | Any (%) | YES | Same check |
| Multiple specifics covering all slots | Any (%) | YES | All slots covered, nothing left for % |
| Multiple specifics NOT covering all | Any (%) | NO | Uncovered slots remain |

## Morphism Interaction

The get cluster's morphism determines whether extra edges can exist between matched nodes:

| Get morphism | Extra edges allowed? | Ban shared edge valid? |
|-------------|---------------------|----------------------|
| Homo | Yes | Yes, if uncovered slots exist |
| Mono | Yes | Yes, if uncovered slots exist |
| SubIso | No — induced subgraph must match pattern exactly | Never — SubIso already forbids extras |
| Iso | No — entire graph must match | Never — Iso already forbids extras |

Under SubIso/Iso, any ban edge between shared nodes is statically dead and should be rejected at compile time.

## Multi-Cluster Semantics

- **Multiple get clusters**: AND — all must match. Edges from all get clusters contribute to the "covered" set for a node pair.
- **Multiple ban clusters**: OR (for rejection) — if ANY ban fires, the match is rejected. Each ban cluster's edges are checked independently.

### Get-Get overlap

Two get clusters referencing the same edge (same node pair, same slot) is always redundant since get clusters are AND'd. Should be ConflictingPred (if either has pred) or Duplicate.

### Get-Ban overlap detection

For each ban edge between shared nodes:
1. Collect all get edge slots between that node pair (union across all get clusters)
2. Determine if the ban edge's slot is covered by the get set
3. If covered → reject at compile time

## Compile-Time Rejection Matrix

### Same edge_map key (same slot or both %)

Already handled: RedundantInBan (no pred) or ConflictingPred (either has pred).

### Ban `%` when get has specific slot(s)

Different edge_map keys — NOT currently caught. Must add a new check:

1. Collect get-covered specific slots for the node pair
2. If get covers ALL slots (count of covered slots == SLOT_COUNT) → ban `%` is dead → reject
3. If get covers SOME slots → ban `%` is meaningful (checks uncovered) → allow
4. Special case: on Undir (SLOT_COUNT=1), get `^` covers the only slot → ban `%` always dead

### Ban `%` under SubIso/Iso get

Any ban edge between shared nodes is dead regardless of slot coverage → reject.

## Ban Morphism on Shared Edges

The ban cluster's own morphism affects how shared edges are evaluated:

| Ban morphism | Shared edge semantics |
|-------------|----------------------|
| Homo/Mono | Ban fires if the ban's edges are satisfied (extra edges in target are ignored) |
| SubIso/Iso | Ban fires only if the induced subgraph on shared nodes exactly matches the ban's edge pattern — extra edges between shared nodes cause SubIso to fail, so the ban does NOT fire |

This means `ban(SubIso) { n(0) % n(1) }` rejects pairs connected by **exactly one** edge. Pairs with 0 or 2+ edges survive because SubIso doesn't match.

Example:
```
// Anydir graph: N(0) >> N(1), N(0) << N(1), N(2), N(3)
// get(Mono) { N(0), N(1) }
// ban(SubIso) { n(0) % n(1) }
//
// Pair (0,1): 2 directed edges. Ban pattern has 1 % edge.
//   SubIso: induced subgraph has 2 edges, pattern has 1 → mismatch → ban doesn't fire → SURVIVES
// Pair (0,2): 0 edges. Ban % not satisfied → ban doesn't fire → SURVIVES
// Pair (2,3): 0 edges. Same → SURVIVES
```

Implementation: `ban_shared_edges_satisfied` must check the ban morphism. Under SubIso/Iso, after verifying the ban's edges exist, iterate ALL pairs of shared nodes in the ban cluster (not just pairs with ban edges). For each pair, count uncovered target edges vs ban-specified edges. If any pair has more target edges than ban edges, SubIso fails and the ban does not fire.

## Engine Behavior

`ban_shared_edges_satisfied` handles two concerns:

1. **Slot subtraction for `%`**: Ban `%` between shared nodes only checks uncovered slots (slots not required by get). Uses `get_covered_slots()` to collect get-required slots, then `has_uncovered_edge()` / `uncovered_pred_matches()` to check only remaining slots.

2. **Ban morphism enforcement**: Under SubIso/Iso ban, the induced subgraph on shared nodes must exactly match the ban's edge pattern. After checking that the ban's edges are satisfied, also verify no extra edges exist. This means counting edges the ban specifies vs edges that actually exist between the pair (excluding get-covered slots).

## Examples

```
// Anydir graph: N(0) >> N(1), N(0) ^ N(1)
// get(Mono) { N(0) >> N(1) } — matches, ignores extra ^ edge
// ban(Mono) { n(0) << n(1) } — reject if reverse dir edge exists (NO → survives)
// ban(Mono) { n(0) ^ n(1) }  — reject if undir edge exists (YES → rejected)
// ban(Mono) { n(0) % n(1) }  — reject if any non->> edge exists (^ exists → rejected)

// Dir graph: N(0) >> N(1)
// get(Mono) { N(0) >> N(1) } — matches
// ban(Mono) { n(0) << n(1) } — reject if reverse edge exists (NO → survives)
// ban(Mono) { n(0) % n(1) }  — expanded to ban { n(0) << n(1) } at compile time

// Undir graph: N(0) ^ N(1)
// get(Mono) { N(0) ^ N(1) } — matches
// ban(Mono) { n(0) % n(1) }  — REJECTED at compile: all slots covered (SLOT_COUNT=1)
// ban(Mono) { n(0) ^ n(1) }  — REJECTED: RedundantInBan (same slot)

// Special case: no get edge between shared nodes
// get(Mono) { N(0), N(1) } — Cartesian product of node assignments (injective under Mono)
// ban(Mono) { n(0) % n(1) } — reject pairs where ANY edge exists between them
// Result: all non-adjacent node pairs. Zero covered slots → ban % checks all slots.
```
