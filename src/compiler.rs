use std::collections::HashMap;

use crate::ast::*;
use crate::error::{Error, Result};
use crate::ir::*;
use crate::span::{SourceLocation, Span};
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

pub fn compile(
    program: &Program,
    _types: &TypeMap,
    _symbols: &SymbolTable,
    native_names: &[String],
    source: &str,
) -> Result<(Vec<BytecodeFn>, Vec<String>)> {
    let line_offsets = build_line_offsets(source);
    let mut globals: HashMap<String, u16> = HashMap::new();
    let mut global_order: Vec<String> = Vec::new();
    let mut function_names: HashMap<String, usize> = HashMap::new();
    let mut errors: Vec<Error> = Vec::new();
    let mut functions: Vec<BytecodeFn> = Vec::new();

    // Pre-register native function names as globals (stable indices)
    for name in native_names {
        if !globals.contains_key(name) {
            let idx = globals.len() as u16;
            globals.insert(name.clone(), idx);
            global_order.push(name.clone());
        }
    }

    // First pass: register all global variables and function indices
    for stmt in &program.stmts {
        register_global_stmt(&stmt.node, &mut globals, &mut global_order);
        if let Stmt::Fn { name, .. } = &stmt.node {
            let idx = function_names.len() + 1; // +1 because main is index 0
            function_names.insert(name.clone(), idx);
        }
    }

    // Second pass: compile top-level statements into a main function
    {
        let mut fc = FunctionCompiler::new("__main__".into(), 0, &mut globals, &function_names, &mut errors, &line_offsets);
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
    for stmt in &program.stmts {
        if let Stmt::Fn { name, params, return_type: _, body } = &stmt.node {
            let arity = params.len() as u32;
            let mut fc = FunctionCompiler::new(name.clone(), arity, &mut globals, &function_names, &mut errors, &line_offsets);

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

    if errors.is_empty() {
        Ok((functions, global_order))
    } else {
        Err(Error::CompileMultiple { errors })
    }
}

fn register_global_stmt(stmt: &Stmt, globals: &mut HashMap<String, u16>, global_order: &mut Vec<String>) {
    match stmt {
        Stmt::Fn { name, .. } | Stmt::Let { name, .. } => {
            if !globals.contains_key(name) {
                let idx = globals.len() as u16;
                globals.insert(name.clone(), idx);
                global_order.push(name.clone());
            }
        }
        Stmt::Impl { methods, .. } => {
            for m in methods {
                if let Stmt::Fn { name, .. } = &m.node {
                    if !globals.contains_key(name) {
                        let idx = globals.len() as u16;
                        globals.insert(name.clone(), idx);
                        global_order.push(name.clone());
                    }
                }
            }
        }
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
    loop_end: Vec<usize>,
    globals: &'a mut HashMap<String, u16>,
    function_names: &'a HashMap<String, usize>,
    errors: &'a mut Vec<Error>,
    current_line: usize,
    line_offsets: &'a [usize],
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
        globals: &'a mut HashMap<String, u16>,
        function_names: &'a HashMap<String, usize>,
        errors: &'a mut Vec<Error>,
        line_offsets: &'a [usize],
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
            loop_end: Vec::new(),
            globals,
            function_names,
            errors,
            current_line: 0,
            line_offsets,
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
            location: SourceLocation::new(None, Span::new(0, 0), 0, 0),
            msg: msg.into(),
        });
    }

    fn add_const(&mut self, val: Value) -> u16 {
        for (i, c) in self.chunk.constants.iter().enumerate() {
            if *c == val {
                return i as u16;
            }
        }
        self.chunk.add_constant(val)
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
        self.globals.get(name).copied()
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
                        let idx = self.globals.len() as u16;
                        self.globals.insert(name.clone(), idx);
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
                } else if let Some(&fn_idx) = self.function_names.get(name) {
                    // Function reference — push function constant
                    self.load_const(Value::Function(fn_idx));
                } else if let Some(idx) = self.resolve_global(name) {
                    self.emit_op(Opcode::LoadGlobal(idx));
                } else {
                    let idx = self.globals.len() as u16;
                    self.globals.insert(name.clone(), idx);
                    self.emit_op(Opcode::LoadGlobal(idx));
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
                }
            }

            Expr::Call { func, args } => {
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
                self.loop_end.push(exit);
                self.compile_expr(body);
                self.loop_start.pop();
                self.loop_continue.pop();
                self.loop_end.pop();
                self.emit_op(Opcode::Loop(start as u16));
                self.patch_jump(exit);
                self.none();
            }

            Expr::Loop(body) => {
                let start = self.current_offset();
                self.loop_start.push(start);
                self.loop_continue.push(start);
                let exit_placeholder = self.current_offset();
                self.emit_op(Opcode::JumpIfFalse(0));
                self.loop_end.push(exit_placeholder);
                self.compile_expr(body);
                self.loop_start.pop();
                self.loop_continue.pop();
                self.loop_end.pop();
                self.emit_op(Opcode::Loop(start as u16));
                self.patch_jump(exit_placeholder);
                self.none();
            }

            Expr::For { var, iter, body } => {
                if let Expr::Range { start, end, inclusive } = iter.as_ref() {
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
                    self.loop_end.push(exit);
                    self.compile_expr(body);
                    self.loop_start.pop();
                    self.loop_continue.pop();
                    self.loop_end.pop();

                    let one = self.add_const(Value::Int(1));
                    self.emit_op(Opcode::LoadLocal(var_slot));
                    self.emit_op(Opcode::LoadConst(one));
                    self.emit_op(Opcode::Add);
                    self.emit_op(Opcode::StoreLocal(var_slot));
                    self.emit_op(Opcode::Loop(loop_start as u16));

                    self.patch_jump(exit);
                    self.exit_scope();
                    self.none();
                } else {
                    self.error("for loop requires a range expression");
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
                        Pattern::Ident(_) => {
                            self.emit_op(Opcode::Pop);
                            self.compile_expr(&arm.body);
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
                    }
                }
                self.emit_op(Opcode::Pop);
                self.none();
                for j in end_jumps {
                    self.patch_jump(j);
                }
            }

            Expr::Break => {
                if let Some(&_end) = self.loop_end.last() {
                    let _j = self.current_offset();
                    self.emit_op(Opcode::JumpIfFalse(0));
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

            Expr::StructLit { name: _, fields } => {
                // Push field names then values; MakeStruct pops in reverse order
                for (field_name, val) in fields {
                    self.load_const(Value::Str(field_name.clone().into()));
                    self.compile_expr(val);
                }
                self.emit_op(Opcode::MakeStruct(fields.len() as u16));
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

            Expr::Lambda { .. } => {
                self.error("closures not yet supported");
            }
        }
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
            BinOp::Assign => unreachable!(),
        }
    }
}
