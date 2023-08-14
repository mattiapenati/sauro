use proc_macro2::Span;
use syn::{punctuated::Punctuated, spanned::Spanned, Pat, Token, Visibility};

use crate::typescript;

use super::{
    Field, FnArg, Item, ItemFn, ItemStruct, Module, ReturnType, Signature, Type, TypeKind,
    TypeNative,
};

pub fn parse_module(input: syn::ItemMod) -> syn::Result<Module> {
    let Some((brace_token, items)) = input.content else {
        return Err(syn::Error::new_spanned(&input, "modules can not be empty"));
    };

    let items = items
        .into_iter()
        .map(Item::try_from)
        .collect::<syn::Result<_>>()?;

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

impl TryFrom<syn::Item> for Item {
    type Error = syn::Error;

    fn try_from(value: syn::Item) -> syn::Result<Self> {
        match value {
            syn::Item::Struct(value) => ItemStruct::try_from(value).map(Item::Struct),
            syn::Item::Fn(value) => ItemFn::try_from(value).map(Item::Fn),
            input => Err(syn::Error::new_spanned(input, "unsupported item")),
        }
    }
}

impl TryFrom<syn::ItemStruct> for ItemStruct {
    type Error = syn::Error;

    fn try_from(value: syn::ItemStruct) -> syn::Result<Self> {
        let params = &value.generics.params;
        if !params.is_empty() {
            return Err(syn::Error::new_spanned(
                params,
                "type parameters are not supported",
            ));
        }

        let mut fields = Punctuated::new();
        let brace_token = match value.fields {
            syn::Fields::Named(named_fields) => {
                for pair in named_fields.named.into_pairs() {
                    let (field, punct) = pair.into_tuple();

                    let field = field.try_into()?;
                    fields.push_value(field);

                    if let Some(punct) = punct {
                        fields.push_punct(punct);
                    }
                }

                named_fields.brace_token
            }
            syn::Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    value,
                    "tuple structs are not supported",
                ))
            }
            syn::Fields::Unit => {
                return Err(syn::Error::new_spanned(
                    value,
                    "unit structs are not supported",
                ))
            }
        };

        let attrs = value.attrs;
        let vis = visibility_pub(&value.vis, value.ident.span());
        let struct_token = value.struct_token;
        let ident = value.ident.clone();

        Ok(ItemStruct {
            attrs,
            vis,
            struct_token,
            ident,
            brace_token,
            fields,
        })
    }
}

impl TryFrom<syn::ItemFn> for ItemFn {
    type Error = syn::Error;

    fn try_from(value: syn::ItemFn) -> syn::Result<Self> {
        let attrs = value.attrs;
        let vis = visibility_pub(&value.vis, value.sig.span());
        let sig = value.sig.try_into()?;
        let block = value.block;

        Ok(ItemFn {
            attrs,
            vis,
            sig,
            block,
        })
    }
}

impl TryFrom<syn::Field> for Field {
    type Error = syn::Error;

    fn try_from(value: syn::Field) -> syn::Result<Self> {
        let attrs = value.attrs;
        let vis = visibility_pub(&value.vis, value.ident.span());
        let ident = value.ident.unwrap();
        let colon_token = value.colon_token.unwrap();
        let ty = Type::try_from(&value.ty)?;

        Ok(Field {
            attrs,
            vis,
            ident,
            colon_token,
            ty,
        })
    }
}

impl TryFrom<syn::Signature> for Signature {
    type Error = syn::Error;

    fn try_from(value: syn::Signature) -> syn::Result<Signature> {
        if value.constness.is_some() {
            return Err(syn::Error::new_spanned(
                value.constness,
                "const functions are not supported",
            ));
        }
        if value.asyncness.is_some() {
            return Err(syn::Error::new_spanned(
                value.asyncness,
                "async functions are not supported",
            ));
        }
        if value.unsafety.is_some() {
            return Err(syn::Error::new_spanned(
                value.unsafety,
                "unsafe functions are not supported",
            ));
        }
        if value.abi.is_some() {
            // variadic argument is allowed only in extern function, then it should not be checked
            return Err(syn::Error::new_spanned(
                value.abi,
                "extern functions are not supported",
            ));
        }
        if !value.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                value.generics,
                "function parameters are not supported",
            ));
        }

        let fn_token = value.fn_token;
        let ident = value.ident;
        let paren_token = value.paren_token;

        let mut inputs = Punctuated::new();
        for pair in value.inputs.into_pairs() {
            let (fn_arg, punct) = pair.into_tuple();

            let fn_arg = fn_arg.try_into()?;
            inputs.push_value(fn_arg);

            if let Some(punct) = punct {
                inputs.push_punct(punct);
            }
        }

        let output = value.output.try_into()?;

        Ok(Signature {
            fn_token,
            ident,
            paren_token,
            inputs,
            output,
        })
    }
}

