# Visualizer Protocol Design

**Goal:** Enable remote visualization of grw graphs and interactive search debugging via a pluggable websocket protocol.

**Architecture:** Three layers — observer traits in grw core (zero-cost when unused), protocol messages and websocket transport in a new grw_viz crate, thin integration glue in grw_repl.

**Crates:** grw (observer traits), grw_viz (protocol + transport), grw_repl (integration)

---

## Layer 1: Watcher Trait in `grw`

### Unified Watcher

New module `grw::watch`. A single `Watcher<NV, E: Edge>` trait combines search observation and mutation observation:

```rust
pub trait Watcher<NV, E: Edge> {
    const ACTIVE: bool = true;

    // search events
    fn on_bind(&mut self, step: usize, pattern_node: LocalId, graph_node: id::N) -> Control;
    fn on_unbind(&mut self, step: usize, pattern_node: LocalId);
    fn on_edge_test(
        &mut self, pattern_edge: usize, src: id::N, tgt: id::N,
        exists: bool, pred_pass: bool, negated: bool,
    ) -> Control;
    fn on_ban_verdict(&mut self, cluster: usize, verdict: BanVerdict) -> Control;
    fn on_match(&mut self, mapping: &[(LocalId, id::N)]) -> Control;

    // mutation events
    fn on_node_added(&mut self, id: id::N, val: &NV);
    fn on_node_removed(&mut self, id: id::N);
    fn on_node_changed(&mut self, id: id::N, val: &NV);
    fn on_edge_added(&mut self, id: id::E, n1: id::N, n2: id::N, slot: &E::Slot, val: &E::Val);
    fn on_edge_removed(&mut self, id: id::E);
    fn on_edge_changed(&mut self, id: id::E, val: &E::Val);
}

pub enum Control { Continue, Pause, Stop }

pub enum BanVerdict {
    SharedEdgeFail,
    NoCandidate,
    Fired,
}
```

- `LocalId` re-exported publicly from `grw::watch` (requires making `graph::dsl` module `pub`).
- `const ACTIVE: bool` enables compile-time elimination. `Silent` sets this to `false`; all methods are `#[inline(always)]` no-ops. When `!W::ACTIVE`, the compiler eliminates all hook calls and fast-path optimizations in the matcher remain untouched.
- `Control::Pause` causes the watcher to block internally (e.g. waits on a condvar for a websocket signal). The matcher just sees a function call that takes longer.
- `Control::Stop` causes the matcher to terminate iteration early — `Iterator::next()` returns `None`.

### WatchedGraph

`graph.watched(&mut watcher)` returns `WatchedGraph<'_, NV, E, W>`. This wrapper:
- Delegates all read operations to the inner graph
- Intercepts mutations (via `apply`/`modify`), fires mutation events from the `Modification` result
- Passes the same `&mut W` to `Seq::search_watched` for search events
- Since the same watcher instance handles both search and mutation, the visualizer connection (e.g. `VizClient`) lives in one place

Runtime toggles: a concrete watcher impl can have `watch_search: bool` and `watch_mutations: bool` fields. Each method checks the flag and short-circuits. When the graph is wrapped in `WatchedGraph`, hooks are compiled in but individually togglable. When the graph is NOT wrapped (bare `Graph` + `Seq::search` with `Silent`), everything compiles away completely.

---

## Layer 2: Protocol Messages in `grw_viz`

### Wire Format

Negotiated at connection time. Two supported formats:
- `Json` — serde_json, for browser-based visualizers
- `Binary` — bincode, for native visualizers (Bevy)

```rust
pub enum WireFormat { Json, Binary }
```

### Wire Types

All node and edge IDs are transmitted as `u64` on the wire, regardless of the compile-time `Id` size (`u32` or `u64` feature flag). The sender widens `u32` IDs to `u64`; the receiver narrows if needed. This keeps the protocol stable across builds.

`edge_kind` encoding: `0` = Undir, `1` = Dir, `2` = Anydir.

### Messages: Repl to Visualizer (Push / Response)

