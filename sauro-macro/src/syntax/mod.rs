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
    /// Native types are passed as is
    Native(Ident),
    /// Struct and enum are passed using json serialization
    Json(Ident),
    /// String
    String(Ident),
    /// Str
    Str { and_token: Token![&], ident: Ident },
}

pub enum ReturnType {
    Default,
    Type(Token![->], Type),
}
