use proc_macro2::Span;
use syn::{
    punctuated::Punctuated, spanned::Spanned, Error, GenericArgument, Pat, PathArguments, Result,
    Token, Visibility,
};

use super::{
    Field, FnArg, Function, Item, Module, NativeKind, ReturnType, Signature, Struct, Type,
};

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
    match input {
        // handling native type
        syn::Type::Path(path_ty) => {
            let path = &path_ty.path;
            if path_ty.qself.is_none() && path.leading_colon.is_none() && path.segments.len() == 1 {
                let segment = path.segments.first().unwrap();
                let ident = segment.ident.clone();
                if segment.arguments.is_none() {
                    let ty_name = ident.to_string();
                    match ty_name.as_str() {
                        "i8" => return Ok(Type::Native(NativeKind::I8, ident)),
                        "u8" => return Ok(Type::Native(NativeKind::U8, ident)),
                        "i16" => return Ok(Type::Native(NativeKind::I16, ident)),
                        "u16" => return Ok(Type::Native(NativeKind::U16, ident)),
                        "i32" => return Ok(Type::Native(NativeKind::I32, ident)),
                        "u32" => return Ok(Type::Native(NativeKind::U32, ident)),
                        "i64" => return Ok(Type::Native(NativeKind::I64, ident)),
                        "u64" => return Ok(Type::Native(NativeKind::U64, ident)),
                        "isize" => return Ok(Type::Native(NativeKind::ISize, ident)),
                        "usize" => return Ok(Type::Native(NativeKind::USize, ident)),
                        "f32" => return Ok(Type::Native(NativeKind::F32, ident)),
                        "f64" => return Ok(Type::Native(NativeKind::F64, ident)),
                        "String" => return Ok(Type::OwnedString(path_ty.clone())),
                        "Box" => {
                            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                if let Some(GenericArgument::Type(syn::Type::Slice(box_ty))) =
                                    args.args.first()
                                {
                                    if let syn::Type::Path(elem_ty) = &*box_ty.elem {
                                        if let Some(segment) = elem_ty.path.segments.first() {
                                            let ident = &segment.ident;

                                            if *ident == "u8" {
                                                return Ok(Type::OwnedBuffer(path_ty.clone()));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        "Vec" => {
                            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                if let Some(GenericArgument::Type(syn::Type::Path(vec_ty))) =
                                    args.args.first()
                                {
                                    if let Some(segment) = vec_ty.path.segments.first() {
                                        let ident = &segment.ident;

                                        if *ident == "u8" {
                                            return Ok(Type::OwnedBuffer(path_ty.clone()));
                                        }
                                    }
                                }
                            }
                        }
                        _ => return Ok(Type::Json(path_ty.clone())),
                    };
                }
            }
        }
        syn::Type::Reference(reference_ty) => {
            // only reference without lifetime and mutability are supported
            if reference_ty.lifetime.is_none() {
                match &*reference_ty.elem {
                    syn::Type::Path(ty) => {
                        let path = &ty.path;
                        if ty.qself.is_none()
                            && path.leading_colon.is_none()
                            && path.segments.len() == 1
                        {
                            let segment = &path.segments[0];
                            let ident = segment.ident.clone();
                            if segment.arguments.is_none() {
                                let ty_name = ident.to_string();
                                if ty_name == "str" {
                                    let ty = Type::BorrowedString(reference_ty.clone());
                                    return Ok(ty);
                                }
                            }
                        }
                    }
                    syn::Type::Slice(ty) => {
                        if let syn::Type::Path(ty) = &*ty.elem {
                            let path = &ty.path;
                            if ty.qself.is_none()
                                && path.leading_colon.is_none()
                                && path.segments.len() == 1
                            {
                                let segment = &path.segments[0];
                                let ident = segment.ident.clone();
                                if segment.arguments.is_none() {
                                    let ty_name = ident.to_string();
                                    if ty_name == "u8" {
                                        let ty = Type::BorrowedBuffer(reference_ty.clone());
                                        return Ok(ty);
                                    }
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
        syn::ReturnType::Type(rarrow, ty) => {
            let ty = parse_type(ty)?;
            match &ty {
                Type::Native(_, _)
                | Type::Json(_)
                | Type::OwnedString(_)
                | Type::OwnedBuffer(_) => ReturnType::Type(*rarrow, ty),
                _ => return Err(Error::new_spanned(input, "unsupported return type")),
            }
        }
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