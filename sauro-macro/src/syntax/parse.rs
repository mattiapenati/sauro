use proc_macro2::Span;
use syn::{punctuated::Punctuated, spanned::Spanned, Error, Pat, Result, Token, Visibility};

use super::{Field, FnArg, Function, Item, Module, ReturnType, Signature, Struct, Type};

pub fn parse_module(input: syn::ItemMod) -> Result<Module> {
    let Some((brace_token, items)) = input.content else {
        return Err(Error::new_spanned(&input, "modules can not be empty"));
    };

    let items = items.into_iter().map(parse_item).collect::<Result<_>>()?;

    let attrs = input.attrs;
    let vis = visibility_pub(&input.vis, input.ident.span());
    let mod_token = input.mod_token;
    let ident = input.ident;

    Ok(Module {
        attrs,
        vis,
        mod_token,
        ident,
        brace_token,
        items,
    })
}

fn parse_item(input: syn::Item) -> Result<Item> {
    match input {
        syn::Item::Struct(input) => Ok(Item::Struct(parse_struct(input)?)),
        syn::Item::Fn(input) => Ok(Item::Function(parse_function(input)?)),
        input => Err(Error::new_spanned(input, "unsupported item")),
    }
}

fn parse_struct(input: syn::ItemStruct) -> Result<Struct> {
    let params = &input.generics.params;
    if !params.is_empty() {
        return Err(Error::new_spanned(
            params,
            "type parameters are not supported",
        ));
    }

    let mut fields = Punctuated::new();
    let brace_token = match input.fields {
        syn::Fields::Named(named_fields) => {
            for pair in named_fields.named.into_pairs() {
                let (field, punct) = pair.into_tuple();
                let field = parse_field(field)?;

                fields.push_value(field);
                if let Some(punct) = punct {
                    fields.push_punct(punct);
                }
            }

            named_fields.brace_token
        }
        syn::Fields::Unnamed(_) => {
            return Err(Error::new_spanned(input, "tuple structs are not supported"))
        }
        syn::Fields::Unit => {
            return Err(Error::new_spanned(input, "unit structs are not supported"))
        }
    };

    let attrs = input.attrs;
    let vis = visibility_pub(&input.vis, input.ident.span());
    let struct_token = input.struct_token;
    let ident = input.ident.clone();

    Ok(Struct {
        attrs,
        vis,
        struct_token,
        ident,
        brace_token,
        fields,
    })
}

fn parse_field(input: syn::Field) -> Result<Field> {
    let attrs = input.attrs;
    let vis = visibility_pub(&input.vis, input.ident.span());
    let ident = input.ident.unwrap();
    let colon_token = input.colon_token.unwrap();
    let ty = parse_type(&input.ty)?;

    Ok(Field {
        attrs,
        vis,
        ident,
        colon_token,
        ty,
    })
}

fn parse_function(input: syn::ItemFn) -> Result<Function> {
    let attrs = input.attrs;
    let vis = visibility_pub(&input.vis, input.sig.span());
    let sig = parse_signature(input.sig)?;
    let block = input.block;

    Ok(Function {
        attrs,
        vis,
        sig,
        block,
    })
}

fn parse_signature(input: syn::Signature) -> Result<Signature> {
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
        let fn_arg = parse_function_arg(&fn_arg)?;

        inputs.push_value(fn_arg);
        if let Some(punct) = punct {
            inputs.push_punct(punct);
        }
    }

    let output = parse_return_type(&input.output)?;

    Ok(Signature {
        fn_token,
        ident,
        paren_token,
        inputs,
        output,
    })
}

fn parse_function_arg(input: &syn::FnArg) -> Result<FnArg> {
    match input {
        syn::FnArg::Receiver(_) => Err(Error::new_spanned(input, "self argument is not supported")),
        syn::FnArg::Typed(fn_arg) => {
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
            let ty = parse_type(&fn_arg.ty)?;

            Ok(FnArg {
                mutability,
                ident,
                colon_token,
                ty,
            })
        }
    }
}

fn parse_type(input: &syn::Type) -> Result<Type> {
    match &input {
        // handling native type
        syn::Type::Path(input) => {
            let path = &input.path;
            if input.qself.is_none() && path.leading_colon.is_none() && path.segments.len() == 1 {
                let segment = &path.segments[0];
                let ident = segment.ident.clone();
                if segment.arguments.is_none() {
                    let ty_name = ident.to_string();
                    let ty_name = ty_name.as_str();
                    let ty = match ty_name {
                        "i8" | "u8" | "i16" | "u16" | "i32" | "u32" | "i64" | "u64" | "isize"
                        | "usize" | "f32" | "f64" => Type::Native(ident),
                        "String" => Type::String(ident),
                        _ => Type::Json(ident),
                    };
                    return Ok(ty);
                }
            }
        }
        syn::Type::Reference(input) => {
            // only reference without lifetime and mutability are supported
            if input.lifetime.is_none() && input.mutability.is_none() {
                let and_token = input.and_token;
                match &*input.elem {
                    syn::Type::Path(input) => {
                        let path = &input.path;
                        if input.qself.is_none()
                            && path.leading_colon.is_none()
                            && path.segments.len() == 1
                        {
                            let segment = &path.segments[0];
                            let ident = segment.ident.clone();
                            if segment.arguments.is_none() {
                                let ty_name = ident.to_string();
                                if ty_name == "str" {
                                    let ty = Type::Str { and_token, ident };
                                    return Ok(ty);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Err(Error::new_spanned(input, "unsupported type"))
}

fn parse_return_type(input: &syn::ReturnType) -> Result<ReturnType> {
    let return_type = match input {
        syn::ReturnType::Default => ReturnType::Default,
        syn::ReturnType::Type(rarrow, ty) => ReturnType::Type(*rarrow, parse_type(ty)?),
    };
    Ok(return_type)
}

fn visibility_pub(vis: &Visibility, span: Span) -> Token![pub] {
    Token![pub](match vis {
        Visibility::Public(vis) => vis.span(),
        Visibility::Restricted(vis) => vis.pub_token.span(),
        Visibility::Inherited => span,
    })
}
