use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Data, DeriveInput, Fields, FnArg, ImplItem, ItemImpl, ReturnType, Type,
};

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
            return syn::Error::new_spanned(&input, "ZenForeign only supports structs")
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
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Str(p.#field_name.clone().into())))
                        }
                    }
                }
                FieldType::I64 => {
                    quote! {
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name)))
                        }
                    }
                }
                FieldType::I32 => {
                    quote! {
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::I16 => {
                    quote! {
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::I8 => {
                    quote! {
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::U64 => {
                    quote! {
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::U32 => {
                    quote! {
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::U16 => {
                    quote! {
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::U8 => {
                    quote! {
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Int(p.#field_name as i64)))
                        }
                    }
                }
                FieldType::F64 => {
                    quote! {
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Float(p.#field_name)))
                        }
                    }
                }
                FieldType::F32 => {
                    quote! {
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Float(p.#field_name as f64)))
                        }
                    }
                }
                FieldType::Bool => {
                    quote! {
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(::zenlang::value::Value::Bool(p.#field_name)))
                        }
                    }
                }
                FieldType::Value => {
                    quote! {
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(p.#field_name.clone()))
                        }
                    }
                }
                FieldType::ForeignReference => {
                    quote! {
                        |vm: &::zenlang::VM, obj: &::zenlang::value::Value| -> ::zenlang::error::Result<::zenlang::value::Value> {
                            ::zenlang::interop::with_foreign::<#name, _, _>(vm, obj, |p| ::std::result::Result::Ok(p.#field_name.clone()))
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
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let s = val.as_str().map(|s| s.to_string()).unwrap_or_default();
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = s; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::I64 => {
                    quote! {
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0);
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::I32 => {
                    quote! {
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as i32;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::I16 => {
                    quote! {
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as i16;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::I8 => {
                    quote! {
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as i8;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::U64 => {
                    quote! {
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as u64;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::U32 => {
                    quote! {
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as u32;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::U16 => {
                    quote! {
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as u16;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::U8 => {
                    quote! {
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_int().unwrap_or(0) as u8;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::F64 => {
                    quote! {
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_float().unwrap_or(0.0);
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::F32 => {
                    quote! {
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_float().unwrap_or(0.0) as f32;
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::Bool => {
                    quote! {
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            let v = val.as_bool().unwrap_or(false);
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = v; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::Value => {
                    quote! {
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = val; ::std::result::Result::Ok(()) })
                        }
                    }
                }
                FieldType::ForeignReference => {
                    quote! {
                        |vm: &mut ::zenlang::VM, obj: &mut ::zenlang::value::Value, val: ::zenlang::value::Value| -> ::zenlang::error::Result<()> {
                            ::zenlang::interop::with_foreign_mut::<#name, _, _>(vm, obj, |p| { p.#field_name = val; ::std::result::Result::Ok(()) })
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

/// Attribute macro to register all methods in an `impl` block as callable
/// Zenlang methods on the foreign type.
///
/// Must be used on an `impl TypeName { ... }` block where `TypeName` is a
/// struct that also has `#[derive(ZenForeign)]`.
///
/// Supports `&self` and `&mut self` methods with parameters of types:
/// `i64`, `i32`, `i16`, `i8`, `u64`, `u32`, `u16`, `u8`, `f64`, `f32`,
/// `bool`, `String`, `Value`.
///
/// Return types can be: `()`, `i64`, `i32`, `i16`, `i8`, `u64`, `u32`,
/// `u16`, `u8`, `f64`, `f32`, `bool`, `String`, `Value`.
///
/// Generates `TypeName::register_zen_methods(vm)`.
#[proc_macro_attribute]
pub fn zen_methods(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let impl_block = parse_macro_input!(item as ItemImpl);
    let self_ty = &impl_block.self_ty;

    let mut method_registrations = Vec::new();

    for item in &impl_block.items {
        let method = match item {
            ImplItem::Fn(m) => m,
            _ => continue,
        };

        // Skip methods without &self / &mut self (e.g. constructors)
        let has_self = method.sig.inputs.first().map_or(false, |arg| {
            matches!(arg, FnArg::Receiver(_))
        });
        if !has_self {
            continue;
        }

        let method_name = method.sig.ident.to_string();
        let method_ident = &method.sig.ident;
        let is_mut = method.sig.inputs.first().map_or(false, |arg| {
            matches!(arg, FnArg::Receiver(r) if r.mutability.is_some())
        });

        let mut param_types = Vec::new();
        for arg in method.sig.inputs.iter().skip(1) {
            match arg {
                FnArg::Typed(pat_type) => {
                    param_types.push(pat_type.ty.as_ref());
                }
                _ => {}
            }
        }

        let param_extractions: Vec<_> = param_types
            .iter()
            .enumerate()
            .map(|(i, ty)| {
                let fi = i + 1;
                let pname = syn::Ident::new(&format!("p{}", fi), proc_macro2::Span::call_site());
                match ty_to_field_type(ty) {
                    FieldType::String => {
                        quote! { let #pname = ::std::convert::Into::<String>::into(args[#fi].as_str().unwrap_or_default()); }
                    }
                    FieldType::I64 => {
                        quote! { let #pname: i64 = args[#fi].as_int().unwrap_or(0); }
                    }
                    FieldType::I32 => {
                        quote! { let #pname: i32 = args[#fi].as_int().unwrap_or(0) as i32; }
                    }
                    FieldType::I16 => {
                        quote! { let #pname: i16 = args[#fi].as_int().unwrap_or(0) as i16; }
                    }
                    FieldType::I8 => {
                        quote! { let #pname: i8 = args[#fi].as_int().unwrap_or(0) as i8; }
                    }
                    FieldType::U64 => {
                        quote! { let #pname: u64 = args[#fi].as_int().unwrap_or(0) as u64; }
                    }
                    FieldType::U32 => {
                        quote! { let #pname: u32 = args[#fi].as_int().unwrap_or(0) as u32; }
                    }
                    FieldType::U16 => {
                        quote! { let #pname: u16 = args[#fi].as_int().unwrap_or(0) as u16; }
                    }
                    FieldType::U8 => {
                        quote! { let #pname: u8 = args[#fi].as_int().unwrap_or(0) as u8; }
                    }
                    FieldType::F64 => {
                        quote! { let #pname: f64 = args[#fi].as_float().unwrap_or(0.0); }
                    }
                    FieldType::F32 => {
                        quote! { let #pname: f32 = args[#fi].as_float().unwrap_or(0.0) as f32; }
                    }
                    FieldType::Bool => {
                        quote! { let #pname: bool = args[#fi].as_bool().unwrap_or(false); }
                    }
                    FieldType::Value | FieldType::ForeignReference => {
                        quote! { let #pname = args[#fi].clone(); }
                    }
                    FieldType::Unknown => {
                        return syn::Error::new_spanned(
                            ty,
                            format!(
                                "unsupported parameter type '{}' in method '{}'",
                                quote!(#ty),
                                method_name
                            ),
                        )
                        .to_compile_error()
                        .into();
                    }
                }
            })
            .collect();

        let param_idents: Vec<_> = (1..=param_types.len())
            .map(|i| {
                let id = syn::Ident::new(&format!("p{}", i), proc_macro2::Span::call_site());
                quote! { #id }
            })
            .collect();

        let self_accessor = if is_mut {
            quote! { ::zenlang::interop::with_foreign_value_mut::<#self_ty, _, _> }
        } else {
            quote! { ::zenlang::interop::with_foreign::<#self_ty, _, _> }
        };

        let return_conversion = match &method.sig.output {
            ReturnType::Default => {
                quote! {
                    s.#method_ident(#(#param_idents)*);
                    ::std::result::Result::Ok(::zenlang::value::Value::Nil)
                }
            }
            ReturnType::Type(_, ty) => match ty_to_field_type(ty) {
                FieldType::String => {
                    quote! {
                        ::std::result::Result::Ok(::zenlang::value::Value::Str(s.#method_ident(#(#param_idents)*).into()))
                    }
                }
                FieldType::I64 => {
                    quote! {
                        ::std::result::Result::Ok(::zenlang::value::Value::Int(s.#method_ident(#(#param_idents)*)))
                    }
                }
                FieldType::I32 | FieldType::I16 | FieldType::I8
                | FieldType::U64 | FieldType::U32 | FieldType::U16 | FieldType::U8 => {
                    quote! {
                        ::std::result::Result::Ok(::zenlang::value::Value::Int(s.#method_ident(#(#param_idents)*) as i64))
                    }
                }
                FieldType::F64 => {
                    quote! {
                        ::std::result::Result::Ok(::zenlang::value::Value::Float(s.#method_ident(#(#param_idents)*)))
                    }
                }
                FieldType::F32 => {
                    quote! {
                        ::std::result::Result::Ok(::zenlang::value::Value::Float(s.#method_ident(#(#param_idents)*) as f64))
                    }
                }
                FieldType::Bool => {
                    quote! {
                        ::std::result::Result::Ok(::zenlang::value::Value::Bool(s.#method_ident(#(#param_idents)*)))
                    }
                }
                FieldType::Value | FieldType::ForeignReference => {
                    quote! {
                        ::std::result::Result::Ok(s.#method_ident(#(#param_idents)*))
                    }
                }
                FieldType::Unknown => {
                    return syn::Error::new_spanned(
                        ty,
                        format!(
                            "unsupported return type '{}' in method '{}'",
                            quote!(#ty),
                            method_name
                        ),
                    )
                    .to_compile_error()
                    .into()
                }
            },
        };

        method_registrations.push(quote! {
            def.method(#method_name, ::std::rc::Rc::new(
                |ctx: &mut ::zenlang::vm::VMContext , args: &[::zenlang::value::Value]| -> ::zenlang::error::Result<::zenlang::value::Value> {
                    #(#param_extractions)*
                    let vm: &::zenlang::VM = unsafe { &*ctx.raw_vm };
                    let result = #self_accessor(vm, &args[0], |s| {
                        #return_conversion
                    })?;
                    ::std::result::Result::Ok(result)
                }
            ));
        });
    }

    if method_registrations.is_empty() {
        return quote! { #impl_block }.into();
    }

    let expanded = quote! {
        #impl_block

        impl #self_ty {
            #[allow(non_snake_case)]
            pub fn register_zen_methods(vm: &mut ::zenlang::VM) {
                let tid = ::std::any::TypeId::of::<#self_ty>();
                if let Some(def) = ::std::rc::Rc::make_mut(&mut vm.foreign_registry).get_mut(&tid) {
                    #(#method_registrations)*
                }
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
