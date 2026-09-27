use proc_macro2::TokenStream;
use quote::quote;
use syn::{ImplItem, ItemImpl, ReturnType, Type, FnArg, Pat};

use crate::val;

enum Receiver {
    RefSelf,
    Static,
}

struct ParsedMethod {
    name: syn::Ident,
    receiver: Receiver,
    params: Vec<(syn::Ident, Type)>,
    ret_type: Option<Type>,
}

fn is_scalar_type(ty: &Type) -> bool {
    let Type::Path(tp) = ty else { return false };
    let Some(seg) = tp.path.segments.last() else { return false };
    val::is_supported_scalar(seg)
}

fn returns_self(ty: &Type, self_ty: &Type) -> bool {
    match ty {
        Type::Path(tp) => {
            let last = tp.path.segments.last().map(|s| s.ident.to_string());
            last.as_deref() == Some("Self")
                || quote!(#ty).to_string() == quote!(#self_ty).to_string()
        }
        _ => false,
    }
}

pub fn expand(krate: &TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let item: ItemImpl = syn::parse2(input)?;

    if item.trait_.is_some() {
        return Err(syn::Error::new_spanned(&item, "#[grw::repl] can only be used on inherent impl blocks"));
    }

    let self_ty = &item.self_ty;

    let mut parsed = Vec::new();

    for impl_item in &item.items {
        let ImplItem::Fn(method) = impl_item else { continue };

        if method.sig.asyncness.is_some() { continue }
        if !method.sig.generics.params.is_empty() { continue }

        let mut inputs = method.sig.inputs.iter();

        let receiver = match inputs.next() {
            Some(FnArg::Receiver(r)) => {
                if r.mutability.is_some() { continue }
                if r.reference.is_none() { continue }
                Receiver::RefSelf
            }
            Some(FnArg::Typed(_)) => Receiver::Static,
            None => Receiver::Static,
        };

        let extra_params: Vec<(syn::Ident, Type)> = match &receiver {
            Receiver::RefSelf => {
                method.sig.inputs.iter().skip(1).filter_map(|arg| {
                    let FnArg::Typed(pt) = arg else { return None };
                    let Pat::Ident(pi) = pt.pat.as_ref() else { return None };
                    Some((pi.ident.clone(), (*pt.ty).clone()))
                }).collect()
            }
            Receiver::Static => {
                method.sig.inputs.iter().filter_map(|arg| {
                    let FnArg::Typed(pt) = arg else { return None };
                    let Pat::Ident(pi) = pt.pat.as_ref() else { return None };
                    Some((pi.ident.clone(), (*pt.ty).clone()))
                }).collect()
            }
        };

        let ret_type = match &method.sig.output {
            ReturnType::Default => None,
            ReturnType::Type(_, ty) => Some((*ty.clone()).clone()),
        };

        parsed.push(ParsedMethod {
            name: method.sig.ident.clone(),
            receiver,
            params: extra_params,
            ret_type,
        });
    }

    let mut entries = Vec::new();

    for m in &parsed {
        let method_name = &m.name;
        let method_name_str = method_name.to_string();
        let is_static = matches!(m.receiver, Receiver::Static);

        let resolved_ret = m.ret_type.as_ref().map(|ty| {
            if returns_self(ty, self_ty) {
                (**self_ty).clone()
            } else {
                ty.clone()
            }
        });

        let ret_type_expr = match &resolved_ret {
            Some(ty) => val::type_to_field_type_pub(krate, ty),
            None => quote! { #krate::layout::FieldType::Bool },
        };

        let param_entries: Vec<TokenStream> = m.params.iter().map(|(pname, pty)| {
            let pname_str = pname.to_string();
            let pty_expr = val::type_to_field_type_pub(krate, pty);
            quote! {
                #krate::layout::MethodParam { name: #pname_str, ty: #pty_expr }
            }
        }).collect();
        let param_count = param_entries.len();

        let all_params_scalar = m.params.iter().all(|(_, ty)| is_scalar_type(ty));
        let ret_is_scalar = m.ret_type.as_ref().is_some_and(is_scalar_type);
        let ret_is_self = m.ret_type.as_ref().is_some_and(|ty| returns_self(ty, self_ty));
        let has_return = m.ret_type.is_some();

        let can_wrap = all_params_scalar && has_return && (ret_is_scalar || (is_static && ret_is_self));

        let fn_ptr_expr = if can_wrap {
            let param_names: Vec<&syn::Ident> = m.params.iter().map(|(n, _)| n).collect();
            let param_types: Vec<&Type> = m.params.iter().map(|(_, t)| t).collect();
            let ret_ty = resolved_ret.as_ref().unwrap();

            if is_static && ret_is_self {
                quote! {
                    {
                        extern "C" fn __grw_w(#(#param_names: #param_types,)* __out: *mut u8) {
                            let __val = #self_ty::#method_name(#(#param_names),*);
                            unsafe { ::core::ptr::write(__out as *mut #self_ty, __val) };
                        }
                        __grw_w as usize as *const u8
                    }
                }
            } else if is_static {
                quote! {
                    {
                        extern "C" fn __grw_w(#(#param_names: #param_types),*) -> #ret_ty {
                            #self_ty::#method_name(#(#param_names),*)
                        }
                        __grw_w as usize as *const u8
                    }
                }
            } else {
                quote! {
                    {
                        extern "C" fn __grw_w(__p: *const u8, #(#param_names: #param_types),*) -> #ret_ty {
                            unsafe { &*(__p as *const #self_ty) }.#method_name(#(#param_names),*)
                        }
                        __grw_w as usize as *const u8
                    }
                }
            }
        } else {
            quote! { ::core::ptr::null::<u8>() }
        };

        let params_expr = if param_entries.is_empty() {
            quote! { &[] }
        } else {
            quote! {
                {
                    static PARAMS: ::std::sync::LazyLock<[#krate::layout::MethodParam; #param_count]> =
                        ::std::sync::LazyLock::new(|| [#(#param_entries),*]);
                    &*PARAMS as &'static [#krate::layout::MethodParam]
                }
            }
        };

        entries.push(quote! {
            {
                #krate::layout::MethodMeta {
                    name: #method_name_str,
                    ret_type: #ret_type_expr,
                    params: #params_expr,
                    is_static: #is_static,
                    fn_ptr: #fn_ptr_expr,
                }
            }
        });
    }

    let method_count = entries.len();

    Ok(quote! {
        #item

        impl #self_ty {
            #[doc(hidden)]
            pub fn __grw_method_table() -> &'static [#krate::layout::MethodMeta] {
                static METHODS: ::std::sync::LazyLock<[#krate::layout::MethodMeta; #method_count]> =
                    ::std::sync::LazyLock::new(|| [
                        #(#entries),*
                    ]);
                &*METHODS
            }
        }
    })
}
