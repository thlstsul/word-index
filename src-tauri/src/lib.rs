use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

mod command_result;

/// 之所以将代码放到lib，是因为proc-macros 必须在 proc-macro crate 中定义；
#[proc_macro_derive(CommandError)]
pub fn derive_command_error(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    let expanded = quote! {
        impl std::convert::From<#name> for crate::command_result::CommandError {
            fn from(e: #name) -> crate::command_result::CommandError {
                crate::command_result::CommandError(e.to_string())
            }
        }
    };

    TokenStream::from(expanded)
}
