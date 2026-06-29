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
    // Pre-register native functions so the resolver knows these names exist
    for name in native_names {
        let sig = FnSignature {
            name: name.clone(),
            params: vec![],
            return_type: Some(Type::I64),
        };
        let _ = resolver.symbols.define(name, SymKind::Function(sig));
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

    fn register_top_level(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Fn { name, params, return_type, body: _ } => {
                let sig = FnSignature {
                    name: name.to_string(),
                    params: params.iter().map(|p| {
                        let ty = p.type_ann.clone().unwrap_or(Type::Unit);
                        (p.name.to_string(), ty)
                    }).collect(),
                    return_type: return_type.clone(),
                };
                if let Err(e) = self.symbols.define(name, SymKind::Function(sig)) {
                    self.error(e);
                }
            }
            Stmt::Struct { name, fields } => {
                let def = StructDef { name: name.to_string(), fields: fields.clone() };
                if let Err(e) = self.symbols.define(name, SymKind::Struct(def)) {
                    self.error(e);
                }
            }
            Stmt::Enum { name, variants } => {
                let v = variants.iter()
                    .map(|v| (v.name.to_string(), v.fields.clone()))
                    .collect();
                let def = EnumDef { name: name.to_string(), variants: v };
                if let Err(e) = self.symbols.define(name, SymKind::Enum(def)) {
                    self.error(e);
                }
            }
            Stmt::Impl { type_name, methods } => {
                for method in methods {
                    if let Stmt::Fn { name, params, return_type, body: _ } = &method.node {
                        let sig = FnSignature {
                            name: format!("{}::{}", type_name, name),
                            params: params.iter().map(|p| {
                                let ty = p.type_ann.clone().unwrap_or(Type::Unit);
                                (p.name.to_string(), ty)
                            }).collect(),
                            return_type: return_type.clone(),
                        };
                        if let Err(e) = self.symbols.define(
                            &format!("{}::{}", type_name, name),
                            SymKind::Function(sig),
                        ) {
                            self.error(e);
                        }
                    }
                }
            }
            _ => {} // let, expr stmts are handled in second pass
        }
    }

    // ---------- Resolution (second pass) ----------

    fn resolve_decl(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Fn { name: _, params, body, .. } => {
                self.symbols.enter_scope();
                for param in params {
                    // Infer type for now (will be refined by type checker)
                    let ty = param.type_ann.clone().unwrap_or(Type::Unit);
                    if let Err(e) = self.symbols.define(&param.name, SymKind::Variable(ty)) {
                        self.error(e);
                    }
                }
                self.resolve_block(body);
                self.symbols.exit_scope();
            }
            Stmt::Impl { methods, .. } => {
                for method in methods {
                    self.resolve_decl(&method.node);
                }
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
            Stmt::Expr(expr) => {
                self.resolve_expr(expr);
            }
            Stmt::Return(Some(expr)) => {
                self.resolve_expr(expr);
            }
            Stmt::Return(None) => {}
            Stmt::Struct { .. } | Stmt::Enum { .. } => {
                // Already registered, nothing to resolve
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
                    if let Pattern::Ident(name) = &arm.pattern {
                        self.symbols.enter_scope();
                        if let Err(e) = self.symbols.define(name, SymKind::Variable(Type::Unit)) {
                            self.error(e);
                        }
                    }
                    if let Some(guard) = &arm.guard {
                        self.resolve_expr(guard);
                    }
                    self.resolve_expr(&arm.body);
                    if matches!(arm.pattern, Pattern::Ident(_)) {
                        self.symbols.exit_scope();
                    }
                }
            }
            Expr::Return(Some(inner)) => {
                self.resolve_expr(inner);
            }
            Expr::StructLit { fields, .. } => {
                for (_, val) in fields {
                    self.resolve_expr(val);
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
        assert!(table.globals().is_empty());
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
}
