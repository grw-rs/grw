use crate::graph::{self, Graph, dsl::LocalId};
use crate::search::engine::Match;
use crate::search::query::{Query, BanCluster};
use crate::search::Morphism;
use crate::viz::DotEdge;
use crate::Id;
use crate::id;
use crate::search::query::NodeKind;
use rustc_hash::{FxHashMap, FxHashSet};
use std::convert::Infallible;
use std::io::{IsTerminal, Write};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;

pub trait Ref<NV, ER: graph::Edge> {
    fn graph_ref(&self) -> &Graph<NV, ER>;
}

impl<NV, ER: graph::Edge> Ref<NV, ER> for Graph<NV, ER> {
    fn graph_ref(&self) -> &Graph<NV, ER> { self }
}

impl<'a, NV, ER: graph::Edge> Ref<NV, ER> for &'a Graph<NV, ER> {
    fn graph_ref(&self) -> &Graph<NV, ER> { *self }
}

pub trait ResolveGraph<NV, ER: graph::Edge> {
    type Err;
    type Out: Ref<NV, ER>;
    fn resolve(self) -> Result<Self::Out, Self::Err>;
}

impl<NV, ER: graph::Edge> ResolveGraph<NV, ER> for Graph<NV, ER> {
    type Err = Infallible;
    type Out = Graph<NV, ER>;
    fn resolve(self) -> Result<Graph<NV, ER>, Infallible> { Ok(self) }
}

impl<NV, ER: graph::Edge, E> ResolveGraph<NV, ER> for Result<Graph<NV, ER>, E> {
    type Err = E;
    type Out = Graph<NV, ER>;
    fn resolve(self) -> Result<Graph<NV, ER>, E> { self }
}

impl<'a, NV, ER: graph::Edge> ResolveGraph<NV, ER> for &'a Graph<NV, ER> {
    type Err = Infallible;
    type Out = &'a Graph<NV, ER>;
    fn resolve(self) -> Result<&'a Graph<NV, ER>, Infallible> { Ok(self) }
}

struct EdgeMatch<S> {
    pattern_source_id: LocalId,
    pattern_target_id: LocalId,
    graph_source: id::N,
    graph_target: id::N,
    slot: S,
    negated: bool,
    exists: bool,
    pred_pass: bool,
}

impl<S> EdgeMatch<S> {
    fn satisfied(&self) -> bool {
        self.exists && self.pred_pass
    }
}

fn reconstruct_edges<NV, ER: graph::Edge>(
    query: &Query<NV, ER>,
    graph: &Graph<NV, ER>,
    m: &Match,
) -> Vec<EdgeMatch<ER::Slot>> {
    let mut result = Vec::new();
    for (ei, pe) in query.edges.iter().enumerate() {
        let src_local = query.nodes[pe.source].local_id;
        let tgt_local = query.nodes[pe.target].local_id;

        let src_mapped = m.get(src_local);
        let tgt_mapped = m.get(tgt_local);

        let (src_n, tgt_n) = match (src_mapped, tgt_mapped) {
            (Some(s), Some(t)) => (s, t),
            _ => continue,
        };

        let edge_def = ER::edge(pe.slot, (*src_n, *tgt_n));
        let val = graph.get_edge_val(edge_def);
        let exists = val.is_some();
        let pred_pass = match (&query.edge_preds[ei], val) {
            (Some(pred), Some(v)) => pred(v),
            (Some(_), None) => false,
            (None, _) => exists,
        };

        result.push(EdgeMatch {
            pattern_source_id: src_local,
            pattern_target_id: tgt_local,
            graph_source: src_n,
            graph_target: tgt_n,
            slot: pe.slot,
            negated: pe.negated,
            exists,
            pred_pass,
        });
    }
    result
}

enum BanVerdict {
    SharedEdgeFail { src_lid: Id, tgt_lid: Id },
    NoCandidate { ban_only_lid: Id },
    Pass,
}

