use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use super::ir::{Decision, Dir, Edge, Node, NodeKind, Pattern, Stmt};

struct IdAssigner {
    named_ids: HashMap<String, String>,
    anon_counter: usize,
}

impl IdAssigner {
    fn new() -> Self {
        Self {
            named_ids: HashMap::default(),
            anon_counter: 0,
        }
    }

    fn assign_named(&mut self, node: &Node) -> String {
        let key = match (node.kind, node.id) {
            (NodeKind::Free | NodeKind::FreeRef, Some(id)) => format!("N{id}"),
            (NodeKind::Context | NodeKind::ContextRef, Some(id)) => format!("X{id}"),
            _ => unreachable!(),
        };
        if let Some(existing) = self.named_ids.get(&key) {
            return existing.clone();
        }
        let dot_id = format!("n_{key}");
        self.named_ids.insert(key, dot_id.clone());
        dot_id
    }

    fn assign_anon(&mut self) -> String {
        let id = self.anon_counter;
        self.anon_counter += 1;
        format!("n_anon_{id}")
    }
}

struct AssignedNode<'a> {
    dot_id: String,
    node: &'a Node,
}

struct AssignedEdge<'a> {
    source_dot_id: String,
    target_dot_id: String,
    edge: &'a Edge,
    target_nodes: Vec<AssignedNode<'a>>,
    target_edges: Vec<AssignedEdge<'a>>,
}

struct AssignedStmt<'a> {
    root: AssignedNode<'a>,
    edges: Vec<AssignedEdge<'a>>,
}

fn assign_stmt<'a>(assigner: &mut IdAssigner, stmt: &'a Stmt) -> AssignedStmt<'a> {
    let root_id = match (stmt.node.kind, stmt.node.id) {
        (NodeKind::Free, None) => assigner.assign_anon(),
        _ => assigner.assign_named(&stmt.node),
    };
    let root = AssignedNode {
        dot_id: root_id.clone(),
        node: &stmt.node,
    };
    let edges = assign_edges(assigner, &root_id, &stmt.edges);
    AssignedStmt { root, edges }
}

fn assign_edges<'a>(
    assigner: &mut IdAssigner,
    source_id: &str,
    edges: &'a [Edge],
) -> Vec<AssignedEdge<'a>> {
    let mut result = Vec::new();
    for edge in edges {
        let target_id = match (edge.target.node.kind, edge.target.node.id) {
            (NodeKind::Free, None) => assigner.assign_anon(),
            _ => assigner.assign_named(&edge.target.node),
        };
        let target_nodes = vec![AssignedNode {
            dot_id: target_id.clone(),
            node: &edge.target.node,
        }];
        let target_edges = assign_edges(assigner, &target_id, &edge.target.edges);
        result.push(AssignedEdge {
            source_dot_id: source_id.to_string(),
            target_dot_id: target_id,
            edge,
            target_nodes,
            target_edges,
        });
    }
    result
}

fn has_directed_edges(pattern: &Pattern) -> bool {
    fn scan_stmt(stmt: &Stmt) -> bool {
        stmt.edges.iter().any(|e| {
            matches!(e.dir, Dir::Forward | Dir::Backward) || scan_stmt(&e.target)
        })
    }
    pattern
        .clusters
        .iter()
        .any(|c| c.stmts.iter().any(|s| scan_stmt(s)))
}

fn node_label(node: &Node) -> String {
    let base = match (node.kind, node.id) {
        (NodeKind::Free | NodeKind::FreeRef, Some(id)) => format!("N({id})"),
        (NodeKind::Free, None) => "?".to_string(),
        (NodeKind::Context | NodeKind::ContextRef, Some(id)) => format!("X({id})"),
        _ => unreachable!(),
    };
    let mut label = base;
    if node.has_val {
        label.push_str("\\nval");
    }
    if node.has_pred {
        label.push_str("\\npred");
    }
    label
}

fn node_shape(node: &Node) -> &'static str {
    match node.kind {
        NodeKind::Context | NodeKind::ContextRef => "box",
        NodeKind::Free | NodeKind::FreeRef => "circle",
    }
}

fn node_style(node: &Node) -> &'static str {
    if node.negated {
        "dashed,filled"
    } else {
        "filled"
    }
}

