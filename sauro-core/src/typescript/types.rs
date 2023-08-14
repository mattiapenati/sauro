#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Primitive(TypePrimitive),
    Named(TypeNamed),
    Union(TypeUnion),
    Array(TypeArray),
}

impl Type {
    pub fn array(self) -> Self {
        Self::Array(TypeArray {
            members: Box::new(self),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypePrimitive(&'static str);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeNamed {
    pub name: Box<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeUnion {
    members: Vec<Type>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeArray {
    pub members: Box<Type>,
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Primitive(ty) => write!(f, "{}", ty),
            Self::Named(ty) => write!(f, "{}", ty),
            Self::Union(ty) => write!(f, "{}", ty),
            Self::Array(ty) => write!(f, "{}", ty),
        }
    }
}

impl std::fmt::Display for TypePrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for TypeNamed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl std::fmt::Display for TypeUnion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut types = self.members.iter();
        if let Some(first) = types.next() {
            write!(f, "{}", first)?;
            for next in types {
                write!(f, " | {}", next)?;
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for TypeArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self.members {
            Type::Union(ty) if ty.members.len() > 1 => {
                write!(f, "({})[]", ty)
            }
            ty => write!(f, "{}[]", ty),
        }
    }
}

impl std::ops::BitOr for Type {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        let mut members = vec![];

        match self {
            Self::Union(lhs) => members.extend(lhs.members),
            lhs => members.push(lhs),
        }

        match rhs {
            Self::Union(rhs) => members.extend(rhs.members),
            rhs => members.push(rhs),
        }

        Self::Union(TypeUnion { members })
    }
}

macro_rules! primitive_types {
    ($($name:ident),+ $(,)?) => {
        $(
            #[allow(non_upper_case_globals)]
            pub const $name: Type = Type::Primitive(TypePrimitive(::std::stringify!($name)));
        )*
    }
}

primitive_types! {
    null,
    undefined,
    number,
    bigint,
    string,
    Uint8Array,
}
