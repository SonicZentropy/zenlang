use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type};

#[proc_macro_derive(ZenForeign)]
pub fn derive_zen_foreign(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(nf) => &nf.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "ZenForeign only supports structs with named fields",
                )
                .to_compile_error()
                .into()
            }
        },
        _ => {
            return syn::Error::new_spanned(
                &input,
                "ZenForeign only supports structs",
            )
            .to_compile_error()
            .into()
        }
    };

    let field_registrations: Vec<_> = fields
        .iter()
        .map(|f| {
            let field_name = f.ident.as_ref().unwrap();
            let field_name_str = field_name.to_string();
            let ty = &f.ty;
            let field_ty = ty_to_field_type(ty);

            let getter_expr = match field_ty {
                FieldType::String => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Str(p.#field_name.clone().into())))
                        }
                    }
                }
                FieldType::I64 => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name)))
                        }
                    }
                }
                FieldType::I32 => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::I16 => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::I8 => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::U64 => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::U32 => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::U16 => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::U8 => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::F64 => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Float(p.#field_name)))
                        }
                    }
                }
                FieldType::F32 => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Float(p.#field_name as f64)))
                        }
                    }
                }
                FieldType::Bool => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Bool(p.#field_name)))
                        }
                    }
                }
                FieldType::Value => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(p.#field_name.clone()))
                        }
                    }
                }
                FieldType::ForeignReference => {
                    quote! {
                        |obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(obj, |p| ::std::result::Result::Ok(p.#field_name.clone()))
                        }
                    }
                }
                FieldType::Unknown => {
                    return syn::Error::new_spanned(
                        ty,
                        format!(
                            "unsupported field type '{}' in ZenForeign struct '{}'",
                            quote!(#ty),
                            name
                        ),
                    )
                    .to_compile_error()
                    .into()
                }
            };

            let setter_expr = match field_ty {
                FieldType::String => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let s = val.as_str().map(|s| s.to_string()).unwrap_or_default();
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = s; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::I64 => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0);
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::I32 => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as i32;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::I16 => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as i16;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::I8 => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as i8;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::U64 => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as u64;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::U32 => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as u32;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::U16 => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as u16;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::U8 => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as u8;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::F64 => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_float().unwrap_or(0.0);
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::F32 => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_float().unwrap_or(0.0) as f32;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::Bool => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_bool().unwrap_or(false);
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::Value => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = val; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::ForeignReference => {
                    quote! {
                        |obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(obj, |p| { p.#field_name = val; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::Unknown => {
                    return syn::Error::new_spanned(
                        ty,
                        format!(
                            "unsupported field type '{}' in ZenForeign struct '{}'",
                            quote!(#ty),
                            name
                        ),
                    )
                    .to_compile_error()
                    .into()
                }
            };

            quote! {
                .field(#field_name_str, #getter_expr, #setter_expr)
            }
        })
        .collect();

    let expanded = quote! {
        impl #name {
            pub fn register_zen_foreign(vm: &mut ::zenlang::VM) {
                vm.register_type::<#name>(stringify!(#name))
                    #(#field_registrations)*;
            }
        }
    };

    expanded.into()
}

enum FieldType {
    String,
    I64,
    I32,
    I16,
    I8,
    U64,
    U32,
    U16,
    U8,
    F64,
    F32,
    Bool,
    Value,
    ForeignReference,
    Unknown,
}

fn ty_to_field_type(ty: &Type) -> FieldType {
    let ty_str = quote!(#ty).to_string();
    match ty_str.as_str() {
        "String" | "std :: string :: String" | "alloc :: string :: String" => FieldType::String,
        "i64" => FieldType::I64,
        "i32" => FieldType::I32,
        "i16" => FieldType::I16,
        "i8" => FieldType::I8,
        "u64" => FieldType::U64,
        "u32" => FieldType::U32,
        "u16" => FieldType::U16,
        "u8" => FieldType::U8,
        "f64" => FieldType::F64,
        "f32" => FieldType::F32,
        "bool" => FieldType::Bool,
        "Value"
        | "crate :: value :: Value"
        | "zenlang :: Value"
        | ":: zenlang :: value :: Value" => FieldType::Value,
        s if s.starts_with("Rc<") || s.starts_with("std :: rc :: Rc<") => {
            if s.contains("RefCell<") || s.contains("RefCell <") {
                FieldType::ForeignReference
            } else {
                FieldType::Unknown
            }
        }
        _ if ty_str.contains("Foreign") || ty_str.contains("Value") => FieldType::ForeignReference,
        _ => FieldType::Unknown,
    }
}
