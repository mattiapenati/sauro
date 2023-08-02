use proc_macro2::Span;
use syn::{
    punctuated::Punctuated, spanned::Spanned, token, Attribute, Error, Fields, Ident,
    Item as RustItem, ItemStruct, Result, Token, Type as RustType, Visibility,
};

use super::RustModule;

pub struct Module {
    pub attrs: Vec<Attribute>,
    pub vis: Token![pub],
    pub mod_token: Token![mod],
    pub ident: Ident,
    pub brace_token: token::Brace,
    pub items: Vec<Item>,
}

pub enum Item {
    Struct(Struct),
}

pub struct Struct {
    pub attrs: Vec<Attribute>,
    pub vis: Token![pub],
    pub struct_token: Token![struct],
    pub ident: Ident,
    pub brace_token: token::Brace,
    pub fields: Punctuated<Field, Token![,]>,
}

pub struct Field {
    pub attrs: Vec<Attribute>,
    pub vis: Token![pub],
    pub ident: Ident,
    pub colon_token: Token![:],
    pub ty: Type,
}

pub enum Type {
    Ident(Ident),
}

impl Module {
    pub fn parse(input: RustModule) -> Result<Self> {
        let attrs = input.attrs;
        let vis = visibility_pub(&input.vis, input.ident.span());
        let mod_token = input.mod_token;
        let ident = input.ident;
        let brace_token = input.brace_token;
        let items = input
            .items
            .into_iter()
            .map(Item::parse)
            .collect::<Result<_>>()?;

        Ok(Module {
            attrs,
            vis,
            mod_token,
            ident,
            brace_token,
            items,
        })
    }
}

impl Item {
    fn parse(input: RustItem) -> Result<Self> {
        match input {
            RustItem::Struct(input) => Ok(Self::Struct(Struct::parse(input)?)),
            input => Err(Error::new_spanned(input, "unsupported item")),
        }
    }
}

impl Struct {
    fn parse(mut input: ItemStruct) -> Result<Self> {
        let attrs = std::mem::take(&mut input.attrs);
        let vis = visibility_pub(&input.vis, input.ident.span());
        let struct_token = input.struct_token;
        let ident = input.ident.clone();

        let params = &input.generics.params;
        if !params.is_empty() {
            return Err(Error::new_spanned(
                params,
                "type parameters are not supported",
            ));
        }

        let mut fields = Punctuated::new();
        let brace_token = match input.fields {
            Fields::Named(named_fields) => {
                for pair in named_fields.named.into_pairs() {
                    let (field, punct) = pair.into_tuple();
                    let field = Field::parse(field)?;

                    fields.push_value(field);
                    if let Some(punct) = punct {
                        fields.push_punct(punct);
                    }
                }

                named_fields.brace_token
            }
            Fields::Unnamed(_) => {
                return Err(Error::new_spanned(input, "tuple structs are not supported"))
            }
            Fields::Unit => {
                return Err(Error::new_spanned(input, "unit structs are not supported"))
            }
        };

        Ok(Struct {
            attrs,
            vis,
            struct_token,
            ident,
            brace_token,
            fields,
        })
    }
}

impl Field {
    fn parse(input: syn::Field) -> Result<Self> {
        let attrs = input.attrs;
        let vis = visibility_pub(&input.vis, input.ident.span());
        let ident = input.ident.unwrap();
        let colon_token = input.colon_token.unwrap();
        let ty = Type::parse(input.ty)?;

        Ok(Field {
            attrs,
            vis,
            ident,
            colon_token,
            ty,
        })
    }
}

impl Type {
    fn parse(input: RustType) -> Result<Self> {
        if let RustType::Path(input) = &input {
            let path = &input.path;
            if input.qself.is_none() && path.leading_colon.is_none() && path.segments.len() == 1 {
                let segment = &path.segments[0];
                let ident = &segment.ident;
                if segment.arguments.is_none() {
                    return Ok(Type::Ident(ident.clone()));
                }
            }
        }
        Err(Error::new_spanned(input, "unsupported type"))
    }
}

fn visibility_pub(vis: &Visibility, span: Span) -> Token![pub] {
    Token![pub](match vis {
        Visibility::Public(vis) => vis.span(),
        Visibility::Restricted(vis) => vis.pub_token.span(),
        Visibility::Inherited => span,
    })
}
