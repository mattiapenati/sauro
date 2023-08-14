mod parse;

use syn::{punctuated::Punctuated, token, Attribute, Block, Ident, Token};

use crate::typescript;

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

pub struct Type {
    pub ty: Box<syn::Type>,
    pub kind: TypeKind,
    pub ts: typescript::Type,
}

pub enum TypeKind {
    BufferBorrowed,
    BufferBorrowedMut,
    BufferOwned,
    Json,
    Native(TypeNative),
    StringBorrowed,
    StringOwned,
}

#[derive(Clone, Copy)]
pub enum TypeNative {
    I8,
    I16,
    I32,
    I64,
    ISize,
    U8,
    U16,
    U32,
    U64,
    USize,
    F32,
    F64,
}

impl TypeNative {
    pub fn symbol(&self) -> &'static str {
        match self {
            TypeNative::I8 => "i8",
            TypeNative::I16 => "i16",
            TypeNative::I32 => "i32",
            TypeNative::I64 => "i64",
            TypeNative::ISize => "isize",
            TypeNative::U8 => "u8",
            TypeNative::U16 => "u16",
            TypeNative::U32 => "u32",
            TypeNative::U64 => "u64",
            TypeNative::USize => "usize",
            TypeNative::F32 => "f32",
            TypeNative::F64 => "f64",
        }
    }
}