fn reconstruct_ban_verdict<NV, ER: graph::Edge>(
    query: &Query<NV, ER>,
    graph: &Graph<NV, ER>,
    m: &Match,
    ban: &BanCluster,
) -> BanVerdict {
    let mapped: FxHashMap<usize, id::N> = query.nodes.iter().enumerate()
        .filter_map(|(i, pn)| m.get(pn.local_id).map(|gn| (i, gn)))
        .collect();

    for &edge_idx in &ban.edge_indices {
        let edge = &query.edges[edge_idx];
        let src_mapped = mapped.get(&edge.source);
        let tgt_mapped = mapped.get(&edge.target);
        let (src_n, tgt_n) = match (src_mapped, tgt_mapped) {
            (Some(&s), Some(&t)) => (s, t),
            _ => continue,
        };

        if edge.any_slot {
            let adjacent = graph.is_adjacent(*src_n, *tgt_n);
            let ok = if edge.negated {
                if !adjacent { true }
                else if let Some(pred) = &query.edge_preds[edge_idx] {
                    !any_slot_pred_check::<NV, ER>(graph, src_n, tgt_n, pred.as_ref())
                } else { false }
            } else {
                if !adjacent { false }
                else if let Some(pred) = &query.edge_preds[edge_idx] {
                    any_slot_pred_check::<NV, ER>(graph, src_n, tgt_n, pred.as_ref())
                } else { true }
            };
            if !ok {
                return BanVerdict::SharedEdgeFail {
                    src_lid: query.nodes[edge.source].local_id.0,
                    tgt_lid: query.nodes[edge.target].local_id.0,
                };
            }
        } else {
            let edge_def = ER::edge(edge.slot, (*src_n, *tgt_n));
            let val = graph.get_edge_val(edge_def);
            let ok = if edge.negated {
                match val {
                    None => true,
                    Some(v) => match &query.edge_preds[edge_idx] {
                        Some(pred) => !pred(v),
                        None => false,
                    },
                }
            } else {
                match val {
                    None => false,
                    Some(v) => match &query.edge_preds[edge_idx] {
                        Some(pred) => pred(v),
                        None => true,
                    },
                }
            };
            if !ok {
                return BanVerdict::SharedEdgeFail {
                    src_lid: query.nodes[edge.source].local_id.0,
                    tgt_lid: query.nodes[edge.target].local_id.0,
                };
            }
        }
    }

    if ban.ban_only_nodes.is_empty() {
        return BanVerdict::Pass;
    }

    let reverse: FxHashSet<id::N> = mapped.values().copied().collect();
    if !ban_backtrack_check(query, graph, ban, &mapped, &reverse, &mut vec![None; ban.ban_only_nodes.len()], 0) {
        let first_bn = ban.ban_only_nodes[0];
        return BanVerdict::NoCandidate {
            ban_only_lid: query.nodes[first_bn].local_id.0,
        };
    }

    BanVerdict::Pass
}

fn ban_backtrack_check<NV, ER: graph::Edge>(
    query: &Query<NV, ER>,
    graph: &Graph<NV, ER>,
    ban: &BanCluster,
    mapped: &FxHashMap<usize, id::N>,
    reverse: &FxHashSet<id::N>,
    ban_mapping: &mut Vec<Option<id::N>>,
    depth: usize,
) -> bool {
    if depth == ban.ban_only_nodes.len() {
        return true;
    }

    let pattern_idx = ban.ban_only_nodes[depth];

    for (n, val) in graph.nodes.iter() {
        match ban.morphism {
            Morphism::Iso | Morphism::SubIso | Morphism::Mono => {
                if reverse.contains(&n) { continue; }
                if ban_mapping.iter().any(|m| *m == Some(n)) { continue; }
            }
            Morphism::Homo => {}
        }

        if let Some(pred) = &query.node_preds[pattern_idx] {
            if !pred(val) { continue; }
        }

        if !ban_edge_feasible(query, graph, ban, ban_mapping, mapped, depth, n) {
            continue;
        }

        ban_mapping[depth] = Some(n);
        if ban_backtrack_check(query, graph, ban, mapped, reverse, ban_mapping, depth + 1) {
            ban_mapping[depth] = None;
            return true;
        }
        ban_mapping[depth] = None;
    }

    false
}

