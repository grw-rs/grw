use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};

use crate::node_arg::{parse_node_arg, NodeRef};

#[derive(Debug)]
pub struct WithClause {
    pub pins: Vec<(String, TokenStream)>,
}

fn is_with_keyword(tokens: &[TokenTree], i: usize) -> bool {
    if !matches!(&tokens[i], TokenTree::Ident(id) if id == "with") {
        return false;
    }
    match i.checked_sub(1).map(|p| &tokens[p]) {
        Some(TokenTree::Punct(p)) => p.as_char() != '.' && p.as_char() != ':',
        Some(_) => true,
        None => true,
    }
}

pub fn split_with(input: TokenStream) -> (TokenStream, Option<(Span, TokenStream)>) {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    match (0..tokens.len()).find(|&i| is_with_keyword(&tokens, i)) {
        None => (tokens.into_iter().collect(), None),
        Some(pos) => (
            tokens[..pos].iter().cloned().collect(),
            Some((tokens[pos].span(), tokens[pos + 1..].iter().cloned().collect())),
        ),
    }
}

pub fn parse_with_clause(input: TokenStream, with_span: Span) -> syn::Result<WithClause> {
    if input.is_empty() {
        return Err(syn::Error::new(with_span, "expected at least one `X(<name> = <expr>)` after `with`"));
    }
    let mut pins: Vec<(String, TokenStream)> = Vec::new();
    for item in split_top_level_commas(input) {
        let tokens: Vec<TokenTree> = item.into_iter().collect();
        let (Some(TokenTree::Ident(h)), Some(TokenTree::Group(g))) = (tokens.first(), tokens.get(1)) else {
            let span = match tokens.first() {
                Some(t) => t.span(),
                None => Span::call_site(),
            };
            return Err(syn::Error::new(span, "expected `X(<name> = <expr>)`"));
        };
        if h != "X" || tokens.len() != 2 || g.delimiter() != Delimiter::Parenthesis {
            return Err(syn::Error::new(h.span(), "expected `X(<name> = <expr>)`"));
        }
        let arg = parse_node_arg(g.stream())?;
        let (Some(NodeRef::Name(name)), Some(expr), None) = (arg.id, arg.pin, arg.pat) else {
            return Err(syn::Error::new(g.span(), "expected `X(<name> = <expr>)` without a value pattern"));
        };
        if pins.iter().any(|(n, _)| *n == name) {
            return Err(syn::Error::new(g.span(), format!("node `{name}` pinned twice")));
        }
        pins.push((name, expr));
    }
    Ok(WithClause { pins })
}

fn split_top_level_commas(input: TokenStream) -> Vec<TokenStream> {
    let mut out = Vec::new();
    let mut cur = TokenStream::new();
    for t in input {
        match t {
            TokenTree::Punct(p) if p.as_char() == ',' => {
                out.push(std::mem::take(&mut cur));
            }
            other => cur.extend(std::iter::once(other)),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
