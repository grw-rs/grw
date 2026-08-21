# shagra-repl Engine Design

## Goal

An evaluation engine for interactive graph construction, modification, and (future) search using real Rust code with full type safety. Engine-only — no UI in this phase.

## Approach

Thin wrapper around [evcxr](https://github.com/evcxr/evcxr) `EvalContext`. Each REPL input is compiled as a genuine Rust snippet with access to the full shagra API. State (bound variables) persists across evaluations. LLVM backend at opt-level 0 for the prototype.

### Why evcxr

- Solves variable persistence across evals (heap-allocated state registry)
- Solves incremental compilation (caches dep builds, only recompiles user glue)
- Solves error extraction (returns structured rustc diagnostics)
- Maintains a synthetic crate on disk — rust-analyzer can index it for future LSP integration
- Library crate (`EvalContext` API), not just a binary

### Why not alternatives

- **Custom compilation engine**: Reinvents ~8k lines of solved problems (variable persistence, incremental compilation, error extraction). Build later if evcxr hits walls.
- **Interpreted DSL**: Two languages to learn, maintenance parity burden with Rust DSL.
- **Cranelift/WASM hybrid**: FFI boundary between WASM and native shagra types is non-trivial. Not worth the complexity since shagra itself is pre-compiled as a dep (only user glue code compiles per eval).

## Architecture

New workspace crate `shagra-repl`:

```
shagra-repl/
  src/
    lib.rs        -- public API: Session, Output, Error
    session.rs    -- wraps EvalContext, manages shagra prelude
    inspect.rs    -- introspection codegen (graph stats, shape tree)
    output.rs     -- structured eval results, diagnostic parsing
  Cargo.toml      -- depends on evcxr, shagra (for re-exports only)
```

### Core Types

```rust
pub struct Session {
    ctx: evcxr::EvalContext,
    outputs: evcxr::EvalContextOutputs,
}

pub struct SessionConfig {
    pub opt_level: OptLevel,
    pub sccache: bool,
}

pub enum Output {
    Value(String),     // expression with Display-able result
    Silent,            // statement executed, no return value
    Stdout(String),    // eval produced stdout
}

pub struct EvalError {
    pub diagnostics: Vec<Diagnostic>,
}

pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
}
```

### Session Lifecycle

1. `Session::new(config)` creates an `EvalContext`
2. Adds `shagra` as a dependency
3. Injects prelude: `use shagra::*; use shagra::modify::dsl::*;`
4. User evals define graph types and create instances
5. Subsequent evals modify/inspect the graph, referencing prior bindings
6. `session.variables()` lists bound names and types
7. `session.reset()` clears all state

### Introspection

Built-in methods that generate and eval Rust snippets against the live graph:

```rust
impl Session {
    pub fn inspect(&mut self, var_name: &str) -> Result<GraphInfo, Error>;
    pub fn show_shapes(&mut self, var_name: &str) -> Result<String, Error>;
}
```

These construct code strings (e.g. `format!("{} nodes", g.nodes.by_id.len())`) and feed them through `eval()`. The engine doesn't interpret anything — it compiles real Rust.

### Error Handling

evcxr returns rustc errors as formatted text. We parse into structured `Diagnostic` values with severity, message, and span (line/col in user's snippet). This lets any future UI layer render errors its own way.

### Eval Latency

- First eval (shagra dep build): ~5-15s (one-time, cached after)
- Subsequent evals: ~100-300ms at opt-level 0 (only user glue compiles)
- Shagra's shape pipeline runs at full speed (pre-compiled in the dep)

## Future Extensions (not in prototype)

### LSP / Code Completion

evcxr maintains a synthetic crate at a temp path. Point rust-analyzer at it:
- As user types, write partial input to the synthetic source file
- RA provides completions, inline diagnostics, hover info
- This is how evcxr's Jupyter kernel works today

### Cranelift Backend

If eval latency is a problem, add `-Zcodegen-backend=cranelift` support behind a nightly feature flag. Expected to drop eval to ~30-80ms.

### Graph Visualization

Introspection methods could return structured data (not just strings) for rendering: node/edge counts, shape tree, adjacency summaries. A TUI or web frontend consumes these.

### Search Integration

When shagra gains search (isomorphism, subgraph matching), the REPL engine exposes it naturally — user just writes Rust code using the search API. No new engine work needed.