impl TryFrom<syn::FnArg> for FnArg {
    type Error = syn::Error;

    fn try_from(value: syn::FnArg) -> syn::Result<FnArg> {
        let syn::FnArg::Typed(fn_arg) = value else {
            return Err(syn::Error::new_spanned(value, "self argument is not supported"));
        };

        let (mutability, ident) = match fn_arg.pat.as_ref() {
            Pat::Ident(pat) => (pat.mutability, pat.ident.clone()),
            _ => {
                return Err(syn::Error::new_spanned(
                    fn_arg,
                    "pattern matching is not supported",
                ))
            }
        };
        let colon_token = fn_arg.colon_token;
        let ty = Type::try_from(fn_arg.ty.as_ref())?;

        Ok(FnArg {
            mutability,
            ident,
            colon_token,
            ty,
        })
    }
}

impl TryFrom<syn::ReturnType> for ReturnType {
    type Error = syn::Error;

    fn try_from(value: syn::ReturnType) -> syn::Result<ReturnType> {
        let syn::ReturnType::Type(rarrow, ref ty) = value else {
            return Ok(ReturnType::Default)
        };

        let ty = Type::try_from(ty.as_ref())?;
        let return_type = ReturnType::Type(rarrow, ty);
        Ok(return_type)
    }
}

impl TryFrom<&syn::Type> for Type {
    type Error = syn::Error;

    fn try_from(value: &syn::Type) -> syn::Result<Self> {
        match value {
            syn::Type::Path(ty) => parse_type_path(ty),
            syn::Type::Reference(ty) => parse_type_reference(ty),
            _ => Err(syn::Error::new_spanned(value, "unsupported type")),
        }
    }
}

fn parse_type_path(value: &syn::TypePath) -> syn::Result<Type> {
    let segments = &value.path.segments;

    if value.qself.is_none() && segments.len() == 1 {
        let ty = Box::new(syn::Type::Path(value.clone()));

        let segment = &segments[0];
        let is_result = segment.ident == "Result";
        let (kind, ts) = match segment.ident.to_string().as_str() {
            // native types
            "i8" => (TypeKind::Native(TypeNative::I8), typescript::number),
            "i16" => (TypeKind::Native(TypeNative::I16), typescript::number),
            "i32" => (TypeKind::Native(TypeNative::I32), typescript::number),
            "i64" => (
                TypeKind::Native(TypeNative::I64),
                typescript::number | typescript::bigint,
            ),
            "isize" => (
                TypeKind::Native(TypeNative::ISize),
                typescript::number | typescript::bigint,
            ),
            "u8" => (TypeKind::Native(TypeNative::U8), typescript::number),
            "u16" => (TypeKind::Native(TypeNative::I16), typescript::number),
            "u32" => (TypeKind::Native(TypeNative::I32), typescript::number),
            "u64" => (
                TypeKind::Native(TypeNative::U64),
                typescript::number | typescript::bigint,
            ),
            "usize" => (
                TypeKind::Native(TypeNative::USize),
                typescript::number | typescript::bigint,
            ),
            "f32" => (TypeKind::Native(TypeNative::F32), typescript::number),
            "f64" => (TypeKind::Native(TypeNative::F64), typescript::number),
            "Box" => parse_pointer_type(segment)?,
            "Option" => parse_option_type(segment)?,
            "Result" => parse_result_type(segment)?,
            "String" => (TypeKind::StringOwned, typescript::string),
            "Vec" => parse_vector_type(segment)?,
            s => (TypeKind::Json, typescript::Type![s]),
        };

        return Ok(Type {
            ty,
            kind,
            ts,
            is_result,
        });
    }

    // fully qualified types
    if value.qself.is_none() && segments.len() == 3 {
        let ty = Box::new(syn::Type::Path(value.clone()));

        let segment = &segments[2];
        let is_result = segment.ident == "Result";
        let (kind, ts) = if (segments[0].ident == "std" || segments[0].ident == "alloc")
            && segments[1].ident == "box"
            && segment.ident == "Box"
        {
            parse_pointer_type(segment)?
        } else if (segments[0].ident == "std" || segments[0].ident == "core")
            && segments[1].ident == "option"
            && segment.ident == "Option"
        {
            parse_option_type(segment)?
        } else if (segments[0].ident == "std" || segments[0].ident == "core")
            && segments[1].ident == "result"
            && segment.ident == "Result"
        {
            parse_result_type(segment)?
        } else if (segments[0].ident == "std" || segments[0].ident == "alloc")
            && segments[1].ident == "vec"
            && segment.ident == "Vec"
        {
            parse_vector_type(segment)?
        } else {
            return Err(syn::Error::new_spanned(value, "unsupported type"));
        };

        return Ok(Type {
            ty,
            kind,
            ts,
            is_result,
        });
    }

    Err(syn::Error::new_spanned(value, "unsupported type"))
}

