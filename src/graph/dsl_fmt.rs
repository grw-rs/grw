use std::collections::BTreeSet;
use std::fmt::Debug;

use super::{MGraph, Edge};
use crate::Id;
use crate::viz::DotEdge;

impl<NV: Debug, E: Edge<Val: Debug> + DotEdge> MGraph<NV, E> {
    pub fn to_dsl(&self) -> String {
        let mut out = String::new();
        let mut defined = BTreeSet::new();
        let mut stmts = Vec::new();

        for (def, ev) in self.edge_iter() {
            let info = E::edge_endpoints(def);
            let src = self.emit_node(info.from, &mut defined);
            let tgt = self.emit_node(info.to, &mut defined);
            let ev_str = fmt_val(ev);
            let op = if info.directed { " >> " } else { " ^ " };

            let mut stmt = src;
            if let Some(ref ev_s) = ev_str {
                stmt.push_str(&format!(" & E().val({ev_s})"));
            }
            stmt.push_str(op);
            stmt.push_str(&tgt);
            stmts.push(stmt);
        }

        for (nid, _nv) in self.node_iter() {
            if !defined.contains(&*nid) {
                stmts.push(self.emit_node(*nid, &mut defined));
            }
        }

        out.push_str("mgraph![\n");
        for (i, stmt) in stmts.iter().enumerate() {
            out.push_str("    ");
            out.push_str(stmt);
            if i + 1 < stmts.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push(']');
        out
    }

    fn emit_node(&self, id: Id, defined: &mut BTreeSet<Id>) -> String {
        if defined.contains(&id) {
            format!("n({})", id)
        } else {
            defined.insert(id);
            let nv = self.nodes.get(crate::id::N(id));
            match nv.and_then(fmt_val) {
                Some(v) => format!("N({}).val({})", id, v),
                None => format!("N({})", id),
            }
        }
    }
}

fn fmt_val<T: Debug>(v: &T) -> Option<String> {
    let s = format!("{v:?}");
    if s == "()" { None } else { Some(s) }
}
