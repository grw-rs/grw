# REPL CLI Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Interactive REPL binary for loading `.grw` graph files, inspecting graph metadata, and evaluating predicates against node values.

**Architecture:** `main.rs` handles CLI parsing and startup (header read, plugin compile, graph load), then delegates to `repl::run` which owns the readline loop and command dispatch. The `repl` module is `pub(crate)` — only consumed by the binary, not part of the public library API.

**Tech Stack:** clap 4 (derive), rustyline 15 (DefaultEditor), existing `grw::persist::read_header` and `grw_repl::plugin::{load_or_compile, Plugin, LoadedGraph}`

**Spec:** `docs/plans/2026-03-13-repl-cli-design.md`

---

## Chunk 1: Setup and Implementation

### Task 1: Add dependencies and binary target

**Files:**
- Modify: `grw_repl/Cargo.toml`

- [ ] **Step 1: Add clap and rustyline dependencies**

Add `clap` and `rustyline` to `[dependencies]` in `grw_repl/Cargo.toml`:

```toml
clap = { version = "4", features = ["derive"] }
rustyline = "15"
```

Do NOT add the `[[bin]]` section yet — that goes in Task 2 alongside `main.rs` creation, since cargo fails on missing binary source files.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p grw_repl`
Expected: PASS

---

### Task 2: Create main.rs — CLI entry point

**Files:**
- Create: `grw_repl/src/main.rs`
- Modify: `grw_repl/src/lib.rs` (add `mod repl`)

- [ ] **Step 1: Add [[bin]] section to Cargo.toml**

Add before `[dependencies]` in `grw_repl/Cargo.toml`:

```toml
[[bin]]
name = "grw"
path = "src/main.rs"
```

- [ ] **Step 2: Create main.rs with full startup sequence**

```rust
use std::path::PathBuf;
use std::process;

use clap::Parser;

#[derive(Parser)]
#[command(name = "grw", about = "Interactive graph REPL")]
struct Cli {
    path: PathBuf,
    #[arg(long = "types-crate")]
    types_crate: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    let header = match grw::persist::read_header(&cli.path) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    let types_crate = cli.types_crate.as_deref();
    let plugin = match grw_repl::plugin::load_or_compile(&cli.path, types_crate) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    let graph = match plugin.load_graph(&cli.path) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    println!(
        "Loaded {} graph: {} nodes, {} edges",
        header.nv_type,
        graph.node_count(),
        graph.edge_count(),
    );

    grw_repl::repl::run(header, &plugin, graph);
}
```

- [ ] **Step 3: Add `pub mod repl;` to lib.rs**

Add `pub mod repl;` to `grw_repl/src/lib.rs` after the existing `pub mod plugin;` line.

Note: The spec says `mod repl` (private), but `main.rs` is a separate binary crate that imports `grw_repl` as a library dependency. Binary crates access library items through `grw_repl::repl::run`, so `repl` must be `pub`. The `run` function itself is `pub` (not `pub(crate)`) for the same reason — `pub(crate)` would only be visible within the library crate, not the binary.

- [ ] **Step 4: Create a stub repl.rs so main.rs compiles**

Create `grw_repl/src/repl.rs` with a minimal stub:

```rust
use crate::plugin;

pub fn run(_header: grw::persist::Header, _plugin: &plugin::Plugin, _graph: plugin::LoadedGraph<'_>) -> ! {
    std::process::exit(0)
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p grw_repl`
Expected: PASS

---

### Task 3: Implement repl.rs — REPL loop and command dispatch

**Files:**
- Modify: `grw_repl/src/repl.rs`

- [ ] **Step 1: Implement the full REPL module**

Replace the stub `grw_repl/src/repl.rs` with the complete implementation:

```rust
use crate::plugin;

pub fn run(header: grw::persist::Header, plugin: &plugin::Plugin, graph: plugin::LoadedGraph<'_>) -> ! {
    let mut editor = rustyline::DefaultEditor::new()
        .expect("failed to create editor");

    let history_path = dirs::cache_dir()
        .map(|d| d.join("grw").join("history.txt"));

    if let Some(ref path) = history_path {
        let _ = editor.load_history(path);
    }

    loop {
        match editor.readline("grw> ") {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let _ = editor.add_history_entry(trimmed);
                dispatch(trimmed, &header, plugin, &graph);
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                break;
            }
            Err(e) => {
                eprintln!("error: {e}");
                break;
            }
        }
    }

    if let Some(ref path) = history_path {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = editor.save_history(path);
    }

    std::process::exit(0)
}

