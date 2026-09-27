mod krate;
mod val;
mod part;
mod methods;
mod pattern;
mod search;

use proc_macro::TokenStream;

#[proc_macro_derive(Val)]
pub fn derive_val(input: TokenStream) -> TokenStream {
    krate::krate()
        .and_then(|k| val::expand(&k, input.into()))
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_derive(Part)]
pub fn derive_part(input: TokenStream) -> TokenStream {
    krate::krate()
        .and_then(|k| part::expand(&k, input.into()))
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn repl(_attr: TokenStream, input: TokenStream) -> TokenStream {
    krate::krate()
        .and_then(|k| methods::expand(&k, input.into()))
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro]
pub fn pattern(input: TokenStream) -> TokenStream {
    match krate::given(input.into()).and_then(|(k, rest)| pattern::expand(&k, rest)) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn search(input: TokenStream) -> TokenStream {
    match krate::given(input.into()).and_then(|(k, rest)| search::expand(&k, rest)) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
