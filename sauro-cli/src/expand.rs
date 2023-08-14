use sauro_core::syntax;

pub fn expand_module(
    module: &syntax::Module,
    dylib_name: &str,
    dylib_prefix: &str,
) -> anyhow::Result<String> {
    use std::fmt::Write;

    let mut source = String::new();

    let mut structs = String::new();
    let mut functions = String::new();
    let mut utilities = Utilities::default();

    for item in &module.items {
        let item_utilities = match item {
            syntax::Item::Fn(func) => expand_function(&mut functions, func)?,
            syntax::Item::Struct(strct) => expand_struct(&mut structs, strct)?,
        };
        utilities.merge(item_utilities);
    }

    write!(&mut source, "{}", structs)?;
    write!(&mut source, "{}", functions)?;
    utilities.expand(&mut source)?;

    // import external library
    let functions = module.items.iter().filter_map(|item| match item {
        syntax::Item::Fn(func) => Some(func),
        _ => None,
    });
    expand_symbols(&mut source, functions, dylib_name, dylib_prefix)?;

    Ok(source)
}

fn expand_struct(
    out: &mut impl std::fmt::Write,
    strct: &syntax::ItemStruct,
) -> Result<Utilities, std::fmt::Error> {
    writeln!(out, "export type {} = {{", strct.ident)?;
    for field in &strct.fields {
        write!(out, "  {}: ", field.ident)?;
        expand_type(out, &field.ty)?;
        writeln!(out, ";")?;
    }
    writeln!(out, "}}")?;
    writeln!(out)?;

    Ok(Utilities::default())
}

