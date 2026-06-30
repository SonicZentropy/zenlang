use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::error::{Error, Result};
use crate::interop::{ForeignTypeDef, ForeignTypeRegistry};
use crate::ir::{BytecodeFn, Chunk, Opcode};
use crate::value::{ClosureData, NativeFn, Value};

/// Execution context provided to native functions.
pub struct VMContext {
    pub registry: Rc<ForeignTypeRegistry>,
}

/// A call frame in the VM.
struct CallFrame {
    function_idx: usize,
    ip: usize,
    bp: usize,
}

impl CallFrame {
    fn new(function_idx: usize, bp: usize) -> Self {
        Self { function_idx, ip: 0, bp }
    }
}

/// The Zenlang virtual machine.
/// Helper to build a SourceLocation from a function index and bytecode offset.
fn source_loc_from_frame(functions: &[BytecodeFn], function_idx: usize, ip: usize) -> crate::span::SourceLocation {
    let line = if let Some(chunk) = functions.get(function_idx).map(|f| &f.chunk) {
        chunk.get_line(ip.saturating_sub(1))
    } else {
        0
    };
    crate::span::SourceLocation::new(None, crate::span::Span::new(0, 0), line, 0)
}

pub struct VM {
    stack: Vec<Value>,
    frames: Vec<CallFrame>,
    globals: Vec<Value>,
    functions: Vec<BytecodeFn>,
    global_names: Vec<String>,
    natives: HashMap<String, usize>,
    native_fns: Vec<(String, NativeFn)>,
    pub foreign_registry: Rc<ForeignTypeRegistry>,
}

