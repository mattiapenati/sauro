mod expand;
mod syntax;

use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemMod};

#[proc_macro_attribute]
pub fn bindgen(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemMod);
    syntax::parse_module(input)
        .map(expand::bindgen)
        .unwrap_or_else(|err| err.into_compile_error())
        .into()
}
