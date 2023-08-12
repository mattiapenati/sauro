use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::spanned::Spanned;

use crate::syntax::{
    Field, FnArg, Item, ItemFn, ItemStruct, Module, ReturnType, Type, TypeBuffer, TypeNative,
    TypeOption, TypeString,
};

pub fn bindgen(input: Module) -> TokenStream {
    let attrs = input.attrs;
    let attrs = quote!(#(#attrs)*);
    let vis = input.vis;
    let mod_token = input.mod_token;
    let ident = input.ident;

    let span = input.brace_token.span;
    let items = input
        .items
        .into_iter()
        .map(quote::ToTokens::into_token_stream);
    let expanded = quote_spanned!(span => {#(#items)*});

    quote! {
        #attrs
        #vis #mod_token #ident #expanded
    }
}

impl quote::ToTokens for Item {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match &self {
            Item::Fn(input) => input.to_tokens(tokens),
            Item::Struct(input) => input.to_tokens(tokens),
        }
    }
}

impl quote::ToTokens for ItemStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let attrs = self.attrs.iter();
        let vis = &self.vis;
        let struct_token = &self.struct_token;
        let ident = &self.ident;

        let expanded = {
            let span = self.brace_token.span;
            let fields = self.fields.iter();
            quote_spanned!(span => {#(#fields),*})
        };

        tokens.extend(quote! {
            #(#attrs)*
            #[derive(::sauro::serde::Serialize, ::sauro::serde::Deserialize)]
            #[serde(crate = "::sauro::serde")]
            #vis #struct_token #ident #expanded
        })
    }
}

impl quote::ToTokens for ItemFn {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let vis = &self.vis;
        let sig = {
            let fn_token = {
                let span = self.sig.fn_token.span();
                let unsafety = quote!(unsafe);
                let abi = quote!(extern "C");
                let fn_token = &self.sig.fn_token;
                quote_spanned!(span => #unsafety #abi #fn_token)
            };
            let ident = &self.sig.ident;

            let inputs = {
                let span = self.sig.paren_token.span;
                let inputs = self.sig.inputs.iter().enumerate().map(BindingFnArg);
                quote_spanned!(span => (#(#inputs),*))
            };
            let output = BindingReturnType(&self.sig.output);

            quote!(#fn_token #ident #inputs #output)
        };

        let fn_inner_impl = FnInnerImpl(self);

        let overrides = self.sig.inputs.iter().enumerate().map(BindingFnArgOverride);

        let inputs_ident = self.sig.inputs.iter().map(|arg| &arg.ident);

        let return_fn = BindingReturnStmt(&self.sig.output);

        tokens.extend(quote! {
            #[no_mangle]
            #vis #sig {
                #fn_inner_impl
                #(#overrides)*
                let __inner_res = __inner_impl(#(#inputs_ident),*);
                #return_fn(__inner_res)
            }
        })
    }
}

impl quote::ToTokens for Field {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let attrs = self.attrs.iter();
        let vis = &self.vis;
        let ident = &self.ident;
        let colon_token = &self.colon_token;
        let ty = &self.ty;

        tokens.extend(quote! {
            #(#attrs)*
            #vis #ident #colon_token #ty
        })
    }
}

struct FnInnerImpl<'a>(&'a ItemFn);

impl<'a> ToTokens for FnInnerImpl<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let input = self.0;

        let attrs = &input.attrs;
        let fn_token = &input.sig.fn_token;
        let ident = {
            let span = input.sig.ident.span();
            quote_spanned!(span => __inner_impl)
        };

        let inputs = {
            let span = input.sig.paren_token.span;
            let inputs = input.sig.inputs.iter();
            quote_spanned!(span => (#(#inputs),*))
        };

        let output = &input.sig.output;

        let block = &input.block;

        tokens.extend(quote! {
            #(#attrs)*
            #fn_token #ident #inputs #output
            #block
        })
    }
}

impl ToTokens for FnArg {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mutability = &self.mutability;
        let ident = &self.ident;
        let colon_token = &self.colon_token;
        let ty = &self.ty;

        tokens.extend(quote!(#mutability #ident #colon_token #ty))
    }
}

struct BindingFnArg<'a>((usize, &'a FnArg));

impl<'a> ToTokens for BindingFnArg<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let (index, input) = self.0;

        let span = input.span();
        let colon_token = &input.colon_token;

        match &input.ty {
            Type::Native(ty) => match ty {
                TypeNative::I8(ty)
                | TypeNative::I16(ty)
                | TypeNative::I32(ty)
                | TypeNative::I64(ty)
                | TypeNative::ISize(ty)
                | TypeNative::U8(ty)
                | TypeNative::U16(ty)
                | TypeNative::U32(ty)
                | TypeNative::U64(ty)
                | TypeNative::USize(ty)
                | TypeNative::F32(ty)
                | TypeNative::F64(ty) => {
                    let ident = format_ident!("__arg{}", index);
                    tokens.extend(quote_spanned!(span => #ident #colon_token #ty))
                }
            },
            // everything else is passed as a pair (pointer, length)
            _ => {
                let ident_ptr = format_ident!("__arg{}_ptr", index);
                let ident_len = format_ident!("__arg{}_len", index);
                tokens.extend(quote_spanned! {span =>
                    #ident_ptr #colon_token *mut u8,
                    #ident_len #colon_token usize
                })
            }
        }
    }
}

struct BindingFnArgOverride<'a>((usize, &'a FnArg));

