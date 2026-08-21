# REPL CLI Design

Interactive CLI for loading `.grw` graph files and evaluating predicates against node values.

Scope: binary entry point, CLI arg parsing, interactive REPL loop with rustyline, command dispatch, output formatting.
Out of scope: batch mode, piped stdin, tab completion, edge value inspection, search pattern execution.

## Binary

Name: `grw`. Defined via `[[bin]]` in `grw_repl/Cargo.toml`. Entry point: `grw_repl/src/main.rs`.

Invocation:
```
grw <path.grw> [--types-crate <path>]
```

`--types-crate` overrides automatic Cargo.toml discovery. When omitted, `load_or_compile` walks up from the `.grw` file to find the nearest `Cargo.toml`.

## Startup Flow

1. Parse CLI args with `clap` (derive API).
2. Call `grw_repl::plugin::load_or_compile(grw_path, types_crate)`. Plugin compilation progress prints to stderr (already implemented in `load_or_compile`).
3. Call `plugin.load_graph(grw_path)`.
4. Print banner: `Loaded <nv_type> graph: <N> nodes, <E> edges` (nv_type from the header, counts from the loaded graph).
5. Enter REPL loop.

On failure at steps 2 or 3, print the error to stderr and exit with code 1.

## REPL Loop

Prompt: `grw> `

Editor: `rustyline::DefaultEditor` (no custom helper or completer).

History file: resolved via `dirs::cache_dir()` as `<cache_dir>/grw/history.txt` (same base as plugin cache). Created on first write. Missing directory is created automatically. History load failure is silently ignored (fresh session).

Input handling:
- Empty line: skip, re-prompt.
- Ctrl-C: cancel current line input, re-prompt (rustyline default).
- Ctrl-D / EOF: exit cleanly.
- Lines starting with `:`: command dispatch.
- All other input: treated as predicate expression.

## Commands

### `:help` / `:h`

Print available commands with one-line descriptions:

```
Commands:
  :help, :h          show this help
  :info               show graph summary
  :fields             show node value fields
  :node <idx>         inspect a node's field values
  :quit, :q           exit
  <expr>              evaluate predicate (e.g. |n| n.weight > 50.0)
```

### `:info`

Print graph summary as key-value lines:

```
nodes:     1234
edges:     5678
nv_type:   my_crate::Cargo
ev_type:   u32
edge_kind: undir
```

Edge kind displayed as `undir`/`dir`/`anydir` (mapped from 0/1/2). Node and edge counts come from `LoadedGraph::node_count()` / `edge_count()`. Type names come from the `.grw` header (read during `load_or_compile`).

The header must be accessible to the REPL loop. `load_or_compile` currently consumes the header internally. Solution: read the header separately via `grw::persist::read_header` before calling `load_or_compile`, and pass it to the REPL session. This reads the header twice (once for the REPL, once inside `load_or_compile`). Intentional — avoids changing the `load_or_compile` API. Unknown edge_kind values are displayed as `unknown(<n>)`.

### `:fields`

Print node value field names and types. Calls `Plugin::node_fields()` which returns a JSON array of `{"name": "...", "type": "..."}` objects.

Output formatted as a simple table:

```
  name       type
  ────       ────
  weight     F64
  label      String
  active     Bool
```

### `:node <idx>`

Inspect a single node. Calls `LoadedGraph::inspect_node(idx)` which returns a JSON object of field name → value string.

Output formatted as key-value pairs:

```
node 42:
  weight  = 3.14
  label   = "cargo_ship"
  active  = true
```

The `<idx>` argument is parsed as a `u64`. Non-numeric or missing argument prints: `usage: :node <idx>`. Out-of-range index returns an error from the plugin.

### `:quit` / `:q`

Exit the process cleanly (drop `LoadedGraph`, drop `Plugin`, exit 0).

### Predicate Expression

Any input not starting with `:` is treated as a predicate expression (e.g. `|n| n.weight > 50.0`).

Calls `LoadedGraph::eval_pred(input)` which returns a JSON array of matching node indices.

Output: print matching count and all indices (no truncation — user can pipe through external tools if needed):

```
12 matches: [0, 3, 7, 12, 15, 22, 31, 44, 56, 78, 91, 103]
```

If no matches: `0 matches: []`

On predicate parse/eval error: print the error message, continue REPL loop.

## Error Handling

| Error | Behavior |
|-------|----------|
| Plugin compilation failure | Print error to stderr, exit 1 |
| Graph load failure | Print error to stderr, exit 1 |
| Invalid `.grw` file | Print error to stderr, exit 1 |
| v1 `.grw` file | Print "re-save with current version" message, exit 1 |
| Predicate parse error | Print error, continue REPL |
| Invalid `:node` index | Print error, continue REPL |
| Unknown command | Print "unknown command: <cmd>. Type :help for help.", continue REPL |
| History file unwritable | Silently ignore |

## Dependencies

Added to `grw_repl/Cargo.toml`:
- `clap = { version = "4", features = ["derive"] }`
- `rustyline = "15"`

## Files

| File | Action | Responsibility |
|------|--------|---------------|
| `grw_repl/Cargo.toml` | Modify | Add `clap`, `rustyline`, `[[bin]]` section |
| `grw_repl/src/main.rs` | Create | CLI arg parsing, startup sequence, error handling |
| `grw_repl/src/repl.rs` | Create | REPL loop, command parsing, command dispatch, output formatting |
| `grw_repl/src/lib.rs` | Modify | Add `mod repl` (private — only used by the binary, not a public API) |

## Module Interface

`repl.rs` exposes a single entry point consumed by `main.rs`:

```rust
pub(crate) fn run(header: grw::persist::Header, plugin: &plugin::Plugin, graph: plugin::LoadedGraph<'_>) -> !
```

`plugin` is passed by reference — `main.rs` owns `Plugin` on the stack, passes `&plugin` to `run`, and `LoadedGraph` (which also borrows `&Plugin` internally) coexists without a move conflict. `:fields` calls `plugin.node_fields()` directly via this reference.

`run` owns the REPL loop and never returns (exits via `std::process::exit(0)` on `:quit`/EOF). Because `run` diverges, `Plugin` and `LoadedGraph` destructors in `main` never run — this is intentional; the OS reclaims all resources on process exit. `main.rs` handles all fallible startup (arg parsing, header read, plugin compilation, graph loading) and calls `repl::run` only on success.
