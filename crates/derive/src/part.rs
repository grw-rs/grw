use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, Type};

pub fn expand(krate: &TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = syn::parse2(input)?;
    let name = &input.ident;
    let vis = &input.vis;

    let data = match &input.data {
        Data::Enum(e) => e,
        _ => return Err(syn::Error::new_spanned(&input, "Part can only be derived for an enum")),
    };

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "Part cannot be derived for a generic enum: each part kind needs one concrete payload type",
        ));
    }

    let mut variants: Vec<(&Ident, &Type)> = Vec::with_capacity(data.variants.len());
    for variant in &data.variants {
        let payload = match &variant.fields {
            Fields::Unnamed(f) if f.unnamed.len() == 1 => &f.unnamed[0].ty,
            _ => {
                return Err(syn::Error::new_spanned(
                    variant,
                    "Part requires every variant to be a single-field tuple variant, e.g. `Signs(Signs)`",
                ));
            }
        };
        variants.push((&variant.ident, payload));
    }

    let mut seen: HashMap<String, &Ident> = HashMap::with_capacity(variants.len());
    for (ident, ty) in &variants {
        let key = quote!(#ty).to_string();
        if let Some(first) = seen.insert(key, ident) {
            return Err(syn::Error::new_spanned(
                ty,
                format!(
                    "Part: variant `{}` and `{}` both hold `{}` — each payload type may appear in only one variant",
                    first,
                    ident,
                    quote!(#ty),
                ),
            ));
        }
    }

    let kind_name = format_ident!("{}Kind", name);
    let kind_variant_idents: Vec<&Ident> = variants.iter().map(|(ident, _)| *ident).collect();

    let kind_enum = quote! {
        #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::marker::Copy, ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::cmp::PartialOrd, ::core::cmp::Ord)]
        #vis enum #kind_name {
            #(#kind_variant_idents),*
        }
    };

    let kind_arms = variants.iter().map(|(ident, _)| {
        quote! { #name::#ident(_) => #kind_name::#ident }
    });

    let kinded_impl = quote! {
        impl #krate::composite::Kinded for #name {
            type Kind = #kind_name;

            fn kind(&self) -> #kind_name {
                match self {
                    #(#kind_arms),*
                }
            }
        }
    };

    let from_impls = variants.iter().map(|(ident, ty)| {
        quote! {
            impl ::core::convert::From<#ty> for #name {
                fn from(part: #ty) -> Self {
                    #name::#ident(part)
                }
            }
        }
    });

    let part_of_impls = variants.iter().map(|(ident, ty)| {
        quote! {
            impl #krate::composite::PartOf<#ty> for #name {
                fn kind() -> #kind_name {
                    #kind_name::#ident
                }

                #[allow(unreachable_patterns)]
                fn project(&self) -> ::core::option::Option<&#ty> {
                    match self {
                        #name::#ident(part) => ::core::option::Option::Some(part),
                        _ => ::core::option::Option::None,
                    }
                }

                fn inject(part: #ty) -> Self {
                    #name::#ident(part)
                }
            }
        }
    });

    Ok(quote! {
        #kind_enum
        #kinded_impl
        #(#from_impls)*
        #(#part_of_impls)*
    })
}
