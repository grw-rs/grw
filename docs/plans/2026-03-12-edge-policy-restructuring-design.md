# Edge Policy Restructuring: Structural vs Predicate Edge Checks

## Problem

The `Edge` policy trait uses a single `HAS_FEATURES` flag that lumps together fundamentally different cost tiers:

- **Structural checks** (negated edges, any_slot, specific-slot without predicate) — equivalent to an adjacency lookup + boolean flip. No closure, no dyn dispatch.
- **Predicate checks** (`.val()`, `.test()`) — require `check_edge` with `dyn Fn` closure evaluation.

When a node has even one negated or any_slot edge, ALL its edges are routed through the `check_edge` loop, paying closure-dispatch overhead even for edges that only need adjacency.

## Design

### 1. New Index trait method: `has_edge_in_slot`

```rust
fn has_edge_in_slot(&self, n1: u32, n2: u32, slot: ER::Slot, any_slot: bool) -> bool;
```

- For `SLOT_COUNT == 1` or `any_slot`: degenerates to `is_adjacent(n1, n2)`.
- For multi-slot: `edge_store_at(n1, n2)` (same binary search as `is_adjacent`) + `csr_store_val(store, slot).is_some()` (one field access).
- No closure, no `dyn Fn`, no predicate evaluation.

### 2. Split adjacency list in Query

Current single list per node:
```rust
adj: Vec<Vec<(usize, ER::Slot, bool, usize)>>  // (neighbor_idx, slot, negated, edge_idx)
```

Add a new structural-only list:
```rust
adj_check: Vec<Vec<(usize, ER::Slot, bool, bool)>>  // (neighbor_idx, slot, any_slot, negated)
```

And a predicate-only list:
```rust
adj_pred: Vec<Vec<(usize, ER::Slot, bool, usize)>>  // (neighbor_idx, slot, negated, edge_idx)
```

**Classification rule at compile time:** an edge goes into `adj_check` if `edge_preds[edge_idx].is_none()` and not `ban_only`. Otherwise into `adj_pred` (if it has a predicate) or skipped (if `ban_only` — handled by ban cluster logic).

The original `adj` list is **kept unchanged** — it's used by `is_feasible`, `is_feasible_reverse_only`, `get_covered_slots`, `lookahead_ok`, and ban cluster evaluation. These are fallback/structural paths that need the full edge info.

### 3. Rename policy flag

```rust
pub(crate) trait Edge {
    const HAS_PREDICATES: bool;  // was HAS_FEATURES
}

pub(crate) struct PlainEdges;    // was SimpleEdges
pub(crate) struct PredEdges;     // was RichEdges

impl Edge for PlainEdges { const HAS_PREDICATES: bool = false; }
impl Edge for PredEdges { const HAS_PREDICATES: bool = true; }
```

`has_edge_features` → `has_predicates` on Query. `node_has_edge_features` → `node_has_predicates`.

The dispatch macro stays at 8 arms — `(has_predicates, has_negated_connected, has_ban_clusters)`.

### 4. Hot path changes in seq.rs

#### `candidates_for_into` (line ~211)

Current fast-verify condition:
```rust
let can_fast_verify = ER::SLOT_COUNT == 1
    && !ctx.query.node_has_edge_features[pattern_idx]
    && pattern_idx < 64;
```

New condition (broader — only excludes nodes with predicates):
```rust
let can_fast_verify = ER::SLOT_COUNT == 1
    && !ctx.query.node_has_predicates[pattern_idx]
    && pattern_idx < 64;
```

After the existing bitset `is_adjacent` loop for `other_mapped`, add the structural check loop:
```rust
// structural edges (negated, any_slot, multi-slot) — no predicates
for &(neighbor_idx, slot, any_slot, negated) in &ctx.query.adj_check[pattern_idx] {
    let mapped_raw = self.mapping[neighbor_idx];
    if mapped_raw == UNMAPPED { continue; }
    if ctx.index.has_edge_in_slot(raw, mapped_raw, slot, any_slot) == negated {
        return false;
    }
}
```

The `EP::HAS_PREDICATES` block only iterates `adj_pred`:
```rust
if EP::HAS_PREDICATES {
    for &(neighbor_idx, slot, negated, edge_idx) in &ctx.query.adj_pred[pattern_idx] {
        let mapped_raw = self.mapping[neighbor_idx];
        if mapped_raw == UNMAPPED { continue; }
        let any_slot = ctx.query.edges[edge_idx].any_slot;
        if !ctx.index.check_edge(raw, mapped_raw, slot, any_slot, negated,
            ctx.query.edge_preds[edge_idx].as_deref()) {
            return false;
        }
    }
}
```

#### `count_leaf_fused` (line ~385)

Same pattern: after the bitset/`other_mapped` adjacency checks, run `adj_check` loop, then gate `adj_pred` behind `EP::HAS_PREDICATES`.

### 5. Unchanged paths

- **`is_feasible` / `is_feasible_reverse_only`**: Keep using original `adj` + `check_edge`. These are fallback paths for multi-slot feasibility, Iso/SubIso reverse checks. They handle all edge types uniformly.
- **`check_negated_connected`**: Unchanged — operates on `negated_connected` list, not `adj`.
- **`check_ban_clusters` / `ban_backtrack`**: Unchanged — uses `ban_clusters` structure.
- **`get_covered_slots`**: Unchanged — uses `adj` for slot coverage analysis.
- **`lookahead_ok` / `best_mapped_neighbor_count`**: Uses `adj` for neighbor counting. Unchanged.

### 6. Cost model after change

| Edge type | Path | Cost |
|-----------|------|------|
| Positive, SLOT_COUNT==1, no pred | bitset fast path (`pattern_adj_bits`) | bitmask iteration + `is_adjacent` |
| Negated / any_slot / multi-slot, no pred | `adj_check` loop | `has_edge_in_slot` (≈ `is_adjacent`) + bool compare |
| Has predicate | `adj_pred` loop (gated by `HAS_PREDICATES`) | `check_edge` with `dyn Fn` closure |

Common case (no predicates, no negation): bitset fast path only, `adj_check` is empty, `adj_pred` loop compiled away.

Mixed case (some negated edges, no predicates): bitset handles positive edges, `adj_check` handles negated edges with `has_edge_in_slot`, `adj_pred` loop compiled away.

Predicate case: all three paths active, but only predicate edges pay closure cost.
