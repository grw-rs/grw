use crate::graph::{self, Graph, edge};
use crate::search::engine::Match;
use crate::Id;
use rustc_hash::{FxHashMap, FxHashSet};
use std::fmt::{Debug, Write};

pub trait DotEdge: graph::Edge {
    fn use_digraph() -> bool;
    fn edge_endpoints(def: Self::Def) -> EdgeInfo;
}

pub struct EdgeInfo {
    pub from: Id,
    pub to: Id,
    pub directed: bool,
}

impl<V: Sync> DotEdge for edge::Undir<V> {
    fn use_digraph() -> bool {
        false
    }

    fn edge_endpoints(def: Self::Def) -> EdgeInfo {
        let edge::undir::E::U(a, b) = def;
        EdgeInfo { from: a, to: b, directed: false }
    }
}

impl<V: Sync> DotEdge for edge::Dir<V> {
    fn use_digraph() -> bool {
        true
    }

    fn edge_endpoints(def: Self::Def) -> EdgeInfo {
        let edge::dir::E::D(a, b) = def;
        EdgeInfo { from: a, to: b, directed: true }
    }
}

impl<U: Sync, D: Sync> DotEdge for edge::Anydir<U, D> {
    fn use_digraph() -> bool {
        true
    }

    fn edge_endpoints(def: Self::Def) -> EdgeInfo {
        match def {
            edge::anydir::E::U(a, b) => EdgeInfo { from: a, to: b, directed: false },
            edge::anydir::E::D(a, b) => EdgeInfo { from: a, to: b, directed: true },
        }
    }
}

fn fmt_val(v: &impl Debug) -> Option<String> {
    let s = format!("{v:?}");
    if s == "()" { None } else { Some(s) }
}

pub fn to_dot<NV: Debug, ER: graph::Edge<Val: Debug> + DotEdge>(
    graph: &Graph<NV, ER>,
    matches: &[Match],
) -> String {
    let keyword = if ER::use_digraph() { "digraph" } else { "graph" };
    let edge_op = if ER::use_digraph() { "->" } else { "--" };

    let matched_nodes: FxHashSet<Id> = matches.iter()
        .flat_map(|m| m.values().map(|n| n.0))
        .collect();

    let matched_edges: FxHashSet<(Id, Id)> = matches.iter()
        .flat_map(|m| {
            let ids: Vec<Id> = m.values().map(|n| n.0).collect();
            let mut pairs = Vec::new();
            for i in 0..ids.len() {
                for j in (i + 1)..ids.len() {
                    let (a, b) = if ids[i] <= ids[j] {
                        (ids[i], ids[j])
                    } else {
                        (ids[j], ids[i])
                    };
                    pairs.push((a, b));
                }
            }
            pairs
        })
        .collect();

    let mut out = String::new();
    writeln!(out, "{keyword} {{").unwrap();
    writeln!(out, "    rankdir=LR;").unwrap();
    writeln!(out, "    node [shape=circle, style=filled, fillcolor=\"#e0e0e0\", fontname=monospace, fixedsize=true, width=0.6];").unwrap();
    writeln!(out, "    edge [fontname=monospace];").unwrap();

    let mut nodes: Vec<(Id, &NV)> = graph.nodes.iter().map(|(n, v)| (*n, v)).collect();
    nodes.sort_by_key(|(id, _)| *id);
    for (id, val) in &nodes {
        let fill = if matched_nodes.contains(id) {
            "#D3D3D3"
        } else {
            "#FFFFFF"
        };
        let label = match fmt_val(*val) {
            Some(v) => format!("N({id})\\n[{v}]"),
            None => format!("N({id})"),
        };
        writeln!(out, "    {id} [label=\"{label}\", fillcolor=\"{fill}\"];").unwrap();
    }

    for (def, val) in graph.edges.iter() {
        let info = ER::edge_endpoints(def);
        let (lo, hi) = if info.from <= info.to {
            (info.from, info.to)
        } else {
            (info.to, info.from)
        };
        let is_edge_matched = !matched_edges.is_empty()
            && matched_edges.contains(&(lo, hi));

        let mut attrs = Vec::new();
        if ER::use_digraph() && !info.directed {
            attrs.push("dir=none".to_string());
        }
        if is_edge_matched {
            attrs.push("color=\"#2171b5\"".to_string());
            attrs.push("penwidth=2.0".to_string());
        }
        if let Some(v) = fmt_val(val) {
            attrs.push(format!("label=\"[{v}]\""));
        }

        if attrs.is_empty() {
            writeln!(out, "    {} {} {};", info.from, edge_op, info.to).unwrap();
        } else {
            writeln!(out, "    {} {} {} [{}];", info.from, edge_op, info.to, attrs.join(", ")).unwrap();
        }
    }

    writeln!(out, "}}").unwrap();
    out
}

