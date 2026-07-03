use crate::ast::*;
use crate::error::{Error, Result};
use crate::span::{SourceLocation, Spanned};
use crate::symbol::*;

/// Walk the AST and build a symbol table.
/// Reports errors for duplicate declarations and undefined names.
pub fn resolve(program: &mut Program) -> Result<SymbolTable> {
    resolve_with_natives(program, &[])
}

/// Resolve a program with pre-registered native function names.
/// Native functions registered here will be callable from scripts.
pub fn resolve_with_natives(program: &mut Program, native_names: &[String]) -> Result<SymbolTable> {
    let mut resolver = Resolver::new();
    // Pre-register native functions with accurate type signatures
    for sig in crate::stdlib::native_fn_sigs() {
        let name = sig.name.clone();
        let _ = resolver.symbols.define(&name, SymKind::Function(sig));
    }
    // Pre-register user-defined native functions (e.g. from `register_native`)
    // as generic functions with unknown signatures.
    for name in native_names {
        if resolver.symbols.lookup(name).is_none() {
            let sig = crate::symbol::FnSignature {
                name: name.clone(),
                type_params: vec![],
                params: vec![],
                return_type: Some(crate::ast::Type::Unit),
            };
            let _ = resolver.symbols.define(name, SymKind::Function(sig));
        }
    }
    // Pre-register built-in Option type
    let option_param = TypeParam { name: "T".into(), bounds: vec![] };
    let option_def = EnumDef {
        name: "Option".into(),
        type_params: vec![option_param],
        variants: vec![
            ("Some".into(), vec![Type::Unit]), // Unit acts as generic placeholder (compatible with everything)
            ("None".into(), vec![]),
        ],
    };
    let _ = resolver.symbols.define("Option", SymKind::Enum(option_def.clone()));
    for (tag, (vname, fields)) in option_def.variants.iter().enumerate() {
        let cons = SymKind::EnumConstructor {
            enum_name: "Option".into(),
            variant_name: vname.clone(),
            tag: tag as u16,
            fields: fields.clone(),
        };
        let _ = resolver.symbols.define(vname, cons.clone());
        let _ = resolver.symbols.define(&format!("{}::{}", "Option", vname), cons);
    }
    // Pre-register built-in Result type
    let result_t_param = TypeParam { name: "T".into(), bounds: vec![] };
    let result_e_param = TypeParam { name: "E".into(), bounds: vec![] };
    let result_def = EnumDef {
        name: "Result".into(),
        type_params: vec![result_t_param, result_e_param],
        variants: vec![
            ("Ok".into(), vec![Type::Unit]), // Unit acts as generic placeholder
            ("Err".into(), vec![Type::Unit]), // Unit acts as generic placeholder
        ],
    };
    let _ = resolver.symbols.define("Result", SymKind::Enum(result_def.clone()));
    for (tag, (vname, fields)) in result_def.variants.iter().enumerate() {
        let cons = SymKind::EnumConstructor {
            enum_name: "Result".into(),
            variant_name: vname.clone(),
            tag: tag as u16,
            fields: fields.clone(),
        };
        let _ = resolver.symbols.define(vname, cons.clone());
        let _ = resolver.symbols.define(&format!("{}::{}", "Result", vname), cons);
    }
    resolver.resolve_program(program)?;
    Ok(resolver.symbols)
}

struct Resolver {
    symbols: SymbolTable,
    errors: Vec<Error>,
    current_span: crate::span::Span,
}

impl Resolver {
    fn new() -> Self {
        Self { symbols: SymbolTable::new(), errors: Vec::new(), current_span: crate::span::Span::new(0, 0) }
    }

    fn set_span(&mut self, span: crate::span::Span) {
        self.current_span = span;
    }

    fn error(&mut self, msg: String) {
        self.errors.push(Error::Resolve {
            location: SourceLocation::new(None, self.current_span, 0, 0),
            msg,
        });
    }

