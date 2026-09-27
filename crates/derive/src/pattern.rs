use proc_macro2::{TokenStream, TokenTree};
use quote::quote;

use grw_pattern::rewrite_clusters;

pub fn expand(krate: &TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let (types, body) = split_type_header(input)?;
    let r = rewrite_clusters(body)?;
    if let Some(c) = r.context.first() {
        return Err(syn::Error::new(c.span, "pattern!: context nodes X(..) are not allowed; use search![&g, ...]"));
    }
    let names = r.names.iter().map(|(n, i)| {
        let lit = proc_macro2::Literal::u64_unsuffixed(*i);
        quote!((#n, #krate::search::dsl::LocalId(#lit)))
    });
    let clusters_ty = match types {
        TypeHeader::Explicit { nv, er } => quote!(::std::vec::Vec<#krate::search::dsl::ClusterOps<#nv, #er>>),
        TypeHeader::Inferred => quote!(::std::vec::Vec<#krate::search::dsl::ClusterOps<_, _>>),
    };
    let body = r.body;
    Ok(quote! {{
        let __clusters: #clusters_ty = {
            #[allow(unused_imports)]
            use #krate::search::dsl::*;
            #krate::__search_clusters!(@acc [] #body)
        };
        #krate::search::Pattern::from_clusters(__clusters, #krate::search::Names::new(&[#(#names),*]))
    }})
}

pub(crate) enum TypeHeader {
    Explicit { nv: TokenStream, er: TokenStream },
    Inferred,
}

pub(crate) fn split_type_header(input: TokenStream) -> syn::Result<(TypeHeader, TokenStream)> {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    let starts_with_lt = matches!(tokens.first(), Some(TokenTree::Punct(p)) if p.as_char() == '<');
    if !starts_with_lt {
        return Ok((TypeHeader::Inferred, tokens.into_iter().collect()));
    }
    let semi = tokens.iter().position(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == ';'));
    let Some(semi) = semi else {
        return Err(syn::Error::new(tokens[0].span(), "expected `<NV, ER>;` before the clusters"));
    };
    let closed = semi >= 2 && matches!(&tokens[semi - 1], TokenTree::Punct(p) if p.as_char() == '>');
    if !closed {
        return Err(syn::Error::new(tokens[0].span(), "expected `<NV, ER>;` before the clusters"));
    }
    let header: TokenStream = tokens[1..semi - 1].iter().cloned().collect();
    let generics: syn::AngleBracketedGenericArguments = syn::parse2(quote!(< #header >))?;
    let mut args = generics.args.iter();
    let (Some(syn::GenericArgument::Type(nv)), Some(syn::GenericArgument::Type(er)), None) = (args.next(), args.next(), args.next()) else {
        return Err(syn::Error::new(tokens[0].span(), "expected exactly `<NV, ER>` (two types)"));
    };
    Ok((TypeHeader::Explicit { nv: quote!(#nv), er: quote!(#er) }, tokens[semi + 1..].iter().cloned().collect()))
}
