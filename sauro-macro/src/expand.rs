use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::spanned::Spanned;

use crate::syntax::{Field, FnArg, Function, Item, Module, ReturnType, Struct, Type};

pub fn bindgen(input: Module) -> TokenStream {
    let attrs = input.attrs;
    let attrs = quote!(#(#attrs)*);
    let vis = input.vis;
    let mod_token = input.mod_token;
    let ident = input.ident;

    let span = input.brace_token.span;
    let items = input.items.into_iter().map(expand_item);
    let expanded = quote_spanned!(span => {#(#items)*});

    quote! {
        #attrs
        #vis #mod_token #ident #expanded
    }
}

fn expand_item(input: Item) -> TokenStream {
    match &input {
        Item::Struct(input) => expand_struct(input),
        Item::Function(input) => expand_function(input),
    }
}

fn expand_struct(input: &Struct) -> TokenStream {
    let attrs = input.attrs.iter();
    let vis = &input.vis;
    let struct_token = &input.struct_token;
    let ident = &input.ident;

    let expanded = {
        let span = input.brace_token.span;
        let fields = input.fields.iter();
        quote_spanned!(span => {#(#fields),*})
    };

    quote! {
        #(#attrs)*
        #[derive(serde::Serialize, serde::Deserialize)]
        #vis #struct_token #ident #expanded
    }
}

fn expand_function(input: &Function) -> TokenStream {
    let vis = &input.vis;
    let sig = {
        let unsafety = quote!(unsafe);
        let abi = quote!(extern "C");
        let fn_token = &input.sig.fn_token;
        let ident = &input.sig.ident;

        let inputs = {
            let span = input.sig.inputs.span();
            let inputs = input.sig.inputs.iter().enumerate().map(BindingFnArg);
            quote_spanned!(span => (#(#inputs),*))
        };
        let output = BindingReturnType(&input.sig.output);

        quote!(#unsafety #abi #fn_token #ident #inputs #output)
    };

    let impl_fn = ImplFunction(input);

    let overrides = input
        .sig
        .inputs
        .iter()
        .enumerate()
        .map(BindingFnArgOverride);

    let inputs_ident = input.sig.inputs.iter().map(|arg| &arg.ident);

    let return_fn = BindingReturnStmt(&input.sig.output);

    quote! {
        #[no_mangle]
        #vis #sig {
            #impl_fn
            #(#overrides)*
            let __inner_res = __inner_impl(#(#inputs_ident),*);
            #return_fn(__inner_res)
        }
    }
}

impl ToTokens for Field {
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

struct ImplFunction<'a>(&'a Function);

impl<'a> ToTokens for ImplFunction<'a> {
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
            Type::Native(ty) => {
                let ident = format_ident!("__arg{}", index);
                tokens.extend(quote_spanned!(span => #ident #colon_token #ty))
            }
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
            Type::Json(ty) => {
                quote_spanned! {span =>
                    let #ident: #ty = {
                        let buf = unsafe {
                            ::std::slice::from_raw_parts(#ident_ptr, #ident_len)
                        };
                        sauro::serde_json::from_slice(buf).expect("failed to deserialize binding arguments")
                    };
                }
            }
            Type::OwnedString(_) => {
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
            Type::BorrowedString(_) => {
                quote_spanned! {span =>
                    let #ident = {
                        let buf = unsafe {
                            ::std::slice::from_raw_parts(#ident_ptr, #ident_len)
                        };
                        ::std::str::from_utf8(buf).expect("failed to deserialize string")
                    };
                }
            }
            Type::OwnedBuffer(ty) => {
                quote_spanned! {span =>
                    let mut #ident: #ty = {
                        let buf = unsafe {
                            ::std::slice::from_raw_parts_mut(#ident_ptr, #ident_len)
                        };
                        buf.to_owned().into()
                    };
                }
            }
            Type::BorrowedBuffer(ty) if ty.mutability.is_some() => {
                quote_spanned! {span =>
                    let mut #ident = unsafe {
                        ::std::slice::from_raw_parts_mut(#ident_ptr, #ident_len)
                    };
                }
            }
            Type::BorrowedBuffer(_) => {
                quote_spanned! {span =>
                    let #ident = unsafe {
                        ::std::slice::from_raw_parts(#ident_ptr, #ident_len)
                    };
                }
            }
        };

        tokens.extend(expand);
    }
}

impl ToTokens for Type {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Native(ident) => ident.to_tokens(tokens),
            Self::Json(ty) | Self::OwnedString(ty) | Self::OwnedBuffer(ty) => ty.to_tokens(tokens),
            Self::BorrowedString(ty) => ty.to_tokens(tokens),
            Self::BorrowedBuffer(ty) => ty.to_tokens(tokens),
        }
    }
}

impl ToTokens for ReturnType {
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
                    // use length-prefixed buffer to return structs and strings
                    Type::Json(_) => tokens.extend(quote!{(|x: #ty| {
                        let json = sauro::serde_json::to_string(&x).expect("failed to serialize binding result");

                        let encoded_json = json.into_bytes();
                        let encoded_length = (encoded_json.len() as u32).to_be_bytes();

                        let mut buffer = encoded_length.to_vec();
                        buffer.extend(encoded_json);

                        let ptr = buffer.as_ptr();
                        ::std::mem::forget(buffer);
                        ptr
                    })}),
                    Type::OwnedString(_) => tokens.extend(quote!{(|x: #ty| {
                        let encoded_str = x.as_bytes();
                        let encoded_length = (encoded_str.len() as u32).to_be_bytes();

                        let mut buffer = encoded_length.to_vec();
                        buffer.extend(encoded_str);

                        let ptr = buffer.as_ptr();
                        ::std::mem::forget(buffer);
                        ptr
                    })}),
                    Type::OwnedBuffer(_) => tokens.extend(quote!{(|x: #ty| {
                        let encoded_length = (x.len() as u32).to_be_bytes();

                        let mut buffer = encoded_length.to_vec();
                        buffer.extend(x);

                        let ptr = buffer.as_ptr();
                        ::std::mem::forget(buffer);
                        ptr
                    })}),
                    _ => unreachable!("unsupported return type")
                }
            }
        }
    }
}
