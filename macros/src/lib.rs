use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Data, DeriveInput, Fields, FnArg, ImplItem, ItemImpl, ItemStruct, ReturnType, Type,
    parse_macro_input,
};

/// Structured input for the `foreign_type!` macro.
struct ForeignTypeInput {
    name: String,
    strukt: ItemStruct,
    impl_block: ItemImpl,
}

impl syn::parse::Parse for ForeignTypeInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse `name: "TypeName",`
        let name_kw: syn::Ident = input.parse()?;
        if name_kw != "name" {
            return Err(syn::Error::new(name_kw.span(), "expected `name`"));
        }
        input.parse::<syn::Token![:]>()?;
        let name_lit: syn::LitStr = input.parse()?;
        input.parse::<syn::Token![,]>()?;

        // Parse the struct definition
        let strukt: ItemStruct = input.parse()?;

        // Parse the impl block
        let impl_block: ItemImpl = input.parse()?;

        Ok(ForeignTypeInput {
            name: name_lit.value(),
            strukt,
            impl_block,
        })
    }
}

/// Unified macro to define a foreign type with a single entry point.
///
/// Expands to the struct with `#[derive(Clone, Debug, ZenForeign)]`,
/// the `impl` block with `#[zen_methods]`, and a combined
/// `register_zen(vm)` that calls both.
///
/// # Syntax
/// ```ignore
/// foreign_type! {
///     name: "Player",
///     struct Player {
///         name: String,
///         health: i32,
///         max_health: i32,
///     }
///     impl Player {
///         fn new(name: &str) -> Self { ... }
///         fn heal_percent(&self) -> f64 { ... }
///     }
/// }
/// ```
///
/// Generates `Player::register_zen(&mut vm)` that registers
/// both fields and methods in one call.
#[proc_macro]
pub fn foreign_type(input: TokenStream) -> TokenStream {
    let ft_input = parse_macro_input!(input as ForeignTypeInput);
    let name = &ft_input.name;
    let strukt = &ft_input.strukt;
    let struct_name = &strukt.ident;
    let impl_block = &ft_input.impl_block;

    let expanded = quote! {
        #[derive(Clone, Debug, ::zenlang::ZenForeign)]
        #[foreign(name = #name)]
        #strukt

        #[::zenlang::zen_methods]
        #impl_block

        impl #struct_name {
            /// Register both fields and methods with the VM.
            pub fn register_zen(vm: &mut ::zenlang::VM) {
                Self::register_zen_foreign(vm);
                Self::register_zen_methods(vm);
            }
        }
    };
    expanded.into()
}

