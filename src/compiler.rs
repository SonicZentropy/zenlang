use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::ast::*;
use crate::error::{Error, Result};
use crate::ir::*;
use crate::span::{SourceLocation, Span, Spanned};
use crate::symbol::*;
use crate::typeck::TypeMap;
use crate::value::Value;

/// Build a line offset table from source text for byte-offset → line-number conversion.
pub fn build_line_offsets(source: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, c) in source.char_indices() {
        if c == '\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Convert a byte offset to a 0-based line number using the line offset table.
pub fn offset_to_line(offsets: &[usize], byte_offset: usize) -> usize {
    match offsets.binary_search(&byte_offset) {
        Ok(line) => line,
        Err(line) => line.saturating_sub(1),
    }
}

/// Compile a type-checked program into bytecode.
///
/// Returns a tuple of `(functions, global_names)` where:
/// - `functions` contains the compiled `BytecodeFn` for each function.
/// - `global_names` lists the names of global variables in definition order.
///
/// The output is ready to be loaded into a [`VM`](crate::vm::VM) via
/// [`VM::load_bytecode`](crate::vm::VM::load_bytecode).
pub fn compile(
    program: &Program,
    _types: &TypeMap,
    symbols: &SymbolTable,
    native_names: &[String],
    source: &str,
) -> Result<(Vec<BytecodeFn>, Vec<String>)> {
    let line_offsets = build_line_offsets(source);
    let globals: Rc<RefCell<HashMap<String, u16>>> = Rc::new(RefCell::new(HashMap::new()));
    let mut global_order: Vec<String> = Vec::new();
    let mut function_names: HashMap<String, usize> = HashMap::new();
    let mut errors: Vec<Error> = Vec::new();
    let mut functions: Vec<BytecodeFn> = Vec::new();

    // Pre-register native function names as globals (stable indices)
    for name in native_names {
        let mut g = globals.borrow_mut();
        if !g.contains_key(name) {
            let idx = g.len() as u16;
            g.insert(name.clone(), idx);
            global_order.push(name.clone());
        }
    }

    // Pre-register built-in enum constructor names as globals
    // NOTE: Enum constructors are compiled via MakeEnum, not LoadGlobal.
    // They don't need global slots.

    // First pass: register all global variables and function indices
    for stmt in &program.stmts {
        register_global_stmt(&stmt.node, &mut *globals.borrow_mut(), &mut global_order);
        register_function_names(&stmt.node, &mut function_names);
    }

    fn register_function_names(stmt: &Stmt, function_names: &mut HashMap<String, usize>) {
        match stmt {
            Stmt::Fn { name, .. } => {
                let idx = function_names.len() + 1;
                function_names.insert(name.to_string(), idx);
            }
            Stmt::Impl { type_name, methods, .. } => {
                for m in methods {
                    if let Stmt::Fn { name, .. } = &m.node {
                        let qualified = format!("{}::{}", type_name, name);
                        let idx = function_names.len() + 1;
                        function_names.insert(qualified, idx);
                    }
                }
            }
            Stmt::Mod { body, .. } => {
                for s in body {
                    register_function_names(&s.node, function_names);
                }
            }
            Stmt::Trait { .. } => {}
            _ => {}
        }
    }

    // Count lambdas and compute function indices
    let user_fn_count = function_names.len();
    let _lambda_count = count_lambdas_in_stmts(&program.stmts);
    let lambda_base = 1 + user_fn_count; // main=0, user functions start at 1
    let lambda_counter = Rc::new(RefCell::new(lambda_base));
    let lambda_fns: Rc<RefCell<Vec<BytecodeFn>>> = Rc::new(RefCell::new(Vec::new()));

    // Second pass: compile top-level statements into a main function
    {
        let mut fc = FunctionCompiler::new(
            "__main__".into(), 0, globals.clone(), &function_names,
            &mut errors, &line_offsets, lambda_counter.clone(), lambda_fns.clone(), symbols,
        );
        let stmt_count = program.stmts.len();
        for (i, stmt) in program.stmts.iter().enumerate() {
            fc.set_line_by_offset(stmt.span.start());
            let is_last = i == stmt_count - 1;
            if is_last && matches!(&stmt.node, Stmt::Expr(_)) {
                if let Stmt::Expr(expr) = &stmt.node {
                    fc.compile_expr(expr);
                }
            } else {
                fc.compile_stmt(&stmt.node);
            }
        }
        match program.stmts.last() {
            Some(s) if matches!(&s.node, Stmt::Expr(_)) => {}
            _ => { fc.none(); }
        }
        fc.emit_op(Opcode::Return);
        functions.push(fc.finalize());
    }

    // Third pass: compile user-defined functions
    compile_functions(&program.stmts, &mut functions, &globals, &function_names,
        &mut errors, &line_offsets, &lambda_counter, &lambda_fns, symbols);

    // Helper to compile function declarations from a list of stmts (handles Mod recursion)
    fn compile_functions(stmts: &[Spanned<Stmt>], functions: &mut Vec<BytecodeFn>,
        globals: &Rc<RefCell<HashMap<String, u16>>>,
        function_names: &HashMap<String, usize>,
        mut errors: &mut Vec<Error>,
        line_offsets: &[usize],
        lambda_counter: &Rc<RefCell<usize>>,
        lambda_fns: &Rc<RefCell<Vec<BytecodeFn>>>,
        symbols: &SymbolTable) {
        for stmt in stmts {
            if let Stmt::Fn { name, params, return_type: _, body, .. } = &stmt.node {
                let arity = params.len() as u32;
            let mut fc = FunctionCompiler::new(
                name.to_string(), arity, globals.clone(), &function_names,
                &mut errors, &line_offsets, lambda_counter.clone(), lambda_fns.clone(), symbols,
            );

            fc.enter_scope();
            for param in params {
                fc.add_local(&param.name);
            }

            let stmt_count = body.len();
            for (i, s) in body.iter().enumerate() {
                fc.set_line_by_offset(s.span.start());
                if i == stmt_count - 1 && matches!(&s.node, Stmt::Expr(_)) {
                    if let Stmt::Expr(expr) = &s.node {
                        fc.compile_expr(expr);
                    }
                } else {
                    fc.compile_stmt(&s.node);
                }
            }

            match body.last() {
                Some(last) if matches!(last.node, Stmt::Expr(_)) => {}
                _ => { fc.none(); }
            }
            fc.emit_op(Opcode::Return);
            fc.exit_scope();

            functions.push(fc.finalize());
            } else if let Stmt::Impl { type_name, methods, .. } = &stmt.node {
                for m in methods {
                    if let Stmt::Fn { name, params, return_type: _, body, .. } = &m.node {
                        let qualified = format!("{}::{}", type_name, name);
                        let arity = params.len() as u32;
                        let mut fc = FunctionCompiler::new(
                            qualified, arity, globals.clone(), &function_names,
                            &mut errors, &line_offsets, lambda_counter.clone(), lambda_fns.clone(), symbols,
                        );
                        fc.enter_scope();
                        for param in params {
                            fc.add_local(&param.name);
                        }
                        let stmt_count = body.len();
                        for (i, s) in body.iter().enumerate() {
                            fc.set_line_by_offset(s.span.start());
                            if i == stmt_count - 1 && matches!(&s.node, Stmt::Expr(_)) {
                                if let Stmt::Expr(expr) = &s.node {
                                    fc.compile_expr(expr);
                                }
                            } else {
                                fc.compile_stmt(&s.node);
                            }
                        }
                        match body.last() {
                            Some(last) if matches!(last.node, Stmt::Expr(_)) => {}
                            _ => { fc.none(); }
                        }
                        fc.emit_op(Opcode::Return);
                        fc.exit_scope();
                        functions.push(fc.finalize());
                    }
                }
            } else if let Stmt::Mod { body, .. } = &stmt.node {
                compile_functions(body, functions, globals, function_names,
                    errors, line_offsets, lambda_counter, lambda_fns, symbols);
            }
        }
    }

    // Append all collected lambda functions
    functions.extend(std::mem::take(&mut *lambda_fns.borrow_mut()));

    if errors.is_empty() {
        Ok((functions, global_order))
    } else {
        Err(Error::CompileMultiple { errors })
    }
}

/// Count the number of `Expr::Lambda` nodes in a program.
fn count_lambdas_in_stmts(stmts: &[Spanned<Stmt>]) -> usize {
    stmts.iter().map(|s| count_lambdas_in_stmt(&s.node)).sum()
}

fn count_lambdas_in_stmt(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::Fn { body, .. } => count_lambdas_in_stmts(body),
        Stmt::Let { init, .. } => init.as_ref().map_or(0, |e| count_lambdas_in_expr(e)),
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) => count_lambdas_in_expr(expr),
        Stmt::Impl { methods, .. } => methods.iter().map(|m| count_lambdas_in_stmt(&m.node)).sum(),
        Stmt::Trait { methods, .. } => methods.iter().map(|m| count_lambdas_in_stmt(&m.node)).sum(),
        Stmt::Mod { body, .. } => count_lambdas_in_stmts(body),
        _ => 0,
    }
}