fn expand_symbols<'a>(
    out: &mut impl std::fmt::Write,
    funcs: impl Iterator<Item = &'a syntax::ItemFn>,
    dylib: &str,
    prefix: &str,
) -> std::fmt::Result {
    writeln!(out, r#"const {{ symbols: __symbols }} = Deno.dlopen("#)?;
    writeln!(out, r#"  new URL("#)?;
    writeln!(out, r#"    {{"#)?;
    writeln!(out, r#"      darwin: '{}lib{}.dylib',"#, prefix, dylib)?;
    writeln!(out, r#"      linux: '{}lib{}.so',"#, prefix, dylib)?;
    writeln!(out, r#"      windows: '{}{}.dll',"#, prefix, dylib)?;
    writeln!(out, r#"      freebsd: '{}lib{}.so',"#, prefix, dylib)?;
    writeln!(out, r#"      netbsd: '{}lib{}.so',"#, prefix, dylib)?;
    writeln!(out, r#"      aix: '{}lib{}.so',"#, prefix, dylib)?;
    writeln!(out, r#"      solaris: '{}lib{}.so',"#, prefix, dylib)?;
    writeln!(out, r#"      illumos: '{}lib{}.so',"#, prefix, dylib)?;
    writeln!(out, r#"    }}[Deno.build.os],"#)?;
    writeln!(out, r#"    import.meta.url"#)?;
    writeln!(out, r#"  ),"#)?;
    writeln!(out, r#"  {{"#)?;

    for func in funcs {
        let sig = &func.sig;
        writeln!(out, r#"    "{}": {{"#, sig.ident)?;

        // input parameters
        write!(out, r#"      "parameters": ["#)?;
        for (index, input) in sig.inputs.iter().enumerate() {
            if index > 0 {
                write!(out, ", ")?;
            }
            write!(out, r#""{}""#, symbol_type(&input.ty))?;
        }
        writeln!(out, "],")?;

        // output results
        match &sig.output {
            syntax::ReturnType::Default => writeln!(out, r#"      "result": "void","#)?,
            syntax::ReturnType::Type(_, ty) => {
                writeln!(out, r#"      "result": "{}","#, symbol_return_type(ty))?
            }
        }

        // non blocking
        let non_blocking = is_non_blocking_fn(func);
        writeln!(out, r#"      "nonblocking": {:?},"#, non_blocking)?;

        writeln!(out, r#"    }},"#)?;
    }

    writeln!(out, r#"  }}"#)?;
    writeln!(out, r#");"#)?;
    Ok(())
}

fn expand_function(
    out: &mut impl std::fmt::Write,
    func: &syntax::ItemFn,
) -> Result<Utilities, std::fmt::Error> {
    let sig = &func.sig;
    let has_inputs = !sig.inputs.is_empty();
    let non_blocking = is_non_blocking_fn(func);
    let mut utilities = Utilities::default();

    // signature
    if non_blocking {
        write!(out, "export async function {}(", sig.ident)?;
    } else {
        write!(out, "export function {}(", sig.ident)?;
    }
    for (index, input) in sig.inputs.iter().enumerate() {
        if index > 0 {
            write!(out, ", ")?;
        }
        write!(out, "{}: ", input.ident)?;
        expand_type(out, &input.ty)?;
    }
    write!(out, ")")?;
    if let syntax::ReturnType::Type(_, ty) = &sig.output {
        write!(out, ": ")?;
        if non_blocking {
            write!(out, "Promise<")?;
        }
        expand_type(out, ty)?;
        if non_blocking {
            write!(out, ">")?;
        }
    }
    writeln!(out, " {{")?;

    // transform input
    for (index, input) in sig.inputs.iter().enumerate() {
        match input.ty.kind {
            syntax::TypeKind::Native(_) => {
                writeln!(out, "  const __arg{} = {};", index, input.ident)?;
            }
            syntax::TypeKind::BufferBorrowed
            | syntax::TypeKind::BufferBorrowedMut
            | syntax::TypeKind::BufferOwned => {
                writeln!(out, "  const __arg{}_ptr = {};", index, input.ident)?;
                writeln!(
                    out,
                    "  const __arg{0}_len = __arg{0}_ptr.byteLength;",
                    index
                )?;
            }
            syntax::TypeKind::StringBorrowed | syntax::TypeKind::StringOwned => {
                writeln!(
                    out,
                    "  const __arg{}_ptr = __stringEncode({});",
                    index, input.ident
                )?;
                writeln!(
                    out,
                    "  const __arg{0}_len = __arg{0}_ptr.byteLength;",
                    index
                )?;
                utilities.string_encode = true;
            }
            syntax::TypeKind::Json => {
                writeln!(
                    out,
                    "  const __arg{}_ptr = __structEncode({});",
                    index, input.ident
                )?;
                writeln!(
                    out,
                    "  const __arg{0}_len = __arg{0}_ptr.byteLength;",
                    index
                )?;
                utilities.struct_encode = true;
            }
        }
    }

    if has_inputs {
        writeln!(out)?;
    }

    // call imported function
    write!(out, "  const __res = __symbols.{}(", sig.ident)?;
    for (index, input) in sig.inputs.iter().enumerate() {
        if index > 0 {
            write!(out, ", ")?;
        }
        match input.ty.kind {
            syntax::TypeKind::Native(_) => write!(out, "__arg{}", index)?,
            _ => write!(out, "__arg{0}_ptr, __arg{0}_len", index)?,
        }
    }
    writeln!(out, ");")?;

    // transform result
    if let syntax::ReturnType::Type(_, ty) = &sig.output {
        match ty.kind {
            syntax::TypeKind::Native(_) => {
                writeln!(out, "  return __res")?;
            }
            syntax::TypeKind::BufferBorrowed
            | syntax::TypeKind::BufferBorrowedMut
            | syntax::TypeKind::BufferOwned => {
                if non_blocking {
                    writeln!(out, "  return __res.then(__lenPrefixedBuffer);")?;
                } else {
                    writeln!(out, "  return __lenPrefixedBuffer(__res);")?;
                }
                utilities.len_prefixed_buffer = true;
            }
            syntax::TypeKind::StringBorrowed | syntax::TypeKind::StringOwned => {
                if non_blocking {
                    writeln!(
                        out,
                        "  return __res.then(__lenPrefixedBuffer).then(__stringDecode);"
                    )?;
                } else {
                    writeln!(out, "  return __stringDecode(__lenPrefixedBuffer(__res));")?;
                }
                utilities.string_decode = true;
                utilities.len_prefixed_buffer = true;
            }
            syntax::TypeKind::Json => {
                if non_blocking {
                    writeln!(
                        out,
                        "  return __res.then(__lenPrefixedBuffer).then(__structDecode);"
                    )?;
                } else {
                    writeln!(out, "  return __structDecode(__lenPrefixedBuffer(__res));")?;
                }
                utilities.struct_decode = true;
                utilities.len_prefixed_buffer = true;
            }
        }
    }

    writeln!(out, "}}")?;
    writeln!(out)?;

    Ok(utilities)
}

fn expand_type(out: &mut impl std::fmt::Write, ty: &syntax::Type) -> std::fmt::Result {
    write!(out, "{}", ty.ts)
}

fn symbol_type(ty: &syntax::Type) -> &'static str {
    match ty.kind {
        syntax::TypeKind::Native(ty) => ty.symbol(),
        _ => r#"buffer", "usize"#,
    }
}

fn symbol_return_type(ty: &syntax::Type) -> &'static str {
    match ty.kind {
        syntax::TypeKind::Native(ty) => ty.symbol(),
        _ => "buffer",
    }
}

#[derive(Default)]
struct Utilities {
    string_encode: bool,
    string_decode: bool,
    struct_encode: bool,
    struct_decode: bool,
    len_prefixed_buffer: bool,
}
const STRING_ENCODE: &str = r#"function __stringEncode(s: string): Uint8Array {
    return new TextEncoder().encode(s);
}
"#;

const STRING_DECODE: &str = r#"function __stringDecode(a: Uint8Array): string {
    return new TextDecoder().decode(a)
}
"#;

const STRUCT_ENCODE: &str = r#"function __structEncode(v: any): Uint8Array {
    return __stringEncode(JSON.stringify(v));
}
"#;

const STRUCT_DECODE: &str = r#"function __structDecode(v: Uint8Array): any {
    return JSON.parse(__stringDecode(v));
}
"#;

const LEN_PREFIXED_BUFFER: &str = r#"function __lenPrefixedBuffer(v: any): Uint8Array {
    const unsafeView = new Deno.UnsafePointerView(v);

    const lenBigEndian = new Uint8Array(4);
    const lenBigEndianView = new DataView(lenBigEndian.buffer);
    unsafeView.copyInto(lenBigEndian, 0);
    const len = lenBigEndianView.getInt32(0);

    const buffer = new Uint8Array(len);
    unsafeView.copyInto(buffer, 4);

    return buffer;
}
"#;

impl Utilities {
    fn merge(&mut self, other: Self) {
        self.string_encode |= other.string_encode;
        self.string_decode |= other.string_decode;
        self.struct_encode |= other.struct_encode;
        self.struct_decode |= other.struct_decode;
        self.len_prefixed_buffer |= other.len_prefixed_buffer;
    }

    fn expand(&self, out: &mut impl std::fmt::Write) -> std::fmt::Result {
        if self.string_encode | self.struct_encode {
            writeln!(out, "{}", STRING_ENCODE)?;
        }
        if self.string_decode | self.struct_decode {
            writeln!(out, "{}", STRING_DECODE)?;
        }
        if self.struct_encode {
            writeln!(out, "{}", STRUCT_ENCODE)?;
        }
        if self.struct_decode {
            writeln!(out, "{}", STRUCT_DECODE)?;
        }
        if self.len_prefixed_buffer {
            writeln!(out, "{}", LEN_PREFIXED_BUFFER)?;
        }

        Ok(())
    }
}

fn is_non_blocking_fn(func: &syntax::ItemFn) -> bool {
    for attr in &func.attrs {
        if let syn::Meta::Path(path) = &attr.meta {
            let segments = &path.segments;
            if segments.len() == 2
                && segments[0].ident == "sauro"
                && segments[1].ident == "non_blocking"
            {
                return true;
            }
        }
    }
    false
}