fn node_fill(decision: Decision) -> &'static str {
    match decision {
        Decision::Get => "#2d5a3d",
        Decision::Ban => "#5a2d2d",
    }
}

fn edge_label(edge: &Edge) -> String {
    let mut parts = Vec::new();
    if edge.negated {
        parts.push("!".to_string());
    }
    if edge.has_edge_val {
        parts.push("val".to_string());
    }
    if edge.has_edge_pred {
        parts.push("pred".to_string());
    }
    parts.join("\\n")
}

fn edge_style(edge: &Edge) -> &'static str {
    if edge.negated {
        "dashed"
    } else {
        "solid"
    }
}

fn edge_color(edge: &Edge) -> &'static str {
    if edge.negated {
        "#ff6666"
    } else {
        "#6cb4ee"
    }
}

fn emit_assigned_node(
    out: &mut String,
    assigned: &AssignedNode<'_>,
    decision: Decision,
) {
    let dot_id = &assigned.dot_id;
    let label = node_label(assigned.node);
    let shape = node_shape(assigned.node);
    let style = node_style(assigned.node);
    let fill = node_fill(decision);
    let _ = writeln!(
        out,
        "    {dot_id} [label=\"{label}\", shape={shape}, style=\"{style}\", fillcolor=\"{fill}\"];",
    );
}

