use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::parse::{ParseStream, Parser};

pub(crate) fn krate() -> syn::Result<TokenStream> {
    match crate_name("grw") {
        Ok(FoundCrate::Itself) => Ok(quote!(::grw)),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            Ok(quote!(::#ident))
        }
        Err(e) => Err(syn::Error::new(
            Span::call_site(),
            format!("grw_derive needs `grw` as a dependency of this crate: add it to [dependencies] ({e})"),
        )),
    }
}

pub(crate) fn given(input: TokenStream) -> syn::Result<(TokenStream, TokenStream)> {
    let parser = |stream: ParseStream| -> syn::Result<(TokenStream, TokenStream)> {
        let path = syn::Path::parse_mod_style(stream)?;
        stream.parse::<syn::Token![;]>()?;
        let rest: TokenStream = stream.parse()?;
        Ok((quote!(#path), rest))
    };
    parser.parse2(input)
}
