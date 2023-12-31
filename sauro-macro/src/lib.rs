use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemMod};

use sauro_core::{expand, syntax};

#[proc_macro_attribute]
pub fn bindgen(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemMod);
    syntax::parse_module(input)
        .map(expand::bindgen)
        .unwrap_or_else(|err| err.into_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn non_blocking(_args: TokenStream, input: TokenStream) -> TokenStream {
    // check if applied to a function
    syn::parse::<syn::ItemFn>(input)
        .map(quote::ToTokens::into_token_stream)
        .unwrap_or_else(|err| err.into_compile_error())
        .into()
}