```rust
pub enum VizMsg {
    // Connection
    Hello { protocol_version: u32, format: WireFormat, grw_version: String },

    // Graph lifecycle
    GraphAdded {
        name: String,
        node_count: u64,
        edge_count: u64,
        nv_type: String,
        ev_type: String,
        edge_kind: u8,  // 0=Undir, 1=Dir, 2=Anydir
    },
    GraphRemoved { name: String },
    GraphMutated {
        name: String,
        nodes_added: Vec<(u64, Value)>,
        nodes_removed: Vec<u64>,
        nodes_changed: Vec<(u64, Value)>,
        edges_added: Vec<EdgeDesc>,
        edges_removed: Vec<u64>,
        edges_changed: Vec<(u64, Value)>,
    },

    // Bulk data responses (to VizReq pulls)
    NodeValues { graph: String, nodes: Vec<(u64, Value)> },
    EdgeValues { graph: String, edges: Vec<EdgeDesc> },

    // Search events (pushed during stepped search)
    SearchStarted { graph: String, pattern: PatternDesc },
    NodeBound { step: usize, pattern_node: u64, graph_node: u64 },
    NodeUnbound { step: usize, pattern_node: u64 },
    EdgeTested {
        pattern_edge: usize,
        src: u64, tgt: u64,
        exists: bool, pred_pass: bool, negated: bool,
    },
    BanVerdict { cluster: usize, verdict: String },
    MatchFound { mapping: Vec<(u64, u64)> },
    SearchFinished { match_count: usize },
}
```

`Value` is `serde_json::Value` for JSON format, or raw bytes for binary format — abstracted behind a serialization trait.

`EdgeDesc` captures full edge info:
```rust
pub struct EdgeDesc {
    pub edge_id: u64,
    pub n1: u64,
    pub n2: u64,
    pub slot_kind: u8,  // 0=Undir, 1=DirSrc, 2=DirTgt
    pub value: Value,
}
```

`PatternDesc` is a serializable description of the search pattern:
```rust
pub struct PatternDesc {
    pub nodes: Vec<PatternNodeDesc>,
    pub edges: Vec<PatternEdgeDesc>,  // indexed by position — matches pattern_edge in EdgeTested
    pub clusters: Vec<PatternClusterDesc>,
}

pub struct PatternNodeDesc {
    pub local_id: u64,
    pub has_pred: bool,
    pub negated: bool,
}

pub struct PatternEdgeDesc {
    pub n1_local: u64,
    pub n2_local: u64,
    pub direction: u8,   // 0=Undir, 1=Dir, 2=Anydir
    pub negated: bool,
    pub has_pred: bool,
}

pub struct PatternClusterDesc {
    pub morphism: String,  // "iso", "subiso", "mono", "homo"
    pub kind: String,      // "get" or "ban"
    pub node_indices: Vec<usize>,
}
```

### Messages: Visualizer to Repl (Requests)

```rust
pub enum VizReq {
    // Connection
    HelloAck { format: WireFormat },

    // Discovery
    ListGraphs,
    GetGraphMeta { name: String },

    // Bulk data pulls
    GetNodes { graph: String, offset: u64, limit: u64 },
    GetEdges { graph: String, offset: u64, limit: u64 },
    GetNodeValue { graph: String, id: u64 },
    GetEdgeValue { graph: String, id: u64 },

    // Search control (during stepped search)
    Step,        // advance one candidate binding
    NextMatch,   // advance to next complete match
    Continue,    // run to completion (stop pausing)
    StopSearch,  // abort search
}
```

The visualizer decides how much data to pull. It receives `GraphAdded` with counts and type info, then can request everything in bulk (`GetNodes` with large limit) or fetch individual values on demand.

---

## Layer 3: WebSocket Transport in `grw_viz`

### Connection Model

- Visualizer runs a websocket server (e.g. Bevy app on `ws://localhost:9001`)
- Repl connects as a websocket client
- One connection at a time
- Repl command to connect: `:viz ws://localhost:9001`
- Repl command to disconnect: `:viz off`

### Transport

The `grw_viz` crate provides:

```rust
pub struct VizClient {
    // websocket connection, wire format, request channel
}

impl VizClient {
    pub fn connect(url: &str) -> Result<Self, VizError>;
    pub fn disconnect(&mut self);
    pub fn send(&mut self, msg: &VizMsg) -> Result<(), VizError>;
    pub fn recv(&mut self) -> Result<VizReq, VizError>;
    pub fn try_recv(&mut self) -> Result<Option<VizReq>, VizError>;
    pub fn format(&self) -> WireFormat;
}
```

### Format Negotiation

1. Repl connects via websocket
2. Repl sends `Hello { protocol_version, format, grw_version }`
3. Visualizer responds with `HelloAck { format }` — may accept or override the format
4. All subsequent messages use the acknowledged format

---

## Layer 4: grw_repl Integration

### REPL Commands

```
:viz ws://localhost:9001    connect to visualizer
:viz off                    disconnect
:viz                        show connection status
```

### Concurrency Model

The repl runs a background reader thread for incoming `VizReq` messages. This thread:
- Reads from the websocket continuously
- Queues data requests (`ListGraphs`, `GetNodes`, etc.) into an `mpsc` channel polled by the main thread between REPL commands
- For search control messages (`Step`, `NextMatch`, `Continue`, `StopSearch`): signals a `Condvar` that the `SearchObserver` is waiting on

This solves the blocking problem: the main thread runs `Iterator::next()` → observer's `on_bind` blocks on the condvar → background thread receives `Step` from websocket → signals condvar → `on_bind` returns → iteration resumes.

### Event Flow

When a visualizer is connected:

- `graph!` macro → push `GraphAdded`, respond to any data pulls
- `modify!` macro → run mutation with `MutationObserver` impl that collects changes → push `GraphMutated` with diff
- `search!` macro → if visualizer connected, run `search_with` using a `SearchObserver` impl that sends events over websocket and blocks on `Pause` for stepped mode
- Between REPL commands, drain queued `VizReq` data requests and respond

### Watcher Implementation (in grw_repl)

The repl provides a `VizWatcher` that implements `Watcher<NV, E>`:
- Holds a sender to the websocket and a reference to the search control condvar
- Has runtime flags: `watch_search: bool`, `watch_mutations: bool`
- Search methods: translate each `on_*` callback into a `VizMsg` and send it. When stepping is requested, return `Control::Pause` — block on condvar waiting for `Step`/`NextMatch`/`Continue`/`StopSearch`. On `StopSearch`, return `Control::Stop`.
- Mutation methods: translate each `on_*` callback into entries for a `GraphMutated` message, batched and sent after the mutation completes.
- When a flag is off, each method short-circuits immediately — branch prediction handles the cost.

### Plugin ABI Extension

The plugin needs new exports for serializing node/edge values:
- `grw_plugin_serialize_node(graph: *const c_void, id: u64, format: u8) -> PluginResult`
- `grw_plugin_serialize_edge(graph: *const c_void, id: u64, format: u8) -> PluginResult`
- `grw_plugin_serialize_nodes_bulk(graph: *const c_void, offset: u64, limit: u64, format: u8) -> PluginResult`
- `grw_plugin_serialize_edges_bulk(graph: *const c_void, offset: u64, limit: u64, format: u8) -> PluginResult`

These use serde (json or bincode) on the concrete NV/EV types. Format byte: 0 = JSON, 1 = bincode.

---

## Scope Boundaries

**In scope for this design:**
- Watcher trait in grw (Watcher, Control, Silent, BanVerdict)
- WatchedGraph wrapper
- Protocol message types (VizMsg, VizReq, PatternDesc, EdgeDesc)
- WebSocket client in grw_viz
- Wire format negotiation (JSON / bincode)
- REPL `:viz` command and observer implementations
- Plugin ABI serialization exports

**Out of scope:**
- Any specific visualizer implementation (Bevy, xdot, browser)
- Graph layout algorithms (force-directed, hierarchical, etc.)
- Pattern rendering in visualizers
- Multi-connection support