    fn resolve_program(&mut self, program: &mut Program) -> Result<()> {
        // First pass: register all top-level declarations (fn, struct, enum, impl)
        for stmt in &program.stmts {
            self.set_span(stmt.span);
            self.register_top_level(&stmt.node);
        }

        // Second pass: resolve function bodies
        for stmt in &program.stmts {
            self.set_span(stmt.span);
            self.resolve_decl(&stmt.node);
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(Error::ParseMultiple { errors: std::mem::take(&mut self.errors) })
        }
    }

    // ---------- Registration (first pass) ----------

    fn resolve_type(&self, ty: &Type, type_params: &[TypeParam]) -> Type {
        match ty {
            Type::Named(s) if type_params.iter().any(|tp| tp.name == *s) => Type::Generic(s.clone()),
            _ => ty.clone(),
        }
    }

    fn resolve_types_in_vec(&self, tys: &[Type], type_params: &[TypeParam]) -> Vec<Type> {
        tys.iter().map(|t| self.resolve_type(t, type_params)).collect()
    }

    fn register_top_level(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Fn { name, type_params, params, return_type, body: _, .. } => {
                let sig = FnSignature {
                    name: name.to_string(),
                    type_params: type_params.clone(),
                    params: params.iter().map(|p| {
                        let ty = self.resolve_type(&p.type_ann.clone().unwrap_or(Type::Unit), type_params);
                        (p.name.to_string(), ty)
                    }).collect(),
                    return_type: return_type.as_ref().map(|t| self.resolve_type(t, type_params)),
                };
                if let Err(e) = self.symbols.define(name, SymKind::Function(sig)) {
                    self.error(e);
                }
            }
            Stmt::Struct { name, type_params, fields, .. } => {
                let resolved_fields: Vec<StructField> = fields.iter()
                    .map(|f| StructField { name: f.name.clone(), type_ann: self.resolve_type(&f.type_ann, type_params) })
                    .collect();
                let def = StructDef { name: name.to_string(), type_params: type_params.clone(), fields: resolved_fields };
                if let Err(e) = self.symbols.define(name, SymKind::Struct(def)) {
                    self.error(e);
                }
            }
            Stmt::Enum { name, type_params, variants, .. } => {
                let v: Vec<(String, Vec<Type>)> = variants.iter()
                    .map(|v| (v.name.to_string(), self.resolve_types_in_vec(&v.fields, type_params)))
                    .collect();
                let def = EnumDef { name: name.to_string(), type_params: type_params.clone(), variants: v.clone() };
                if let Err(e) = self.symbols.define(name, SymKind::Enum(def)) {
                    self.error(e);
                }
                // Register each variant as a constructor in the current scope, both bare and
                // qualified. The bare name allows ergonomic `Foo(42)` usage in expressions and
                // patterns; the qualified name `MyEnum::Foo(42)` disambiguates. If the bare name
                // clashes with an existing symbol (e.g. prelude `Some`/`None`), the existing
                // binding wins — users should use the qualified form in that case.
                for (tag, variant) in variants.iter().enumerate() {
                    let cons = SymKind::EnumConstructor {
                        enum_name: name.to_string(),
                        variant_name: variant.name.to_string(),
                        tag: tag as u16,
                        fields: self.resolve_types_in_vec(&variant.fields, type_params),
                    };
                    let _ = self.symbols.define(&variant.name, cons.clone());
                    if let Err(e) = self.symbols.define(&format!("{}::{}", name, variant.name), cons) {
                        self.error(e);
                    }
                }
            }
            Stmt::Impl { type_name, type_params, methods, trait_name, .. } => {
                for method in methods {
                    if let Stmt::Fn { name, type_params: fn_type_params, params, return_type, body: _, .. } = &method.node {
                        let combined_type_params: Vec<TypeParam> = type_params.iter().chain(fn_type_params.iter()).cloned().collect();
                        let sig = FnSignature {
                            name: format!("{}::{}", type_name, name),
                            type_params: combined_type_params.clone(),
                            params: params.iter().map(|p| {
                                let ty = self.resolve_type(&p.type_ann.clone().unwrap_or(Type::Unit), &combined_type_params);
                                (p.name.to_string(), ty)
                            }).collect(),
                            return_type: return_type.as_ref().map(|t| self.resolve_type(t, &combined_type_params)),
                        };
                        if let Err(e) = self.symbols.define(
                            &format!("{}::{}", type_name, name),
                            SymKind::Function(sig),
                        ) {
                            self.error(e);
                        }
                    }
                }
                // If this is a trait impl, verify the trait exists
                if let Some(trait_name) = trait_name {
                    if self.symbols.lookup(trait_name).is_none() {
                        self.error(format!("cannot find trait '{}'", trait_name));
                    }
                }
            }
            Stmt::Trait { name, type_params, methods, .. } => {
                let mut method_sigs = Vec::new();
                for method in methods {
                    if let Stmt::Fn { name: fn_name, type_params: fn_type_params, params, return_type, body: _, .. } = &method.node {
                        let combined_type_params: Vec<TypeParam> = type_params.iter().chain(fn_type_params.iter()).cloned().collect();
                        let sig = FnSignature {
                            name: fn_name.to_string(),
                            type_params: combined_type_params.clone(),
                            params: params.iter().map(|p| {
                                let ty = self.resolve_type(&p.type_ann.clone().unwrap_or(Type::Unit), &combined_type_params);
                                (p.name.to_string(), ty)
                            }).collect(),
                            return_type: return_type.clone(),
                        };
                        method_sigs.push(sig);
                    }
                }
                let def = TraitDef { name: name.to_string(), type_params: type_params.clone(), method_sigs };
                if let Err(e) = self.symbols.define(name, SymKind::Trait(def)) {
                    self.error(e);
                }
            }
            Stmt::Type { name, type_params, alias, .. } => {
                let resolved_alias = self.resolve_type(alias, type_params);
                if let Err(e) = self.symbols.define(name, SymKind::TypeAlias {
                    type_params: type_params.clone(),
                    alias: resolved_alias,
                }) {
                    self.error(e);
                }
            }
            Stmt::Mod { name, body, .. } => {
                // Enter scope for the module
                self.symbols.enter_scope();
                let module_scope = self.symbols.current_scope;
                let parent = self.symbols.scopes[module_scope].parent.unwrap();
                // Register module name in the parent scope
                if self.symbols.scopes[parent].symbols.contains_key(name.as_str()) {
                    self.error(format!("duplicate definition of '{}'", name));
                } else {
                    let id = self.symbols.symbols.len();
                    self.symbols.symbols.push((name.to_string(), SymKind::Module(module_scope)));
                    self.symbols.scopes[parent].symbols.insert(
                        name.to_string(),
                        SymEntry { id, kind: SymKind::Module(module_scope) },
                    );
                }
                // Register top-level declarations within the module
                for stmt in body {
                    self.register_top_level(&stmt.node);
                }
                self.symbols.exit_scope();
            }
            _ => {} // let, expr stmts are handled in second pass
        }
    }

    // ---------- Resolution (second pass) ----------

    fn resolve_decl(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Fn { name: _, type_params, params, body, .. } => {
                self.symbols.enter_scope();
                // Register generic type parameters in scope
                for tp in type_params {
                    if let Err(e) = self.symbols.define(&tp.name, SymKind::TypeParam(tp.name.to_string())) {
                        self.error(e);
                    }
                }
                for param in params {
                    let ty = param.type_ann.clone().unwrap_or(Type::Unit);
                    if let Err(e) = self.symbols.define(&param.name, SymKind::Variable(ty)) {
                        self.error(e);
                    }
                }
                self.resolve_block(body);
                self.symbols.exit_scope();
            }
            Stmt::Impl { type_name, type_params, methods, .. } => {
                for method in methods {
                    if let Stmt::Fn { name: _, type_params: method_type_params, params, body, .. } = &method.node {
                        self.symbols.enter_scope();
                        // Register generic type parameters from impl block
                        for tp in type_params {
                            if let Err(e) = self.symbols.define(&tp.name, SymKind::TypeParam(tp.name.to_string())) {
                                self.error(e);
                            }
                        }
                        // Register generic type parameters from method
                        for tp in method_type_params {
                            if let Err(e) = self.symbols.define(&tp.name, SymKind::TypeParam(tp.name.to_string())) {
                                self.error(e);
                            }
                        }
                        for param in params {
                            let ty = if param.type_ann.is_none() && param.name == "self" {
                                Type::Named(type_name.clone())
                            } else {
                                param.type_ann.clone().unwrap_or(Type::Unit)
                            };
                            if let Err(e) = self.symbols.define(&param.name, SymKind::Variable(ty)) {
                                self.error(e);
                            }
                        }
                        self.resolve_block(body);
                        self.symbols.exit_scope();
                    }
                }
            }
            Stmt::Trait { type_params, methods, .. } => {
                // Register generic type parameters in scope for method resolution
                self.symbols.enter_scope();
                for tp in type_params {
                    if let Err(e) = self.symbols.define(&tp.name, SymKind::TypeParam(tp.name.to_string())) {
                        self.error(e);
                    }
                }
                // Resolve types in method signatures
                for method in methods {
                    if let Stmt::Fn { name: _, type_params: fn_type_params, params, body: _, .. } = &method.node {
                        for tp in fn_type_params {
                            if let Err(e) = self.symbols.define(&tp.name, SymKind::TypeParam(tp.name.to_string())) {
                                self.error(e);
                            }
                        }
                        for param in params {
                            let ty = param.type_ann.clone().unwrap_or(Type::Unit);
                            if let Err(e) = self.symbols.define(&param.name, SymKind::Variable(ty)) {
                                self.error(e);
                            }
                        }
                    }
                }
                self.symbols.exit_scope();
            }
            Stmt::Let { name, type_ann, init, .. } => {
                if let Some(expr) = init {
                    self.resolve_expr(expr);
                }
                let ty = type_ann.clone().unwrap_or(Type::Unit);
                if let Err(e) = self.symbols.define(name, SymKind::Variable(ty)) {
                    self.error(e);
                }
            }
            Stmt::Const { name, type_ann, init, .. } => {
                self.resolve_expr(init);
                let ty = type_ann.clone().unwrap_or(Type::Unit);
                if let Err(e) = self.symbols.define(name, SymKind::Variable(ty)) {
                    self.error(e);
                }
            }
            Stmt::Expr(expr) => {
                self.resolve_expr(expr);
            }
            Stmt::Return(Some(expr)) => {
                self.resolve_expr(expr);
            }
            Stmt::Return(None) => {}
            Stmt::Struct { .. } | Stmt::Enum { .. } | Stmt::Type { .. } => {
                // Already registered, nothing to resolve
            }
            Stmt::Use { path, .. } => {
                if path.len() == 1 {
                    // Single-name import: verify it exists
                    if self.symbols.lookup(&path[0]).is_none() {
                        self.error(format!("cannot resolve '{}'", path[0]));
                    }
                } else {
                    // Path-based import: look up module, then import last segment
                    let module_name = &path[0];
                    let item_name = &path[path.len() - 1];
                    let entry = self.symbols.lookup(module_name).cloned();
                    match entry {
                        Some(SymEntry { kind: SymKind::Module(scope_idx), .. }) => {
                            if let Some(item) = self.symbols.lookup_in_scope(scope_idx, item_name) {
                                let _ = self.symbols.define(item_name, item.kind.clone());
                            } else {
                                self.error(format!("cannot find '{}' in module '{}'", item_name, module_name));
                            }
                        }
                        Some(_) => {
                            self.error(format!("'{}' is not a module", module_name));
                        }
                        None => {
                            self.error(format!("cannot find module '{}'", module_name));
                        }
                    }
                }
            }
            Stmt::Mod { name, body, .. } => {
                // Find the module's scope
                let entry = self.symbols.lookup(name).cloned();
                if let Some(SymEntry { kind: SymKind::Module(scope_idx), .. }) = entry {
                    let prev_scope = self.symbols.current_scope;
                    self.symbols.current_scope = scope_idx;
                    for stmt in body {
                        self.resolve_decl(&stmt.node);
                    }
                    self.symbols.current_scope = prev_scope;
                }
            }
        }
    }

    fn resolve_block(&mut self, stmts: &[Spanned<Stmt>]) {
        self.symbols.enter_scope();
        for stmt in stmts {
            self.resolve_decl(&stmt.node);
        }
        self.symbols.exit_scope();
    }

    fn resolve_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident(name) => {
                if self.symbols.lookup(name).is_none() {
                    self.error(format!("undefined name '{}'", name));
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.resolve_expr(lhs);
                self.resolve_expr(rhs);
            }
            Expr::Unary { expr: inner, .. } => {
                self.resolve_expr(inner);
            }
            Expr::Call { func, args } => {
                self.resolve_expr(func);
                for arg in args {
                    self.resolve_expr(arg);
                }
            }
            Expr::MethodCall { obj, args, .. } => {
                self.resolve_expr(obj);
                for arg in args {
                    self.resolve_expr(arg);
                }
            }
            Expr::Field { obj, .. } => {
                self.resolve_expr(obj);
            }
            Expr::Index { obj, index } => {
                self.resolve_expr(obj);
                self.resolve_expr(index);
            }
            Expr::Block(stmts) => {
                self.resolve_block(stmts);
            }
            Expr::If { cond, then, else_ } => {
                self.resolve_expr(cond);
                self.resolve_expr(then);
                if let Some(else_expr) = else_ {
                    self.resolve_expr(else_expr);
                }
            }
            Expr::While { cond, body } => {
                self.resolve_expr(cond);
                self.resolve_expr(body);
            }
            Expr::For { var, iter, body } => {
                self.resolve_expr(iter);
                self.symbols.enter_scope();
                if let Err(e) = self.symbols.define(var, SymKind::Variable(Type::Unit)) {
                    self.error(e);
                }
                self.resolve_expr(body);
                self.symbols.exit_scope();
            }
            Expr::Loop(body) => {
                self.resolve_expr(body);
            }
            Expr::Match { expr, arms } => {
                self.resolve_expr(expr);
                for arm in arms {
                    let enters_scope = matches!(arm.pattern, Pattern::Ident(_) | Pattern::EnumVariant { .. });
                    if enters_scope {
                        self.symbols.enter_scope();
                    }
                    match &arm.pattern {
                        Pattern::Ident(name) => {
                            if let Err(e) = self.symbols.define(name, SymKind::Variable(Type::Unit)) {
                                self.error(e);
                            }
                        }
                        Pattern::EnumVariant { enum_name: _, variant_name: _, bindings } => {
                            for binding in bindings {
                                if binding.is_empty() { continue; } // wildcard _
                                if let Err(e) = self.symbols.define(binding, SymKind::Variable(Type::Unit)) {
                                    self.error(e);
                                }
                            }
                        }
                        _ => {}
                    }
                    if let Some(guard) = &arm.guard {
                        self.resolve_expr(guard);
                    }
                    self.resolve_expr(&arm.body);
                    if enters_scope {
                        self.symbols.exit_scope();
                    }
                }
            }
            Expr::Return(Some(inner)) => {
                self.resolve_expr(inner);
            }
            Expr::StructLit { fields, spread, .. } => {
                for (_, val) in fields {
                    self.resolve_expr(val);
                }
                if let Some(spread_expr) = spread {
                    self.resolve_expr(spread_expr);
                }
            }
            Expr::Array(elems) => {
                for elem in elems {
                    self.resolve_expr(elem);
                }
            }
            Expr::Range { start, end, .. } => {
                self.resolve_expr(start);
                self.resolve_expr(end);
            }
            Expr::Lambda { params, body, .. } => {
                self.symbols.enter_scope();
                for param in params {
                    let ty = param.type_ann.clone().unwrap_or(Type::Unit);
                    if let Err(e) = self.symbols.define(&param.name, SymKind::Variable(ty)) {
                        self.error(e);
                    }
                }
                self.resolve_expr(body);
                self.symbols.exit_scope();
            }
            Expr::Return(None) => {}
            Expr::Yield(inner) => {
                self.resolve_expr(inner);
            }
            Expr::EnumCtor { args, .. } => {
                for arg in args {
                    self.resolve_expr(arg);
                }
            }
            // Literals don't have names to resolve
            Expr::Int(_) | Expr::Float(_) | Expr::Str(_) | Expr::Bool(_)
            | Expr::Unit | Expr::Break | Expr::Continue => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn resolve_program(source: &str) -> std::result::Result<SymbolTable, Vec<Error>> {
        let tokens = Lexer::new(source).tokenize().map_err(|e| vec![e])?;
        let mut program = Parser::new(source, &tokens).parse().map_err(|e| vec![e])?;
        match resolve(&mut program) {
            Ok(t) => Ok(t),
            Err(Error::ParseMultiple { errors }) => Err(errors),
            Err(e) => Err(vec![e]),
        }
    }

    #[test]
    fn test_empty() {
        let table = resolve_program("").unwrap();
        // Option/Result types and their constructors are pre-registered
        assert!(table.lookup("Option").is_some());
        assert!(table.lookup("Result").is_some());
    }

    #[test]
    fn test_let_binding() {
        let table = resolve_program("let x = 42; let y = x;").unwrap();
        assert!(table.lookup("x").is_some());
        assert!(table.lookup("y").is_some());
    }

    #[test]
    fn test_undefined_variable() {
        let result = resolve_program("let x = y;");
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_definition() {
        let result = resolve_program("let x = 1; let x = 2;");
        assert!(result.is_err());
    }

    #[test]
    fn test_function_decl() {
        let table = resolve_program("fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();
        assert!(table.lookup("add").is_some());
    }

    #[test]
    fn test_struct_decl() {
        let table = resolve_program("struct Foo { x: i32 }").unwrap();
        assert!(table.lookup("Foo").is_some());
    }

    #[test]
    fn test_if_else() {
        let table = resolve_program("let x = if true { 1 } else { 2 };").unwrap();
        assert!(table.lookup("x").is_some());
    }

    #[test]
    fn test_mod_decl() {
        let table = resolve_program("mod math { fn add(x, y) { x + y } }").unwrap();
        assert!(table.lookup("math").is_some());
        assert!(matches!(table.lookup("math").unwrap().kind, SymKind::Module(_)));
    }

    #[test]
    fn test_use_import() {
        let table = resolve_program("
            mod math { fn add(x, y) { x + y } }
            use math::add;
        ").unwrap();
        assert!(table.lookup("add").is_some());
        assert!(matches!(table.lookup("add").unwrap().kind, SymKind::Function(_)));
    }

    #[test]
    fn test_use_imports_variable() {
        let table = resolve_program("
            mod config { let pi = 314; }
            use config::pi;
        ").unwrap();
        assert!(table.lookup("pi").is_some());
    }

    #[test]
    fn test_use_nonexistent_module_fails() {
        let result = resolve_program("use foo::bar;");
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_mod_fails() {
        let result = resolve_program("mod a { } mod a { }");
        assert!(result.is_err());
    }
}