impl<'a> ToTokens for BindingFnArgOverride<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let (index, input) = self.0;

        let span = input.span();
        let ident = &input.ident;

        let ident_arg = format_ident!("__arg{}", index);
        let ident_ptr = format_ident!("__arg{}_ptr", index);
        let ident_len = format_ident!("__arg{}_len", index);

        let expand = match &input.ty {
            Type::Native(_) => quote_spanned!(span => let #ident = #ident_arg;),
            Type::Buffer(TypeBuffer::Owned(ty)) => {
                quote_spanned! {span =>
                    let mut #ident: #ty = {
                        let buf = unsafe {
                            ::std::slice::from_raw_parts_mut(#ident_ptr, #ident_len)
                        };
                        buf.to_owned().into()
                    };
                }
            }
            Type::Buffer(TypeBuffer::Borrowed(ty)) if ty.mutability.is_some() => {
                quote_spanned! {span =>
                    let mut #ident = unsafe {
                        ::std::slice::from_raw_parts_mut(#ident_ptr, #ident_len)
                    };
                }
            }
            Type::Buffer(TypeBuffer::Borrowed(_)) => {
                quote_spanned! {span =>
                    let #ident = unsafe {
                        ::std::slice::from_raw_parts(#ident_ptr, #ident_len)
                    };
                }
            }
            Type::String(TypeString::Owned(_)) => {
                quote_spanned! {span =>
                    let #ident = {
                        let buf = unsafe {
                            ::std::slice::from_raw_parts(#ident_ptr, #ident_len)
                        };
                        let buf = buf.to_vec();
                        ::std::string::String::from_utf8(buf).expect("failed to deserialize string")
                    };
                }
            }
            Type::String(TypeString::Borrowed(_)) => {
                quote_spanned! {span =>
                    let #ident = {
                        let buf = unsafe {
                            ::std::slice::from_raw_parts(#ident_ptr, #ident_len)
                        };
                        ::std::str::from_utf8(buf).expect("failed to deserialize string")
                    };
                }
            }
            Type::Option(ty) => {
                let ty = &ty.ty;
                quote_spanned! {span =>
                    let #ident: #ty = {
                        let buf = unsafe {
                            ::std::slice::from_raw_parts(#ident_ptr, #ident_len)
                        };
                        sauro::serde_json::from_slice(buf).expect("failed to deserialize binding arguments")
                    };
                }
            }
            Type::Struct(ty) => {
                quote_spanned! {span =>
                    let #ident: #ty = {
                        let buf = unsafe {
                            ::std::slice::from_raw_parts(#ident_ptr, #ident_len)
                        };
                        sauro::serde_json::from_slice(buf).expect("failed to deserialize binding arguments")
                    };
                }
            }
        };

        tokens.extend(expand);
    }
}

