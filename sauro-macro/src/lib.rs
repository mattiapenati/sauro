mod syntax;

use proc_macro::TokenStream;
use syn::parse_macro_input;

use self::syntax::RustModule;

#[proc_macro_attribute]
pub fn bridge(_args: TokenStream, input: TokenStream) -> TokenStream {
    let _input = parse_macro_input!(input as RustModule);

    todo!()
}