fn parse_type_reference(input: &syn::TypeReference) -> syn::Result<Type> {
    let elem = &*input.elem;
    let is_result = false;
    match elem {
        syn::Type::Path(ty) => {
            let path = &ty.path;
            let segments = &path.segments;
            if ty.qself.is_none() && path.leading_colon.is_none() && segments.len() == 1 {
                let segment = &segments[0];
                // &str (not &mut str)
                if segment.ident == "str"
                    && segment.arguments.is_none()
                    && input.mutability.is_none()
                {
                    let ty = Box::new(syn::Type::Reference(input.clone()));
                    let kind = TypeKind::StringBorrowed;
                    let ts = typescript::string;

                    return Ok(Type {
                        ty,
                        kind,
                        ts,
                        is_result,
                    });
                }
            }
        }
        syn::Type::Slice(ty) => {
            if let syn::Type::Path(elem) = &*ty.elem {
                let segments = &elem.path.segments;
                // &[xx] and &mut [xx]
                if elem.qself.is_none() && segments.len() == 1 {
                    let ident = segments[0].ident.to_string();
                    let (elem_type, ts) = match ident.as_str() {
                        "i8" => (TypeNative::I8, typescript::Int8Array),
                        "i16" => (TypeNative::I16, typescript::Int16Array),
                        "i32" => (TypeNative::I32, typescript::Int32Array),
                        "i64" => (TypeNative::I64, typescript::BigInt64Array),
                        "u8" => (TypeNative::U8, typescript::Uint8Array),
                        "u16" => (TypeNative::U16, typescript::Uint16Array),
                        "u32" => (TypeNative::U32, typescript::Uint32Array),
                        "u64" => (TypeNative::U64, typescript::BigUint64Array),
                        "f32" => (TypeNative::F32, typescript::Float32Array),
                        "f64" => (TypeNative::F64, typescript::Float64Array),
                        _ => return Err(syn::Error::new_spanned(input, "unsupported type")),
                    };

                    let ty = Box::new(syn::Type::Reference(input.clone()));
                    let kind = if input.mutability.is_none() {
                        TypeKind::BufferBorrowed(elem_type)
                    } else {
                        TypeKind::BufferBorrowedMut(elem_type)
                    };

                    return Ok(Type {
                        ty,
                        kind,
                        ts,
                        is_result,
                    });
                }
            }
        }
        _ => {}
    }

    Err(syn::Error::new_spanned(input, "unsupported type"))
}