impl quote::ToTokens for Type {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Native(ty) => ty.to_tokens(tokens),
            Self::Buffer(ty) => ty.to_tokens(tokens),
            Self::String(ty) => ty.to_tokens(tokens),
            Self::Option(ty) => ty.to_tokens(tokens),
            Self::Struct(ty) => ty.to_tokens(tokens),
        }
    }
}

impl quote::ToTokens for TypeNative {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::I8(ident)
            | Self::I16(ident)
            | Self::I32(ident)
            | Self::I64(ident)
            | Self::ISize(ident)
            | Self::U8(ident)
            | Self::U16(ident)
            | Self::U32(ident)
            | Self::U64(ident)
            | Self::USize(ident)
            | Self::F32(ident)
            | Self::F64(ident) => ident.to_tokens(tokens),
        }
    }
}

impl quote::ToTokens for TypeBuffer {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Owned(ty) => ty.to_tokens(tokens),
            Self::Borrowed(ty) => ty.to_tokens(tokens),
        }
    }
}

impl quote::ToTokens for TypeString {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Owned(ty) => ty.to_tokens(tokens),
            Self::Borrowed(ty) => ty.to_tokens(tokens),
        }
    }
}

impl quote::ToTokens for TypeOption {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.ty.to_tokens(tokens);
    }
}

impl quote::ToTokens for ReturnType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Self::Type(rarrow, ty) = self {
            tokens.extend(quote!(#rarrow #ty))
        }
    }
}

struct BindingReturnType<'a>(&'a ReturnType);

impl<'a> ToTokens for BindingReturnType<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let input = self.0;
        if let ReturnType::Type(rarrow, ty) = input {
            match ty {
                Type::Native(ty) => tokens.extend(quote!(#rarrow #ty)),
                _ => tokens.extend(quote!(#rarrow *const u8 )),
            }
        }
    }
}

struct BindingReturnStmt<'a>(&'a ReturnType);

impl<'a> ToTokens for BindingReturnStmt<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let input = self.0;

        match input {
            ReturnType::Default => tokens.extend(quote!((|_| ()))),
            ReturnType::Type(_, ty) => {
                match ty {
                    Type::Native(_) => tokens.extend(quote!((|x: #ty| x))),
                    Type::Buffer(TypeBuffer::Owned(_)) => tokens.extend(quote!{(|x: #ty| {
                        let encoded_length = (x.len() as u32).to_be_bytes();

                        let mut buffer = encoded_length.to_vec();
                        buffer.extend(x);

                        let ptr = buffer.as_ptr();
                        ::std::mem::forget(buffer);
                        ptr
                    })}),
                    Type::String(TypeString::Owned(_)) => tokens.extend(quote!{(|x: #ty| {
                        let encoded_str = x.as_bytes();
                        let encoded_length = (encoded_str.len() as u32).to_be_bytes();

                        let mut buffer = encoded_length.to_vec();
                        buffer.extend(encoded_str);

                        let ptr = buffer.as_ptr();
                        ::std::mem::forget(buffer);
                        ptr
                    })}),
                    Type::Struct(_) | Type::Option(_) => tokens.extend(quote!{(|x: #ty| {
                        let json = sauro::serde_json::to_string(&x).expect("failed to serialize binding result");

                        let encoded_json = json.into_bytes();
                        let encoded_length = (encoded_json.len() as u32).to_be_bytes();

                        let mut buffer = encoded_length.to_vec();
                        buffer.extend(encoded_json);

                        let ptr = buffer.as_ptr();
                        ::std::mem::forget(buffer);
                        ptr
                    })}),
                    Type::Buffer(TypeBuffer::Borrowed(_)) | Type::String(TypeString::Borrowed(_)) => unreachable!("unsupported return type")
                }
            }
        }
    }
}