fn emit_assigned_edges(
    out: &mut String,
    edges: &[AssignedEdge<'_>],
    directed: bool,
    decision: Decision,
    seen: &mut HashSet<String>,
) {
    for ae in edges {
        for tn in &ae.target_nodes {
            if seen.insert(tn.dot_id.clone()) {
                emit_assigned_node(out, tn, decision);
            }
        }

        let dir_attr = if directed {
            match ae.edge.dir {
                Dir::Forward => " dir=forward",
                Dir::Backward => " dir=back",
                Dir::Undirected => " dir=none",
                Dir::Any => " dir=both",
            }
        } else {
            ""
        };

        let label = edge_label(ae.edge);
        let style = edge_style(ae.edge);
        let color = edge_color(ae.edge);
        let src = &ae.source_dot_id;
        let tgt = &ae.target_dot_id;
        let connector = if directed { "->" } else { "--" };
        let label_attr = if label.is_empty() {
            String::new()
        } else {
            format!(" label=\"{label}\", fontcolor=\"{color}\",")
        };
        let _ = writeln!(
            out,
            "    {src} {connector} {tgt} [{label_attr} style={style}, color=\"{color}\"{dir_attr}];",
        );

        emit_assigned_edges(out, &ae.target_edges, directed, decision, seen);
    }
}

pub fn to_dot(pattern: &Pattern) -> String {
    let directed = has_directed_edges(pattern);

    let mut assigner = IdAssigner::new();
    let assigned_clusters: Vec<Vec<AssignedStmt<'_>>> = pattern
        .clusters
        .iter()
        .map(|cluster| {
            cluster
                .stmts
                .iter()
                .map(|stmt| assign_stmt(&mut assigner, stmt))
                .collect()
        })
        .collect();

    let mut out = String::new();
    let graph_kw = if directed { "digraph" } else { "graph" };
    let _ = writeln!(out, "{graph_kw} grw_pattern {{");
    let _ = writeln!(out, "  bgcolor=\"#1a1a1a\";");
    let _ = writeln!(out, "  rankdir=TB;");
    let _ = writeln!(out, "  node [fontname=\"monospace\", fontsize=10, fontcolor=white];");
    let _ = writeln!(out, "  edge [fontname=\"monospace\", fontsize=9, color=white];\n");

    for (i, (cluster, assigned_stmts)) in pattern
        .clusters
        .iter()
        .zip(assigned_clusters.iter())
        .enumerate()
    {
        let label = format!("{}({})", cluster.decision, cluster.morphism);
        let bg = match cluster.decision {
            Decision::Get => "#1c2d3a",
            Decision::Ban => "#3a1c1c",
        };
        let border_style = match cluster.decision {
            Decision::Get => "solid",
            Decision::Ban => "dashed",
        };

        let _ = writeln!(out, "  subgraph cluster_{i} {{");
        let _ = writeln!(out, "    label=\"{label}\";");
        let _ = writeln!(out, "    style=\"filled,{border_style}\";");
        let _ = writeln!(out, "    fillcolor=\"{bg}\";");
        let _ = writeln!(out, "    fontname=\"monospace\";");
        let _ = writeln!(out, "    fontsize=11;");
        let _ = writeln!(out, "    fontcolor=white;");
        let _ = writeln!(out, "    color=white;");

        let mut seen = HashSet::default();
        for astmt in assigned_stmts {
            if seen.insert(astmt.root.dot_id.clone()) {
                emit_assigned_node(&mut out, &astmt.root, cluster.decision);
            }
            emit_assigned_edges(
                &mut out,
                &astmt.edges,
                directed,
                cluster.decision,
                &mut seen,
            );
        }

        let _ = writeln!(out, "  }}");
    }

    let _ = writeln!(out, "}}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::parse;

    fn render(input: &str) -> String {
        let pattern = parse::parse(input).unwrap();
        to_dot(&pattern)
    }

    #[test]
    fn basic_dot_output() {
        let dot = render("get(Morphism::Mono) { N(0) ^ N(1) }");
        assert!(dot.contains("graph grw_pattern"));
        assert!(!dot.contains("digraph"));
        assert!(dot.contains("cluster_0"));
        assert!(dot.contains("n_N0"));
        assert!(dot.contains("n_N1"));
        assert!(dot.contains("get(Mono)"));
        assert!(dot.contains("n_N0 -- n_N1"));
    }

    #[test]
    fn ban_cluster_styling() {
        let dot = render("ban(Morphism::Iso) { N(0) ^ N(1) }");
        assert!(dot.contains("ban(Iso)"));
        assert!(dot.contains("#3a1c1c"));
        assert!(dot.contains("#5a2d2d"));
        assert!(dot.contains("dashed"));
    }

    #[test]
    fn directed_edges_have_dir_attr() {
        let dot = render("get(Morphism::Mono) { N(0) >> N(1) }");
        assert!(dot.contains("digraph"));
        assert!(dot.contains("dir=forward"));
        assert!(dot.contains("->"));
    }

    #[test]
    fn negated_edge_red_dashed() {
        let dot = render("get(Morphism::Mono) { N(0) & !E() ^ N(1) }");
        assert!(dot.contains("#ff6666"));
        assert!(dot.contains("style=dashed"));
    }

    #[test]
    fn context_node_box_shape() {
        let dot = render("get(Morphism::Mono) { X(0) ^ N(1) }");
        assert!(dot.contains("shape=box"));
    }

    #[test]
    fn anonymous_node_question_mark() {
        let dot = render("get(Morphism::Mono) { N(0) ^ !N_() }");
        assert!(dot.contains("label=\"?\"") || dot.contains("label=\"?\\n"));
    }

    #[test]
    fn anonymous_node_consistent_id() {
        let dot = render("get(Morphism::Mono) { N(0) ^ !N_() }");
        let anon_decl = dot.contains("n_anon_0 [label=");
        let anon_edge = dot.contains("-- n_anon_0 [");
        assert!(anon_decl, "anon node must be declared");
        assert!(anon_edge, "edge must reference same anon id");
    }

    #[test]
    fn undirected_edge_no_edge_label() {
        let dot = render("get(Morphism::Mono) { N(0) ^ N(1) }");
        for line in dot.lines() {
            if line.contains("--") {
                assert!(!line.contains("label="), "edge line should have no label: {line}");
            }
        }
    }

    #[test]
    fn edge_val_in_label() {
        let dot = render("get(Morphism::Mono) { N(0) & E().val(5) ^ N(1) }");
        assert!(dot.contains("label=\"val\""));
    }

    #[test]
    fn node_val_in_label() {
        let dot = render("get(Morphism::Mono) { N(0).val(42) ^ N(1) }");
        assert!(dot.contains("val"));
    }

    #[test]
    fn two_clusters() {
        let dot = render(
            "get(Morphism::Mono) { N(0) ^ N(1) }, ban(Morphism::Mono) { n(0) ^ N(2) }",
        );
        assert!(dot.contains("cluster_0"));
        assert!(dot.contains("cluster_1"));
    }

    #[test]
    fn ref_node_reuses_dot_id() {
        let dot = render("get(Morphism::Iso) { N(0) ^ N(1), n(0) ^ n(1) }");
        let n0_count = dot.matches("n_N0").count();
        assert!(n0_count >= 2);
    }
}
