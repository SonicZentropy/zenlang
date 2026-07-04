use std::collections::HashMap;

use crate::ast::*;
use crate::error::{Error, Result};
use crate::span::{SourceLocation, Span};
use crate::symbol::*;
use compact_str::CompactString;

/// Opaque identifier for AST nodes — used by [`TypeMap`] for lookups.
pub type NodeId = usize;

/// Maps expression/statement node addresses to their inferred types.
///
/// Populated by [`check()`] and used by the compiler during code generation
/// to determine the types of intermediate values.
pub struct TypeMap {
    map: HashMap<*const Expr, Type>,
    _sentinel: Vec<Expr>,
}

impl Default for TypeMap {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            _sentinel: Vec::new(),
        }
    }

    pub fn get(&self, expr: &Expr) -> Option<&Type> {
        self.map.get(&(expr as *const Expr))
    }

    fn set(&mut self, expr: &Expr, ty: Type) {
        self.map.insert(expr as *const Expr, ty);
    }
}

/// Run type-checking on the entire program.
///
/// Returns a [`TypeMap`] mapping each expression to its inferred type.
/// Errors are collected and returned as a single `Error::TypeError` (or
/// `Error::ParseMultiple` if there are multiple).
pub fn check(program: &Program, symbols: &mut SymbolTable) -> Result<TypeMap> {
    let mut checker = TypeChecker {
        symbols,
        type_map: TypeMap::new(),
        errors: Vec::new(),
        current_span: Span::new(0, 0),
        type_vars: TypeVarStore::new(),
    };
    for stmt in &program.stmts {
        checker.set_span(stmt.span);
        checker.check_stmt(&stmt.node, None);
    }
    if checker.errors.is_empty() {
        Ok(checker.type_map)
    } else {
        Err(Error::ParseMultiple {
            errors: std::mem::take(&mut checker.errors),
        })
    }
}

/// Storage for local unification type variables.
///
/// During type inference, unresolved types are represented as `Type::Var(id)`.
/// The `constraints` map tracks the type each variable has been unified with.
/// Once a variable is constrained, `resolve_var` returns the concrete type.
struct TypeVarStore {
    constraints: HashMap<u64, Type>,
}

impl TypeVarStore {
    fn new() -> Self {
        Self {
            constraints: HashMap::new(),
        }
    }

    /// Constrain a type variable to a concrete type.
    /// Returns `false` if the variable was already constrained to a different type.
    fn constrain(&mut self, id: u64, ty: Type) -> bool {
        if let Some(existing) = self.constraints.get(&id) {
            // Already constrained — check compatibility
            return existing == &ty;
        }
        self.constraints.insert(id, ty);
        true
    }

    /// Resolve a type variable to its constrained type, if any.
    fn resolve(&self, id: u64) -> Option<&Type> {
        self.constraints.get(&id)
    }
}