/// Attribute macro to generate a `FnSignature` for a native function.
///
/// # Syntax
/// ```ignore
/// #[zen_native_fn(name: "contains", params: [Str, Str], returns: Bool)]
/// fn contains_impl(vm: &mut VMContext, args: &[Value]) -> Result<Value> { ... }
/// ```
///
/// The `name` field is optional; if omitted the Rust function name is used.
/// Generates a `<fn_name>_sig()` function returning `crate::symbol::FnSignature`.
#[proc_macro_attribute]
pub fn zen_native_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ZenNativeFnArgs);
    let func = parse_macro_input!(item as syn::ItemFn);
    let fn_name = &func.sig.ident;
    let fn_name_str = args
        .name
        .as_deref()
        .unwrap_or(&fn_name.to_string())
        .to_string();
    let sig_name = syn::Ident::new(&format!("{}_sig", fn_name), proc_macro2::Span::call_site());

    let param_types = &args.params;
    let return_type = &args.returns;
    let param_count = param_types.len();

    // Build parameter pairs: ("arg0", Type::Str), ("arg1", Type::I64), ...
    let params: Vec<_> = (0..param_count)
        .map(|i| {
            let pname = format!("arg{}", i);
            let ptype = &param_types[i];
            quote! { (#pname.into(), crate::ast::Type::#ptype) }
        })
        .collect();

    let expanded = quote! {
        #func

        fn #sig_name() -> crate::symbol::FnSignature {
            crate::symbol::FnSignature {
                name: #fn_name_str.into(),
                type_params: vec![],
                params: vec![#(#params),*],
                return_type: Some(crate::ast::Type::#return_type),
            }
        }
    };
    expanded.into()
}

struct ZenNativeFnArgs {
    name: Option<String>,
    params: Vec<syn::Ident>,
    returns: syn::Ident,
}

impl syn::parse::Parse for ZenNativeFnArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut params = None;
        let mut returns = None;

        // Parse comma-separated key: value pairs
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![:]>()?;

            match key.to_string().as_str() {
                "name" => {
                    let lit: syn::LitStr = input.parse()?;
                    name = Some(lit.value());
                }
                "params" => {
                    let content;
                    syn::bracketed!(content in input);
                    params = Some(
                        content
                            .parse_terminated(syn::Ident::parse, syn::Token![,])?
                            .into_iter()
                            .collect(),
                    );
                }
                "returns" => {
                    returns = Some(input.parse::<syn::Ident>()?);
                }
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown key '{}'", key),
                    ));
                }
            }

            // Consume optional comma separator
            if !input.is_empty() {
                let _ = input.parse::<syn::Token![,]>();
            }
        }

        let params = params.ok_or_else(|| input.error("missing required 'params:' key"))?;
        let returns = returns.ok_or_else(|| input.error("missing required 'returns:' key"))?;

        Ok(ZenNativeFnArgs {
            name,
            params,
            returns,
        })
    }
}

/// Derive macro to register a Rust struct as a Zenlang foreign type.
///
/// Generates `TypeName::register_zen_foreign(vm: &mut VM)` that registers the
/// type with all its named fields as accessible properties from Zenlang.
///
/// # Supported field types
///
/// `i64`, `i32`, `i16`, `i8`, `u64`, `u32`, `u16`, `u8`, `f64`, `f32`,
/// `bool`, `String`, `Value`, and `Rc<RefCell<...>>` (as foreign references).
///
/// # Example
///
/// ```ignore
/// #[derive(ZenForeign)]
/// struct Player {
///     name: String,
///     health: i64,
/// }
///
/// // Generated impl:
/// // Player::register_zen_foreign(&mut vm);
/// ```
/// Extract the Zenlang type name from `#[foreign(name = "...")]` attribute.
fn foreign_type_name(input: &DeriveInput) -> String {
    for attr in &input.attrs {
        if attr.path().is_ident("foreign") {
            if let syn::Meta::NameValue(nv) = &attr.meta {
                if nv.path.is_ident("name") {
                    if let syn::Expr::Lit(expr_lit) = &nv.value {
                        if let syn::Lit::Str(s) = &expr_lit.lit {
                            return s.value();
                        }
                    }
                }
            }
        }
    }
    // Default to the Rust struct name
    input.ident.to_string()
}

#[proc_macro_derive(ZenForeign, attributes(foreign))]
pub fn derive_zen_foreign(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let type_name = foreign_type_name(&input);

    let fields = match &input.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(nf) => &nf.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "ZenForeign only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "ZenForeign only supports structs")
                .to_compile_error()
                .into();
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
                vm.register_type::<#name>(#type_name)
                    #(#field_registrations)*;
            }
        }
    };

    expanded.into()
}

/// Check if a type is `Self`.
fn is_self_type(ty: &Type) -> bool {
    matches!(ty, Type::Path(type_path) if type_path.qself.is_none()
        && type_path.path.segments.len() == 1
        && type_path.path.segments[0].ident == "Self")
}

