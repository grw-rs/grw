use proc_macro2::{Span, TokenStream, TokenTree};
use quote::quote;
use syn::parse::{ParseStream, Parser};

use grw_pattern::{first_span, is_cluster_head, parse_with_clause, require_pinned, rewrite_clusters, split_with, Rewritten};

use crate::pattern::{split_type_header, TypeHeader};

enum Head {
    Types { nv: TokenStream, er: TokenStream },
    InlineGraph(TokenStream),
    Graph(TokenStream),
    None,
}

pub fn expand(krate: &TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let (head, rest) = split_head(input)?;
    match head {
        Head::Graph(g) if !starts_with_cluster(&rest) => stored_pattern(krate, &g, rest),
        Head::Graph(g) => {
            let r = rewrite_clusters(rest)?;
            require_pinned(&r)?;
            let clusters = clusters_block(krate, quote!(_, _), &r.body);
            let pins = r.pins.iter().map(|p| {
                let lit = proc_macro2::Literal::u64_unsuffixed(p.lid);
                let e = &p.expr;
                quote!((#krate::search::dsl::LocalId(#lit), #e))
            });
            let session = match r.pins.is_empty() {
                true => quote!(#krate::search::Session::from_search(__compiled, #g)),
                false => quote!(#krate::search::Session::from_search_pinned(__compiled, #g, ::std::vec![#(#pins),*])),
            };
            Ok(quote! {{
                #clusters
                match #krate::search::query::compile(__clusters) {
                    ::core::result::Result::Err(e) => ::core::result::Result::Err(e),
                    ::core::result::Result::Ok(__compiled) => #session,
                }
            }})
        }
        Head::Types { nv, er } => {
            let r = require_clusters(rest)?;
            no_pins(&r)?;
            let clusters = clusters_block(krate, quote!(#nv, #er), &r.body);
            Ok(quote! {{
                #clusters
                #krate::search::query::compile(__clusters)
            }})
        }
        Head::None => {
            let r = require_clusters(rest)?;
            no_pins(&r)?;
            let clusters = clusters_block(krate, quote!(_, _), &r.body);
            Ok(quote! {{
                #clusters
                #krate::search::query::compile(__clusters)
            }})
        }
        Head::InlineGraph(g) => {
            let r = require_clusters(rest)?;
            no_pins(&r)?;
            let clusters = clusters_block(krate, quote!(_, _), &r.body);
            Ok(quote! {{
                match #krate::mgraph![#g] {
                    ::core::result::Result::Err(e) => ::core::result::Result::Err(#krate::search::error::Search::GraphBuild(::std::boxed::Box::new(e))),
                    ::core::result::Result::Ok(__g) => {
                        #clusters
                        match #krate::search::query::compile(__clusters) {
                            ::core::result::Result::Err(e) => ::core::result::Result::Err(e),
                            ::core::result::Result::Ok(__compiled) => #krate::search::engine::seq::OwnedIter::from_graph_and_search(__g, __compiled),
                        }
                    }
                }
            }})
        }
    }
}

fn stored_pattern(krate: &TokenStream, g: &TokenStream, rest: TokenStream) -> syn::Result<TokenStream> {
    let (pat_tokens, with_tokens) = split_with(rest);
    let pat: syn::Expr = syn::parse2(pat_tokens)?;
    let pins = match with_tokens {
        None => Vec::new(),
        Some((span, w)) => parse_with_clause(w, span)?.pins,
    };
    let entries = pins.iter().map(|(name, expr)| {
        let lit = proc_macro2::Literal::string(name);
        quote!((#lit, #expr))
    });
    Ok(quote!(#krate::search::Session::from_pattern(&(#pat), #g, &[#(#entries),*])))
}

fn clusters_block(krate: &TokenStream, ty: TokenStream, body: &TokenStream) -> TokenStream {
    quote! {
        let __clusters: ::std::vec::Vec<#krate::search::dsl::ClusterOps<#ty>> = {
            #[allow(unused_imports)]
            use #krate::search::dsl::*;
            #krate::__search_clusters!(@acc [] #body)
        };
    }
}

fn require_clusters(rest: TokenStream) -> syn::Result<Rewritten> {
    if !starts_with_cluster(&rest) {
        return Err(syn::Error::new(span_of(&rest), "expected `get(..)`/`ban(..)` clusters"));
    }
    rewrite_clusters(rest)
}

fn no_pins(r: &Rewritten) -> syn::Result<()> {
    match r.pins.first() {
        None => Ok(()),
        Some(p) => Err(syn::Error::new(
            span_of(&p.expr),
            "X(name = expr) needs a graph: use search![&g, ...]",
        )),
    }
}

fn span_of(ts: &TokenStream) -> Span {
    match first_span(ts) {
        Some(s) => s,
        None => Span::call_site(),
    }
}

fn starts_with_cluster(ts: &TokenStream) -> bool {
    let tokens: Vec<TokenTree> = ts.clone().into_iter().collect();
    is_cluster_head(None, &tokens)
}

fn split_head(input: TokenStream) -> syn::Result<(Head, TokenStream)> {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    match tokens.first() {
        None => Err(syn::Error::new(Span::call_site(), "empty search!")),
        Some(TokenTree::Punct(p)) if p.as_char() == '<' => {
            let (types, rest) = split_type_header(tokens.into_iter().collect())?;
            match types {
                TypeHeader::Explicit { nv, er } => Ok((Head::Types { nv, er }, rest)),
                TypeHeader::Inferred => Err(syn::Error::new(Span::call_site(), "expected `<NV, ER>;` before the clusters")),
            }
        }
        Some(TokenTree::Ident(_)) if is_cluster_head(None, &tokens) => Ok((Head::None, tokens.into_iter().collect())),
        Some(TokenTree::Ident(i))
            if i == "mgraph"
                && matches!(tokens.get(1), Some(TokenTree::Punct(b)) if b.as_char() == '!')
                && matches!(tokens.get(2), Some(TokenTree::Group(_))) =>
        {
            let (inline, rest) = split_graph_head(&tokens)?;
            let inline_tokens: Vec<TokenTree> = inline.into_iter().collect();
            if inline_tokens.len() != 3 {
                return Err(syn::Error::new(i.span(), "expected `mgraph![..]`"));
            }
            let TokenTree::Group(g) = &inline_tokens[2] else {
                return Err(syn::Error::new(i.span(), "expected `mgraph![..]`"));
            };
            Ok((Head::InlineGraph(g.stream()), rest))
        }
        Some(_) => {
            let (graph, rest) = split_graph_head(&tokens)?;
            Ok((Head::Graph(graph), rest))
        }
    }
}

fn split_graph_head(tokens: &[TokenTree]) -> syn::Result<(TokenStream, TokenStream)> {
    let first = tokens[0].span();
    let parser = |stream: ParseStream| -> syn::Result<(TokenStream, TokenStream)> {
        let graph: syn::Expr = stream.parse()?;
        match stream.parse::<Option<syn::Token![,]>>()? {
            Some(_) => {
                let rest: TokenStream = stream.parse()?;
                Ok((quote!(#graph), rest))
            }
            None => {
                let rest: Vec<TokenTree> = stream.parse::<TokenStream>()?.into_iter().collect();
                Err(missing_comma(first, &rest))
            }
        }
    };
    parser.parse2(tokens.iter().cloned().collect())
}

fn missing_comma(first: Span, rest: &[TokenTree]) -> syn::Error {
    match is_cluster_head(None, rest) {
        true => syn::Error::new(rest[0].span(), "expected `,` before the cluster"),
        false => syn::Error::new(first, "expected `, get(..)` after the graph"),
    }
}