fn dispatch(
    input: &str,
    header: &grw::persist::Header,
    plugin: &plugin::Plugin,
    graph: &plugin::LoadedGraph<'_>,
) {
    if let Some(cmd) = input.strip_prefix(':') {
        let parts: Vec<&str> = cmd.splitn(2, char::is_whitespace).collect();
        let name = parts[0];
        let arg = parts.get(1).map(|s| s.trim());
        match name {
            "help" | "h" => cmd_help(),
            "info" => cmd_info(header, graph),
            "fields" => cmd_fields(plugin),
            "node" => cmd_node(graph, arg),
            "quit" | "q" => std::process::exit(0),
            _ => eprintln!("unknown command: :{name}. Type :help for help."),
        }
    } else {
        cmd_eval_pred(graph, input);
    }
}

fn cmd_help() {
    println!("Commands:");
    println!("  :help, :h          show this help");
    println!("  :info               show graph summary");
    println!("  :fields             show node value fields");
    println!("  :node <idx>         inspect a node's field values");
    println!("  :quit, :q           exit");
    println!("  <expr>              evaluate predicate (e.g. |n| n.weight > 50.0)");
}

fn cmd_info(header: &grw::persist::Header, graph: &plugin::LoadedGraph<'_>) {
    let edge_kind_str = match header.edge_kind {
        0 => "undir".to_string(),
        1 => "dir".to_string(),
        2 => "anydir".to_string(),
        n => format!("unknown({n})"),
    };
    println!("nodes:     {}", graph.node_count());
    println!("edges:     {}", graph.edge_count());
    println!("nv_type:   {}", header.nv_type);
    println!("ev_type:   {}", header.ev_type);
    println!("edge_kind: {edge_kind_str}");
}

fn cmd_fields(plugin: &plugin::Plugin) {
    match plugin.node_fields() {
        Ok(json_str) => {
            let fields: Vec<serde_json::Value> = match serde_json::from_str(&json_str) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("error parsing fields: {e}");
                    return;
                }
            };
            if fields.is_empty() {
                println!("  (no fields)");
                return;
            }
            let max_name = fields.iter()
                .filter_map(|f| f.get("name").and_then(|n| n.as_str()))
                .map(|n| n.len())
                .max()
                .unwrap_or(4);
            let name_width = max_name.max(4);
            println!("  {:<name_width$}   type", "name");
            println!("  {:<name_width$}   ────", "────");
            for f in &fields {
                let name = f.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                let ty = f.get("type").and_then(|t| t.as_str()).unwrap_or("?");
                println!("  {:<name_width$}   {ty}", name);
            }
        }
        Err(e) => eprintln!("error: {e}"),
    }
}

fn cmd_node(graph: &plugin::LoadedGraph<'_>, arg: Option<&str>) {
    let idx: u64 = match arg.and_then(|s| s.parse().ok()) {
        Some(i) => i,
        None => {
            eprintln!("usage: :node <idx>");
            return;
        }
    };
    match graph.inspect_node(idx) {
        Ok(json_str) => {
            let map: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(&json_str) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("error parsing node data: {e}");
                    return;
                }
            };
            println!("node {idx}:");
            if map.is_empty() {
                println!("  (no fields)");
                return;
            }
            let max_key = map.keys().map(|k| k.len()).max().unwrap_or(0);
            for (k, v) in &map {
                let val_str = match v.as_str() {
                    Some(s) => s.to_string(),
                    None => v.to_string(),
                };
                println!("  {:<max_key$}  = {val_str}", k);
            }
        }
        Err(e) => eprintln!("error: {e}"),
    }
}

fn cmd_eval_pred(graph: &plugin::LoadedGraph<'_>, input: &str) {
    match graph.eval_pred(input) {
        Ok(json_str) => {
            let indices: Vec<u64> = match serde_json::from_str(&json_str) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("error parsing results: {e}");
                    return;
                }
            };
            println!("{} matches: {:?}", indices.len(), indices);
        }
        Err(e) => eprintln!("error: {e}"),
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p grw_repl`
Expected: PASS

- [ ] **Step 3: Run existing tests to verify nothing is broken**

Run: `cargo test -p grw_repl`
Expected: All existing tests PASS

Run: `cargo test -p grw --features persist`
Expected: All existing tests PASS

---

### Task 4: Manual smoke test

- [ ] **Step 1: Build the binary**

Run: `cargo build -p grw_repl`
Expected: Compiles and produces a `grw` binary in `target/debug/`

- [ ] **Step 2: Verify CLI help**

Run: `cargo run -p grw_repl --bin grw -- --help`
Expected: Shows clap-generated help with `path` argument and `--types-crate` option.

- [ ] **Step 3: Verify error on missing file**

Run: `cargo run -p grw_repl --bin grw -- /nonexistent.grw`
Expected: Prints error to stderr, exits with code 1.
