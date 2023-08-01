use quote::quote;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    token, Attribute, Error, Ident, Item, Result, Token, Visibility,
};

pub struct RustModule {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub mod_token: Token![mod],
    pub ident: Ident,
    pub brace_token: token::Brace,
    pub items: Vec<Item>,
}

impl Parse for RustModule {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut attrs = input.call(Attribute::parse_outer)?;

        let vis = input.parse()?;
        let mod_token = input.parse()?;
        let ident = input.parse()?;

        // empty `mod` are not supported
        let semi: Option<Token![;]> = input.parse()?;
        if let Some(semi) = semi {
            let span = quote!(#vis #mod_token #ident #semi);
            return Err(Error::new_spanned(span, "modules can not be empty"));
        }

        let content;
        let brace_token = braced!(content in input);

        attrs.extend(content.call(Attribute::parse_inner)?);

        let mut items = vec![];
        while !content.is_empty() {
            items.push(content.parse()?);
        }

        Ok(RustModule {
            attrs,
            vis,
            mod_token,
            ident,
            brace_token,
            items,
        })
    }
}
