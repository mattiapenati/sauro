use proc_macro2::Span;
use syn::{
    punctuated::Punctuated, spanned::Spanned, GenericArgument, Pat, PathArguments, Token,
    Visibility,
};

use super::{
    Field, FnArg, Item, ItemFn, ItemStruct, Module, ReturnType, Signature, Type, TypeBuffer,
    TypeNative, TypeString,
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
        let return_type = match ty {
            Type::Native(_)
            | Type::Struct(_)
            | Type::String(TypeString::Owned(_))
            | Type::Buffer(TypeBuffer::Owned(_)) => ReturnType::Type(rarrow, ty),
            _ => return Err(syn::Error::new_spanned(value, "unsupported return type")),
        };
        Ok(return_type)
    }
}

impl TryFrom<&syn::TypePath> for TypeNative {
    type Error = syn::Error;

    fn try_from(value: &syn::TypePath) -> syn::Result<Self> {
        let path = &value.path;
        if value.qself.is_some() || path.leading_colon.is_some() || path.segments.len() != 1 {
            return Err(syn::Error::new_spanned(value, "unsupported type"));
        }

        let ident = path.segments[0].ident.clone();
        let res = match ident.to_string().as_str() {
            "i8" => Self::I8(ident),
            "i16" => Self::I16(ident),
            "i32" => Self::I32(ident),
            "i64" => Self::I64(ident),
            "isize" => Self::ISize(ident),
            "u8" => Self::U8(ident),
            "u16" => Self::U16(ident),
            "u32" => Self::U32(ident),
            "u64" => Self::U64(ident),
            "usize" => Self::USize(ident),
            "f32" => Self::F32(ident),
            "f64" => Self::F64(ident),
            _ => return Err(syn::Error::new_spanned(value, "unsupported type")),
        };

        Ok(res)
    }
}

impl TryFrom<&syn::TypePath> for TypeBuffer {
    type Error = syn::Error;

    fn try_from(value: &syn::TypePath) -> syn::Result<Self> {
        if value.qself.is_none() {
            let path = &value.path;
            let segments = &path.segments;

            let is_box = (segments.len() == 1 && segments[0].ident == "Box")
                || (segments.len() == 3
                    && (segments[0].ident == "alloc" || segments[0].ident == "std")
                    && segments[1].ident == "boxed"
                    && segments[2].ident == "Box");
            if is_box {
                let segment = segments.last().unwrap();
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(syn::Type::Slice(box_ty))) = args.args.first()
                    {
                        if let syn::Type::Path(elem_ty) = &*box_ty.elem {
                            if let Some(segment) = elem_ty.path.segments.first() {
                                if segment.ident == "u8" {
                                    return Ok(Self::Owned(value.clone()));
                                }
                            }
                        }
                    }
                }
            }

            let is_vec = (segments.len() == 1 && segments[0].ident == "Vec")
                || (segments.len() == 3
                    && (segments[0].ident == "alloc" || segments[0].ident == "std")
                    && segments[1].ident == "vec"
                    && segments[2].ident == "Vec");
            if is_vec {
                let segment = segments.last().unwrap();
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(syn::Type::Path(vec_ty))) = args.args.first()
                    {
                        if let Some(segment) = vec_ty.path.segments.first() {
                            let ident = &segment.ident;

                            if *ident == "u8" {
                                return Ok(Self::Owned(value.clone()));
                            }
                        }
                    }
                }
            }
        }

        Err(syn::Error::new_spanned(value, "unsupported type"))
    }
}

impl TryFrom<&syn::TypeReference> for TypeBuffer {
    type Error = syn::Error;

    fn try_from(value: &syn::TypeReference) -> syn::Result<Self> {
        if value.lifetime.is_none() {
            if let syn::Type::Slice(ty) = value.elem.as_ref() {
                if let syn::Type::Path(ty) = ty.elem.as_ref() {
                    let path = &ty.path;
                    if ty.qself.is_none()
                        && path.leading_colon.is_none()
                        && path.segments.len() == 1
                    {
                        let segment = &path.segments[0];
                        let ident = &segment.ident;
                        if segment.arguments.is_none() && ident == "u8" {
                            return Ok(TypeBuffer::Borrowed(value.clone()));
                        }
                    }
                }
            }
        }

        Err(syn::Error::new_spanned(value, "unsupported type"))
    }
}

impl TryFrom<&syn::TypePath> for TypeString {
    type Error = syn::Error;

    fn try_from(value: &syn::TypePath) -> syn::Result<Self> {
        if value.qself.is_none() {
            let path = &value.path;
            let segments = &path.segments;

            let is_box = (segments.len() == 1 && segments[0].ident == "Box")
                || (segments.len() == 3
                    && (segments[0].ident == "alloc" || segments[0].ident == "std")
                    && segments[1].ident == "boxed"
                    && segments[2].ident == "Box");
            if is_box {
                let segment = segments.last().unwrap();
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(syn::Type::Slice(box_ty))) = args.args.first()
                    {
                        if let syn::Type::Path(elem_ty) = box_ty.elem.as_ref() {
                            if let Some(segment) = elem_ty.path.segments.first() {
                                if segment.ident == "str" {
                                    return Ok(Self::Owned(value.clone()));
                                }
                            }
                        }
                    }
                }
            }

            let is_string = (segments.len() == 1 && segments[0].ident == "String")
                || (segments.len() == 3
                    && (segments[0].ident == "alloc" || segments[0].ident == "std")
                    && segments[1].ident == "string"
                    && segments[2].ident == "String");
            if is_string {
                return Ok(Self::Owned(value.clone()));
            }
        }

        Err(syn::Error::new_spanned(value, "unsupported type"))
    }
}

impl TryFrom<&syn::TypeReference> for TypeString {
    type Error = syn::Error;

    fn try_from(value: &syn::TypeReference) -> syn::Result<Self> {
        if value.lifetime.is_none() {
            if let syn::Type::Path(ty) = value.elem.as_ref() {
                let path = &ty.path;
                if ty.qself.is_none() && path.leading_colon.is_none() && path.segments.len() == 1 {
                    let segment = &path.segments[0];
                    let ident = &segment.ident;
                    if segment.arguments.is_none() && ident == "str" {
                        return Ok(TypeString::Borrowed(value.clone()));
                    }
                }
            }
        }

        Err(syn::Error::new_spanned(value, "unsupported type"))
    }
}

impl TryFrom<&syn::Type> for Type {
    type Error = syn::Error;

    fn try_from(value: &syn::Type) -> syn::Result<Self> {
        let res = match value {
            syn::Type::Path(ty) => TypeNative::try_from(ty)
                .map(Type::Native)
                .or_else(|_| TypeBuffer::try_from(ty).map(Type::Buffer))
                .or_else(|_| TypeString::try_from(ty).map(Type::String))
                .unwrap_or_else(|_| Type::Struct(ty.clone())),
            syn::Type::Reference(ty) => TypeBuffer::try_from(ty)
                .map(Type::Buffer)
                .or_else(|_| TypeString::try_from(ty).map(Type::String))?,
            _ => return Err(syn::Error::new_spanned(value, "unsupported type")),
        };
        Ok(res)
    }
}

fn visibility_pub(vis: &Visibility, span: Span) -> Token![pub] {
    Token![pub](match vis {
        Visibility::Public(vis) => vis.span(),
        Visibility::Restricted(vis) => vis.pub_token.span(),
        Visibility::Inherited => span,
    })
}
