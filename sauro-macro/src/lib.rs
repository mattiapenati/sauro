mod syntax;

use proc_macro::TokenStream;
use syn::parse_macro_input;

use crate::syntax::Module;

use self::syntax::RustModule;

#[proc_macro_attribute]
pub fn bridge(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as RustModule);
    let _module = match Module::parse(input) {
        Ok(module) => module,
        Err(err) => return err.into_compile_error().into(),
    };

    todo!("fn bridge")
}
