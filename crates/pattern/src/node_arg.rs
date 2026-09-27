use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use syn::parse::Parser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeRef {
    Int(u64),
    Name(String),
}

#[derive(Debug, Clone)]
pub enum NodePat {
    Match(TokenStream),
    Key { index: TokenStream, key: TokenStream },
    KeyIn { index: TokenStream, keys: TokenStream },
}

#[derive(Debug, Clone)]
pub struct NodeArg {
    pub id: Option<NodeRef>,
    pub pin: Option<TokenStream>,
    pub pat: Option<NodePat>,
}

pub fn parse_node_arg(inner: TokenStream) -> syn::Result<NodeArg> {
    let tokens: Vec<TokenTree> = inner.into_iter().collect();
    if tokens.is_empty() {
        return Ok(NodeArg { id: None, pin: None, pat: None });
    }
    let (head, rest) = split_head(&tokens)?;
    let (pin, pat_tokens) = split_pin_pat(rest)?;
    if let Some(e) = &pin {
        syn::parse2::<syn::Expr>(e.clone())?;
    }
    let pat = match pat_tokens {
        Some(ts) => Some(parse_node_pat(ts)?),
        None => None,
    };
    Ok(NodeArg { id: Some(head), pin, pat })
}

pub(crate) fn parse_node_pat(ts: TokenStream) -> syn::Result<NodePat> {
    if let Some(pat) = try_key_form(&ts, "key")? {
        return Ok(pat);
    }
    if let Some(pat) = try_key_form(&ts, "key_in")? {
        return Ok(pat);
    }
    syn::Pat::parse_multi_with_leading_vert.parse2(ts.clone())?;
    Ok(NodePat::Match(ts))
}

fn try_key_form(ts: &TokenStream, name: &str) -> syn::Result<Option<NodePat>> {
    let tokens: Vec<TokenTree> = ts.clone().into_iter().collect();
    let (Some(TokenTree::Ident(id)), Some(TokenTree::Group(g))) = (tokens.first(), tokens.get(1)) else {
        return Ok(None);
    };
    if id != name || tokens.len() != 2 || g.delimiter() != Delimiter::Parenthesis {
        return Ok(None);
    }
    let inner: Vec<TokenTree> = g.stream().into_iter().collect();
    let comma = inner
        .iter()
        .position(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == ','))
        .ok_or_else(|| syn::Error::new(g.span(), format!("expected `{name}(<index>, <key>)`")))?;
    let index: TokenStream = inner[..comma].iter().cloned().collect();
    let rest: TokenStream = inner[comma + 1..].iter().cloned().collect();
    if index.is_empty() || rest.is_empty() {
        return Err(syn::Error::new(g.span(), format!("expected `{name}(<index>, <key>)`")));
    }
    syn::parse2::<syn::Expr>(index.clone())?;
    syn::parse2::<syn::Expr>(rest.clone())?;
    Ok(Some(if name == "key" {
        NodePat::Key { index, key: rest }
    } else {
        NodePat::KeyIn { index, keys: rest }
    }))
}

fn split_head(tokens: &[TokenTree]) -> syn::Result<(NodeRef, &[TokenTree])> {
    match &tokens[0] {
        TokenTree::Literal(l) => {
            let ts = TokenStream::from(TokenTree::Literal(l.clone()));
            let v: syn::LitInt = syn::parse2(ts)?;
            if !v.suffix().is_empty() {
                return Err(syn::Error::new(l.span(), "node ids are unsuffixed integers"));
            }
            Ok((NodeRef::Int(v.base10_parse::<u64>()?), &tokens[1..]))
        }
        TokenTree::Ident(i) => Ok((NodeRef::Name(i.to_string()), &tokens[1..])),
        other => Err(syn::Error::new(other.span(), "expected an integer id or a name")),
    }
}

fn split_pin_pat(rest: &[TokenTree]) -> syn::Result<(Option<TokenStream>, Option<TokenStream>)> {
    if rest.is_empty() {
        return Ok((None, None));
    }
    match &rest[0] {
        TokenTree::Punct(p) if p.as_char() == '=' => {
            let after = &rest[1..];
            match top_level_colon(after) {
                None => Ok((Some(after.iter().cloned().collect()), None)),
                Some(c) => Ok((
                    Some(after[..c].iter().cloned().collect()),
                    Some(after[c + 1..].iter().cloned().collect()),
                )),
            }
        }
        TokenTree::Punct(p) if p.as_char() == ':' && p.spacing() == Spacing::Alone => {
            Ok((None, Some(rest[1..].iter().cloned().collect())))
        }
        other => Err(syn::Error::new(other.span(), "expected `= <expr>` or `: <pattern>` after the id")),
    }
}

fn top_level_colon(tokens: &[TokenTree]) -> Option<usize> {
    for (i, t) in tokens.iter().enumerate() {
        let TokenTree::Punct(p) = t else { continue };
        if p.as_char() != ':' || p.spacing() != Spacing::Alone {
            continue;
        }
        let prev_is_colon = i > 0
            && match &tokens[i - 1] {
                TokenTree::Punct(pp) => pp.as_char() == ':',
                _ => false,
            };
        if !prev_is_colon {
            return Some(i);
        }
    }
    None
}