fn count_lambdas_in_expr(expr: &Expr) -> usize {
    match expr {
        Expr::Lambda { body, .. } => 1 + count_lambdas_in_expr(body),
        Expr::Block(stmts) => count_lambdas_in_stmts(stmts),
        Expr::If { cond, then, else_ } => {
            count_lambdas_in_expr(cond)
                + count_lambdas_in_expr(then)
                + else_.as_ref().map_or(0, |e| count_lambdas_in_expr(e))
        }
        Expr::While { cond, body } => count_lambdas_in_expr(cond) + count_lambdas_in_expr(body),
        Expr::Loop(body) => count_lambdas_in_expr(body),
        Expr::For { iter, body, .. } => count_lambdas_in_expr(iter) + count_lambdas_in_expr(body),
        Expr::Match { expr, arms } => {
            count_lambdas_in_expr(expr)
                + arms.iter().map(|arm| {
                    let mut c = count_lambdas_in_expr(&arm.body);
                    if let Some(g) = &arm.guard {
                        c += count_lambdas_in_expr(g);
                    }
                    c
                }).sum::<usize>()
        }
        Expr::Binary { lhs, rhs, .. } => count_lambdas_in_expr(lhs) + count_lambdas_in_expr(rhs),
        Expr::Unary { expr, .. } => count_lambdas_in_expr(expr),
        Expr::Call { func, args } => {
            count_lambdas_in_expr(func) + args.iter().map(count_lambdas_in_expr).sum::<usize>()
        }
        Expr::MethodCall { obj, args, .. } => {
            count_lambdas_in_expr(obj) + args.iter().map(count_lambdas_in_expr).sum::<usize>()
        }
        Expr::Field { obj, .. } => count_lambdas_in_expr(obj),
        Expr::Index { obj, index } => count_lambdas_in_expr(obj) + count_lambdas_in_expr(index),
        Expr::StructLit { fields, .. } => fields.iter().map(|(_, v)| count_lambdas_in_expr(v)).sum(),
        Expr::Array(elems) => elems.iter().map(count_lambdas_in_expr).sum(),
        Expr::Range { start, end, .. } => count_lambdas_in_expr(start) + count_lambdas_in_expr(end),
        Expr::Return(Some(inner)) => count_lambdas_in_expr(inner),
        _ => 0,
    }
}

