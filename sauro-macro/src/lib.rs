mod syntax;

use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemMod};

#[proc_macro_attribute]
pub fn bridge(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemMod);
    let _module = match syntax::parse_module(input) {
        Ok(module) => module,
        Err(err) => return err.into_compile_error().into(),
    };

    todo!("fn bridge")
}
