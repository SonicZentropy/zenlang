use std::collections::HashMap;

use crate::ast::*;
use crate::error::{Error, Result};
use crate::span::{SourceLocation, Span};
use crate::symbol::*;

pub type NodeId = usize;

/// Maps expression/statement node addresses to their inferred types.
pub struct TypeMap {
    map: HashMap<*const Expr, Type>,
    _sentinel: Vec<Expr>,
}

impl TypeMap {
    pub fn new() -> Self {
        Self { map: HashMap::new(), _sentinel: Vec::new() }
    }

    pub fn get(&self, expr: &Expr) -> Option<&Type> {
        self.map.get(&(expr as *const Expr))
    }

    fn set(&mut self, expr: &Expr, ty: Type) {
        self.map.insert(expr as *const Expr, ty);
    }
}

pub fn check(program: &Program, symbols: &mut SymbolTable) -> Result<TypeMap> {
    let mut checker = TypeChecker { symbols, type_map: TypeMap::new(), errors: Vec::new(), current_span: Span::new(0, 0) };
    for stmt in &program.stmts {
        checker.set_span(stmt.span);
        checker.check_stmt(&stmt.node, None);
    }
    if checker.errors.is_empty() {
        Ok(checker.type_map)
    } else {
        Err(Error::ParseMultiple { errors: std::mem::take(&mut checker.errors) })
    }
}

struct TypeChecker<'a> {
    symbols: &'a mut SymbolTable,
    type_map: TypeMap,
    errors: Vec<Error>,
    current_span: Span,
}

impl<'a> TypeChecker<'a> {
    fn set_span(&mut self, span: Span) {
        self.current_span = span;
    }

    fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(Error::TypeError {
            location: SourceLocation::new(None, self.current_span, 0, 0),
            msg: msg.into(),
        });
    }

    fn error_at(&mut self, span: Span, msg: impl Into<String>) {
        self.errors.push(Error::TypeError {
            location: SourceLocation::new(None, span, 0, 0),
            msg: msg.into(),
        });
    }

    fn check_stmt(&mut self, stmt: &Stmt, _return_type: Option<&Type>) {
        match stmt {
            Stmt::Let { name, type_ann, init, .. } => {
                let declared = type_ann.as_ref();
                // Temporarily remove the binding so the init expression sees
                // the outer variable (handles `let x = x + 1` shadowing).
                let removed = self.symbols.remove_from_current_scope(name);
                let init_ty = if let Some(init_expr) = init {
                    let ty = self.check_expr(init_expr);
                    if let Some(dt) = declared {
                        if !self.types_compatible(&ty, dt) {
                            self.error(format!(
                                "type mismatch: expected '{}', got '{}'",
                                self.type_display(dt),
                                self.type_display(&ty),
                            ));
                        }
                    }
                    ty
                } else {
                    Type::Unit
                };
                // Restore the binding with the inferred type
                if let Some(entry) = removed {
                    self.symbols.insert_into_current_scope(name, SymKind::Variable(init_ty.clone()));
                    // Update the flat symbol list entry too (preserving its id)
                    drop(entry);
                } else {
                    self.symbols.insert_into_current_scope(name, SymKind::Variable(init_ty.clone()));
                }
            }
            Stmt::Expr(expr) => {
                self.check_expr(expr);
            }
            Stmt::Return(Some(expr)) => {
                self.check_expr(expr);
            }
            Stmt::Return(None) => {}
            Stmt::Fn { name: _, params, return_type, body } => {
                self.symbols.enter_scope();
                for param in params {
                    let ty = param.type_ann.clone().unwrap_or(Type::Unit);
                    if self.symbols.lookup(&param.name).is_none() {
                        let _ = self.symbols.define(&param.name, SymKind::Variable(ty));
                    }
                }
                let expected_ret = return_type.as_ref();
                for stmt in body {
                    self.check_stmt(&stmt.node, expected_ret);
                }
                // Last expression's type should match return type
                if let Some(last) = body.last() {
                    if let Stmt::Expr(e) = &last.node {
                        let ty = self.check_expr(e);
                        if let Some(rt) = expected_ret {
                            if !self.types_compatible(&ty, rt) {
                                self.error_at(last.span, format!(
                                    "function return type mismatch: expected '{}', got '{}'",
                                    self.type_display(rt),
                                    self.type_display(&ty),
                                ));
                            }
                        }
                    }
                }
                self.symbols.exit_scope();
            }
            Stmt::Impl { methods, .. } => {
                for method in methods {
                    self.check_stmt(&method.node, None);
                }
            }
            Stmt::Struct { .. } | Stmt::Enum { .. } => {}
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Type {
        let ty = match expr {
            Expr::Int(_) => Type::I64,
            Expr::Float(_) => Type::F64,
            Expr::Str(_) => Type::Str,
            Expr::Bool(_) => Type::Bool,
            Expr::Unit => Type::Unit,
            Expr::Ident(name) => {
                match self.symbols.lookup(name) {
                    Some(entry) => match &entry.kind {
                        SymKind::Variable(ty) => ty.clone(),
                        SymKind::Function(sig) => {
                            let ret = sig.return_type.clone().unwrap_or(Type::Unit);
                            Type::Fn {
                                params: sig.params.iter().map(|(_, t)| t.clone()).collect(),
                                ret: Box::new(ret),
                            }
                        }
                        _ => {
                            self.error(format!("'{}' is not a variable", name));
                            Type::Unit
                        }
                    },
                    None => {
                        self.error(format!("undefined name '{}'", name));
                        Type::Unit
                    }
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let lt = self.check_expr(lhs);
                let rt = self.check_expr(rhs);
                match op {
                    BinOp::Assign => {
                        // Assignment: rhs type must be compatible with lhs
                        if !self.types_compatible(&rt, &lt) {
                            self.error(format!(
                                "assignment type mismatch: '{}' vs '{}'",
                                self.type_display(&lt),
                                self.type_display(&rt),
                            ));
                        }
                        lt
                    }
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod
                    | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                        if !self.types_compatible(&lt, &rt) {
                            self.error(format!(
                                "type mismatch in arithmetic: '{}' vs '{}'",
                                self.type_display(&lt),
                                self.type_display(&rt),
                            ));
                        }
                        lt
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        if !self.types_compatible(&lt, &rt) {
                            self.error(format!(
                                "type mismatch in comparison: '{}' vs '{}'",
                                self.type_display(&lt),
                                self.type_display(&rt),
                            ));
                        }
                        Type::Bool
                    }
                    BinOp::And | BinOp::Or => {
                        if !matches!(&lt, Type::Bool) {
                            self.error("logical operator requires bool operands");
                        }
                        Type::Bool
                    }
                }
            }
            Expr::Unary { op, expr: inner } => {
                let it = self.check_expr(inner);
                match op {
                    UnOp::Neg => {
                        if !matches!(it, Type::I64 | Type::F32 | Type::F64) {
                            self.error("negation requires numeric type");
                        }
                        it
                    }
                    UnOp::Not => {
                        if !matches!(it, Type::Bool) {
                            self.error("logical not requires bool");
                        }
                        Type::Bool
                    }
                    UnOp::BitNot => {
                        if !matches!(it, Type::I64 | Type::F32 | Type::F64) {
                            self.error("bitwise not requires numeric type");
                        }
                        it
                    }
                }
            }
            Expr::Call { func, args } => {
                let ft = self.check_expr(func);
                for arg in args {
                    self.check_expr(arg);
                }
                match &ft {
                    Type::Fn { ret, .. } => *ret.clone(),
                    _ => {
                        self.error("calling non-function type");
                        Type::Unit
                    }
                }
            }
            Expr::MethodCall { obj, args, .. } => {
                self.check_expr(obj);
                for arg in args {
                    self.check_expr(arg);
                }
                Type::Unit // methods return unit by default
            }
            Expr::Field { obj, field: _ } => {
                self.check_expr(obj);
                Type::Unit // fields typed by struct definition
            }
            Expr::Index { obj, index } => {
                let ot = self.check_expr(obj);
                self.check_expr(index);
                match &ot {
                    Type::Array(inner) => *inner.clone(),
                    _ => {
                        self.error("indexing non-array type");
                        Type::Unit
                    }
                }
            }
            Expr::Block(stmts) => {
                self.symbols.enter_scope();
                for stmt in stmts {
                    self.check_stmt(&stmt.node, None);
                }
                // Last expression is the block's value
                let result = match stmts.last() {
                    Some(last) => match &last.node {
                        Stmt::Expr(e) => self.check_expr(e),
                        _ => Type::Unit,
                    },
                    None => Type::Unit,
                };
                self.symbols.exit_scope();
                result
            }
            Expr::If { cond, then, else_ } => {
                let ct = self.check_expr(cond);
                if !matches!(ct, Type::Bool) {
                    self.error("if condition must be bool");
                }
                let tt = self.check_expr(then);
                let et = else_.as_ref().map(|e| self.check_expr(e));
                match et {
                    Some(et) if self.types_compatible(&tt, &et) => tt,
                    Some(_) => {
                        self.error("if/else branches must have compatible types");
                        Type::Unit
                    }
                    None => Type::Unit,
                }
            }
            Expr::While { cond, body } => {
                let ct = self.check_expr(cond);
                if !matches!(ct, Type::Bool) {
                    self.error("while condition must be bool");
                }
                self.check_expr(body);
                Type::Unit
            }
            Expr::For { var, iter, body } => {
                let iter_ty = self.check_expr(iter);
                self.symbols.enter_scope();
                let loop_ty = match iter.as_ref() {
                    Expr::Range { start, .. } => self.check_expr(start),
                    _ => match &iter_ty {
                        Type::Array(inner) => *inner.clone(),
                        Type::Str => Type::Str,
                        _ => Type::I64,
                    },
                };
                // Remove any existing binding (from resolver) and re-insert
                self.symbols.remove_from_current_scope(var);
                self.symbols.insert_into_current_scope(var, SymKind::Variable(loop_ty));
                self.check_expr(body);
                self.symbols.exit_scope();
                Type::Unit
            }
            Expr::Loop(body) => {
                self.check_expr(body);
                Type::Unit
            }
            Expr::Match { expr, arms } => {
                let _mt = self.check_expr(expr);
                let mut arm_types = Vec::new();
                for arm in arms {
                    if let Pattern::Ident(name) = &arm.pattern {
                        self.symbols.enter_scope();
                        if self.symbols.lookup(name).is_none() {
                            let _ = self.symbols.define(name, SymKind::Variable(Type::Unit));
                        }
                    }
                    if let Some(guard) = &arm.guard {
                        self.check_expr(guard);
                    }
                    arm_types.push(self.check_expr(&arm.body));
                    if matches!(arm.pattern, Pattern::Ident(_)) {
                        self.symbols.exit_scope();
                    }
                }
                // All arms should have compatible types
                let first = arm_types.first().cloned().unwrap_or(Type::Unit);
                for at in &arm_types {
                    if !self.types_compatible(&first, at) {
                        self.error("match arms must have compatible types");
                    }
                }
                first
            }
            Expr::Return(Some(inner)) => {
                self.check_expr(inner);
                Type::Unit
            }
            Expr::Return(None) => Type::Unit,
            Expr::Break | Expr::Continue => Type::Unit,
            Expr::StructLit { name, fields } => {
                let entry = self.symbols.lookup(name).cloned();
                match entry {
                    Some(SymEntry { kind: SymKind::Struct(def), .. }) => {
                        for (fname, fval) in fields {
                            let found = def.fields.iter().find(|f| f.name == *fname);
                            if found.is_none() {
                                self.error(format!(
                                    "struct '{}' has no field '{}'",
                                    name, fname
                                ));
                            }
                            self.check_expr(fval);
                        }
                    }
                    Some(_) => {
                        self.error(format!("'{}' is not a struct", name));
                    }
                    None => {
                        self.error(format!("undefined struct '{}'", name));
                    }
                }
                Type::Named(name.clone())
            }
            Expr::Array(elems) => {
                let mut elem_types = Vec::new();
                for elem in elems {
                    elem_types.push(self.check_expr(elem));
                }
                let inner = elem_types.first().cloned().unwrap_or(Type::Unit);
                Type::Array(Box::new(inner))
            }
            Expr::Range { start, end, .. } => {
                self.check_expr(start);
                self.check_expr(end);
                Type::Unit // TODO: proper range type
            }
            Expr::Lambda { params: _, return_type: _, body } => {
                let ret = self.check_expr(body);
                Type::Fn { params: Vec::new(), ret: Box::new(ret) }
            }
        };
        self.type_map.set(expr, ty.clone());
        ty
    }

    fn types_compatible(&self, a: &Type, b: &Type) -> bool {
        let a = self.resolve_named(a);
        let b = self.resolve_named(b);
        match (&a, &b) {
            // Unit (from foreign field access, unknown at compile time) is compatible with everything
            (Type::Unit, _) | (_, Type::Unit) => true,
            (Type::I64, Type::I64) => true,
            (Type::F32, Type::F32) => true,
            (Type::F64, Type::F64) => true,
            (Type::F64, Type::I64) | (Type::I64, Type::F64) => true, // implicit i64↔f64
            (Type::F32, Type::I64) | (Type::I64, Type::F32) => true, // implicit i64↔f32
            (Type::F32, Type::F64) | (Type::F64, Type::F32) => true, // implicit f32↔f64
            (Type::Bool, Type::Bool) => true,
            (Type::Str, Type::Str) => true,
            (Type::Named(a), Type::Named(b)) => a == b,
            (Type::Array(a), Type::Array(b)) => self.types_compatible(a, b),
            _ => false,
        }
    }

    fn resolve_named(&self, ty: &Type) -> Type {
        match ty {
            Type::Named(s) if s == "int" => Type::I64,
            Type::Named(s) if s == "i32" => Type::I64,
            Type::Named(s) if s == "f32" => Type::F32,
            Type::Named(s) if s == "float" => Type::F64,
            Type::Named(s) if s == "bool" => Type::Bool,
            Type::Named(s) if s == "str" => Type::Str,
            _ => ty.clone(),
        }
    }

    fn type_display(&self, ty: &Type) -> String {
        match ty {
            Type::I64 => "i64".into(),
            Type::F32 => "f32".into(),
            Type::F64 => "f64".into(),
            Type::Bool => "bool".into(),
            Type::Str => "str".into(),
            Type::Unit => "()".into(),
            Type::Named(s) => s.to_string(),
            Type::Array(inner) => format!("[{}]", self.type_display(inner)),
            Type::Fn { params, ret } => {
                let p: Vec<String> = params.iter().map(|t| self.type_display(t)).collect();
                format!("({}) -> {}", p.join(", "), self.type_display(ret))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::resolver;

    fn type_check(source: &str) -> Result<TypeMap> {
        let tokens = Lexer::new(source).tokenize()?;
        let mut program = Parser::new(source, &tokens).parse()?;
        let mut symbols = resolver::resolve(&mut program)?;
        check(&program, &mut symbols)
    }

    #[test]
    fn test_literal_types() {
        let _tm = type_check("let x = 42; let y = 3.14; let z = true; let w = \"hello\";").unwrap();
        // Just check no errors
    }

    #[test]
    fn test_type_mismatch() {
        let result = type_check("let x: i32 = true;");
        assert!(result.is_err());
    }

    #[test]
    fn test_arithmetic() {
        let result = type_check("let x = 1 + 2; let y = 1.0 + 2.0; let z = 1 + 2.0;");
        assert!(result.is_ok());
    }

    #[test]
    fn test_if_else_types() {
        let result = type_check("let x = if true { 1 } else { 2 };");
        assert!(result.is_ok());
    }

    #[test]
    fn test_bool_condition() {
        let result = type_check("if 42 { 1 } else { 2 };");
        assert!(result.is_err());
    }
}
