use quote::quote;
use syn::{DeriveInput, parse_macro_input};

/// Implement FromArgValue trait of argh for custom type
#[proc_macro_derive(FromKeyValues)]
pub fn kv_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse Rust code to a syntax tree, then build expanded Rust code as tokens
    let DeriveInput {
        ident, generics, ..
    } = parse_macro_input!(input);
    // Where clause is like where T: Clone
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics ::argh::FromArgValue for #ident #ty_generics #where_clause {
            fn from_arg_value(value: &str) -> std::result::Result<Self, std::string::String> {
                ::serde_keyvalue::from_key_values(value).map_err(|e| e.to_string())
            }
        }
    }
    .into()
}
