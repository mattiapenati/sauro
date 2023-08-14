mod types;

pub use self::types::*;

macro_rules! Type {
    [$name:ident] => {
        $crate::typescript::Type::Named($crate::typescript::TypeNamed {
            name: $name.into(),
        })
    }
}

pub(crate) use Type;
