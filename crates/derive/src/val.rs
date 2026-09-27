use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type, PathSegment};

pub fn expand(krate: &TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = syn::parse2(input)?;

    match &input.data {
        Data::Struct(s) => expand_struct(krate, &input, s),
        Data::Enum(e) => expand_enum(krate, &input, e),
        Data::Union(_) => Err(syn::Error::new_spanned(&input, "Val cannot be derived for unions")),
    }
}

fn expand_struct(krate: &TokenStream, input: &DeriveInput, s: &syn::DataStruct) -> syn::Result<TokenStream> {
    let name = &input.ident;

    let fields = match &s.fields {
        Fields::Named(f) => &f.named,
        _ => return Err(syn::Error::new_spanned(input, "Val requires named fields")),
    };

    let field_entries: Vec<TokenStream> = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let ty = &f.ty;
        let field_type_expr = type_to_field_type(krate, ty);

        quote! {
            #krate::layout::FieldInfo {
                name: #field_name_str,
                ty: #field_type_expr,
                offset: ::core::mem::offset_of!(#name, #field_name),
            }
        }
    }).collect();

    let field_count = field_entries.len();

    let hash_steps: Vec<TokenStream> = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let name_bytes = field_name_str.as_bytes();
        let ty = &f.ty;

        if let Type::Path(tp) = ty
            && let Some(seg) = tp.path.segments.last()
            && let Some(disc) = type_discriminant(seg)
        {
            return quote! {
                h = #krate::layout::fnv_hash_bytes(h, &[#(#name_bytes),*]);
                h = #krate::layout::fnv_hash_byte(h, #disc);
            };
        }
        quote! {
            h = #krate::layout::fnv_hash_bytes(h, &[#(#name_bytes),*]);
            h = #krate::layout::fnv_hash_byte(h, 13u8);
            h = #krate::layout::fnv_hash_u64(h, <#ty as #krate::layout::Val>::layout_hash());
        }
    }).collect();

    Ok(quote! {
        impl #krate::layout::__GrwMethodFallback for #name {}

        impl #krate::layout::Val for #name {
            fn fields() -> &'static [#krate::layout::FieldInfo] {
                static FIELDS: ::std::sync::LazyLock<[#krate::layout::FieldInfo; #field_count]> =
                    ::std::sync::LazyLock::new(|| [
                        #(#field_entries),*
                    ]);
                &*FIELDS
            }

            fn methods() -> &'static [#krate::layout::MethodMeta] {
                #[allow(unused_imports)]
                use #krate::layout::__GrwMethodFallback as _;
                Self::__grw_method_table()
            }

            fn field_type() -> #krate::layout::FieldType {
                #krate::layout::FieldType::Struct(Self::fields)
            }

            fn layout_hash() -> u64 {
                let mut h = #krate::layout::FNV_OFFSET;
                h = #krate::layout::fnv_hash_u64(h, #field_count as u64);
                #(#hash_steps)*
                h
            }

            fn size() -> usize {
                ::core::mem::size_of::<#name>()
            }

            fn align() -> usize {
                ::core::mem::align_of::<#name>()
            }
        }
    })
}