struct TypeChecker<'a> {
    symbols: &'a mut SymbolTable,
    type_map: TypeMap,
    errors: Vec<Error>,
    current_span: Span,
    type_vars: TypeVarStore,
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

    /// Resolve a type by following `Type::Var` chains to their constrained type.
    /// If a variable is unconstrained, it remains as `Type::Var`.
    fn resolve_var(&self, ty: &Type) -> Type {
        match ty {
            Type::Var(id) => {
                if let Some(resolved) = self.type_vars.resolve(*id) {
                    // Recurse in case the constraint is itself a variable
                    self.resolve_var(resolved)
                } else {
                    ty.clone()
                }
            }
            _ => ty.clone(),
        }
    }

    /// Unify two types: constrain any type variables so that both types become
    /// equal.  Returns `true` if unification succeeds, `false` on conflict.
    fn unify(&mut self, a: &Type, b: &Type) -> bool {
        let a = self.resolve_var(a).clone();
        let b = self.resolve_var(b).clone();
        match (&a, &b) {
            // Same concrete type — always compatible
            (Type::I64, Type::I64)
            | (Type::F32, Type::F32)
            | (Type::F64, Type::F64)
            | (Type::Bool, Type::Bool)
            | (Type::Str, Type::Str)
            | (Type::Unit, Type::Unit)
            | (Type::Any, _)
            | (_, Type::Any)
            | (_, Type::Unknown) => true,
            // Numeric coercion
            (Type::I64, Type::F64) | (Type::F64, Type::I64) => true,
            (Type::I64, Type::F32) | (Type::F32, Type::I64) => true,
            (Type::F32, Type::F64) | (Type::F64, Type::F32) => true,
            // Generic type parameters are type-erased
            (Type::Generic(_), _) | (_, Type::Generic(_)) => true,
            // Unconstrained variable — constrain to the other type
            (Type::Var(id), _) => {
                self.type_vars.constrain(*id, b.clone());
                true
            }
            (_, Type::Var(id)) => {
                self.type_vars.constrain(*id, a.clone());
                true
            }
            // Named types — delegate to types_compatible
            (Type::Named(_), _) | (_, Type::Named(_)) => self.types_compatible(&a, &b),
            // Compound types — recurse
            (Type::Array(ae), Type::Array(be)) => self.unify(ae, be),
            (Type::Option(ao), Type::Option(bo)) => self.unify(ao, bo),
            (Type::Iter(ae), Type::Iter(be)) => self.unify(ae, be),
            (Type::Result(oka, erra), Type::Result(okb, errb)) => {
                self.unify(oka, okb) && self.unify(erra, errb)
            }
            (
                Type::Fn {
                    params: pa,
                    ret: ra,
                },
                Type::Fn {
                    params: pb,
                    ret: rb,
                },
            ) => {
                if pa.len() != pb.len() {
                    return false;
                }
                for (a, b) in pa.iter().zip(pb.iter()) {
                    if !self.unify(a, b) {
                        return false;
                    }
                }
                self.unify(ra, rb)
            }
            _ => false,
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt, _return_type: Option<&Type>) {
        match stmt {
            Stmt::Let {
                name,
                type_ann,
                init,
                ..
            } => {
                let declared = type_ann.as_ref();
                // Temporarily remove the binding so the init expression sees
                // the outer variable (handles `let x = x + 1` shadowing).
                let removed = self.symbols.remove_from_current_scope(name);
                let init_ty = if let Some(init_expr) = init {
                    let ty = self.check_expr(init_expr);
                    if let Some(dt) = declared {
                        // Unify init type with the declared type annotation
                        if !self.unify(&ty, dt) {
                            self.error(format!(
                                "type mismatch: expected '{}', got '{}'",
                                self.type_display(dt),
                                self.type_display(&ty),
                            ));
                        }
                        self.resolve_var(dt)
                    } else {
                        ty
                    }
                } else {
                    Type::Any
                };
                // Restore the binding with the inferred type
                if let Some(entry) = removed {
                    self.symbols
                        .insert_into_current_scope(name, SymKind::Variable(init_ty.clone()));
                    // Update the flat symbol list entry too (preserving its id)
                    drop(entry);
                } else {
                    self.symbols
                        .insert_into_current_scope(name, SymKind::Variable(init_ty.clone()));
                }
            }
            Stmt::Const {
                name,
                type_ann,
                init,
                ..
            } => {
                let declared = type_ann.as_ref();
                let ty = self.check_expr(init);
                if let Some(dt) = declared
                    && !self.types_compatible(&ty, dt)
                {
                    self.error(format!(
                        "type mismatch: expected '{}', got '{}'",
                        self.type_display(dt),
                        self.type_display(&ty),
                    ));
                }
                self.symbols
                    .insert_into_current_scope(name, SymKind::Variable(ty));
            }
            Stmt::Expr(expr) => {
                self.check_expr(expr);
            }
            Stmt::Return(Some(expr)) => {
                let ty = self.check_expr(expr);
                if let Some(rt) = _return_type
                    && !self.types_compatible(&ty, rt)
                {
                    self.error(format!(
                        "return type mismatch: expected '{}', got '{}'",
                        self.type_display(rt),
                        self.type_display(&ty),
                    ));
                }
            }
            Stmt::Return(None) => {
                if let Some(rt) = _return_type
                    && !self.types_compatible(&Type::Unit, rt)
                {
                    self.error(format!(
                        "return type mismatch: expected '{}', got '()'",
                        self.type_display(rt),
                    ));
                }
            }
            Stmt::Fn {
                name: _,
                type_params,
                params,
                return_type,
                body,
                ..
            } => {
                self.symbols.enter_scope();
                // Register generic type parameters in scope
                for tp in type_params {
                    if self.symbols.lookup(&tp.name).is_none() {
                        let _ = self
                            .symbols
                            .define(&tp.name, SymKind::TypeParam(tp.name.to_string()));
                    }
                }
                for param in params {
                    let ty = param.type_ann.clone().unwrap_or(Type::Any);
                    self.symbols.remove_from_current_scope(&param.name);
                    let _ = self.symbols.define(&param.name, SymKind::Variable(ty));
                }
                let expected_ret = return_type.as_ref();
                for stmt in body {
                    self.check_stmt(&stmt.node, expected_ret);
                }
                // Last expression's type should match return type
                if let Some(last) = body.last()
                    && let Stmt::Expr(e) = &last.node
                {
                    let ty = self.check_expr(e);
                    if let Some(rt) = expected_ret
                        && !self.types_compatible(&ty, rt)
                    {
                        self.error_at(
                            last.span,
                            format!(
                                "function return type mismatch: expected '{}', got '{}'",
                                self.type_display(rt),
                                self.type_display(&ty),
                            ),
                        );
                    }
                }
                self.symbols.exit_scope();
            }
            Stmt::Impl {
                type_name,
                type_params,
                methods,
                ..
            } => {
                for method in methods {
                    if let Stmt::Fn {
                        name: _,
                        type_params: method_type_params,
                        params,
                        return_type,
                        body,
                        ..
                    } = &method.node
                    {
                        self.symbols.enter_scope();
                        // Register generic type parameters from impl block
                        for tp in type_params {
                            if self.symbols.lookup(&tp.name).is_none() {
                                let _ = self
                                    .symbols
                                    .define(&tp.name, SymKind::TypeParam(tp.name.to_string()));
                            }
                        }
                        // Register generic type parameters from method
                        for tp in method_type_params {
                            if self.symbols.lookup(&tp.name).is_none() {
                                let _ = self
                                    .symbols
                                    .define(&tp.name, SymKind::TypeParam(tp.name.to_string()));
                            }
                        }
                        for param in params {
                            let ty = if param.type_ann.is_none() && param.name == "self" {
                                Type::Named(type_name.clone())
                            } else {
                                param.type_ann.clone().unwrap_or(Type::Any)
                            };
                            if self.symbols.lookup(&param.name).is_none() {
                                let _ = self.symbols.define(&param.name, SymKind::Variable(ty));
                            }
                        }
                        let expected_ret = return_type.as_ref();
                        for stmt in body {
                            self.check_stmt(&stmt.node, expected_ret);
                        }
                        if let Some(last) = body.last()
                            && let Stmt::Expr(e) = &last.node
                        {
                            let ty = self.check_expr(e);
                            if let Some(rt) = expected_ret
                                && !self.types_compatible(&ty, rt)
                            {
                                self.error_at(
                                    last.span,
                                    format!(
                                        "function return type mismatch: expected '{}', got '{}'",
                                        self.type_display(rt),
                                        self.type_display(&ty),
                                    ),
                                );
                            }
                        }
                        self.symbols.exit_scope();
                    }
                }
            }
            Stmt::Struct { .. }
            | Stmt::Enum { .. }
            | Stmt::Use { .. }
            | Stmt::Trait { .. }
            | Stmt::Type { .. } => {}
            Stmt::Mod { body, .. } => {
                self.symbols.enter_scope();
                for stmt in body {
                    self.check_stmt(&stmt.node, _return_type);
                }
                self.symbols.exit_scope();
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Type {
        let ty = match expr {
            Expr::Int(_) => Type::I64,
            Expr::Float(_) => Type::F64,
            Expr::Str(_) => Type::Str,
            Expr::Bool(_) => Type::Bool,
            Expr::Unit => Type::Unit,
            Expr::Ident(name) => match self.symbols.lookup(name) {
                Some(entry) => match &entry.kind {
                    SymKind::Variable(ty) => ty.clone(),
                    SymKind::Function(sig) => {
                        let ret = sig.return_type.clone().unwrap_or(Type::Any);
                        Type::Fn {
                            params: sig.params.iter().map(|(_, t)| t.clone()).collect(),
                            ret: Box::new(ret),
                        }
                    }
                    SymKind::EnumConstructor {
                        enum_name,
                        variant_name: _,
                        tag: _,
                        fields,
                    } if fields.is_empty() => Type::Named(enum_name.clone().into()),
                    _ => {
                        self.error(format!("'{}' is not a variable", name));
                        Type::Unit
                    }
                },
                None => {
                    self.error(format!("undefined name '{}'", name));
                    Type::Unit
                }
            },
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
                    BinOp::Add
                    | BinOp::Sub
                    | BinOp::Mul
                    | BinOp::Div
                    | BinOp::Mod
                    | BinOp::BitAnd
                    | BinOp::BitOr
                    | BinOp::BitXor
                    | BinOp::Shl
                    | BinOp::Shr => {
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
                        if !matches!(&lt, Type::Bool | Type::Any) {
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
                        if !matches!(it, Type::Bool | Type::Any) {
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
                // Check if this is an enum constructor call
                let constructor_info: Option<(String, String, Vec<Type>)> =
                    if let Expr::Ident(name) = func.as_ref() {
                        if let Some(entry) = self.symbols.lookup(name) {
                            if let SymKind::EnumConstructor {
                                enum_name,
                                variant_name,
                                tag: _,
                                fields,
                            } = &entry.kind
                            {
                                Some((enum_name.clone(), variant_name.clone(), fields.clone()))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                if let Some((enum_name, variant_name, fields)) = constructor_info {
                    if args.len() != fields.len() {
                        self.error(format!(
                            "'{}' expects {} arguments, got {}",
                            variant_name,
                            fields.len(),
                            args.len()
                        ));
                        return Type::Unit;
                    }
                    for (arg, field_ty) in args.iter().zip(fields.iter()) {
                        let arg_ty = self.check_expr(arg);
                        if !self.types_compatible(&arg_ty, field_ty) {
                            self.error(format!(
                                "argument type mismatch for '{}': expected '{}', got '{}'",
                                variant_name,
                                self.type_display(field_ty),
                                self.type_display(&arg_ty),
                            ));
                        }
                    }
                    return Type::Named(enum_name.into());
                }
                let ft = self.check_expr(func);
                match &ft {
                    Type::Fn { params, ret } => {
                        // Empty params = variadic (e.g. print), skip validation
                        if !params.is_empty() {
                            if params.len() != args.len() {
                                self.error(format!(
                                    "expected {} arguments, got {}",
                                    params.len(),
                                    args.len(),
                                ));
                            }
                            for (i, arg) in args.iter().enumerate() {
                                let arg_ty = self.check_expr(arg);
                                if let Some(param_ty) = params.get(i)
                                    && !self.types_compatible(&arg_ty, param_ty)
                                {
                                    self.error(format!(
                                        "argument {} type mismatch: expected '{}', got '{}'",
                                        i,
                                        self.type_display(param_ty),
                                        self.type_display(&arg_ty),
                                    ));
                                }
                            }
                        } else {
                            for arg in args {
                                self.check_expr(arg);
                            }
                        }
                        *ret.clone()
                    }
                    // Type-erased value (e.g. an untyped function
                    // parameter used as a callback, like `f` in
                    // `fn map(iterable, f) { ... f(v) ... }`). We can't
                    // statically validate the call, so let it through —
                    // the VM will raise a runtime error if it's genuinely
                    // not callable.
                    Type::Any => {
                        for arg in args {
                            self.check_expr(arg);
                        }
                        Type::Any
                    }
                    _ => {
                        for arg in args {
                            self.check_expr(arg);
                        }
                        self.error("calling non-function type");
                        Type::Unit
                    }
                }
            }
            Expr::EnumCtor {
                enum_name,
                variant_name,
                args,
            } => {
                let qualified = format!("{}::{}", enum_name, variant_name);
                let lookup_fields = self
                    .symbols
                    .lookup(&qualified)
                    .and_then(|entry| match &entry.kind {
                        SymKind::EnumConstructor { fields, .. } => Some(fields.clone()),
                        _ => None,
                    });
                if let Some(fields) = lookup_fields {
                    if args.len() != fields.len() {
                        self.error(format!(
                            "'{}' expects {} arguments, got {}",
                            qualified,
                            fields.len(),
                            args.len()
                        ));
                        for arg in args {
                            self.check_expr(arg);
                        }
                        return Type::Unit;
                    }
                    for (arg, field_ty) in args.iter().zip(fields.iter()) {
                        let arg_ty = self.check_expr(arg);
                        if !self.types_compatible(&arg_ty, field_ty) {
                            self.error(format!(
                                "argument type mismatch for '{}': expected '{}', got '{}'",
                                qualified,
                                self.type_display(field_ty),
                                self.type_display(&arg_ty),
                            ));
                        }
                    }
                    return Type::Named(enum_name.clone());
                }
                for arg in args {
                    self.check_expr(arg);
                }
                self.error(format!("no enum constructor '{}'", qualified));
                Type::Unit
            }
            Expr::MethodCall { obj, method, args } => {
                let obj_ty = self.check_expr(obj);
                match &obj_ty {
                    Type::Named(struct_name) => {
                        let qualified = format!("{}::{}", struct_name, method);
                        // Extract info from symbols before calling self methods (avoid borrow conflict)
                        let method_info = self.symbols.lookup(&qualified).and_then(|entry| {
                            if let SymKind::Function(sig) = &entry.kind {
                                let has_self = sig
                                    .params
                                    .first()
                                    .map(|(n, _)| n == "self")
                                    .unwrap_or(false);
                                let param_tys: Vec<Type> = if has_self {
                                    sig.params[1..].iter().map(|(_, t)| t.clone()).collect()
                                } else {
                                    sig.params.iter().map(|(_, t)| t.clone()).collect()
                                };
                                Some((sig.return_type.clone().unwrap_or(Type::Unit), param_tys))
                            } else {
                                None
                            }
                        });
                        match method_info {
                            Some((ret_ty, param_tys)) => {
                                if args.len() != param_tys.len() {
                                    self.error(format!(
                                        "method '{}::{}' expects {} argument(s), got {}",
                                        struct_name,
                                        method,
                                        param_tys.len(),
                                        args.len(),
                                    ));
                                }
                                for (i, arg) in args.iter().enumerate() {
                                    let arg_ty = self.check_expr(arg);
                                    if let Some(param_ty) = param_tys.get(i)
                                        && !self.types_compatible(&arg_ty, param_ty)
                                    {
                                        self.error(format!(
                                                "argument {} type mismatch for '{}::{}': expected '{}', got '{}'",
                                                i, struct_name, method,
                                                self.type_display(param_ty),
                                                self.type_display(&arg_ty),
                                            ));
                                    }
                                }
                                ret_ty
                            }
                            None => {
                                if self.symbols.lookup(&qualified).is_some() {
                                    self.error(format!("'{}' is not a function", qualified));
                                } else {
                                    self.error(format!(
                                        "no method named '{}' on struct '{}'",
                                        method, struct_name
                                    ));
                                }
                                Type::Unit
                            }
                        }
                    }
                    // `Type::Any` is the type-erased "compatible with
                    // anything" placeholder for generic/native values
                    // (see native_fn_sigs()); method calls on such values
                    // (e.g. `.next()` on an iterator returned by `iter()`)
                    // can't be statically validated, so allow them through.
                    Type::Any => {
                        for arg in args {
                            self.check_expr(arg);
                        }
                        Type::Any
                    }
                    // `Type::Str` — built-in string with methods:
                    // len, contains, trim, to_upper, to_lower, substring,
                    // is_empty, starts_with, ends_with
                    Type::Str => {
                        for arg in args {
                            self.check_expr(arg);
                        }
                        Type::Any
                    }
                    // `Type::Array(..)` — built-in array with methods:
                    // push, pop, len, insert, remove, contains, is_empty, clear
                    Type::Array(_) => {
                        for arg in args {
                            self.check_expr(arg);
                        }
                        Type::Any
                    }
                    // `Type::Unknown` requires narrowing before method calls
                    Type::Unknown => {
                        for arg in args {
                            self.check_expr(arg);
                        }
                        self.error(
                            "cannot call method on 'unknown' type; narrow via match or cast first",
                        );
                        Type::Unit
                    }
                    _ => {
                        for arg in args {
                            self.check_expr(arg);
                        }
                        self.error(format!(
                            "cannot call method on type '{}'",
                            self.type_display(&obj_ty)
                        ));
                        Type::Unit
                    }
                }
            }
            Expr::Field { obj, field } => {
                let obj_ty = self.check_expr(obj);
                match &obj_ty {
                    Type::Named(struct_name) => {
                        // Resolve through transparent type aliases
                        let resolved_name = self.resolve_type_alias_name(struct_name);
                        if let Some(entry) = self.symbols.lookup(&resolved_name) {
                            if let SymKind::Struct(def) = &entry.kind {
                                if let Some(f) = def.fields.iter().find(|f| f.name == *field) {
                                    f.type_ann.clone()
                                } else {
                                    self.error(format!(
                                        "struct '{}' has no field '{}'",
                                        resolved_name, field
                                    ));
                                    Type::Unit
                                }
                            } else {
                                self.error(format!("'{}' is not a struct", resolved_name));
                                Type::Unit
                            }
                        } else {
                            self.error(format!("undefined struct '{}'", resolved_name));
                            Type::Unit
                        }
                    }
                    // `Type::Any` is the type-erased placeholder for
                    // foreign values and generic native returns. Field access
                    // on such values can't be statically validated, so allow
                    // it through (field access will be resolved at runtime).
                    Type::Any => Type::Any,
                    // `Type::Unknown` requires narrowing before field access
                    Type::Unknown => {
                        self.error(
                            "cannot access field on 'unknown' type; narrow via match or cast first",
                        );
                        Type::Unit
                    }
                    _ => {
                        self.error(format!(
                            "cannot access field on type '{}'",
                            self.type_display(&obj_ty)
                        ));
                        Type::Unit
                    }
                }
            }
            Expr::Index { obj, index } => {
                let ot = self.check_expr(obj);
                self.check_expr(index);
                match &ot {
                    Type::Array(inner) => *inner.clone(),
                    Type::Str => Type::Str,
                    Type::Iter(_) => {
                        self.error(
                            "cannot index lazy iterator; call collect() to \
                             materialize it into an array (e.g. \
                             collect(arr |> map(_, f))[0])",
                        );
                        Type::Unit
                    }
                    // `Type::Any` and `Type::Var` / `Type::Generic` are
                    // type-erased placeholders — indexing them may or may not
                    // work at runtime, so we let it through.
                    Type::Any | Type::Var(_) | Type::Generic(_) => Type::Any,
                    // `Type::Unknown` requires narrowing before access.
                    Type::Unknown => {
                        self.error(
                            "cannot index 'unknown' type; \
                             narrow via match or cast first",
                        );
                        Type::Unit
                    }
                    // Everything else (structs, functions, scalars, etc.)
                    // definitively does not support indexing.
                    _ => {
                        self.error(format!(
                            "type '{}' does not support indexing",
                            self.type_display(&ot),
                        ));
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
                // `Type::Any` covers type-erased values (e.g. the result of
                // calling an untyped callback parameter) that can't be
                // statically proven boolean; let the VM enforce it at runtime.
                if !matches!(ct, Type::Bool | Type::Any) {
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
                if !matches!(ct, Type::Bool | Type::Any) {
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
                        Type::Iter(inner) => *inner.clone(),
                        // Ranges (as a value, not an inline literal), maps,
                        // custom struct/foreign iterators, and anything else
                        // type-erased (`Type::Any`) can't have their element
                        // type known statically here — `Type::Any` is the
                        // "compatible with anything" placeholder, so uses of
                        // the loop variable aren't spuriously rejected by
                        // later type checks.
                        _ => Type::Any,
                    },
                };
                // Remove any existing binding (from resolver) and re-insert
                self.symbols.remove_from_current_scope(var);
                self.symbols
                    .insert_into_current_scope(var, SymKind::Variable(loop_ty));
                self.check_expr(body);
                self.symbols.exit_scope();
                Type::Unit
            }
            Expr::Loop(body) => {
                self.check_expr(body);
                Type::Unit
            }
            Expr::Match { expr, arms } => {
                let mt = self.check_expr(expr);
                let mut arm_types = Vec::new();
                let enum_def: Option<EnumDef> = match &mt {
                    Type::Named(n) => {
                        if let Some(entry) = self.symbols.lookup(n) {
                            if let SymKind::Enum(def) = &entry.kind {
                                Some(def.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    Type::Result(_, _) | Type::Option(_) => {
                        let name = match &mt {
                            Type::Result(_, _) => "Result",
                            Type::Option(_) => "Option",
                            _ => unreachable!(),
                        };
                        if let Some(entry) = self.symbols.lookup(name) {
                            if let SymKind::Enum(def) = &entry.kind {
                                Some(def.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                for arm in arms {
                    let enters_scope =
                        matches!(arm.pattern, Pattern::Ident(_) | Pattern::EnumVariant { .. });
                    if enters_scope {
                        self.symbols.enter_scope();
                    }
                    match &arm.pattern {
                        Pattern::Ident(name) => {
                            if self.symbols.lookup(name).is_none() {
                                let _ = self.symbols.define(name, SymKind::Variable(Type::Any));
                            }
                        }
                        Pattern::EnumVariant {
                            enum_name: _,
                            variant_name,
                            bindings,
                        } => {
                            if let Some(ref def) = enum_def {
                                if let Some((_, field_types)) = def
                                    .variants
                                    .iter()
                                    .find(|(n, _)| n == variant_name.as_str())
                                {
                                    if bindings.len() != field_types.len() {
                                        self.error(format!(
                                            "'{}' has {} fields, but pattern has {} bindings",
                                            variant_name,
                                            field_types.len(),
                                            bindings.len(),
                                        ));
                                    }
                                    for (i, binding) in bindings.iter().enumerate() {
                                        if binding.is_empty() {
                                            continue;
                                        } // wildcard _
                                        // Substitute generic placeholder types with actual types from Result/Option
                                        let ty = if let Type::Result(ref ok_ty, ref err_ty) = mt {
                                            match variant_name.as_str() {
                                                "Ok" => *ok_ty.clone(),
                                                "Err" => *err_ty.clone(),
                                                _ => field_types
                                                    .get(i)
                                                    .cloned()
                                                    .unwrap_or(Type::Unit),
                                            }
                                        } else if let Type::Option(ref some_ty) = mt {
                                            match variant_name.as_str() {
                                                "Some" => *some_ty.clone(),
                                                _ => field_types
                                                    .get(i)
                                                    .cloned()
                                                    .unwrap_or(Type::Unit),
                                            }
                                        } else {
                                            field_types.get(i).cloned().unwrap_or(Type::Unit)
                                        };
                                        // Remove Unit placeholder from resolver, insert proper type
                                        self.symbols.remove_from_current_scope(binding);
                                        let _ = self.symbols.define(binding, SymKind::Variable(ty));
                                    }
                                } else {
                                    self.error(format!(
                                        "'{}' is not a variant of this enum",
                                        variant_name
                                    ));
                                }
                            } else if matches!(mt, Type::Any) {
                                // The matched value's type is type-erased
                                // (e.g. the result of a method call on a
                                // generically-typed receiver, like
                                // `it.next()` on an iterator). We can't
                                // statically validate the variant/bindings,
                                // so bind each capture as `Type::Any` and
                                // let the runtime handle the actual dispatch
                                // — consistent with the rest of the
                                // type-erased-generics design.
                                for binding in bindings {
                                    if binding.is_empty() {
                                        continue;
                                    }
                                    self.symbols.remove_from_current_scope(binding);
                                    let _ =
                                        self.symbols.define(binding, SymKind::Variable(Type::Any));
                                }
                            } else {
                                self.error("cannot match enum variant on non-enum type");
                            }
                        }
                        _ => {}
                    }
                    if let Some(guard) = &arm.guard {
                        self.check_expr(guard);
                    }
                    arm_types.push(self.check_expr(&arm.body));
                    if enters_scope {
                        self.symbols.exit_scope();
                    }
                }
                // Exhaustiveness check: if matching on an enum, all variants must be covered
                if let Some(ref def) = enum_def {
                    let mut covered: Vec<CompactString> = Vec::new();
                    let mut has_wildcard = false;
                    for arm in arms {
                        match &arm.pattern {
                            Pattern::EnumVariant { variant_name, .. } => {
                                if !covered.contains(variant_name) {
                                    covered.push(variant_name.clone());
                                }
                            }
                            Pattern::Ident(name) => {
                                // Pattern::Ident could be a catch-all or a zero-field enum variant
                                if def.variants.iter().any(|(vname, fields)| {
                                    fields.is_empty() && vname == name.as_str()
                                }) {
                                    if !covered.contains(name) {
                                        covered.push(name.clone());
                                    }
                                } else {
                                    has_wildcard = true;
                                }
                            }
                            Pattern::Wildcard => has_wildcard = true,
                            _ => {}
                        }
                    }
                    if !has_wildcard {
                        for (vname, _) in &def.variants {
                            if !covered.iter().any(|c| c.as_str() == vname) {
                                self.error(format!(
                                    "non-exhaustive match: missing variant '{}'",
                                    vname
                                ));
                            }
                        }
                    }
                }
                // All arms should have compatible types.
                // Diverging arms (return/break/continue) are excluded from the
                // compatibility check — they never produce a value and should
                // not constrain the match type.  This mirrors Rust's treatment
                // of the never type `!`.
                let diverges =
                    |body: &Expr| matches!(body, Expr::Return(..) | Expr::Break | Expr::Continue);
                let non_diverging: Vec<Type> = arm_types
                    .iter()
                    .zip(arms.iter())
                    .filter(|(_, arm)| !diverges(&arm.body))
                    .map(|(ty, _)| ty.clone())
                    .collect();
                let first = non_diverging.first().cloned().unwrap_or(Type::Unit);
                for at in &non_diverging {
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
            Expr::Yield(inner) => {
                self.check_expr(inner);
                Type::Unit
            }
            Expr::StructLit {
                name,
                fields,
                spread,
            } => {
                let entry = self.symbols.lookup(name).cloned();
                // Resolve through type aliases to find the actual struct
                let resolved = match &entry {
                    Some(SymEntry {
                        kind:
                            SymKind::TypeAlias {
                                alias,
                                opaque: false,
                                ..
                            },
                        ..
                    }) => {
                        if let Type::Named(base) = alias {
                            self.symbols.lookup(base).cloned()
                        } else {
                            entry.clone()
                        }
                    }
                    _ => entry.clone(),
                };
                match resolved {
                    Some(SymEntry {
                        kind: SymKind::Struct(def),
                        ..
                    }) => {
                        for (fname, fval) in fields {
                            let found = def.fields.iter().find(|f| f.name == *fname);
                            if found.is_none() {
                                self.error(format!("struct '{}' has no field '{}'", name, fname));
                            }
                            self.check_expr(fval);
                        }
                        if let Some(spread_expr) = spread {
                            let spread_ty = self.check_expr(spread_expr);
                            let expected = Type::Named(name.clone());
                            if spread_ty != expected {
                                self.error(format!(
                                    "spread expression has type '{}' but expected struct '{}'",
                                    self.type_display(&spread_ty),
                                    name
                                ));
                            }
                        }
                    }
                    Some(_) => {
                        self.error(format!("'{}' is not a struct", name));
                    }
                    None => {
                        self.error(format!("undefined struct '{}'", name));
                    }
                };
                Type::Named(name.clone())
            }
            Expr::Array(elems) => {
                let mut elem_types = Vec::new();
                for elem in elems {
                    elem_types.push(self.check_expr(elem));
                }
                let inner = elem_types.first().cloned().unwrap_or(Type::Any);
                Type::Array(Box::new(inner))
            }
            Expr::Range { start, end, .. } => {
                self.check_expr(start);
                self.check_expr(end);
                Type::Any // ranges can be used as values with method calls
            }
            Expr::Lambda {
                params,
                return_type: _,
                body,
            } => {
                self.symbols.enter_scope();
                for param in params {
                    let ty = param.type_ann.clone().unwrap_or(Type::Any);
                    if self.symbols.lookup(&param.name).is_none() {
                        let _ = self.symbols.define(&param.name, SymKind::Variable(ty));
                    }
                }
                let ret = self.check_expr(body);
                self.symbols.exit_scope();
                Type::Fn {
                    params: params
                        .iter()
                        .map(|p| p.type_ann.clone().unwrap_or(Type::Any))
                        .collect(),
                    ret: Box::new(ret),
                }
            }
        };
        self.type_map.set(expr, ty.clone());
        ty
    }

    fn types_compatible(&self, a: &Type, b: &Type) -> bool {
        let a = self.resolve_var(&self.resolve_named(a));
        let b = self.resolve_var(&self.resolve_named(b));
        match (&a, &b) {
            // `any` is compatible with everything — the dynamic type wildcard
            (Type::Any, _) | (_, Type::Any) => true,
            // Unconstrained type variable — compatible with anything
            (Type::Var(_), _) | (_, Type::Var(_)) => true,
            // Unit type: only compatible with itself
            (Type::Unit, Type::Unit) => true,
            // Generic type parameters are compatible with any type (type erasure)
            (Type::Generic(_), _) | (_, Type::Generic(_)) => true,
            (Type::I64, Type::I64) => true,
            (Type::F32, Type::F32) => true,
            (Type::F64, Type::F64) => true,
            (Type::F64, Type::I64) | (Type::I64, Type::F64) => true, // implicit i64↔f64
            (Type::F32, Type::I64) | (Type::I64, Type::F32) => true, // implicit i64↔f32
            (Type::F32, Type::F64) | (Type::F64, Type::F32) => true, // implicit f32↔f64
            (Type::Bool, Type::Bool) => true,
            (Type::Str, Type::Str) => true,
            (Type::Named(a), Type::Named(b)) => {
                if a == b {
                    true
                } else if self.is_opaque(&Type::Named(a.clone()))
                    || self.is_opaque(&Type::Named(b.clone()))
                {
                    // Opaque types: name mismatch → incompatible
                    false
                } else {
                    // Structural compatibility: check if fields match
                    self.structurally_compatible(a, b)
                }
            }
            (Type::Array(a), Type::Array(b)) => self.types_compatible(a, b),
            (Type::Option(a), Type::Option(b)) => self.types_compatible(a, b),
            (Type::Iter(a), Type::Iter(b)) => self.types_compatible(a, b),
            (Type::Result(oka, erra), Type::Result(okb, errb)) => {
                self.types_compatible(oka, okb) && self.types_compatible(erra, errb)
            }
            // Named("Option") / Named("Result") from enum constructors are compatible with generic Option/Result
            (Type::Named(a), Type::Option(_)) if a == "Option" => true,
            (Type::Option(_), Type::Named(a)) if a == "Option" => true,
            (Type::Named(a), Type::Result(_, _)) if a == "Result" => true,
            (Type::Result(_, _), Type::Named(a)) if a == "Result" => true,
            // Unknown: safe top type, only compatible with itself and Any
            (Type::Unknown, Type::Unknown) => true,
            // Unknown is NOT compatible with other types (unlike Any)
            // — must narrow first via match or cast
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
            Type::Named(s) => {
                // Check if this name is a type alias
                if let Some(entry) = self.symbols.lookup(s) {
                    if let SymKind::TypeAlias { alias, opaque, .. } = &entry.kind {
                        if *opaque {
                            // Opaque types are NOT transparent — they are
                            // nominally distinct and must be compared by name only.
                            return ty.clone();
                        }
                        return alias.clone();
                    }
                    if matches!(entry.kind, SymKind::TypeParam(_)) {
                        return Type::Generic(s.clone());
                    }
                }
                ty.clone()
            }
            _ => ty.clone(),
        }
    }

    /// Returns `true` if `ty` is an opaque type alias (nominally distinct,
    /// not structurally compatible with its base).
    fn is_opaque(&self, ty: &Type) -> bool {
        if let Type::Named(s) = ty
            && let Some(entry) = self.symbols.lookup(s)
            && let SymKind::TypeAlias { opaque, .. } = &entry.kind
        {
            return *opaque;
        }
        false
    }

    /// If `name` is a transparent type alias, resolve to the underlying type name.
    fn resolve_type_alias_name(&self, name: &str) -> CompactString {
        if let Some(entry) = self.symbols.lookup(name)
            && let SymKind::TypeAlias {
                alias,
                opaque: false,
                ..
            } = &entry.kind
            && let Type::Named(base) = alias
        {
            return base.clone();
        }
        name.into()
    }

    /// Check structural compatibility between two named types.
    /// A provided type `P` is structurally compatible with expected type `E`
    /// if every field in `E` exists in `P` with a compatible type (width
    /// subtyping: extra fields in the provided type are OK).
    fn structurally_compatible(&self, provided: &str, expected: &str) -> bool {
        let provided_fields = self.get_struct_fields(provided);
        let expected_fields = self.get_struct_fields(expected);
        match (provided_fields, expected_fields) {
            (Some(pf), Some(ef)) => {
                // Every field in the expected type must exist in the provided type
                for (ename, ety) in &ef {
                    if let Some((_, pty)) = pf.iter().find(|(pn, _)| pn == ename) {
                        if !self.types_compatible(pty, ety) {
                            return false;
                        }
                    } else {
                        return false; // field `ename` missing in provided type
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Look up the fields of a struct type by name.
    fn get_struct_fields(&self, name: &str) -> Option<Vec<(String, Type)>> {
        if let Some(entry) = self.symbols.lookup(name) {
            match &entry.kind {
                SymKind::Struct(def) => {
                    return Some(
                        def.fields
                            .iter()
                            .map(|f| (f.name.to_string(), f.type_ann.clone()))
                            .collect(),
                    );
                }
                SymKind::TypeAlias {
                    alias: Type::Named(base),
                    opaque: false,
                    ..
                } => {
                    // Transparent alias — resolve and recurse
                    return self.get_struct_fields(base);
                }
                _ => {}
            }
        }
        None
    }

    fn type_display(&self, ty: &Type) -> String {
        match ty {
            Type::I64 => "i64".into(),
            Type::F32 => "f32".into(),
            Type::F64 => "f64".into(),
            Type::Bool => "bool".into(),
            Type::Str => "str".into(),
            Type::Unit => "()".into(),
            Type::Any => "any".into(),
            Type::Var(id) => format!("?{}", id),
            Type::Unknown => "unknown".into(),
            Type::Named(s) => s.to_string(),
            Type::Generic(s) => s.to_string(),
            Type::Array(inner) => format!("[{}]", self.type_display(inner)),
            Type::Fn { params, ret } => {
                let p: Vec<String> = params.iter().map(|t| self.type_display(t)).collect();
                format!("({}) -> {}", p.join(", "), self.type_display(ret))
            }
            Type::Option(inner) => format!("Option<{}>", self.type_display(inner)),
            Type::Result(ok, err) => format!(
                "Result<{}, {}>",
                self.type_display(ok),
                self.type_display(err)
            ),
            Type::Iter(inner) => format!("Iter<{}>", self.type_display(inner)),
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