/// Collect free (captured) variable names from a lambda body.
/// Returns names referenced in `expr` that are NOT in `own_names` (params/locals of the lambda).
fn collect_free_vars(expr: &Expr, own_names: &HashSet<String>) -> Vec<String> {
    let mut vars = Vec::new();
    collect_free_vars_inner(expr, own_names, &mut vars);
    vars
}

fn collect_free_vars_inner(expr: &Expr, own: &HashSet<String>, result: &mut Vec<String>) {
    match expr {
        Expr::Ident(name) => {
            if !own.contains(name.as_str()) && !result.iter().any(|s| s == name.as_str()) {
                result.push(name.to_string());
            }
        }
        Expr::Block(stmts) => collect_free_in_stmts(stmts, own, result),
        Expr::Lambda { params, body, .. } => {
            let mut local_own = own.clone();
            for p in params {
                local_own.insert(p.name.to_string());
            }
            collect_free_vars_inner(body, &local_own, result);
        }
        Expr::If { cond, then, else_ } => {
            collect_free_vars_inner(cond, own, result);
            collect_free_vars_inner(then, own, result);
            if let Some(e) = else_ {
                collect_free_vars_inner(e, own, result);
            }
        }
        Expr::While { cond, body } => {
            collect_free_vars_inner(cond, own, result);
            collect_free_vars_inner(body, own, result);
        }
        Expr::Loop(body) => collect_free_vars_inner(body, own, result),
        Expr::For { iter, body, .. } => {
            collect_free_vars_inner(iter, own, result);
            collect_free_vars_inner(body, own, result);
        }
        Expr::Match { expr, arms } => {
            collect_free_vars_inner(expr, own, result);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_free_vars_inner(g, own, result);
                }
                collect_free_vars_inner(&arm.body, own, result);
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_free_vars_inner(lhs, own, result);
            collect_free_vars_inner(rhs, own, result);
        }
        Expr::Unary { expr, .. } => collect_free_vars_inner(expr, own, result),
        Expr::Call { func, args } => {
            collect_free_vars_inner(func, own, result);
            for arg in args {
                collect_free_vars_inner(arg, own, result);
            }
        }
        Expr::MethodCall { obj, args, .. } => {
            collect_free_vars_inner(obj, own, result);
            for arg in args {
                collect_free_vars_inner(arg, own, result);
            }
        }
        Expr::Field { obj, .. } => collect_free_vars_inner(obj, own, result),
        Expr::Index { obj, index } => {
            collect_free_vars_inner(obj, own, result);
            collect_free_vars_inner(index, own, result);
        }
        Expr::StructLit { fields, .. } => {
            for (_, v) in fields {
                collect_free_vars_inner(v, own, result);
            }
        }
        Expr::Array(elems) => {
            for e in elems {
                collect_free_vars_inner(e, own, result);
            }
        }
        Expr::Range { start, end, .. } => {
            collect_free_vars_inner(start, own, result);
            collect_free_vars_inner(end, own, result);
        }
        Expr::Return(Some(inner)) => collect_free_vars_inner(inner, own, result),
        _ => {}
    }
}

fn collect_free_in_stmts(stmts: &[Spanned<Stmt>], own: &HashSet<String>, result: &mut Vec<String>) {
    let mut local_own = own.clone();
    for stmt in stmts {
        match &stmt.node {
            Stmt::Let { name, init, .. } => {
                if let Some(init_expr) = init {
                    collect_free_vars_inner(init_expr, &local_own, result);
                }
                local_own.insert(name.to_string());
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) => {
                collect_free_vars_inner(expr, &local_own, result);
            }
            Stmt::Fn { params, body, .. } => {
                let mut fn_own = local_own.clone();
                for p in params {
                    fn_own.insert(p.name.to_string());
                }
                collect_free_in_stmts(body, &fn_own, result);
            }
            Stmt::Impl { methods, .. } => {
                for m in methods {
                    collect_free_in_stmts(
                        &[Spanned::new(m.node.clone(), m.span)], // cheap clone for analysis
                        &local_own,
                        result,
                    );
                }
            }
            Stmt::Trait { .. } => {}
            Stmt::Return(None) => {}
            Stmt::Mod { body, .. } => {
                collect_free_in_stmts(body, &local_own, result);
            }
            _ => {}
        }
    }
}

fn register_global_stmt(stmt: &Stmt, globals: &mut HashMap<String, u16>, global_order: &mut Vec<String>) {
    match stmt {
        Stmt::Fn { name, .. } | Stmt::Let { name, .. } => {
            if !globals.contains_key(name.as_str()) {
                let idx = globals.len() as u16;
                globals.insert(name.to_string(), idx);
                global_order.push(name.to_string());
            }
        }
        Stmt::Impl { type_name, methods, .. } => {
            for m in methods {
                if let Stmt::Fn { name, .. } = &m.node {
                    let qualified = format!("{}::{}", type_name, name);
                    if !globals.contains_key(&qualified) {
                        let idx = globals.len() as u16;
                        globals.insert(qualified.clone(), idx);
                        global_order.push(qualified);
                    }
                }
            }
        }
        Stmt::Mod { body, .. } => {
            for stmt in body {
                register_global_stmt(&stmt.node, globals, global_order);
            }
        }
        Stmt::Use { path } => {
            let name = &path[path.len() - 1];
            if !globals.contains_key(name.as_str()) {
                let idx = globals.len() as u16;
                globals.insert(name.to_string(), idx);
                global_order.push(name.to_string());
            }
        }
        Stmt::Enum { .. } => {}
        Stmt::Trait { .. } => {}
        _ => {}
    }
}