pub fn to_dot_traced<NV: Debug, ER: graph::Edge<Val: Debug> + DotEdge>(
    graph: &Graph<NV, ER>,
    m: &Match,
    labels: &FxHashMap<Id, String>,
    context_nodes: &FxHashSet<Id>,
) -> String {
    let keyword = if ER::use_digraph() { "digraph" } else { "graph" };
    let edge_op = if ER::use_digraph() { "->" } else { "--" };

    let matched_nodes: FxHashSet<Id> = m.values().map(|n| n.0).collect();

    let matched_edges: FxHashSet<(Id, Id)> = {
        let ids: Vec<Id> = m.values().map(|n| n.0).collect();
        let mut pairs = FxHashSet::default();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let (a, b) = if ids[i] <= ids[j] {
                    (ids[i], ids[j])
                } else {
                    (ids[j], ids[i])
                };
                pairs.insert((a, b));
            }
        }
        pairs
    };

    let mut out = String::new();
    writeln!(out, "{keyword} {{").unwrap();
    writeln!(out, "    rankdir=LR;").unwrap();
    writeln!(out, "    node [shape=circle, style=filled, fillcolor=\"#e0e0e0\", fontname=monospace, fixedsize=true, width=0.6];").unwrap();
    writeln!(out, "    edge [fontname=monospace];").unwrap();

    let mut nodes: Vec<(Id, &NV)> = graph.nodes.iter().map(|(n, v)| (*n, v)).collect();
    nodes.sort_by_key(|(id, _)| *id);
    for (id, val) in &nodes {
        let fill = if matched_nodes.contains(id) {
            "#D3D3D3"
        } else {
            "#FFFFFF"
        };
        let base = match labels.get(id) {
            Some(overlay) => overlay.clone(),
            None => format!("N({id})"),
        };
        let label = match fmt_val(*val) {
            Some(v) => format!("{base}<BR/>[{v}]"),
            None => base,
        };
        let border = if matched_nodes.contains(id) {
            ", color=\"#2171b5\", penwidth=2"
        } else {
            ""
        };
        if context_nodes.contains(id) {
            writeln!(out, "    {id} [label=<{label}>, fillcolor=\"{fill}\"{border}, shape=box];").unwrap();
        } else {
            writeln!(out, "    {id} [label=<{label}>, fillcolor=\"{fill}\"{border}];").unwrap();
        }
    }

    for (def, val) in graph.edges.iter() {
        let info = ER::edge_endpoints(def);
        let (lo, hi) = if info.from <= info.to {
            (info.from, info.to)
        } else {
            (info.to, info.from)
        };
        let is_edge_matched = matched_edges.contains(&(lo, hi));

        let mut attrs = Vec::new();
        if ER::use_digraph() && !info.directed {
            attrs.push("dir=none".to_string());
        }
        if is_edge_matched {
            attrs.push("color=\"#2171b5\"".to_string());
            attrs.push("penwidth=2.0".to_string());
        }
        if let Some(v) = fmt_val(val) {
            attrs.push(format!("label=\"[{v}]\""));
        }

        if attrs.is_empty() {
            writeln!(out, "    {} {} {};", info.from, edge_op, info.to).unwrap();
        } else {
            writeln!(out, "    {} {} {} [{}];", info.from, edge_op, info.to, attrs.join(", ")).unwrap();
        }
    }

    writeln!(out, "}}").unwrap();
    out
}

pub fn render<NV: Debug, ER: graph::Edge<Val: Debug> + DotEdge>(
    graph: &Graph<NV, ER>,
    matches: &[Match],
) {
    let dot = to_dot(graph, matches);
    let path = "/tmp/grw_viz.dot";
    std::fs::write(path, &dot).expect("failed to write /tmp/grw_viz.dot");
    println!("viz: {path}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undir_basic_dot() {
        let g: graph::Undir0 = Graph::try_from(
            vec![edge::undir::E::U(0, 1), edge::undir::E::U(1, 2)]
        ).unwrap();
        let dot = to_dot(&g, &[]);
        assert!(dot.starts_with("graph {"));
        assert!(dot.contains(" -- "));
        assert!(dot.contains("label=\"N(0)\""));
        assert!(dot.contains("label=\"N(1)\""));
        assert!(dot.contains("label=\"N(2)\""));
    }

    #[test]
    fn dir_basic_dot() {
        let g: graph::Dir0 = Graph::try_from(
            vec![edge::dir::E::D(0, 1), edge::dir::E::D(1, 2)]
        ).unwrap();
        let dot = to_dot(&g, &[]);
        assert!(dot.starts_with("digraph {"));
        assert!(dot.contains("->"));
    }

    #[test]
    fn anydir_mixed_dot() {
        let g: graph::Anydir0 = Graph::try_from(
            vec![edge::anydir::E::D(0, 1), edge::anydir::E::U(1, 2)]
        ).unwrap();
        let dot = to_dot(&g, &[]);
        assert!(dot.starts_with("digraph {"));
        assert!(dot.contains("dir=none"));
    }

    #[test]
    fn undir_with_match_highlights_nodes() {
        type ER = edge::Undir<()>;
        let g: graph::Undir0 = Graph::try_from(
            vec![edge::undir::E::U(0, 1), edge::undir::E::U(1, 2), edge::undir::E::U(2, 3)]
        ).unwrap();

        use crate::search::{Search, Seq, RevCsr, dsl};
        let Search::Resolved(r) = crate::search![<(), ER>;
            get(Morphism::Mono) {
                dsl::N(0) ^ dsl::N(1),
            }
        ].expect("valid pattern")
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let sg = g.index(RevCsr);
        let matches: Vec<Match> = Seq::search(&query, &sg).collect();
        let dot = to_dot(sg.graph, &matches);
        assert!(dot.contains("#D3D3D3"));
    }
}
