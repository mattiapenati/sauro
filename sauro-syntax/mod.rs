mod parse;

use syn::{punctuated::Punctuated, token, Attribute, Block, Ident, Token};

pub use self::parse::parse_module;

pub struct Module {
    pub attrs: Vec<Attribute>,
    pub vis: Token![pub],
    pub mod_token: Token![mod],
    pub ident: Ident,
    pub brace_token: token::Brace,
    pub items: Vec<Item>,
}

pub enum Item {
    Fn(ItemFn),
    Struct(ItemStruct),
}

pub struct ItemStruct {
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

pub struct ItemFn {
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

pub enum ReturnType {
    Default,
    Type(Token![->], Type),
}

pub enum Type {
    Native(TypeNative),
    Buffer(TypeBuffer),
    String(TypeString),
    Option(TypeOption),
    Struct(syn::TypePath),
}

pub enum TypeNative {
    I8(Ident),
    I16(Ident),
    I32(Ident),
    I64(Ident),
    ISize(Ident),
    U8(Ident),
    U16(Ident),
    U32(Ident),
    U64(Ident),
    USize(Ident),
    F32(Ident),
    F64(Ident),
}

pub enum TypeBuffer {
    Borrowed(syn::TypeReference),
    Owned(syn::TypePath),
}

pub enum TypeString {
    Borrowed(syn::TypeReference),
    Owned(syn::TypePath),
}

pub struct TypeOption {
    pub ty: syn::TypePath,
    pub argument: Box<Type>,
}
