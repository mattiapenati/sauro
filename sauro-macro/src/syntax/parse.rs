use proc_macro2::Span;
use syn::{
    punctuated::Punctuated, spanned::Spanned, token, Attribute, Block, Error, Fields,
    FnArg as RustFnArg, Ident, Item as RustItem, ItemFn, ItemStruct, Pat, Result,
    ReturnType as RustReturnType, Signature as RustSignature, Token, Type as RustType, Visibility,
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
    Function(Function),
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

pub struct Function {
    pub attrs: Vec<Attribute>,
    pub vis: Token![pub],
    pub sig: Signature,
    pub block: Box<Block>,
}

pub struct Signature {
    pub fn_token: Token![fn],
    pub ident: Ident,
    pub paren_token: token::Paren,
    pub inputs: Punctuated<FnArg, Token![,]>,
    pub output: ReturnType,
}

pub struct FnArg {
    pub mutability: Option<Token![mut]>,
    pub ident: Ident,
    pub colon_token: Token![:],
    pub ty: Type,
}

pub enum Type {
    Ident(Ident),
}

pub enum ReturnType {
    Default,
    Type(Token![->], Type),
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

        Ok(Self {
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
            RustItem::Fn(input) => Ok(Self::Function(Function::parse(input)?)),
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

        Ok(Self {
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
        let ty = Type::parse(&input.ty)?;

        Ok(Self {
            attrs,
            vis,
            ident,
            colon_token,
            ty,
        })
    }
}

impl Function {
    fn parse(input: ItemFn) -> Result<Self> {
        let attrs = input.attrs;
        let vis = visibility_pub(&input.vis, input.sig.span());
        let sig = Signature::parse(input.sig)?;
        let block = input.block;

        Ok(Self {
            attrs,
            vis,
            sig,
            block,
        })
    }
}

impl Signature {
    fn parse(input: RustSignature) -> Result<Self> {
        if input.constness.is_some() {
            return Err(Error::new_spanned(
                input.constness,
                "const functions are not supported",
            ));
        }
        if input.asyncness.is_some() {
            return Err(Error::new_spanned(
                input.asyncness,
                "async functions are not supported",
            ));
        }
        if input.unsafety.is_some() {
            return Err(Error::new_spanned(
                input.unsafety,
                "unsafe functions are not supported",
            ));
        }
        if input.abi.is_some() {
            // variadic argument is allowed only in extern function, then it should not be checked
            return Err(Error::new_spanned(
                input.abi,
                "extern functions are not supported",
            ));
        }
        if !input.generics.params.is_empty() {
            return Err(Error::new_spanned(
                input.generics,
                "function parameters are not supported",
            ));
        }

        let fn_token = input.fn_token;
        let ident = input.ident;
        let paren_token = input.paren_token;

        let mut inputs = Punctuated::new();
        for pair in input.inputs.into_pairs() {
            let (fn_arg, punct) = pair.into_tuple();
            let fn_arg = FnArg::parse(&fn_arg)?;

            inputs.push_value(fn_arg);
            if let Some(punct) = punct {
                inputs.push_punct(punct);
            }
        }

        let output = ReturnType::parse(&input.output)?;

        Ok(Self {
            fn_token,
            ident,
            paren_token,
            inputs,
            output,
        })
    }
}

impl FnArg {
    fn parse(input: &RustFnArg) -> Result<Self> {
        let fn_arg = match input {
            RustFnArg::Receiver(_) => {
                return Err(Error::new_spanned(input, "self argument is not supported"))
            }
            RustFnArg::Typed(fn_arg) => {
                let (mutability, ident) = match &*fn_arg.pat {
                    Pat::Ident(pat) => (pat.mutability, pat.ident.clone()),
                    _ => {
                        return Err(Error::new_spanned(
                            fn_arg,
                            "pattern matching is not supported",
                        ))
                    }
                };
                let colon_token = fn_arg.colon_token;
                let ty = Type::parse(&fn_arg.ty)?;

                FnArg {
                    mutability,
                    ident,
                    colon_token,
                    ty,
                }
            }
        };
        Ok(fn_arg)
    }
}

impl Type {
    fn parse(input: &RustType) -> Result<Self> {
        if let RustType::Path(input) = &input {
            let path = &input.path;
            if input.qself.is_none() && path.leading_colon.is_none() && path.segments.len() == 1 {
                let segment = &path.segments[0];
                let ident = &segment.ident;
                if segment.arguments.is_none() {
                    return Ok(Self::Ident(ident.clone()));
                }
            }
        }
        Err(Error::new_spanned(input, "unsupported type"))
    }
}

impl ReturnType {
    fn parse(input: &RustReturnType) -> Result<Self> {
        let return_type = match input {
            RustReturnType::Default => Self::Default,
            RustReturnType::Type(rarrow, ty) => Self::Type(*rarrow, Type::parse(ty)?),
        };
        Ok(return_type)
    }
}

fn visibility_pub(vis: &Visibility, span: Span) -> Token![pub] {
    Token![pub](match vis {
        Visibility::Public(vis) => vis.span(),
        Visibility::Restricted(vis) => vis.pub_token.span(),
        Visibility::Inherited => span,
    })
}