fn ban_edge_feasible<NV, ER: graph::Edge>(
    query: &Query<NV, ER>,
    graph: &Graph<NV, ER>,
    ban: &BanCluster,
    ban_mapping: &[Option<id::N>],
    mapped: &FxHashMap<usize, id::N>,
    depth: usize,
    candidate: id::N,
) -> bool {
    let pattern_idx = ban.ban_only_nodes[depth];
    let candidate_id: Id = *candidate;

    for &edge_idx in &ban.edge_indices {
        let edge = &query.edges[edge_idx];

        let (this_is_source, other_pattern_idx) = if edge.source == pattern_idx {
            (true, edge.target)
        } else if edge.target == pattern_idx {
            (false, edge.source)
        } else {
            continue;
        };

        let other_target = if let Some(&m) = mapped.get(&other_pattern_idx) {
            m
        } else if let Some(pos) = ban.ban_only_nodes.iter().position(|&ni| ni == other_pattern_idx) {
            if let Some(m) = ban_mapping[pos] { m } else { continue; }
        } else {
            continue;
        };

        let other_id: Id = *other_target;

        if edge.any_slot {
            let adjacent = graph.is_adjacent(candidate_id, other_id);
            if edge.negated {
                if adjacent {
                    if let Some(pred) = &query.edge_preds[edge_idx] {
                        if any_slot_pred_check::<NV, ER>(graph, candidate, other_target, pred.as_ref()) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            } else {
                if !adjacent { return false; }
                if let Some(pred) = &query.edge_preds[edge_idx] {
                    if !any_slot_pred_check::<NV, ER>(graph, candidate, other_target, pred.as_ref()) {
                        return false;
                    }
                }
            }
        } else {
            let slot = if this_is_source { edge.slot } else { ER::reverse_slot(edge.slot) };

            if edge.negated {
                let adjacent = graph.is_adjacent(candidate_id, other_id);
                if adjacent {
                    let edge_def = ER::edge(slot, (candidate_id, other_id));
                    match graph.get_edge_val(edge_def) {
                        Some(val) => {
                            if let Some(pred) = &query.edge_preds[edge_idx] {
                                if pred(val) { return false; }
                            } else {
                                return false;
                            }
                        }
                        None => {}
                    }
                }
            } else {
                if !graph.is_adjacent(candidate_id, other_id) { return false; }
                let edge_def = ER::edge(slot, (candidate_id, other_id));
                match graph.get_edge_val(edge_def) {
                    Some(val) => {
                        if let Some(pred) = &query.edge_preds[edge_idx] {
                            if !pred(val) { return false; }
                        }
                    }
                    None => return false,
                }
            }
        }
    }

    true
}

fn any_slot_pred_check<NV, ER: graph::Edge>(
    graph: &Graph<NV, ER>,
    a: id::N,
    b: id::N,
    pred: &dyn Fn(&ER::Val) -> bool,
) -> bool {
    graph.edges_between(a, b)
        .any(|(_, val)| pred(val))
}

struct RawModeGuard;

impl RawModeGuard {
    fn enter() -> Self {
        terminal::enable_raw_mode().expect("failed to enable raw terminal mode");
        RawModeGuard
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

pub struct Trace<'g, NV: std::fmt::Debug, ER: graph::Edge<Val: std::fmt::Debug> + DotEdge> {
    graph: &'g Graph<NV, ER>,
    query: &'g Query<NV, ER>,
    matches: Vec<Match>,
    verbose: bool,
    file: &'static str,
    line: u32,
}

impl<'g, NV: std::fmt::Debug, ER: graph::Edge<Val: std::fmt::Debug> + DotEdge> Trace<'g, NV, ER> {
    pub fn new(
        graph: &'g Graph<NV, ER>,
        query: &'g Query<NV, ER>,
        matches: Vec<Match>,
        file: &'static str,
        line: u32,
    ) -> Self {
        Trace { graph, query, matches, verbose: false, file, line }
    }

    pub fn verbose(mut self, v: bool) -> Self {
        self.verbose = v;
        self
    }

    pub fn run(self) {
        match std::env::args().nth(1) {
            Some(src) => println!("{src}:{}", self.line),
            None => println!("{}:{}", self.file, self.line),
        }
        if self.matches.is_empty() {
            let dot = crate::viz::to_dot(self.graph, &[]);
            std::fs::write("/tmp/grw_viz.dot", &dot)
                .unwrap_or_else(|e| panic!("failed to write /tmp/grw_viz.dot: {e}"));
            println!("0 matches");
            return;
        }

        let interactive = std::io::stdout().is_terminal()
            && std::env::var_os("INSIDE_EMACS").is_none();
        if interactive {
            self.run_interactive();
        } else {
            self.run_batch();
        }
    }

    fn run_batch(self) {
        for idx in 0..self.matches.len() {
            self.write_dot_to(idx, &format!("/tmp/grw_viz_{}.dot", idx + 1));
            self.print_match(idx);
        }
        self.write_dot_to(0, "/tmp/grw_viz.dot");
    }

    fn run_interactive(self) {
        let mut idx: usize = 0;
        self.render_step(idx);

        let _guard = RawModeGuard::enter();
        let mut stdout = std::io::stdout();

        loop {
            let ev = event::read().expect("failed to read terminal event");
            match ev {
                Event::Key(KeyEvent { code: KeyCode::Char('q'), .. }) => break,
                Event::Key(KeyEvent { code: KeyCode::Esc, .. }) => break,
                Event::Key(KeyEvent { code: KeyCode::Char('c'), modifiers, .. })
                    if modifiers.contains(KeyModifiers::CONTROL) => break,

                Event::Key(KeyEvent { code: KeyCode::Right, .. })
                | Event::Key(KeyEvent { code: KeyCode::Char('l'), .. }) => {
                    if idx + 1 < self.matches.len() {
                        idx += 1;
                        self.render_step(idx);
                    }
                }

                Event::Key(KeyEvent { code: KeyCode::Left, .. })
                | Event::Key(KeyEvent { code: KeyCode::Char('h'), .. }) => {
                    if idx > 0 {
                        idx -= 1;
                        self.render_step(idx);
                    }
                }

                Event::Key(KeyEvent { code: KeyCode::Home, .. }) => {
                    idx = 0;
                    self.render_step(idx);
                }

                Event::Key(KeyEvent { code: KeyCode::End, .. }) => {
                    idx = self.matches.len() - 1;
                    self.render_step(idx);
                }

                _ => {}
            }
        }

        write!(stdout, "\r\n").unwrap();
        stdout.flush().unwrap();
    }

    fn sorted_entries(&self, idx: usize) -> &[(LocalId, id::N)] {
        self.matches[idx].as_slice()
    }

    fn mapping_str(&self, idx: usize) -> String {
        self.sorted_entries(idx).iter()
            .map(|(lid, gn)| format!("N({})={}", lid.0, gn.0))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn write_dot(&self, idx: usize) {
        self.write_dot_to(idx, "/tmp/grw_viz.dot");
    }

    fn write_dot_to(&self, idx: usize, path: &str) {
        let m = &self.matches[idx];
        let mut labels: FxHashMap<Id, String> = FxHashMap::default();
        let mut context_nodes: FxHashSet<Id> = FxHashSet::default();
        for (lid, gn) in self.sorted_entries(idx) {
            let node_idx = self.query.nodes.iter()
                .position(|n| n.local_id == *lid)
                .expect("pattern node must exist");
            if self.query.nodes[node_idx].kind == NodeKind::Translated {
                context_nodes.insert(gn.0);
                labels.insert(gn.0, format!(
                    "N({})<BR/>=<FONT COLOR=\"#8B008B\">X({})</FONT>",
                    gn.0, lid.0,
                ));
            } else {
                labels.insert(gn.0, format!(
                    "N({})<BR/>=<FONT COLOR=\"#228B22\">N({})</FONT>",
                    gn.0, lid.0,
                ));
            }
        }
        let mut dot = crate::viz::to_dot_traced(self.graph, m, &labels, &context_nodes);
        if let Ok(label) = std::env::var("SHAGRA_LABEL") {
            if let Some(pos) = dot.find('\n') {
                dot.insert_str(pos + 1, &format!(
                    "    label=\"{label}\";\n    labelloc=t;\n    labeljust=l;\n"
                ));
            }
        }
        std::fs::write(path, &dot)
            .unwrap_or_else(|e| panic!("failed to write {path}: {e}"));
    }

    fn print_edges(&self, idx: usize, nl: &str) {
        let m = &self.matches[idx];
        let mut stdout = std::io::stdout();
        let edges = reconstruct_edges(self.query, self.graph, m);
        for em in &edges {
            let sat = em.satisfied();
            let mark = if em.negated {
                if sat { "FAIL" } else { " ok " }
            } else {
                if sat { " ok " } else { "MISS" }
            };
            let neg = if em.negated { "!" } else { " " };
            let slot_str = format!("{:?}", em.slot);
            let detail = if !em.exists {
                "  absent"
            } else if !em.pred_pass {
                "  pred miss"
            } else {
                ""
            };
            write!(
                stdout,
                "  {mark} {neg}N({})-N({}) => {}-{} ({slot_str}){detail}{nl}",
                em.pattern_source_id.0,
                em.pattern_target_id.0,
                *em.graph_source,
                *em.graph_target,
            ).unwrap();
        }
        for (bi, ban) in self.query.ban_clusters.iter().enumerate() {
            let verdict = reconstruct_ban_verdict(self.query, self.graph, m, ban);
            let morphism = format!("{:?}", ban.morphism);
            match verdict {
                BanVerdict::SharedEdgeFail { src_lid, tgt_lid } => {
                    write!(stdout,
                        "  ban {}: pass — shared edge n({src_lid})-n({tgt_lid}) not satisfied{nl}",
                        bi + 1,
                    ).unwrap();
                }
                BanVerdict::NoCandidate { ban_only_lid } => {
                    write!(stdout,
                        "  ban {}: pass — no valid N({ban_only_lid}) found ({morphism}){nl}",
                        bi + 1,
                    ).unwrap();
                }
                BanVerdict::Pass => {
                    write!(stdout,
                        "  ban {}: FIRED{nl}",
                        bi + 1,
                    ).unwrap();
                }
            }
        }
        stdout.flush().unwrap();
    }

    fn print_match(&self, idx: usize) {
        println!("[{}/{}] {}", idx + 1, self.matches.len(), self.mapping_str(idx));
        if self.verbose {
            self.print_edges(idx, "\n");
        }
    }

    fn render_step(&self, idx: usize) {
        self.write_dot(idx);
        let mut stdout = std::io::stdout();
        write!(
            stdout, "\r\x1b[J[{}/{}] {}\r\n",
            idx + 1, self.matches.len(), self.mapping_str(idx),
        ).unwrap();
        if self.verbose {
            self.print_edges(idx, "\r\n");
        }
        stdout.flush().unwrap();
    }
}

#[macro_export]
macro_rules! trace {
    (verbose $graph:expr, $pattern:expr) => {{
        let __g = $crate::trace::ResolveGraph::resolve($graph)?;
        let __g_ref = $crate::trace::Ref::graph_ref(&__g);
        let __r = match $pattern? {
            $crate::search::Search::Resolved(r) => r,
            $crate::search::Search::Unresolved(_) =>
                panic!("trace!: pattern has context nodes, provide context as 3rd arg"),
        };
        let __query = __r.into_query();
        let matches: Vec<_> = $crate::search::Seq::search(&__query, __g_ref).collect();
        $crate::trace::Trace::new(__g_ref, &__query, matches, file!(), line!()).verbose(true).run()
    }};
    (verbose $graph:expr, $pattern:expr, $ctx:expr) => {{
        let __g = $crate::trace::ResolveGraph::resolve($graph)?;
        let __g_ref = $crate::trace::Ref::graph_ref(&__g);
        let __u = match $pattern? {
            $crate::search::Search::Unresolved(u) => u,
            $crate::search::Search::Resolved(_) =>
                panic!("trace!: pattern has no context nodes, remove context arg"),
        };
        let __bindings = __u.bind($ctx).map_err(|e| $crate::search::error::Search::Bind(e))?.bindings;
        let __query = __u.into_query();
        let matches: Vec<_> = $crate::search::Seq::search_bound(&__query, __g_ref, __bindings).collect();
        $crate::trace::Trace::new(__g_ref, &__query, matches, file!(), line!()).verbose(true).run()
    }};
    ($graph:expr, $pattern:expr) => {{
        let __g = $crate::trace::ResolveGraph::resolve($graph)?;
        let __g_ref = $crate::trace::Ref::graph_ref(&__g);
        let __r = match $pattern? {
            $crate::search::Search::Resolved(r) => r,
            $crate::search::Search::Unresolved(_) =>
                panic!("trace!: pattern has context nodes, provide context as 3rd arg"),
        };
        let __query = __r.into_query();
        let matches: Vec<_> = $crate::search::Seq::search(&__query, __g_ref).collect();
        $crate::trace::Trace::new(__g_ref, &__query, matches, file!(), line!()).run()
    }};
    ($graph:expr, $pattern:expr, $ctx:expr) => {{
        let __g = $crate::trace::ResolveGraph::resolve($graph)?;
        let __g_ref = $crate::trace::Ref::graph_ref(&__g);
        let __u = match $pattern? {
            $crate::search::Search::Unresolved(u) => u,
            $crate::search::Search::Resolved(_) =>
                panic!("trace!: pattern has no context nodes, remove context arg"),
        };
        let __bindings = __u.bind($ctx).map_err(|e| $crate::search::error::Search::Bind(e))?.bindings;
        let __query = __u.into_query();
        let matches: Vec<_> = $crate::search::Seq::search_bound(&__query, __g_ref, __bindings).collect();
        $crate::trace::Trace::new(__g_ref, &__query, matches, file!(), line!()).run()
    }};
}
