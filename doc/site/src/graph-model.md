# Graph Model

A graph `MGraph<NV, ER>` is parameterized by two type parameters:

- **`NV`** — node value type (use `()` for no attributes)
- **`ER`** — edge relation type, which determines the edge topology

## Edge Types

GRW supports three edge relation types:

| Edge type | Operators | Max edges between 2 nodes | Slots |
|-----------|-----------|---------------------------|-------|
| `edge::Undir<EV>` | `^` | 1 undirected | `UND` |
| `edge::Dir<EV>` | `>>` `<<` | 2 directed (incoming + outgoing) | `SRC` `TGT` |
| `edge::Anydir<EV>` | `^` `>>` `<<` | 3 (undirected + incoming + outgoing) | `UND` `SRC` `TGT` |

Between any two nodes, there can be at most **one edge per slot**. This means:
- **Undirected** graphs allow 1 edge per node pair
- **Directed** graphs allow 2 edges per node pair (one in each direction)
- **Anydirected** graphs allow 3 edges per node pair (both directions + undirected)

If you need multiple edges between the same pair with the same orientation, model that as a collection in the edge value type.

## Type Aliases

Common configurations have convenient aliases:

```rust
// Undirected
type MUndir0      = MGraph<(), edge::Undir<()>>;      // no attributes
type MUndirN<NV>  = MGraph<NV, edge::Undir<()>>;      // node values only
type MUndirE<EV>  = MGraph<(), edge::Undir<EV>>;      // edge values only
type MUndir<N, E> = MGraph<NV, edge::Undir<EV>>;      // both

// Directed
type MDir0         = MGraph<(), edge::Dir<()>>;
type MDirN<NV>     = MGraph<NV, edge::Dir<()>>;
type MDirE<EV>     = MGraph<(), edge::Dir<EV>>;
type MDir<NV, EV>  = MGraph<NV, edge::Dir<EV>>;

// Anydirected
type MAnydir0         = MGraph<(), edge::Anydir<()>>;
type MAnydirN<NV>     = MGraph<NV, edge::Anydir<()>>;
type MAnydirE<EV>     = MGraph<(), edge::Anydir<EV>>;
type MAnydir<NV, EV>  = MGraph<NV, edge::Anydir<EV>>;
```

## Node and Edge Access

```rust
// count
g.node_count()
g.edge_count()

// check existence
g.has(0)              // node by Id
g.has((0, 1))         // edge between nodes

// get values
g.get(0)              // Option<&NV>
g.get_mut(0)          // Option<&mut NV>

// iterate
for (nid, val) in g.node_iter() { /* ... */ }
for (edef, val) in g.edge_iter() { /* ... */ }

// adjacency
g.is_adjacent(0, 1)

// edge relation (typed access to all edges between a pair)
let rel = g.rel((0, 1));   // returns Rel with typed slot access
```