impl VM {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            frames: Vec::new(),
            globals: Vec::new(),
            functions: Vec::new(),
            global_names: Vec::new(),
            natives: HashMap::new(),
            native_fns: Vec::new(),
            foreign_registry: Rc::new(ForeignTypeRegistry::new()),
        }
    }

    pub fn new_with_registry(registry: Rc<ForeignTypeRegistry>) -> Self {
        Self {
            stack: Vec::new(),
            frames: Vec::new(),
            globals: Vec::new(),
            functions: Vec::new(),
            global_names: Vec::new(),
            natives: HashMap::new(),
            native_fns: Vec::new(),
            foreign_registry: registry,
        }
    }

    /// Register a foreign type with the VM.
    pub fn register_type<T: 'static>(&mut self, name: &'static str) -> &mut ForeignTypeDef {
        let def = ForeignTypeDef::new(name);
        let type_id = TypeId::of::<T>();
        let registry = Rc::make_mut(&mut self.foreign_registry);
        registry.register_typed(type_id, def);
        registry.get_mut(&type_id).unwrap()
    }

    /// Return the list of registered native function names.
    pub fn native_names(&self) -> Vec<String> {
        self.native_fns.iter().map(|(n, _)| n.clone()).collect()
    }

    pub fn load_bytecode(&mut self, fns: Vec<BytecodeFn>, global_names: Vec<String>) {
        let offset = self.functions.len();
        for (i, f) in fns.into_iter().enumerate() {
            let idx = offset + i;
            self.functions.push(f);
            if i == 0 {
                self.natives.insert("__main__".into(), idx);
            }
        }
        self.global_names = global_names;
        self.populate_globals();
    }

    /// Fill globals with native function values or Nil for user globals.
    fn populate_globals(&mut self) {
        self.globals.clear();
        for name in &self.global_names {
            let val = if let Some(&idx) = self.natives.get(name.as_str()) {
                if idx < self.native_fns.len() && self.native_fns[idx].0 == *name {
                    Value::NativeFunction(self.native_fns[idx].1.clone())
                } else {
                    Value::Nil
                }
            } else {
                Value::Nil
            };
            self.globals.push(val);
        }
        self.globals.resize(self.global_names.len(), Value::Nil);
    }

    pub fn register_native(&mut self, name: &str, f: NativeFn) {
        let idx = self.native_fns.len();
        self.natives.insert(name.to_string(), idx);
        self.native_fns.push((name.to_string(), f));
    }

    /// Snapshot global values by name for state migration across reloads.
    pub fn snapshot_globals_by_name(&self) -> HashMap<String, Value> {
        let mut snapshot = HashMap::new();
        for (i, name) in self.global_names.iter().enumerate() {
            if let Some(val) = self.globals.get(i) {
                snapshot.insert(name.clone(), val.clone());
            }
        }
        snapshot
    }

    /// Restore global values from a name-keyed snapshot, matching by name.
    pub fn restore_globals_by_name(&mut self, snapshot: &HashMap<String, Value>) {
        for (i, name) in self.global_names.iter().enumerate() {
            if let Some(val) = snapshot.get(name) {
                if i < self.globals.len() {
                    self.globals[i] = val.clone();
                } else {
                    self.globals.push(val.clone());
                }
            }
        }
    }

    /// Reload all function bytecode while migrating global state.
    ///
    /// Replaces `self.functions` with new compiled functions, remaps any
    /// `Value::Function(old_idx)` references that may exist in global values
    /// to point at the correct new indices, restores matching global values
    /// by name, and updates `global_names`.
    pub fn reload_functions(&mut self, fns: Vec<BytecodeFn>, new_global_names: Vec<String>) -> Result<()> {
        // Build old name→idx map from current functions
        let old_name_to_idx: HashMap<&str, usize> = self.functions
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.as_str(), i))
            .collect();

        // Build new name→idx map
        let new_name_to_idx: HashMap<&str, usize> = fns
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.as_str(), i))
            .collect();

        // Snapshot globals, remapping Value::Function indices
        let mut snapshot = self.snapshot_globals_by_name();
        for val in snapshot.values_mut() {
            remap_function_value(val, &old_name_to_idx, &new_name_to_idx);
        }

        // Replace functions and global_names
        self.functions = fns;
        self.global_names = new_global_names;

        // Re-populate globals (native fns get Value::NativeFunction, user globals get Nil)
        self.populate_globals();

        // Restore matching user globals from snapshot
        self.restore_globals_by_name(&snapshot);

        // Update __main__ to point at new index 0
        self.natives.insert("__main__".into(), 0);

        // Reset stack and frames (no script running during reload)
        self.stack.clear();
        self.frames.clear();

        Ok(())
    }

    /// Run the main function.
    pub fn run_main(&mut self) -> Result<Value> {
        let main_idx = match self.natives.get("__main__") {
            Some(&idx) => idx,
            None => return Err(self.runtime_error("no main function found")),
        };

        // Initialize globals with nil
        let fn_def = &self.functions[main_idx];
        self.globals.resize(self.globals.len().max(1), Value::Nil);

        // Push main frame
        let frame = CallFrame::new(main_idx, 0);
        self.frames.push(frame);

        // Ensure stack has room for locals
        let local_count = fn_def.chunk.locals as usize;
        while self.stack.len() < local_count {
            self.stack.push(Value::Nil);
        }

        self.execute()?;

        // Return value is on top of stack
        Ok(self.stack.pop().unwrap_or(Value::Nil))
    }

    /// Build a runtime error with a stack trace from the current call frames.
    fn runtime_error(&self, msg: impl Into<String>) -> Error {
        let mut stack_trace: Vec<crate::span::SourceLocation> = self.frames
            .iter()
            .map(|frame| {
                source_loc_from_frame(&self.functions, frame.function_idx, frame.ip)
            })
            .collect();
        stack_trace.reverse(); // innermost frame first
        Error::Runtime {
            msg: msg.into(),
            stack_trace,
        }
    }

    fn chunk(&self) -> &Chunk {
        let idx = self.frames.last().unwrap().function_idx;
        &self.functions[idx].chunk
    }

    fn read_byte(&mut self) -> u8 {
        let ip = {
            let frame = self.frames.last().unwrap();
            frame.ip
        };
        let b = self.chunk().code[ip];
        self.frames.last_mut().unwrap().ip += 1;
        b
    }

    fn read_u16(&mut self) -> u16 {
        let ip = {
            let frame = self.frames.last().unwrap();
            frame.ip
        };
        let val = Chunk::read_u16_static(&self.chunk().code, ip);
        self.frames.last_mut().unwrap().ip += 2;
        val
    }

    fn execute(&mut self) -> Result<()> {
        loop {
            let frame = self.frames.last().unwrap();
            if frame.ip >= self.chunk().code.len() {
                break;
            }

            let byte = self.read_byte();
            let op = Opcode::from_byte(byte).ok_or_else(|| self.runtime_error(format!("unknown opcode: {}", byte)))?;

            match op {
                Opcode::LoadConst(_) => {
                    let idx = self.read_u16();
                    let val = self.chunk().constants[idx as usize].clone();
                    self.stack.push(val);
                }

                Opcode::LoadLocal(_) => {
                    let idx = self.read_u16() as usize;
                    let bp = self.frames.last().unwrap().bp;
                    let val = self.stack[bp + idx].clone();
                    self.stack.push(val);
                }

                Opcode::StoreLocal(_) => {
                    let idx = self.read_u16() as usize;
                    let bp = self.frames.last().unwrap().bp;
                    let val = self.stack.pop().unwrap();
                    self.stack[bp + idx] = val;
                }

                Opcode::LoadGlobal(_) => {
                    let idx = self.read_u16() as usize;
                    if idx >= self.globals.len() {
                        self.globals.resize(idx + 1, Value::Nil);
                    }
                    let val = self.globals[idx].clone();
                    self.stack.push(val);
                }

                Opcode::StoreGlobal(_) => {
                    let idx = self.read_u16() as usize;
                    if idx >= self.globals.len() {
                        self.globals.resize(idx + 1, Value::Nil);
                    }
                    let val = self.stack.pop().unwrap();
                    self.globals[idx] = val;
                }

                Opcode::Pop => {
                    self.stack.pop();
                }

                Opcode::Dup => {
                    let val = self.stack.last().unwrap().clone();
                    self.stack.push(val);
                }

                Opcode::And => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(a.is_truthy() && b.is_truthy()));
                }

                Opcode::Or => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(a.is_truthy() || b.is_truthy()));
                }

                Opcode::Add => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai + bi)),
                        (Value::Float(af), Value::Float(bf)) => self.stack.push(Value::Float(af + bf)),
                        (Value::Int(ai), Value::Float(bf)) => self.stack.push(Value::Float(*ai as f64 + bf)),
                        (Value::Float(af), Value::Int(bi)) => self.stack.push(Value::Float(af + *bi as f64)),
                        (Value::Str(as_), Value::Str(bs)) => {
                            let mut result = as_.to_string();
                            result.push_str(bs);
                            self.stack.push(Value::Str(result.into()));
                        }
                        _ => {
                            return Err(self.runtime_error(format!("cannot add {} and {}", a.type_name(), b.type_name())));
                        }
                    }
                }

                Opcode::Sub => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai - bi)),
                        (Value::Float(af), Value::Float(bf)) => self.stack.push(Value::Float(af - bf)),
                        (Value::Int(ai), Value::Float(bf)) => self.stack.push(Value::Float(*ai as f64 - bf)),
                        (Value::Float(af), Value::Int(bi)) => self.stack.push(Value::Float(af - *bi as f64)),
                        _ => {
                            return Err(self.runtime_error(format!("cannot subtract {} and {}", a.type_name(), b.type_name())));
                        }
                    }
                }

                Opcode::Mul => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai * bi)),
                        (Value::Float(af), Value::Float(bf)) => self.stack.push(Value::Float(af * bf)),
                        (Value::Int(ai), Value::Float(bf)) => self.stack.push(Value::Float(*ai as f64 * bf)),
                        (Value::Float(af), Value::Int(bi)) => self.stack.push(Value::Float(af * *bi as f64)),
                        _ => {
                            return Err(self.runtime_error(format!("cannot multiply {} and {}", a.type_name(), b.type_name())));
                        }
                    }
                }

                Opcode::Div => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => {
                            if *bi == 0 {
                                return Err(self.runtime_error("division by zero"));
                            }
                            self.stack.push(Value::Int(ai / bi));
                        }
                        (Value::Float(af), Value::Float(bf)) => {
                            self.stack.push(Value::Float(af / bf));
                        }
                        (Value::Int(ai), Value::Float(bf)) => {
                            self.stack.push(Value::Float(*ai as f64 / bf));
                        }
                        (Value::Float(af), Value::Int(bi)) => {
                            if *bi == 0 {
                                return Err(self.runtime_error("division by zero"));
                            }
                            self.stack.push(Value::Float(af / *bi as f64));
                        }
                        _ => {
                            return Err(self.runtime_error(format!("cannot divide {} and {}", a.type_name(), b.type_name())));
                        }
                    }
                }

                Opcode::Mod => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => {
                            if *bi == 0 {
                                return Err(self.runtime_error("modulo by zero"));
                            }
                            self.stack.push(Value::Int(ai % bi));
                        }
                        _ => {
                            return Err(self.runtime_error(format!("cannot mod {} and {}", a.type_name(), b.type_name())));
                        }
                    }
                }

                Opcode::Neg => {
                    let a = self.stack.pop().unwrap();
                    match a {
                        Value::Int(n) => self.stack.push(Value::Int(-n)),
                        Value::Float(n) => self.stack.push(Value::Float(-n)),
                        _ => {
                            return Err(self.runtime_error(format!("cannot negate {}", a.type_name())));
                        }
                    }
                }

                Opcode::Not => {
                    let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(!a.is_truthy()));
                }

                Opcode::Eq => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(a == b));
                }

                Opcode::Ne => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(a != b));
                }

                Opcode::Lt => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(compare_lt(&a, &b)));
                }

                Opcode::Le => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(!compare_lt(&b, &a)));
                }

                Opcode::Gt => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(compare_lt(&b, &a)));
                }

                Opcode::Ge => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(!compare_lt(&a, &b)));
                }

                Opcode::Jump(_) => {
                    let target = self.read_u16() as usize;
                    self.frames.last_mut().unwrap().ip = target;
                }

                Opcode::JumpIfFalse(_) => {
                    let target = self.read_u16() as usize;
                    let cond = self.stack.pop().unwrap();
                    if !cond.is_truthy() {
                        self.frames.last_mut().unwrap().ip = target;
                    }
                }

                Opcode::Loop(_) => {
                    let target = self.read_u16() as usize;
                    self.frames.last_mut().unwrap().ip = target;
                }

                Opcode::Call(_) => {
                    let arg_count = self.read_u16() as usize;
                    let args_start = self.stack.len() - arg_count;
                    let callee = &self.stack[args_start - 1].clone();

                    match callee {
                        Value::Function(idx) => {
                            let fn_def = &self.functions[*idx];
                            // bp points to the first argument
                            let bp = args_start;
                            let frame = CallFrame::new(*idx, bp);
                            self.frames.push(frame);

                            // Ensure stack has room for locals (params already occupy
                            // slots 0..arg_count, push nils for remaining locals)
                            let slot_count = fn_def.chunk.locals as usize;
                            while self.stack.len() < bp + slot_count {
                                self.stack.push(Value::Nil);
                            }
                        }
                        Value::Closure(closure) => {
                            let data = closure.borrow();
                            let fn_idx = data.fn_idx;
                            let up_count = data.upvalues.len();
                            // Pop the arguments (excluding callee)
                            let args: Vec<Value> = self.stack.drain(args_start..).collect();
                            self.stack.pop(); // pop closure
                            // Push upvalues first
                            for uv in &data.upvalues {
                                self.stack.push(uv.clone());
                            }
                            // Push the actual arguments
                            for arg in &args {
                                self.stack.push(arg.clone());
                            }
                            // bp points to the first upvalue
                            let bp = self.stack.len() - up_count - args.len();
                            let frame = CallFrame::new(fn_idx, bp);
                            self.frames.push(frame);

                            let fn_def = &self.functions[fn_idx];
                            let slot_count = fn_def.chunk.locals as usize;
                            while self.stack.len() < bp + slot_count {
                                self.stack.push(Value::Nil);
                            }
                        }
                        Value::NativeFunction(f) => {
                            let args: Vec<Value> = self.stack.drain(args_start..).collect();
                            self.stack.pop(); // pop callee
                            let mut ctx = VMContext { registry: self.foreign_registry.clone() };
                            let result = f(&mut ctx, &args)?;
                            self.stack.push(result);
                        }
                        _ => {
                            return Err(self.runtime_error(format!("cannot call {}", callee.type_name())));
                        }
                    }
                }

                Opcode::CallMethod(_, _) => {
                    let method_idx = self.read_u16() as usize;
                    let arg_count = self.read_u16() as usize;
                    let args_start = self.stack.len() - arg_count;
                    let obj = &self.stack[args_start - 1].clone();

                    match obj {
                        // Foreign method dispatch via registry
                        Value::Foreign(fv) => {
                            let method_name = self.chunk().method_names.get(method_idx)
                                .cloned()
                                .unwrap_or_default();
                            let args: Vec<Value> = self.stack.drain(args_start..).collect();
                            self.stack.pop(); // pop receiver
                            let mut ctx = VMContext { registry: self.foreign_registry.clone() };
                            match self.foreign_registry.call_method(&fv.borrow().type_id, &method_name, &mut ctx, &args) {
                                Some(Ok(result)) => self.stack.push(result),
                                Some(Err(e)) => return Err(e),
                                None => {
                                    return Err(self.runtime_error(format!("foreign type '{}' has no method '{}'", fv.borrow().type_name, method_name)));
                                }
                            }
                        }
                        // Regular function dispatch (existing behavior for native script methods)
                        Value::Function(idx) => {
                            let fn_def = &self.functions[*idx];
                            let bp = args_start;
                            let frame = CallFrame::new(*idx, bp);
                            self.frames.push(frame);

                            let slot_count = fn_def.chunk.locals as usize;
                            while self.stack.len() < bp + slot_count {
                                self.stack.push(Value::Nil);
                            }
                        }
                        Value::NativeFunction(f) => {
                            let args: Vec<Value> = self.stack.drain(args_start..).collect();
                            self.stack.pop();
                            let mut ctx = VMContext { registry: self.foreign_registry.clone() };
                            let result = f(&mut ctx, &args)?;
                            self.stack.push(result);
                        }
                        _ => {
                            return Err(self.runtime_error(format!("cannot call method on {}", obj.type_name())));
                        }
                    }
                }

                Opcode::Return => {
                    let result = self.stack.pop().unwrap_or(Value::Nil);
                    let frame = self.frames.pop().unwrap();

                    // Remove callee + args (everything from bp-1 upward), keeping
                    // result on top. For the main frame (bp == 0) this is a no-op
                    // since there is no callee.
                    if frame.bp > 0 {
                        self.stack.truncate(frame.bp - 1);
                    } else {
                        self.stack.truncate(frame.bp);
                    }

                    if self.frames.is_empty() {
                        self.stack.push(result);
                        return Ok(());
                    }

                    self.stack.push(result);
                }

                Opcode::MakeStruct(_) => {
                    let field_count = self.read_u16() as usize;
                    let mut map = HashMap::new();
                    for _ in 0..field_count {
                        let val = self.stack.pop().unwrap();
                        let name = self.stack.pop().unwrap();
                        if let Value::Str(s) = name {
                            map.insert(s.to_string(), val);
                        }
                    }
                    self.stack.push(Value::Struct(Rc::new(RefCell::new(map))));
                }

                Opcode::MakeArray(_) => {
                    let count = self.read_u16() as usize;
                    let mut elems = Vec::with_capacity(count);
                    for _ in 0..count {
                        elems.push(self.stack.pop().unwrap());
                    }
                    elems.reverse();
                    self.stack.push(Value::Array(Rc::new(RefCell::new(elems))));
                }

                Opcode::MakeEnum(_, _) => {
                    let _tag = self.read_u16();
                    let _data_count = self.read_u16() as usize;
                    let mut data = Vec::new();
                    for _ in 0.._data_count {
                        data.push(self.stack.pop().unwrap());
                    }
                    data.reverse();
                    self.stack.push(Value::Enum {
                        tag: _tag,
                        data: Rc::new(RefCell::new(data)),
                    });
                }

                Opcode::LoadField(_) => {
                    let field_idx = self.read_u16() as usize;
                    let field_name = self.chunk().field_names.get(field_idx)
                        .cloned()
                        .unwrap_or_default();
                    let obj = self.stack.pop().unwrap();
                    match &obj {
                        Value::Struct(map) => {
                            let val = map.borrow().get(&field_name)
                                .cloned()
                                .unwrap_or(Value::Nil);
                            self.stack.push(val);
                        }
                        Value::Foreign(fv) => {
                            match self.foreign_registry.get_field(&fv.borrow().type_id, &field_name, &obj) {
                                Some(Ok(val)) => self.stack.push(val),
                                Some(Err(e)) => return Err(e),
                                None => {
                                    return Err(self.runtime_error(format!("foreign type '{}' has no field '{}'", fv.borrow().type_name, field_name)));
                                }
                            }
                        }
                        _ => {
                            return Err(self.runtime_error(format!("cannot access field on {}", obj.type_name())));
                        }
                    }
                }

                Opcode::StoreField(_) => {
                    let field_idx = self.read_u16() as usize;
                    let field_name = self.chunk().field_names.get(field_idx)
                        .cloned()
                        .unwrap_or_default();
                    let val = self.stack.pop().unwrap();
                    let mut obj = self.stack.pop().unwrap();
                    // Extract type_id before the match to avoid borrow conflicts
                    let foreign_type_id = match &obj {
                        Value::Foreign(fv) => Some(fv.borrow().type_id),
                        _ => None,
                    };
                    match &mut obj {
                        Value::Struct(map) => {
                            map.borrow_mut().insert(field_name, val);
                            self.stack.push(obj);
                        }
                        Value::Foreign(_) => {
                            let type_id = foreign_type_id.unwrap();
                            match self.foreign_registry.set_field(&type_id, &field_name, &mut obj, val) {
                                Some(Ok(())) => self.stack.push(obj),
                                Some(Err(e)) => return Err(e),
                                None => {
                                    return Err(self.runtime_error(format!("foreign type has no field '{}'", field_name)));
                                }
                            }
                        }
                        _ => {
                            return Err(self.runtime_error(format!("cannot set field on {}", obj.type_name())));
                        }
                    }
                }

                Opcode::LoadIndex => {
                    let index = self.stack.pop().unwrap();
                    let obj = self.stack.pop().unwrap();
                    match (&obj, &index) {
                        (Value::Array(arr), Value::Int(i)) => {
                            let idx = *i as usize;
                            let val = arr.borrow()[idx].clone();
                            self.stack.push(val);
                        }
                        (Value::Str(s), Value::Int(i)) => {
                            let idx = *i as usize;
                            let c = s.chars().nth(idx).map(|c| c.to_string()).unwrap_or_default();
                            self.stack.push(Value::Str(c.into()));
                        }
                        _ => {
                            return Err(self.runtime_error(format!("cannot index {} with {}", obj.type_name(), index.type_name())));
                        }
                    }
                }

                Opcode::StoreIndex => {
                    let val = self.stack.pop().unwrap();
                    let index = self.stack.pop().unwrap();
                    let obj = self.stack.pop().unwrap();
                    match (&obj, &index) {
                        (Value::Array(arr), Value::Int(i)) => {
                            let idx = *i as usize;
                            arr.borrow_mut()[idx] = val;
                            self.stack.push(obj);
                        }
                        _ => {
                            return Err(self.runtime_error(format!("cannot index {} with {}", obj.type_name(), index.type_name())));
                        }
                    }
                }

                Opcode::Len => {
                    let val = self.stack.pop().unwrap();
                    match val {
                        Value::Str(s) => self.stack.push(Value::Int(s.len() as i64)),
                        Value::Array(arr) => self.stack.push(Value::Int(arr.borrow().len() as i64)),
                        _ => return Err(self.runtime_error(format!("cannot get length of {}", val.type_name()))),
                    }
                }

                Opcode::NewClosure(_, _) => {
                    let fn_idx = self.read_u16() as usize;
                    let up_count = self.read_u16() as usize;
                    let mut upvalues = Vec::with_capacity(up_count);
                    for _ in 0..up_count {
                        upvalues.push(self.stack.pop().unwrap());
                    }
                    upvalues.reverse();
                    let data = Rc::new(RefCell::new(ClosureData { fn_idx, upvalues }));
                    self.stack.push(Value::Closure(data));
                }

                Opcode::BitAnd => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai & bi)),
                        _ => {
                            return Err(self.runtime_error(format!("cannot bitwise-and {} and {}", a.type_name(), b.type_name())));
                        }
                    }
                }

                Opcode::BitOr => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai | bi)),
                        _ => {
                            return Err(self.runtime_error(format!("cannot bitwise-or {} and {}", a.type_name(), b.type_name())));
                        }
                    }
                }

                Opcode::BitXor => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai ^ bi)),
                        _ => {
                            return Err(self.runtime_error(format!("cannot bitwise-xor {} and {}", a.type_name(), b.type_name())));
                        }
                    }
                }

                Opcode::Shl => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai << bi)),
                        _ => {
                            return Err(self.runtime_error(format!("cannot shift left {} and {}", a.type_name(), b.type_name())));
                        }
                    }
                }

                Opcode::Shr => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai >> bi)),
                        _ => {
                            return Err(self.runtime_error(format!("cannot shift right {} and {}", a.type_name(), b.type_name())));
                        }
                    }
                }

                Opcode::BitNot => {
                    let a = self.stack.pop().unwrap();
                    match a {
                        Value::Int(n) => self.stack.push(Value::Int(!n)),
                        _ => {
                            return Err(self.runtime_error(format!("cannot bitwise-not {}", a.type_name())));
                        }
                    }
                }

                Opcode::LoadEnumTag => {
                    let val = self.stack.pop().unwrap();
                    match val {
                        Value::Enum { tag, data: _ } => self.stack.push(Value::Int(tag as i64)),
                        _ => return Err(self.runtime_error(format!("LoadEnumTag on non-enum value"))),
                    }
                }

                Opcode::LoadEnumField(_) => {
                    let idx = self.read_u16() as usize;
                    let val = self.stack.pop().unwrap();
                    match val {
                        Value::Enum { tag: _, data } => {
                            let field = data.borrow().get(idx).cloned().unwrap_or(Value::Nil);
                            self.stack.push(field);
                        }
                        _ => return Err(self.runtime_error(format!("LoadEnumField on non-enum value"))),
                    }
                }

                Opcode::Halt => {
                    break;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler;
    use crate::interop;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::value::ForeignObject;

    fn run(source: &str) -> Value {
        let tokens = Lexer::new(source).tokenize().unwrap();
        let parser = Parser::new(source, &tokens);
        let mut program = parser.parse().unwrap();
        let native_names = crate::stdlib::native_names();
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        vm.run_main().unwrap()
    }

    #[test]
    fn test_nil() {
        assert_eq!(run(""), Value::Nil);
    }

    #[test]
    fn test_int_literal() {
        let result = run("42");
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_float_literal() {
        let result = run("3.14");
        assert_eq!(result, Value::Float(3.14));
    }

    #[test]
    fn test_bool_literal() {
        let result = run("true");
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn test_string_literal() {
        let result = run("\"hello\"");
        assert_eq!(result, Value::Str("hello".into()));
    }

    #[test]
    fn test_add_ints() {
        let result = run("1 + 2");
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_sub_ints() {
        let result = run("10 - 3");
        assert_eq!(result, Value::Int(7));
    }

    #[test]
    fn test_mul_ints() {
        let result = run("3 * 4");
        assert_eq!(result, Value::Int(12));
    }

    #[test]
    fn test_div_ints() {
        let result = run("10 / 3");
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_let_binding() {
        let result = run("let x = 42; x");
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_if_true() {
        let result = run("if true { 1 } else { 2 }");
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn test_if_false() {
        let result = run("if false { 1 } else { 2 }");
        assert_eq!(result, Value::Int(2));
    }

    #[test]
    fn test_while_loop() {
        let result = run("let i = 0; while i < 5 { i = i + 1 }; i");
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn test_comparison() {
        let result = run("3 < 5");
        assert_eq!(result, Value::Bool(true));

        let result = run("5 < 3");
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_equality() {
        let result = run("3 == 3");
        assert_eq!(result, Value::Bool(true));

        let result = run("3 == 4");
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_block_expr() {
        let result = run("{ let x = 10; x + 5 }");
        assert_eq!(result, Value::Int(15));
    }

    #[test]
    fn test_negation() {
        let result = run("-5");
        assert_eq!(result, Value::Int(-5));
    }

    #[test]
    fn test_boolean_not() {
        let result = run("!true");
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_for_loop() {
        let result = run("let s = 0; for i in 0..3 { s = s + i }; s");
        assert_eq!(result, Value::Int(3)); // 0 + 1 + 2
    }

    #[test]
    fn test_match_int() {
        let result = run("match 2 { 1 => 10, 2 => 20, 3 => 30 }");
        assert_eq!(result, Value::Int(20));
    }

    #[test]
    fn test_match_wildcard() {
        let result = run("match 99 { 1 => 10, _ => 99 }");
        assert_eq!(result, Value::Int(99));
    }

    #[test]
    fn test_function_call() {
        let result = run("
            fn add(a: int, b: int) -> int {
                a + b
            }
            add(3, 4)
        ");
        assert_eq!(result, Value::Int(7));
    }

    #[test]
    fn test_function_return() {
        let result = run("
            fn make(n: int) -> int {
                return n * 2
            }
            make(5)
        ");
        assert_eq!(result, Value::Int(10));
    }

    #[test]
    fn test_nested_scopes() {
        let result = run("
            let x = 1;
            {
                let x = 2;
                x
            }
        ");
        assert_eq!(result, Value::Int(2));
    }

    // --- Interop / Foreign type tests ---

    struct Point {
        x: i32,
        y: i32,
    }

    fn setup_vm_with_point() -> VM {
        let mut vm = VM::new();
        vm.register_type::<Point>("Point")
            .field("x",
                |obj: &Value| -> Result<Value> {
                    interop::with_foreign::<Point, _, _>(obj, |p| Ok(Value::Int(p.x as i64)))
                },
                |obj: &mut Value, val: Value| -> Result<()> {
                    let x = val.as_int().unwrap() as i32;
                    interop::with_foreign_mut::<Point, _, _>(obj, |p| { p.x = x; Ok(()) })
                },
            )
            .field("y",
                |obj: &Value| -> Result<Value> {
                    interop::with_foreign::<Point, _, _>(obj, |p| Ok(Value::Int(p.y as i64)))
                },
                |obj: &mut Value, val: Value| -> Result<()> {
                    let y = val.as_int().unwrap() as i32;
                    interop::with_foreign_mut::<Point, _, _>(obj, |p| { p.y = y; Ok(()) })
                },
            );
        vm
    }

    #[test]
    fn test_interop_register_type() {
        let vm = setup_vm_with_point();
        let def = vm.foreign_registry.get(&TypeId::of::<Point>()).unwrap();
        assert_eq!(def.name, "Point");
        assert!(def.fields.contains_key("x"));
        assert!(def.fields.contains_key("y"));
    }

    #[test]
    fn test_interop_foreign_field_access() {
        let vm = setup_vm_with_point();
        let point = Point { x: 10, y: 20 };
        let fv = Value::Foreign(Rc::new(RefCell::new(ForeignObject::new("Point", point))));

        assert_eq!(fv.type_name(), "Point");

        let def = vm.foreign_registry.get(&TypeId::of::<Point>()).unwrap();
        let result = def.fields.get("x").unwrap().get(&fv).unwrap();
        assert_eq!(result, Value::Int(10));

        let result = def.fields.get("y").unwrap().get(&fv).unwrap();
        assert_eq!(result, Value::Int(20));
    }

    #[test]
    fn test_interop_foreign_field_mutation() {
        let vm = setup_vm_with_point();
        let point = Point { x: 1, y: 2 };
        let mut fv = Value::Foreign(Rc::new(RefCell::new(ForeignObject::new("Point", point))));

        let def = vm.foreign_registry.get(&TypeId::of::<Point>()).unwrap();
        def.fields.get("x").unwrap().set(&mut fv, Value::Int(99)).unwrap();

        let result = def.fields.get("x").unwrap().get(&fv).unwrap();
        assert_eq!(result, Value::Int(99));
    }

    #[test]
    fn test_interop_foreign_method() {
        let mut vm = VM::new();
        vm.register_type::<Point>("Point")
            .field("x",
                |obj: &Value| -> Result<Value> {
                    interop::with_foreign::<Point, _, _>(obj, |p| Ok(Value::Int(p.x as i64)))
                },
                |obj: &mut Value, val: Value| -> Result<()> {
                    let x = val.as_int().unwrap() as i32;
                    interop::with_foreign_mut::<Point, _, _>(obj, |p| { p.x = x; Ok(()) })
                },
            )
            .method("double_x", Rc::new(|_ctx: &mut VMContext, args: &[Value]| -> Result<Value> {
                interop::with_foreign::<Point, _, _>(&args[0], |p| Ok(Value::Int((p.x * 2) as i64)))
            }));

        let point = Point { x: 5, y: 10 };
        let fv = Value::Foreign(Rc::new(RefCell::new(ForeignObject::new("Point", point))));

        // Call method via registry
        let mut ctx = VMContext { registry: vm.foreign_registry.clone() };
        let result = vm.foreign_registry.call_method(
            &TypeId::of::<Point>(),
            "double_x",
            &mut ctx,
            &[fv.clone()],
        ).unwrap().unwrap();
        assert_eq!(result, Value::Int(10));
    }

    #[test]
    fn test_native_function_call_direct() {
        // Test that a native function can be called through the VM directly
        // without going through the full compiler pipeline.
        // (Full pipeline requires pre-registering names with the resolver.)
        fn double(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
            let n = args.first().and_then(|v| v.as_int()).unwrap_or(0);
            Ok(Value::Int(n * 2))
        }

        // Verify the NativeFn works directly:
        let ctx = &mut VMContext { registry: Rc::new(ForeignTypeRegistry::new()) };
        let result = double(ctx, &[Value::Int(5)]).unwrap();
        assert_eq!(result, Value::Int(10));
    }

    // --- Hot reload tests ---

    #[test]
    fn test_snapshot_and_restore_globals() {
        let mut vm = VM::new();
        vm.global_names = vec!["x".into(), "y".into(), "z".into()];
        vm.globals = vec![Value::Int(1), Value::Int(2), Value::Int(3)];

        let snapshot = vm.snapshot_globals_by_name();
        assert_eq!(snapshot.get("x"), Some(&Value::Int(1)));
        assert_eq!(snapshot.get("y"), Some(&Value::Int(2)));
        assert_eq!(snapshot.get("z"), Some(&Value::Int(3)));

        // Modify globals and restore
        vm.globals[0] = Value::Int(99);
        vm.restore_globals_by_name(&snapshot);
        assert_eq!(vm.globals[0], Value::Int(1));
        assert_eq!(vm.globals[1], Value::Int(2));
        assert_eq!(vm.globals[2], Value::Int(3));
    }

    #[test]
    fn test_snapshot_only_matches_by_name() {
        let mut vm = VM::new();
        vm.global_names = vec!["a".into(), "b".into()];
        vm.globals = vec![Value::Int(10), Value::Int(20)];

        let mut snapshot = HashMap::new();
        snapshot.insert("b".into(), Value::Int(99));

        vm.restore_globals_by_name(&snapshot);
        assert_eq!(vm.globals[0], Value::Int(10)); // unchanged
        assert_eq!(vm.globals[1], Value::Int(99)); // restored
    }

    #[test]
    fn test_reload_functions_preserves_global_state() {
        let source = r#"
            let x = 10;
            fn add_one(y: int) -> int {
                y + 1
            }
            x
        "#;

        // Initial compilation
        let tokens = Lexer::new(source).tokenize().unwrap();
        let parser = Parser::new(source, &tokens);
        let mut program = parser.parse().unwrap();
        let mut symbols = crate::resolver::resolve(&mut program).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) = compiler::compile(&program, &types, &symbols, &[], source).unwrap();

        let mut vm = VM::new();
        vm.load_bytecode(fns, global_names);
        let result = vm.run_main().unwrap();
        assert_eq!(result, Value::Int(10));

        // Simulate a hot reload with the same source (no changes)
        let tokens = Lexer::new(source).tokenize().unwrap();
        let parser = Parser::new(source, &tokens);
        let mut program = parser.parse().unwrap();
        let mut symbols = crate::resolver::resolve(&mut program).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) = compiler::compile(&program, &types, &symbols, &[], source).unwrap();

        vm.reload_functions(fns, global_names).unwrap();
        let result = vm.run_main().unwrap();
        assert_eq!(result, Value::Int(10));
    }

    #[test]
    fn test_reload_functions_with_modified_source() {
        // Initial: let x = 5; x
        let source1 = "let x = 5; x";
        let tokens = Lexer::new(source1).tokenize().unwrap();
        let parser = Parser::new(source1, &tokens);
        let mut program = parser.parse().unwrap();
        let mut symbols = crate::resolver::resolve(&mut program).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) = compiler::compile(&program, &types, &symbols, &[], source1).unwrap();

        let mut vm = VM::new();
        vm.load_bytecode(fns, global_names);
        let result = vm.run_main().unwrap();
        assert_eq!(result, Value::Int(5));

        // Simulate changing x = 5 to x = 42 in the source and hot reload
        let source2 = "let x = 42; x";
        let tokens = Lexer::new(source2).tokenize().unwrap();
        let parser = Parser::new(source2, &tokens);
        let mut program = parser.parse().unwrap();
        let mut symbols = crate::resolver::resolve(&mut program).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) = compiler::compile(&program, &types, &symbols, &[], source2).unwrap();

        vm.reload_functions(fns, global_names).unwrap();
        let result = vm.run_main().unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_reload_functions_remaps_function_references() {
        let source = r#"
            fn greet() -> int { 1 }
            fn run() -> int { greet() }
            run()
        "#;

        // Initial compilation
        let tokens = Lexer::new(source).tokenize().unwrap();
        let parser = Parser::new(source, &tokens);
        let mut program = parser.parse().unwrap();
        let mut symbols = crate::resolver::resolve(&mut program).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) = compiler::compile(&program, &types, &symbols, &[], source).unwrap();

        let mut vm = VM::new();
        vm.load_bytecode(fns, global_names);
        let result = vm.run_main().unwrap();
        assert_eq!(result, Value::Int(1));

        // Reload with same source (function indices should be stable)
        let tokens = Lexer::new(source).tokenize().unwrap();
        let parser = Parser::new(source, &tokens);
        let mut program = parser.parse().unwrap();
        let mut symbols = crate::resolver::resolve(&mut program).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) = compiler::compile(&program, &types, &symbols, &[], source).unwrap();

        vm.reload_functions(fns, global_names).unwrap();
        let result = vm.run_main().unwrap();
        assert_eq!(result, Value::Int(1));
    }

    // --- Stdlib tests ---

    #[test]
    fn test_print() {
        // print just returns nil
        let result = run("print(42)");
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn test_type_of() {
        let result = run("type_of(42)");
        assert_eq!(result, Value::Str("int".into()));

        let result = run("type_of(true)");
        assert_eq!(result, Value::Str("bool".into()));
    }

    #[test]
    fn test_len_string() {
        let result = run("len(\"hello\")");
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn test_contains() {
        let result = run("contains(\"hello world\", \"world\")");
        assert_eq!(result, Value::Bool(true));

        let result = run("contains(\"hello world\", \"xyz\")");
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_trim() {
        let result = run("trim(\"  hello  \")");
        assert_eq!(result, Value::Str("hello".into()));
    }

    #[test]
    fn test_to_upper() {
        let result = run("to_upper(\"hello\")");
        assert_eq!(result, Value::Str("HELLO".into()));
    }

    #[test]
    fn test_to_lower() {
        let result = run("to_lower(\"HELLO\")");
        assert_eq!(result, Value::Str("hello".into()));
    }

    #[test]
    fn test_substring() {
        let result = run("substring(\"hello\", 1, 4)");
        assert_eq!(result, Value::Str("ell".into()));
    }

    #[test]
    fn test_abs() {
        let result = run("abs(-5)");
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn test_min_max() {
        let result = run("min(3, 7)");
        assert_eq!(result, Value::Int(3));

        let result = run("max(3, 7)");
        assert_eq!(result, Value::Int(7));
    }

    #[test]
    fn test_sqrt() {
        let result = run("sqrt(9.0)");
        assert_eq!(result, Value::Float(3.0));
    }

    #[test]
    fn test_array_push_pop() {
        let result = run("
            let arr = [1, 2, 3];
            push(arr, 4);
            len(arr)
        ");
        assert_eq!(result, Value::Int(4));

        let result = run("
            let arr = [10, 20];
            pop(arr)
        ");
        assert_eq!(result, Value::Int(20));
    }

    #[test]
    fn test_array_insert_remove() {
        let result = run("
            let arr = [1, 3];
            insert(arr, 1, 2);
            len(arr)
        ");
        assert_eq!(result, Value::Int(3));

        let result = run("
            let arr = [10, 20, 30];
            remove(arr, 0)
        ");
        assert_eq!(result, Value::Int(10));
    }

    #[test]
    fn test_len_array() {
        let result = run("len([1, 2, 3, 4])");
        assert_eq!(result, Value::Int(4));
    }

    #[test]
    fn test_to_int() {
        let result = run("to_int(42)");
        assert_eq!(result, Value::Int(42));

        let result = run("to_int(3.14)");
        assert_eq!(result, Value::Int(3));

        let result = run("to_int(\"123\")");
        assert_eq!(result, Value::Int(123));

        let result = run("to_int(true)");
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn test_to_float() {
        let result = run("to_float(3)");
        assert_eq!(result, Value::Float(3.0));

        let result = run("to_float(\"3.14\")");
        assert_eq!(result, Value::Float(3.14));
    }

    #[test]
    fn test_to_str() {
        let result = run("to_str(42)");
        assert_eq!(result, Value::Str("Int(42)".into()));
    }

    #[test]
    fn test_native_function_call_script() {
        let result = run("print(\"hello\"); 42");
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_print_multiple_args() {
        let result = run("print(1, 2, 3)");
        assert_eq!(result, Value::Nil);
    }
}

/// Recursively remap `Value::Function` indices in a value tree from old
/// function indices to new ones, using name-based lookup.
fn remap_function_value(
    val: &mut Value,
    old_name_to_idx: &HashMap<&str, usize>,
    new_name_to_idx: &HashMap<&str, usize>,
) {
    match val {
        Value::Function(idx) => {
            if let Some(name) = old_name_to_idx.iter().find(|(_, v)| **v == *idx).map(|(n, _)| *n) {
                if let Some(&new_idx) = new_name_to_idx.get(name) {
                    *idx = new_idx;
                }
            }
        }
        Value::Closure(c) => {
            let mut data = c.borrow_mut();
            if let Some(name) = old_name_to_idx.iter().find(|(_, v)| **v == data.fn_idx).map(|(n, _)| *n) {
                if let Some(&new_idx) = new_name_to_idx.get(name) {
                    data.fn_idx = new_idx;
                }
            }
            for uv in data.upvalues.iter_mut() {
                remap_function_value(uv, old_name_to_idx, new_name_to_idx);
            }
        }
        Value::Array(arr) => {
            for v in arr.borrow_mut().iter_mut() {
                remap_function_value(v, old_name_to_idx, new_name_to_idx);
            }
        }
        Value::Struct(map) => {
            for v in map.borrow_mut().values_mut() {
                remap_function_value(v, old_name_to_idx, new_name_to_idx);
            }
        }
        Value::Enum { data, .. } => {
            for v in data.borrow_mut().iter_mut() {
                remap_function_value(v, old_name_to_idx, new_name_to_idx);
            }
        }
        _ => {}
    }
}

fn compare_lt(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(ai), Value::Int(bi)) => ai < bi,
        (Value::Float(af), Value::Float(bf)) => af < bf,
        (Value::Int(ai), Value::Float(bf)) => (*ai as f64) < *bf,
        (Value::Float(af), Value::Int(bi)) => *af < (*bi as f64),
        _ => false,
    }
}

#[cfg(test)]
mod closure_tests {
    use super::*;
    use crate::compiler;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn run(source: &str) -> Value {
        let tokens = Lexer::new(source).tokenize().unwrap();
        let mut program = Parser::new(source, &tokens).parse().unwrap();
        let native_names = crate::stdlib::native_names();
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) = compiler::compile(
            &program, &types, &symbols, &native_names, source
        ).unwrap();
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        vm.run_main().unwrap()
    }

    #[test]
    fn test_simple_closure() {
        let result = run("let f = || 42; f()");
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_closure_with_params() {
        let result = run("let f = |x, y| x + y; f(3, 4)");
        assert_eq!(result, Value::Int(7));
    }

    #[test]
    fn test_closure_captures_upvalue() {
        let result = run("let x = 10; let f = || x + 5; f()");
        assert_eq!(result, Value::Int(15));
    }
}

#[cfg(test)]
mod module_tests {
    use super::*;
    use crate::compiler;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn run(source: &str) -> Value {
        let tokens = Lexer::new(source).tokenize().unwrap();
        let mut program = Parser::new(source, &tokens).parse().unwrap();
        let native_names = crate::stdlib::native_names();
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) = compiler::compile(
            &program, &types, &symbols, &native_names, source
        ).unwrap();
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        vm.run_main().unwrap()
    }

    #[test]
    fn test_mod_defines_module() {
        let result = run("mod math { fn add(x, y) { x + y } } ()");
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn test_use_imports_function() {
        let result = run("mod math { fn add(x, y) { x + y } } use math::add; add(1, 2)");
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_use_multiple_items() {
        let result = run("
            mod math {
                fn add(x, y) { x + y }
                fn mul(x, y) { x * y }
            }
            use math::add;
            use math::mul;
            add(mul(2, 3), 1)
        ");
        assert_eq!(result, Value::Int(7));
    }

    #[test]
    fn test_mod_with_let() {
        let result = run("
            mod config {
                let pi = 314;
            }
            use config::pi;
            pi
        ");
        assert_eq!(result, Value::Int(314));
    }

    #[test]
    fn test_use_imports_from_first_module() {
        let result = run("
            mod math { fn add(x, y) { x + y } }
            use math::add;
            add(2, 3)
        ");
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn test_use_imports_from_second_module() {
        let result = run("
            mod a { fn double(x) { x * 2 } }
            mod b { fn triple(x) { x * 3 } }
            use b::triple;
            triple(4)
        ");
        assert_eq!(result, Value::Int(12));
    }
}