fn parse_pointer_type(ident: &syn::PathSegment) -> syn::Result<(TypeKind, typescript::Type)> {
    assert!(ident.ident == "Box");

    let arguments = &ident.arguments;
    if let syn::PathArguments::AngleBracketed(arguments) = arguments {
        let args = &arguments.args;
        if args.len() == 1 {
            match &args[0] {
                // Box<str>
                syn::GenericArgument::Type(syn::Type::Path(ty)) => {
                    let segments = &ty.path.segments;
                    if ty.qself.is_none() && segments.len() == 1 && segments[0].ident == "str" {
                        return Ok((TypeKind::StringOwned, typescript::string));
                    }
                }
                // Box<[xx]>
                syn::GenericArgument::Type(syn::Type::Slice(ty)) => {
                    if let syn::Type::Path(elem) = &*ty.elem {
                        let segments = &elem.path.segments;
                        if elem.qself.is_none() && segments.len() == 1 {
                            let ident = segments[0].ident.to_string();
                            let (elem_type, ts) = match ident.as_str() {
                                "i8" => (TypeNative::I8, typescript::Int8Array),
                                "i16" => (TypeNative::I16, typescript::Int16Array),
                                "i32" => (TypeNative::I32, typescript::Int32Array),
                                "i64" => (TypeNative::I64, typescript::BigInt64Array),
                                "u8" => (TypeNative::U8, typescript::Uint8Array),
                                "u16" => (TypeNative::U16, typescript::Uint16Array),
                                "u32" => (TypeNative::U32, typescript::Uint32Array),
                                "u64" => (TypeNative::U64, typescript::BigUint64Array),
                                "f32" => (TypeNative::I32, typescript::Float32Array),
                                "f64" => (TypeNative::I64, typescript::Float64Array),
                                _ => {
                                    return Err(syn::Error::new_spanned(ident, "unsupported type"))
                                }
                            };
                            return Ok((TypeKind::BufferOwned(elem_type), ts));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Err(syn::Error::new_spanned(ident, "unsupported type"))
}

fn parse_option_type(value: &syn::PathSegment) -> syn::Result<(TypeKind, typescript::Type)> {
    assert!(value.ident == "Option");

    let arguments = &value.arguments;
    if let syn::PathArguments::AngleBracketed(arguments) = arguments {
        let args = &arguments.args;
        if args.len() == 1 {
            // Option<T> (where T is a valid type)
            if let syn::GenericArgument::Type(syn::Type::Path(ty)) = &args[0] {
                if let Ok(elem) = parse_type_path(ty) {
                    return Ok((TypeKind::Json, elem.ts | typescript::null));
                }
            }
        }
    }
    Err(syn::Error::new_spanned(value, "unsupported type"))
}

fn parse_result_type(value: &syn::PathSegment) -> syn::Result<(TypeKind, typescript::Type)> {
    assert!(value.ident == "Result");

    let arguments = &value.arguments;
    if let syn::PathArguments::AngleBracketed(arguments) = arguments {
        let args = &arguments.args;
        if args.len() == 2 {
            // Result<T, E> (where both T and E are a valid types)
            if let (
                syn::GenericArgument::Type(syn::Type::Path(ok_ty)),
                syn::GenericArgument::Type(syn::Type::Path(err_ty)),
            ) = (&args[0], &args[1])
            {
                if let Ok(ok_ty) = parse_type_path(ok_ty) {
                    if parse_type_path(err_ty).is_ok() {
                        return Ok((TypeKind::Json, ok_ty.ts));
                    }
                }
            }
        }
    }
    Err(syn::Error::new_spanned(value, "unsupported type"))
}

fn parse_vector_type(value: &syn::PathSegment) -> syn::Result<(TypeKind, typescript::Type)> {
    assert!(value.ident == "Vec");

    let arguments = &value.arguments;
    if let syn::PathArguments::AngleBracketed(arguments) = arguments {
        let args = &arguments.args;
        if args.len() == 1 {
            if let syn::GenericArgument::Type(syn::Type::Path(ty)) = &args[0] {
                let segments = &ty.path.segments;
                // Vec<u8>
                if ty.qself.is_none() && segments.len() == 1 {
                    let ident = segments[0].ident.to_string();
                    let (elem_type, ts) = match ident.as_str() {
                        "i8" => (TypeNative::I8, typescript::Int8Array),
                        "i16" => (TypeNative::I16, typescript::Int16Array),
                        "i32" => (TypeNative::I32, typescript::Int32Array),
                        "i64" => (TypeNative::I64, typescript::BigInt64Array),
                        "u8" => (TypeNative::U8, typescript::Uint8Array),
                        "u16" => (TypeNative::U16, typescript::Uint16Array),
                        "u32" => (TypeNative::U32, typescript::Uint32Array),
                        "u64" => (TypeNative::U64, typescript::BigUint64Array),
                        "f32" => (TypeNative::I32, typescript::Float32Array),
                        "f64" => (TypeNative::I64, typescript::Float64Array),
                        _ => {
                            // Vec<T> (where T is a valid type)
                            if let Ok(elem) = parse_type_path(ty) {
                                return Ok((TypeKind::Json, elem.ts.array()));
                            } else {
                                return Err(syn::Error::new_spanned(ident, "unsupported type"));
                            }
                        }
                    };
                    return Ok((TypeKind::BufferOwned(elem_type), ts));
                }
            }
        }
    }
    Err(syn::Error::new_spanned(value, "unsupported type"))
}

fn visibility_pub(vis: &Visibility, span: Span) -> Token![pub] {
    Token![pub](match vis {
        Visibility::Public(vis) => vis.span(),
        Visibility::Restricted(vis) => vis.pub_token.span(),
        Visibility::Inherited => span,
    })
}