struct FunctionCompiler<'a> {
    name: String,
    arity: u32,
    chunk: Chunk,
    locals: Vec<Local>,
    scope_depth: usize,
    loop_start: Vec<usize>,
    loop_continue: Vec<usize>,
    loop_end_jumps: Vec<Vec<usize>>,
    globals: Rc<RefCell<HashMap<String, u16>>>,
    function_names: &'a HashMap<String, usize>,
    errors: &'a mut Vec<Error>,
    current_line: usize,
    line_offsets: &'a [usize],
    const_map: HashMap<u64, u16>,
    lambda_counter: Rc<RefCell<usize>>,
    lambda_fns: Rc<RefCell<Vec<BytecodeFn>>>,
    symbols: &'a SymbolTable,
}

struct Local {
    name: String,
    depth: usize,
    slot: u16,
}

impl<'a> FunctionCompiler<'a> {
    fn new(
        name: String,
        arity: u32,
        globals: Rc<RefCell<HashMap<String, u16>>>,
        function_names: &'a HashMap<String, usize>,
        errors: &'a mut Vec<Error>,
        line_offsets: &'a [usize],
        lambda_counter: Rc<RefCell<usize>>,
        lambda_fns: Rc<RefCell<Vec<BytecodeFn>>>,
        symbols: &'a SymbolTable,
    ) -> Self {
        let mut chunk = Chunk::new();
        chunk.locals = 0;
        Self {
            name,
            arity,
            chunk,
            locals: Vec::new(),
            scope_depth: 0,
            loop_start: Vec::new(),
            loop_continue: Vec::new(),
            loop_end_jumps: Vec::new(),
            const_map: HashMap::new(),
            globals,
            function_names,
            errors,
            current_line: 0,
            line_offsets,
            lambda_counter,
            lambda_fns,
            symbols,
        }
    }

    fn finalize(self) -> BytecodeFn {
        BytecodeFn {
            name: self.name,
            chunk: self.chunk,
            arity: self.arity,
            upvalues: Vec::new(),
        }
    }

    fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(Error::Compile {
            location: SourceLocation::new(None, Span::new(0, 0), self.current_line, 0),
            msg: msg.into(),
        });
    }

    fn add_const(&mut self, val: Value) -> u16 {
        let key = const_hash(&val);
        if let Some(&idx) = self.const_map.get(&key) {
            // Verify no collision (shouldn't happen with good hash, but safe)
            if self.chunk.constants[idx as usize] == val {
                return idx;
            }
        }
        // Linear scan as fallback for hash collision or first insert
        for (i, c) in self.chunk.constants.iter().enumerate() {
            if *c == val {
                self.const_map.insert(key, i as u16);
                return i as u16;
            }
        }
        let idx = self.chunk.add_constant(val);
        self.const_map.insert(key, idx);
        idx
    }

    fn load_const(&mut self, val: Value) {
        let idx = self.add_const(val);
        self.emit_op(Opcode::LoadConst(idx));
    }

    fn none(&mut self) {
        self.load_const(Value::Nil);
    }

    fn emit_op(&mut self, op: Opcode) {
        self.chunk.emit_op(op, self.current_line);
    }

    fn set_line_by_offset(&mut self, byte_offset: usize) {
        self.current_line = offset_to_line(self.line_offsets, byte_offset);
    }

    fn current_offset(&self) -> usize {
        self.chunk.code.len()
    }

    fn patch_jump(&mut self, opcode_offset: usize) {
        // operand starts right after the opcode byte
        let target = self.chunk.code.len() as u16;
        self.chunk.patch_u16(opcode_offset + 1, target);
    }

    fn add_local(&mut self, name: &str) -> u16 {
        let slot = self.chunk.locals as u16;
        self.chunk.locals += 1;
        self.locals.push(Local { name: name.to_string(), depth: self.scope_depth, slot });
        slot
    }

    fn resolve_local(&self, name: &str) -> Option<u16> {
        self.locals.iter().rev().find(|l| l.name == name).map(|l| l.slot)
    }

    fn resolve_global(&self, name: &str) -> Option<u16> {
        self.globals.borrow().get(name).copied()
    }

    fn enter_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn exit_scope(&mut self) {
        while let Some(last) = self.locals.last() {
            if last.depth == self.scope_depth {
                self.locals.pop();
            } else {
                break;
            }
        }
        self.scope_depth -= 1;
    }

    // ---------- Statement compilation ----------

    fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, init, .. } => {
                if self.scope_depth == 0 {
                    match init {
                        Some(expr) => self.compile_expr(expr),
                        None => self.none(),
                    }
                    if let Some(idx) = self.resolve_global(name) {
                        self.emit_op(Opcode::StoreGlobal(idx));
                    } else {
                        let idx = self.globals.borrow().len() as u16;
                        self.globals.borrow_mut().insert(name.to_string(), idx);
                        self.emit_op(Opcode::StoreGlobal(idx));
                    }
                } else {
                    if self.resolve_local(name).is_some() {
                        self.error(format!("variable '{}' already defined", name));
                        return;
                    }
                    match init {
                        Some(expr) => self.compile_expr(expr),
                        None => self.none(),
                    }
                    let slot = self.add_local(name);
                    self.emit_op(Opcode::StoreLocal(slot));
                }
            }
            Stmt::Expr(expr) => {
                self.compile_expr(expr);
                self.emit_op(Opcode::Pop);
            }
            Stmt::Return(Some(expr)) => {
                self.compile_expr(expr);
                self.emit_op(Opcode::Return);
            }
            Stmt::Return(None) => {
                self.none();
                self.emit_op(Opcode::Return);
            }
            Stmt::Fn { .. } | Stmt::Struct { .. } | Stmt::Enum { .. } => {}
        Stmt::Impl { methods, .. } => {
            for m in methods {
                self.compile_stmt(&m.node);
            }
        }
        Stmt::Trait { .. } => {
            // Traits are compile-time only; nothing to compile
        }
        Stmt::Use { .. } => {
            // Already resolved by the resolver; nothing to compile
        }
        Stmt::Mod { body, .. } => {
            for stmt in body {
                self.compile_stmt(&stmt.node);
            }
        }
    }
    }

    // ---------- Expression compilation ----------

    fn compile_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Int(n) => self.load_const(Value::Int(*n)),
            Expr::Float(n) => self.load_const(Value::Float(*n)),
            Expr::Str(s) => self.load_const(Value::Str(s.clone().into())),
            Expr::Bool(b) => self.load_const(Value::Bool(*b)),
            Expr::Unit => self.none(),

            Expr::Ident(name) => {
                if let Some(idx) = self.resolve_local(name) {
                    self.emit_op(Opcode::LoadLocal(idx));
                } else if let Some(&fn_idx) = self.function_names.get(name.as_str()) {
                    // Function reference — push function constant
                    self.load_const(Value::Function(fn_idx));
                } else if let Some(idx) = self.resolve_global(name) {
                    self.emit_op(Opcode::LoadGlobal(idx));
                } else if let Some(entry) = self.symbols.lookup(name) {
                    if let SymKind::EnumConstructor { enum_name: _, variant_name: _, tag, fields } = &entry.kind {
                        if fields.is_empty() {
                            self.emit_op(Opcode::MakeEnum(*tag, 0));
                            return;
                        }
                    }
                    self.error(format!("undefined name '{}'", name));
                } else {
                    self.error(format!("undefined name '{}'", name));
                }
            }

            Expr::Binary { op, lhs, rhs } => {
                if *op == BinOp::Assign {
                    self.compile_assignment(lhs, rhs);
                    return;
                }
                self.compile_expr(lhs);
                self.compile_expr(rhs);
                self.emit_binary_op(*op);
            }

            Expr::Unary { op, expr: inner } => {
                self.compile_expr(inner);
                match op {
                    UnOp::Neg => self.emit_op(Opcode::Neg),
                    UnOp::Not => self.emit_op(Opcode::Not),
                    UnOp::BitNot => self.emit_op(Opcode::BitNot),
                }
            }

            Expr::Call { func, args } => {
                // Check if this is an enum constructor call
                if let Expr::Ident(name) = func.as_ref() {
                    if let Some(entry) = self.symbols.lookup(name) {
                        if let SymKind::EnumConstructor { enum_name: _, variant_name: _, tag, fields: _ } = &entry.kind {
                            for arg in args {
                                self.compile_expr(arg);
                            }
                            self.emit_op(Opcode::MakeEnum(*tag, args.len() as u16));
                            return;
                        }
                    }
                }
                self.compile_expr(func);
                for arg in args {
                    self.compile_expr(arg);
                }
                self.emit_op(Opcode::Call(args.len() as u16));
            }

            Expr::MethodCall { obj, method, args } => {
                self.compile_expr(obj);
                for arg in args {
                    self.compile_expr(arg);
                }
                let method_idx = self.chunk.add_method_name(method);
                self.emit_op(Opcode::CallMethod(method_idx, args.len() as u16));
            }

            Expr::Field { obj, field } => {
                self.compile_expr(obj);
                let idx = self.chunk.add_field_name(field);
                self.emit_op(Opcode::LoadField(idx));
            }

            Expr::Index { obj, index } => {
                self.compile_expr(obj);
                self.compile_expr(index);
                self.emit_op(Opcode::LoadIndex);
            }

            Expr::Block(stmts) => {
                self.enter_scope();
                let count = stmts.len();
                for (i, stmt) in stmts.iter().enumerate() {
                    if i == count - 1 && matches!(&stmt.node, Stmt::Expr(_)) {
                        if let Stmt::Expr(expr) = &stmt.node {
                            self.compile_expr(expr);
                        }
                    } else {
                        self.compile_stmt(&stmt.node);
                    }
                }
                self.exit_scope();
            }

            Expr::If { cond, then, else_ } => {
                self.compile_expr(cond);
                let else_jump = self.current_offset();
                self.emit_op(Opcode::JumpIfFalse(0));
                self.compile_expr(then);
                let end_jump = self.current_offset();
                self.emit_op(Opcode::Jump(0));
                self.patch_jump(else_jump);
                match else_ {
                    Some(e) => self.compile_expr(e),
                    None => self.none(),
                }
                self.patch_jump(end_jump);
            }

            Expr::While { cond, body } => {
                let start = self.current_offset();
                self.compile_expr(cond);
                let exit = self.current_offset();
                self.emit_op(Opcode::JumpIfFalse(0));
                self.loop_start.push(start);
                self.loop_continue.push(start);
                self.loop_end_jumps.push(vec![exit]);
                self.compile_expr(body);
                self.loop_start.pop();
                self.loop_continue.pop();
                let jumps = self.loop_end_jumps.pop().unwrap();
                self.emit_op(Opcode::Loop(start as u16));
                for j in &jumps {
                    self.patch_jump(*j);
                }
                self.none();
            }

            Expr::Loop(body) => {
                let start = self.current_offset();
                self.loop_start.push(start);
                self.loop_continue.push(start);
                self.loop_end_jumps.push(Vec::new());
                self.compile_expr(body);
                self.loop_start.pop();
                self.loop_continue.pop();
                let jumps = self.loop_end_jumps.pop().unwrap();
                self.emit_op(Opcode::Loop(start as u16));
                for j in &jumps {
                    self.patch_jump(*j);
                }
                self.none();
            }

            Expr::For { var, iter, body } => {
                match iter.as_ref() {
                    Expr::Range { start, end, inclusive } => {
                        self.enter_scope();
                        self.compile_expr(start);
                        let var_slot = self.add_local("__i");
                        self.emit_op(Opcode::StoreLocal(var_slot));
                        self.compile_expr(end);
                        let limit_slot = self.add_local("__limit");
                        self.emit_op(Opcode::StoreLocal(limit_slot));

                        let _user_var_slot = self.add_local(var);

                        let loop_start = self.current_offset();
                        self.emit_op(Opcode::LoadLocal(var_slot));
                        self.emit_op(Opcode::LoadLocal(limit_slot));
                        if *inclusive {
                            self.emit_op(Opcode::Le);
                        } else {
                            self.emit_op(Opcode::Lt);
                        }
                        let exit = self.current_offset();
                        self.emit_op(Opcode::JumpIfFalse(0));

                        self.emit_op(Opcode::LoadLocal(var_slot));
                        self.emit_op(Opcode::StoreLocal(_user_var_slot));

                        self.loop_start.push(loop_start);
                        self.loop_continue.push(loop_start);
                        self.loop_end_jumps.push(vec![exit]);
                        self.compile_expr(body);
                        self.loop_start.pop();
                        self.loop_continue.pop();
                        let jumps = self.loop_end_jumps.pop().unwrap();

                        let one = self.add_const(Value::Int(1));
                        self.emit_op(Opcode::LoadLocal(var_slot));
                        self.emit_op(Opcode::LoadConst(one));
                        self.emit_op(Opcode::Add);
                        self.emit_op(Opcode::StoreLocal(var_slot));
                        self.emit_op(Opcode::Loop(loop_start as u16));

                        for j in &jumps {
                            self.patch_jump(*j);
                        }
                        self.exit_scope();
                        self.none();
                    }
                    _ => {
                        // Generic iterable: array or string
                        // Evaluate the iterable once and store it
                        self.enter_scope();
                        self.compile_expr(iter);
                        let iter_slot = self.add_local("__iter");
                        self.emit_op(Opcode::StoreLocal(iter_slot));

                        // __i = 0
                        let zero = self.add_const(Value::Int(0));
                        self.emit_op(Opcode::LoadConst(zero));
                        let idx_slot = self.add_local("__i");
                        self.emit_op(Opcode::StoreLocal(idx_slot));

                        // __len = __iter.len()
                        self.emit_op(Opcode::LoadLocal(iter_slot));
                        self.emit_op(Opcode::Len);
                        let len_slot = self.add_local("__len");
                        self.emit_op(Opcode::StoreLocal(len_slot));

                        let _user_var_slot = self.add_local(var);

                        let loop_start = self.current_offset();
                        // while __i < __len
                        self.emit_op(Opcode::LoadLocal(idx_slot));
                        self.emit_op(Opcode::LoadLocal(len_slot));
                        self.emit_op(Opcode::Lt);
                        let exit = self.current_offset();
                        self.emit_op(Opcode::JumpIfFalse(0));

                        // var = __iter[__i]
                        self.emit_op(Opcode::LoadLocal(iter_slot));
                        self.emit_op(Opcode::LoadLocal(idx_slot));
                        self.emit_op(Opcode::LoadIndex);
                        self.emit_op(Opcode::StoreLocal(_user_var_slot));

                        self.loop_start.push(loop_start);
                        self.loop_continue.push(loop_start);
                        self.loop_end_jumps.push(vec![exit]);
                        self.compile_expr(body);
                        self.loop_start.pop();
                        self.loop_continue.pop();
                        let jumps = self.loop_end_jumps.pop().unwrap();

                        // __i += 1
                        let one = self.add_const(Value::Int(1));
                        self.emit_op(Opcode::LoadLocal(idx_slot));
                        self.emit_op(Opcode::LoadConst(one));
                        self.emit_op(Opcode::Add);
                        self.emit_op(Opcode::StoreLocal(idx_slot));
                        self.emit_op(Opcode::Loop(loop_start as u16));

                        for j in &jumps {
                            self.patch_jump(*j);
                        }
                        self.exit_scope();
                        self.none();
                    }
                }
            }

            Expr::Match { expr, arms } => {
                self.compile_expr(expr);
                let mut end_jumps = Vec::new();
                for arm in arms {
                    self.emit_op(Opcode::Dup);
                    match &arm.pattern {
                        Pattern::Wildcard => {
                            self.emit_op(Opcode::Pop);
                            self.compile_expr(&arm.body);
                            let j = self.current_offset();
                            self.emit_op(Opcode::Jump(0));
                            end_jumps.push(j);
                            break;
                        }
                        Pattern::Ident(name) => {
                            self.enter_scope();
                            let slot = self.add_local(name);
                            self.emit_op(Opcode::StoreLocal(slot));
                            self.compile_expr(&arm.body);
                            self.exit_scope();
                            let j = self.current_offset();
                            self.emit_op(Opcode::Jump(0));
                            end_jumps.push(j);
                            break;
                        }
                        Pattern::Int(n) => {
                            self.load_const(Value::Int(*n));
                            self.emit_op(Opcode::Eq);
                            let next = self.current_offset();
                            self.emit_op(Opcode::JumpIfFalse(0));
                            self.emit_op(Opcode::Pop);
                            self.compile_expr(&arm.body);
                            let j = self.current_offset();
                            self.emit_op(Opcode::Jump(0));
                            end_jumps.push(j);
                            self.patch_jump(next);
                        }
                        Pattern::Float(n) => {
                            self.load_const(Value::Float(*n));
                            self.emit_op(Opcode::Eq);
                            let next = self.current_offset();
                            self.emit_op(Opcode::JumpIfFalse(0));
                            self.emit_op(Opcode::Pop);
                            self.compile_expr(&arm.body);
                            let j = self.current_offset();
                            self.emit_op(Opcode::Jump(0));
                            end_jumps.push(j);
                            self.patch_jump(next);
                        }
                        Pattern::Str(s) => {
                            self.load_const(Value::Str(s.clone().into()));
                            self.emit_op(Opcode::Eq);
                            let next = self.current_offset();
                            self.emit_op(Opcode::JumpIfFalse(0));
                            self.emit_op(Opcode::Pop);
                            self.compile_expr(&arm.body);
                            let j = self.current_offset();
                            self.emit_op(Opcode::Jump(0));
                            end_jumps.push(j);
                            self.patch_jump(next);
                        }
                        Pattern::Bool(b) => {
                            self.load_const(Value::Bool(*b));
                            self.emit_op(Opcode::Eq);
                            let next = self.current_offset();
                            self.emit_op(Opcode::JumpIfFalse(0));
                            self.emit_op(Opcode::Pop);
                            self.compile_expr(&arm.body);
                            let j = self.current_offset();
                            self.emit_op(Opcode::Jump(0));
                            end_jumps.push(j);
                            self.patch_jump(next);
                        }
                        Pattern::EnumVariant { variant_name, bindings } => {
                            let tag: u16 = self.symbols.lookup(variant_name)
                                .and_then(|entry| {
                                    if let SymKind::EnumConstructor { enum_name: _, variant_name: _, tag, fields: _ } = &entry.kind {
                                        Some(*tag)
                                    } else { None }
                                })
                                .unwrap_or(0);
                            self.emit_op(Opcode::LoadEnumTag);
                            self.load_const(Value::Int(tag as i64));
                            self.emit_op(Opcode::Eq);
                            let next = self.current_offset();
                            self.emit_op(Opcode::JumpIfFalse(0));
                            // NOTE: No Pop here — the original enum value is still
                            // on the stack (after Dup + LoadEnumTag + Eq consumed the
                            // dup'ed copy). It's needed by the binding loop below.
                            self.enter_scope();
                            for (i, binding) in bindings.iter().enumerate() {
                                self.emit_op(Opcode::Dup);
                                self.emit_op(Opcode::LoadEnumField(i as u16));
                                if binding.is_empty() {
                                    self.emit_op(Opcode::Pop); // wildcard _: discard value
                                } else {
                                    let slot = self.add_local(binding);
                                    self.emit_op(Opcode::StoreLocal(slot));
                                }
                            }
                            self.emit_op(Opcode::Pop);
                            self.compile_expr(&arm.body);
                            self.exit_scope();
                            let j = self.current_offset();
                            self.emit_op(Opcode::Jump(0));
                            end_jumps.push(j);
                            self.patch_jump(next);
                        }
                    }
                }
                self.emit_op(Opcode::Pop);
                self.none();
                for j in end_jumps {
                    self.patch_jump(j);
                }
            }

            Expr::Break => {
                if self.loop_end_jumps.last().is_some() {
                    let len = self.loop_end_jumps.len();
                    let j = self.current_offset();
                    self.emit_op(Opcode::Jump(0));
                    self.loop_end_jumps[len - 1].push(j);
                }
            }

            Expr::Continue => {
                if let Some(&start) = self.loop_continue.last() {
                    self.emit_op(Opcode::Loop(start as u16));
                }
            }

            Expr::Return(Some(inner)) => {
                self.compile_expr(inner);
                self.emit_op(Opcode::Return);
            }

            Expr::Return(None) => {
                self.none();
                self.emit_op(Opcode::Return);
            }

            Expr::StructLit { name, fields, spread } => {
                if let Some(spread_expr) = spread {
                    // Compile spread base, then override with explicit fields via StoreField
                    self.compile_expr(spread_expr);
                    for (field_name, val) in fields {
                        self.compile_expr(val);
                        let idx = self.chunk.add_field_name(field_name);
                        self.emit_op(Opcode::StoreField(idx));
                    }
                } else {
                    // Push field names then values; MakeStruct pops in reverse order
                    for (field_name, val) in fields {
                        self.load_const(Value::Str(field_name.clone().into()));
                        self.compile_expr(val);
                    }
                    let type_const_idx = self.chunk.add_constant(Value::Str(name.clone().into()));
                    self.emit_op(Opcode::MakeStruct(type_const_idx, fields.len() as u16));
                }
            }

            Expr::Array(elems) => {
                for elem in elems {
                    self.compile_expr(elem);
                }
                self.emit_op(Opcode::MakeArray(elems.len() as u16));
            }

            Expr::Range { start, end, inclusive: _ } => {
                self.compile_expr(start);
                self.compile_expr(end);
            }

            Expr::Lambda { params, body, .. } => {
                self.compile_lambda(params, body);
            }
        }
    }

    fn compile_lambda(&mut self, params: &[Param], body: &Expr) {
        // Collect free variables referenced in the lambda body
        let own_names: HashSet<String> = params.iter().map(|p| p.name.to_string()).collect();
        let free_vars = collect_free_vars(body, &own_names);

        // For each free var, verify it's a local in the current function and emit LoadLocal
        let mut upvalue_names: Vec<String> = Vec::new();
        for var in &free_vars {
            if let Some(idx) = self.resolve_local(var) {
                self.emit_op(Opcode::LoadLocal(idx));
                upvalue_names.push(var.clone());
            }
            // If not local, it'll be resolved via globals in the lambda body; no capture needed
        }

        // Assign a function index for this lambda
        let fn_idx = {
            let mut c = self.lambda_counter.borrow_mut();
            let idx = *c;
            *c += 1;
            idx
        };

        // Create a new FunctionCompiler for the lambda body
        let actual_arity = upvalue_names.len() as u32 + params.len() as u32;
        let mut lambda_fc = FunctionCompiler::new(
            format!("__lambda_{}", fn_idx),
            actual_arity,
            self.globals.clone(),
            self.function_names,
            self.errors,
            self.line_offsets,
            self.lambda_counter.clone(),
            self.lambda_fns.clone(),
            self.symbols,
        );

        // Enter scope, add upvalues as locals first, then params
        lambda_fc.enter_scope();
        for name in &upvalue_names {
            lambda_fc.add_local(name);
        }
        for param in params {
            lambda_fc.add_local(&param.name);
        }

        // Compile the lambda body (it produces a value on the stack)
        lambda_fc.compile_expr(body);
        lambda_fc.emit_op(Opcode::Return);
        lambda_fc.exit_scope();

        // Store the compiled lambda
        self.lambda_fns.borrow_mut().push(lambda_fc.finalize());

        // Emit NewClosure with function index and upvalue count
        self.emit_op(Opcode::NewClosure(fn_idx as u16, upvalue_names.len() as u16));
    }

    fn compile_assignment(&mut self, target: &Expr, value: &Expr) {
        match target {
            Expr::Ident(name) => {
                self.compile_expr(value);
                if let Some(idx) = self.resolve_local(name) {
                    self.emit_op(Opcode::StoreLocal(idx));
                } else if let Some(idx) = self.resolve_global(name) {
                    self.emit_op(Opcode::StoreGlobal(idx));
                } else {
                    self.error(format!("cannot assign to undefined variable '{}'", name));
                }
            }
            Expr::Field { obj, field } => {
                self.compile_expr(obj);
                self.compile_expr(value);
                let idx = self.chunk.add_field_name(field);
                self.emit_op(Opcode::StoreField(idx));
            }
            Expr::Index { obj, index } => {
                self.compile_expr(obj);
                self.compile_expr(index);
                self.compile_expr(value);
                self.emit_op(Opcode::StoreIndex);
            }
            _ => {
                self.error("invalid assignment target");
            }
        }
    }

    fn emit_binary_op(&mut self, op: BinOp) {
        match op {
            BinOp::Add => self.emit_op(Opcode::Add),
            BinOp::Sub => self.emit_op(Opcode::Sub),
            BinOp::Mul => self.emit_op(Opcode::Mul),
            BinOp::Div => self.emit_op(Opcode::Div),
            BinOp::Mod => self.emit_op(Opcode::Mod),
            BinOp::Eq => self.emit_op(Opcode::Eq),
            BinOp::Ne => self.emit_op(Opcode::Ne),
            BinOp::Lt => self.emit_op(Opcode::Lt),
            BinOp::Le => self.emit_op(Opcode::Le),
            BinOp::Gt => self.emit_op(Opcode::Gt),
            BinOp::Ge => self.emit_op(Opcode::Ge),
            BinOp::And => self.emit_op(Opcode::And),
            BinOp::Or => self.emit_op(Opcode::Or),
            BinOp::BitAnd => self.emit_op(Opcode::BitAnd),
            BinOp::BitOr => self.emit_op(Opcode::BitOr),
            BinOp::BitXor => self.emit_op(Opcode::BitXor),
            BinOp::Shl => self.emit_op(Opcode::Shl),
            BinOp::Shr => self.emit_op(Opcode::Shr),
            BinOp::Assign => unreachable!(),
        }
    }
}

fn const_hash(val: &Value) -> u64 {
    match val {
        Value::Nil => 0,
        Value::Bool(b) => 1 ^ ((*b as u64) << 1),
        Value::Int(n) => 2 ^ (n.wrapping_mul(0x9e3779b97f4a7c15u64 as i64) as u64),
        Value::Float(n) => 3 ^ n.to_bits(),
        Value::Str(s) => {
            let mut h = 4u64;
            for b in s.as_bytes() {
                h = h.wrapping_mul(31).wrapping_add(*b as u64);
            }
            h
        }
        Value::Function(idx) => 5 ^ (*idx as u64),
        Value::Array(_) | Value::Struct(..) | Value::Enum { .. }
        | Value::NativeFunction(_) | Value::Foreign(_)
        | Value::Closure(_) => {
            // These types use pointer identity, hash is not stable;
            // fall back to linear scan (handled in add_const)
            6
        }
    }
}