fn expand_enum(krate: &TokenStream, input: &DeriveInput, e: &syn::DataEnum) -> syn::Result<TokenStream> {
    let name = &input.ident;

    for variant in &e.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "Val derive only supports unit variants (C-like enums)",
            ));
        }
    }

    let variant_count = e.variants.len();

    let variant_entries: Vec<TokenStream> = e.variants.iter().map(|v| {
        let vname = &v.ident;
        let vname_str = vname.to_string();
        quote! {
            #krate::layout::EnumVariant {
                name: #vname_str,
                discriminant: #name::#vname as i128,
            }
        }
    }).collect();

    let hash_variant_steps: Vec<TokenStream> = e.variants.iter().map(|v| {
        let vname = &v.ident;
        let vname_str = vname.to_string();
        let name_bytes = vname_str.as_bytes();
        quote! {
            h = #krate::layout::fnv_hash_bytes(h, &[#(#name_bytes),*]);
            h = #krate::layout::fnv_hash_bytes(h, &(#name::#vname as i128).to_le_bytes());
        }
    }).collect();

    Ok(quote! {
        impl #krate::layout::Val for #name {
            fn fields() -> &'static [#krate::layout::FieldInfo] {
                &[]
            }

            fn field_type() -> #krate::layout::FieldType {
                static VARIANTS: ::std::sync::LazyLock<[#krate::layout::EnumVariant; #variant_count]> =
                    ::std::sync::LazyLock::new(|| [
                        #(#variant_entries),*
                    ]);
                static META: ::std::sync::LazyLock<#krate::layout::EnumMeta> =
                    ::std::sync::LazyLock::new(|| #krate::layout::EnumMeta {
                        type_name: ::core::stringify!(#name),
                        variants: &*VARIANTS,
                    });
                #krate::layout::FieldType::Enum(&META)
            }

            fn layout_hash() -> u64 {
                let mut h = #krate::layout::FNV_OFFSET;
                h = #krate::layout::fnv_hash_byte(h, 14u8);
                h = #krate::layout::fnv_hash_u64(h, #variant_count as u64);
                #(#hash_variant_steps)*
                h
            }

            fn size() -> usize {
                ::core::mem::size_of::<#name>()
            }

            fn align() -> usize {
                ::core::mem::align_of::<#name>()
            }
        }
    })
}

fn type_to_field_type(krate: &TokenStream, ty: &Type) -> TokenStream {
    if let Type::Path(tp) = ty
        && let Some(seg) = tp.path.segments.last()
    {
        return primitive_field_type(krate, seg)
            .unwrap_or_else(|| {
                let ty = &tp.path;
                quote! { <#ty as #krate::layout::Val>::field_type() }
            });
    }
    quote! { ::core::compile_error!("unsupported field type for Val derive") }
}

pub fn type_to_field_type_pub(krate: &TokenStream, ty: &Type) -> TokenStream {
    type_to_field_type(krate, ty)
}

fn type_discriminant(seg: &PathSegment) -> Option<u8> {
    match seg.ident.to_string().as_str() {
        "bool" => Some(1),
        "i8" => Some(2),
        "i16" => Some(3),
        "i32" => Some(4),
        "i64" => Some(5),
        "u8" => Some(6),
        "u16" => Some(7),
        "u32" => Some(8),
        "u64" => Some(9),
        "f32" => Some(10),
        "f64" => Some(11),
        "String" => Some(12),
        _ => None,
    }
}

pub fn is_supported_scalar(seg: &PathSegment) -> bool {
    type_discriminant(seg).is_some() && seg.ident != "String"
}

fn primitive_field_type(krate: &TokenStream, seg: &PathSegment) -> Option<TokenStream> {
    let ft = match seg.ident.to_string().as_str() {
        "bool" => quote! { #krate::layout::FieldType::Bool },
        "i8"   => quote! { #krate::layout::FieldType::I8 },
        "i16"  => quote! { #krate::layout::FieldType::I16 },
        "i32"  => quote! { #krate::layout::FieldType::I32 },
        "i64"  => quote! { #krate::layout::FieldType::I64 },
        "u8"   => quote! { #krate::layout::FieldType::U8 },
        "u16"  => quote! { #krate::layout::FieldType::U16 },
        "u32"  => quote! { #krate::layout::FieldType::U32 },
        "u64"  => quote! { #krate::layout::FieldType::U64 },
        "f32"  => quote! { #krate::layout::FieldType::F32 },
        "f64"  => quote! { #krate::layout::FieldType::F64 },
        "String" => quote! { #krate::layout::FieldType::String },
        _ => return None,
    };
    Some(ft)
}
