use proc_macro2::{Delimiter, Group, Ident, Span, TokenStream, TokenTree};
use quote::quote;

use crate::node_arg::{parse_node_arg, parse_node_pat, NodeArg, NodePat, NodeRef};

#[derive(Debug)]
pub struct Pin {
    pub lid: u64,
    pub expr: TokenStream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pinning {
    Pinned,
    Unpinned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextCtor {
    X,
    T,
}

impl ContextCtor {
    fn of(ident: &Ident) -> ContextCtor {
        match ident.to_string().as_str() {
            "X" | "x" => ContextCtor::X,
            "T" | "t" => ContextCtor::T,
            other => unreachable!("`{other}` is not a context ctor"),
        }
    }

    pub fn letter(self) -> &'static str {
        match self {
            ContextCtor::X => "X",
            ContextCtor::T => "T",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ContextNode {
    pub lid: u64,
    pub span: Span,
    pub ctor: ContextCtor,
    pub pin: Pinning,
}

#[derive(Debug)]
pub struct Rewritten {
    pub body: TokenStream,
    pub names: Vec<(String, u64)>,
    pub pins: Vec<Pin>,
    pub context: Vec<ContextNode>,
}

pub fn require_pinned(r: &Rewritten) -> syn::Result<()> {
    match r.context.iter().find(|c| c.pin == Pinning::Unpinned) {
        None => Ok(()),
        Some(c) => {
            let label = match r.names.iter().find(|(_, lid)| *lid == c.lid) {
                Some((name, _)) => name.clone(),
                None => c.lid.to_string(),
            };
            let letter = c.ctor.letter();
            Err(syn::Error::new(
                c.span,
                format!("context node `{label}` is not pinned to a graph node: write `{letter}({label} = <expr>)`"),
            ))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ctor {
    Def,
    Ref,
    CtxDef,
    CtxRef,
    Edge,
}

impl Ctor {
    pub fn from_ident(ident: &Ident) -> Option<Ctor> {
        match ident.to_string().as_str() {
            "N" => Some(Ctor::Def),
            "n" => Some(Ctor::Ref),
            "X" | "T" => Some(Ctor::CtxDef),
            "x" | "t" => Some(Ctor::CtxRef),
            "E" => Some(Ctor::Edge),
            _ => None,
        }
    }
}

fn preceded_by_access(prev: Option<&TokenTree>) -> bool {
    matches!(prev, Some(TokenTree::Punct(p)) if matches!(p.as_char(), '.' | ':' | '\'' | '#'))
}

pub fn is_cluster_head(prev: Option<&TokenTree>, tokens: &[TokenTree]) -> bool {
    let Some(TokenTree::Ident(i)) = tokens.first() else {
        return false;
    };
    if i != "get" && i != "ban" {
        return false;
    }
    if preceded_by_access(prev) {
        return false;
    }
    let (Some(TokenTree::Group(paren)), Some(TokenTree::Group(brace))) = (tokens.get(1), tokens.get(2)) else {
        return false;
    };
    paren.delimiter() == Delimiter::Parenthesis && brace.delimiter() == Delimiter::Brace
}

struct Occurrence {
    cluster: usize,
    ctor: Ctor,
    ident: Ident,
    group: Group,
    arg: NodeArg,
    span: Span,
}

enum Item {
    Verbatim(TokenTree),
    Group(Delimiter, Span, Vec<Item>),
    Call(usize),
}

struct Table {
    names: Vec<(String, u64)>,
}

impl Table {
    fn index(&self, name: &str) -> Option<u64> {
        self.names.iter().find(|(n, _)| n == name).map(|(_, i)| *i)
    }
}

enum AssignError {
    DuplicateInCluster { name: String, def_index: usize },
}

fn assign_names_impl(ints: &[u64], defs: &[(String, usize)]) -> Result<Vec<(String, u64)>, AssignError> {
    let mut next = match ints.iter().copied().max() {
        Some(m) => m + 1,
        None => 0,
    };
    let mut table: Vec<(String, u64)> = Vec::new();
    let mut seen_clusters: Vec<(String, Vec<usize>)> = Vec::new();
    for (def_index, (name, cluster)) in defs.iter().enumerate() {
        let entry = match seen_clusters.iter_mut().find(|(n, _)| n == name) {
            Some(e) => e,
            None => {
                seen_clusters.push((name.clone(), Vec::new()));
                seen_clusters.last_mut().expect("just pushed")
            }
        };
        if entry.1.contains(cluster) {
            return Err(AssignError::DuplicateInCluster { name: name.clone(), def_index });
        }
        entry.1.push(*cluster);
        if !table.iter().any(|(n, _)| n == name) {
            table.push((name.clone(), next));
            next += 1;
        }
    }
    Ok(table)
}

pub fn assign_names(ints: &[u64], defs: &[(String, usize)]) -> syn::Result<Vec<(String, u64)>> {
    assign_names_impl(ints, defs).map_err(|AssignError::DuplicateInCluster { name, .. }| {
        syn::Error::new(Span::call_site(), format!("node `{name}` defined twice in one cluster"))
    })
}

pub fn first_span(ts: &TokenStream) -> Option<Span> {
    ts.clone().into_iter().next().map(|t| t.span())
}

fn arg_span(ts: &TokenStream) -> Span {
    first_span(ts).expect("pin/pat already validated non-empty by parse_node_arg")
}

fn pat_span(pat: &NodePat) -> Span {
    match pat {
        NodePat::Match(ts) => arg_span(ts),
        NodePat::Key { index, .. } | NodePat::KeyIn { index, .. } => arg_span(index),
    }
}

pub fn validate_arg(ctor: Ctor, arg: &NodeArg, span: Span) -> syn::Result<()> {
    if arg.id.is_none() && !matches!(ctor, Ctor::Edge) {
        return Err(syn::Error::new(span, "expected an integer id or a name"));
    }
    if let Some(pat) = &arg.pat
        && matches!(pat, NodePat::Key { .. } | NodePat::KeyIn { .. })
        && !matches!(ctor, Ctor::Def | Ctor::CtxDef)
    {
        return Err(syn::Error::new(pat_span(pat), "key predicates apply to node definitions"));
    }
    match ctor {
        Ctor::Def => match &arg.pin {
            Some(pin) => Err(syn::Error::new(arg_span(pin), "only context nodes `X(..)` take `= <expr>`")),
            None => Ok(()),
        },
        Ctor::Ref | Ctor::CtxRef => {
            if let Some(pin) = &arg.pin {
                return Err(syn::Error::new(arg_span(pin), "references cannot take `= <expr>`"));
            }
            if let Some(pat) = &arg.pat {
                return Err(syn::Error::new(pat_span(pat), "references cannot take `: <pattern>`"));
            }
            Ok(())
        }
        Ctor::CtxDef | Ctor::Edge => Ok(()),
    }
}

fn edge_arg(stream: TokenStream) -> syn::Result<NodeArg> {
    if stream.is_empty() {
        return Ok(NodeArg { id: None, pin: None, pat: None });
    }
    let pat = parse_node_pat(stream)?;
    Ok(NodeArg { id: None, pin: None, pat: Some(pat) })
}

fn build_items(tokens: &[TokenTree], cluster: Option<usize>, next_cluster: &mut usize, occurrences: &mut Vec<Occurrence>) -> syn::Result<Vec<Item>> {
    let mut items = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let prev = if i == 0 { None } else { Some(&tokens[i - 1]) };
        if let TokenTree::Ident(id) = &tokens[i] {
            if is_cluster_head(prev, &tokens[i..])
                && let Some(TokenTree::Group(brace)) = tokens.get(i + 2)
            {
                items.push(Item::Verbatim(tokens[i].clone()));
                items.push(Item::Verbatim(tokens[i + 1].clone()));
                let this_cluster = *next_cluster;
                *next_cluster += 1;
                let inner: Vec<TokenTree> = brace.stream().into_iter().collect();
                let inner_items = build_items(&inner, Some(this_cluster), next_cluster, occurrences)?;
                items.push(Item::Group(Delimiter::Brace, brace.span(), inner_items));
                i += 3;
                continue;
            }
            if let Some(c) = Ctor::from_ident(id)
                && !preceded_by_access(prev)
                && let Some(TokenTree::Group(g)) = tokens.get(i + 1)
                && g.delimiter() == Delimiter::Parenthesis
            {
                let arg = if c == Ctor::Edge {
                    let a = edge_arg(g.stream())?;
                    validate_arg(c, &a, id.span())?;
                    a
                } else {
                    let a = parse_node_arg(g.stream())?;
                    validate_arg(c, &a, id.span())?;
                    a
                };
                let cl = match cluster {
                    Some(cl) => cl,
                    None => return Err(syn::Error::new(id.span(), "node outside of a get/ban cluster")),
                };
                let occurrence_index = occurrences.len();
                occurrences.push(Occurrence { cluster: cl, ctor: c, ident: id.clone(), group: g.clone(), arg, span: id.span() });
                items.push(Item::Call(occurrence_index));
                i += 2;
                continue;
            }
        }
        match &tokens[i] {
            TokenTree::Group(g) => {
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                let inner_items = build_items(&inner, cluster, next_cluster, occurrences)?;
                items.push(Item::Group(g.delimiter(), g.span(), inner_items));
            }
            other => items.push(Item::Verbatim(other.clone())),
        }
        i += 1;
    }
    Ok(items)
}

fn is_unchanged(ctor: Ctor, arg: &NodeArg) -> bool {
    match ctor {
        Ctor::Edge => arg.pat.is_none() && arg.pin.is_none(),
        Ctor::Def | Ctor::Ref | Ctor::CtxDef | Ctor::CtxRef => {
            matches!(arg.id, Some(NodeRef::Int(_))) && arg.pat.is_none() && arg.pin.is_none()
        }
    }
}

fn rewritten_call(ctor: Ctor, arg: &NodeArg, ident: &Ident, table: &Table, pins: &mut Vec<Pin>) -> syn::Result<TokenStream> {
    match ctor {
        Ctor::Edge => {
            let pat = arg.pat.as_ref().expect("edge unchanged case handled by is_unchanged");
            let pat = match pat {
                NodePat::Match(ts) => ts,
                NodePat::Key { .. } | NodePat::KeyIn { .. } => {
                    unreachable!("key predicates on edges are rejected by validate_arg")
                }
            };
            Ok(quote!(#ident().test(|__e| ::core::matches!(__e, #pat))))
        }
        Ctor::Def | Ctor::Ref | Ctor::CtxDef | Ctor::CtxRef => {
            let idx = match arg.id.as_ref().expect("node ctor without id rejected by validate_arg") {
                NodeRef::Int(i) => *i,
                NodeRef::Name(n) => table.index(n).expect("undefined names rejected before emit"),
            };
            if matches!(ctor, Ctor::CtxDef)
                && let Some(expr) = &arg.pin
            {
                pins.push(Pin { lid: idx, expr: expr.clone() });
            }
            let lit = proc_macro2::Literal::u64_unsuffixed(idx);
            let call = quote!(#ident(#lit));
            let out = match &arg.pat {
                Some(NodePat::Match(pat)) => quote!(#call.test(|__v| ::core::matches!(__v, #pat))),
                Some(NodePat::Key { index, key }) => quote!(#call.key(#index, #key)),
                Some(NodePat::KeyIn { index, keys }) => quote!(#call.key_in(#index, #keys)),
                None => call,
            };
            Ok(out)
        }
    }
}

fn emit_items(items: &[Item], occurrences: &[Occurrence], table: &Table, pins: &mut Vec<Pin>) -> syn::Result<TokenStream> {
    let mut out = TokenStream::new();
    for item in items {
        match item {
            Item::Verbatim(t) => out.extend(std::iter::once(t.clone())),
            Item::Group(delim, span, inner) => {
                let rewritten_inner = emit_items(inner, occurrences, table, pins)?;
                let mut g = Group::new(*delim, rewritten_inner);
                g.set_span(*span);
                out.extend(std::iter::once(TokenTree::Group(g)));
            }
            Item::Call(occurrence_index) => {
                let o = &occurrences[*occurrence_index];
                if is_unchanged(o.ctor, &o.arg) {
                    out.extend(std::iter::once(TokenTree::Ident(o.ident.clone())));
                    out.extend(std::iter::once(TokenTree::Group(o.group.clone())));
                    continue;
                }
                let piece = rewritten_call(o.ctor, &o.arg, &o.ident, table, pins)?;
                out.extend(piece);
            }
        }
    }
    Ok(out)
}

pub fn rewrite_clusters(input: TokenStream) -> syn::Result<Rewritten> {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    let mut next_cluster = 0usize;
    let mut occurrences = Vec::new();
    let items = build_items(&tokens, None, &mut next_cluster, &mut occurrences)?;

    let ints: Vec<u64> = occurrences
        .iter()
        .filter_map(|o| match &o.arg.id {
            Some(NodeRef::Int(i)) => Some(*i),
            _ => None,
        })
        .collect();
    let mut defs: Vec<(String, usize)> = Vec::new();
    let mut def_spans: Vec<Span> = Vec::new();
    for o in &occurrences {
        if !matches!(o.ctor, Ctor::Def | Ctor::CtxDef) {
            continue;
        }
        if let Some(NodeRef::Name(n)) = &o.arg.id {
            defs.push((n.clone(), o.cluster));
            def_spans.push(o.span);
        }
    }

    let names = match assign_names_impl(&ints, &defs) {
        Ok(names) => names,
        Err(AssignError::DuplicateInCluster { name, def_index }) => {
            return Err(syn::Error::new(def_spans[def_index], format!("node `{name}` defined twice in one cluster")));
        }
    };
    let table = Table { names };

    for o in &occurrences {
        if !matches!(o.ctor, Ctor::Ref | Ctor::CtxRef) {
            continue;
        }
        let Some(NodeRef::Name(n)) = &o.arg.id else {
            continue;
        };
        if table.index(n).is_none() {
            return Err(syn::Error::new(o.span, format!("node `{n}` is never defined")));
        }
    }

    let context = context_nodes(&occurrences, &table);

    let mut pins = Vec::new();
    let body = emit_items(&items, &occurrences, &table, &mut pins)?;
    let names = table.names.clone();
    Ok(Rewritten { body, names, pins, context })
}

fn occurrence_lid(o: &Occurrence, table: &Table) -> u64 {
    match o.arg.id.as_ref().expect("node ctor without id rejected by validate_arg") {
        NodeRef::Int(i) => *i,
        NodeRef::Name(n) => table.index(n).expect("undefined names rejected before context collection"),
    }
}

fn context_nodes(occurrences: &[Occurrence], table: &Table) -> Vec<ContextNode> {
    let mut context: Vec<ContextNode> = Vec::new();
    for o in occurrences {
        if !matches!(o.ctor, Ctor::CtxDef | Ctor::CtxRef) {
            continue;
        }
        let lid = occurrence_lid(o, table);
        let pin = match (o.ctor, &o.arg.pin) {
            (Ctor::CtxDef, Some(_)) => Pinning::Pinned,
            _ => Pinning::Unpinned,
        };
        match context.iter_mut().find(|c| c.lid == lid) {
            Some(c) => {
                if pin == Pinning::Pinned {
                    c.pin = Pinning::Pinned;
                }
            }
            None => context.push(ContextNode { lid, span: o.span, ctor: ContextCtor::of(&o.ident), pin }),
        }
    }
    context
}

#[cfg(test)]
mod tests {
    use super::assign_names;

    #[test]
    fn assign_names_no_ints_starts_at_zero() {
        let table = assign_names(&[], &[("a".to_string(), 0), ("b".to_string(), 0)]).unwrap();
        assert_eq!(table, vec![("a".to_string(), 0), ("b".to_string(), 1)]);
    }

    #[test]
    fn assign_names_starts_after_max_int() {
        let table = assign_names(&[3, 7], &[("i".to_string(), 0), ("j".to_string(), 0)]).unwrap();
        assert_eq!(table, vec![("i".to_string(), 8), ("j".to_string(), 9)]);
    }

    #[test]
    fn assign_names_duplicate_in_same_cluster_errors() {
        let err = assign_names(&[], &[("i".to_string(), 0), ("i".to_string(), 0)]).unwrap_err();
        assert!(err.to_string().contains("`i` defined twice"));
    }

    #[test]
    fn assign_names_same_name_two_clusters_is_one_entry() {
        let table = assign_names(&[], &[("i".to_string(), 0), ("i".to_string(), 1)]).unwrap();
        assert_eq!(table, vec![("i".to_string(), 0)]);
    }
}