/// Check if a type is a reference (used to generate `&p_i` for &str params).
fn is_ref_type(ty: &Type) -> bool {
    matches!(ty, Type::Reference(_))
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
/// Methods without `&self` that return `Self` are treated as constructors
/// and auto-registered as native functions.
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
    // Constructors (no &self, returns Self) are registered as native fns
    let mut constructor_registrations = Vec::new();

    for item in &impl_block.items {
        let method = match item {
            ImplItem::Fn(m) => m,
            _ => continue,
        };

        let has_self = method
            .sig
            .inputs
            .first()
            .is_some_and(|arg| matches!(arg, FnArg::Receiver(_)));
        let method_name = method.sig.ident.to_string();
        let method_ident = &method.sig.ident;

        // ── Detect constructor: no self receiver, returns Self ──
        let is_constructor =
            !has_self && matches!(&method.sig.output, ReturnType::Type(_, ty) if is_self_type(ty));

        if is_constructor {
            let mut param_types = Vec::new();
            for arg in &method.sig.inputs {
                if let FnArg::Typed(pat_type) = arg {
                    param_types.push(pat_type.ty.as_ref());
                }
            }

            let param_extractions: Vec<_> = param_types
                .iter()
                .enumerate()
                .map(|(i, ty)| {
                    let fi = i;
                    let pname = syn::Ident::new(&format!("p{}", i), proc_macro2::Span::call_site());
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
                            syn::Error::new_spanned(
                                ty,
                                format!(
                                    "unsupported field type '{}' in ZenForeign struct '{}'",
                                    quote!(#ty),
                                    quote!(#self_ty)
                                ),
                            )
                            .to_compile_error()
                        }
                    }
                })
                .collect();

            let param_idents: Vec<_> = param_types
                .iter()
                .enumerate()
                .map(|(i, ty)| {
                    let id = syn::Ident::new(&format!("p{}", i), proc_macro2::Span::call_site());
                    if is_ref_type(ty) {
                        quote! { &#id }
                    } else {
                        quote! { #id }
                    }
                })
                .collect();

            // Use stringify! and strip spaces to get the type name
            let type_name_str = stringify!(#self_ty).replace(' ', "");

            constructor_registrations.push(quote! {
                vm.register_native(#method_name, ::std::rc::Rc::new(
                    |ctx: &mut ::zenlang::vm::VMContext, args: &[::zenlang::value::Value]| -> ::zenlang::error::Result<::zenlang::value::Value> {
                        #(#param_extractions)*
                        let obj = #self_ty::#method_ident(#(#param_idents)*);
                        let vm: &mut ::zenlang::VM = unsafe { &mut *ctx.raw_vm };
                        let h = vm.foreigns.insert(::zenlang::value::ForeignObject::new(#type_name_str, obj));
                        ::std::result::Result::Ok(::zenlang::value::Value::Foreign(h))
                    }
                ));
            });
            continue;
        }

        // ── Skip non-constructor methods without &self / &mut self ──
        if !has_self {
            continue;
        }

        let is_mut = method
            .sig
            .inputs
            .first()
            .is_some_and(|arg| matches!(arg, FnArg::Receiver(r) if r.mutability.is_some()));

        let mut param_types = Vec::new();
        for arg in method.sig.inputs.iter().skip(1) {
            if let FnArg::Typed(pat_type) = arg {
                param_types.push(pat_type.ty.as_ref());
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
                        syn::Error::new_spanned(
                            ty,
                            format!(
                                "unsupported field type '{}' in ZenForeign struct '{}'",
                                quote!(#ty),
                                quote!(#self_ty)
                            ),
                        )
                        .to_compile_error()
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
                FieldType::I32
                | FieldType::I16
                | FieldType::I8
                | FieldType::U64
                | FieldType::U32
                | FieldType::U16
                | FieldType::U8 => {
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
                    .into();
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

    if method_registrations.is_empty() && constructor_registrations.is_empty() {
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
                #(#constructor_registrations)*
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
        "String"
        | "std :: string :: String"
        | "alloc :: string :: String"
        | "& str"
        | "&mut str" => FieldType::String,
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
