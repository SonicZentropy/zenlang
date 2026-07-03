use std::any::TypeId;
use std::collections::HashMap;
use std::rc::Rc;

use crate::error::{Error, Result};
use crate::interop::{ForeignTypeDef, ForeignTypeRegistry};
use crate::ir::{BytecodeFn, Chunk, Opcode};
use crate::slab::{Handle, Slab};
use crate::value::{
    ArrayData, ClosureData, EnumData, ForeignObject, GeneratorState, MapData, NativeFn,
    StructBuilder, StructData, Value, WeakData,
};

/// Execution context provided to native functions.
///
/// Exposes `call_value` for calling back into the Zenlang runtime,
/// and `register_timer` / `remove_timer` for scheduling.
pub struct VMContext {
    pub registry: Rc<ForeignTypeRegistry>,
    pub raw_vm: *mut VM,
}

impl VMContext {
    /// Register a one-shot or interval timer from a native function.
    ///
    /// The `callback` is a Zenlang function/closure invoked after `delay`
    /// seconds. If `interval` is `Some(dur)`, the timer repeats every `dur`
    /// seconds. Returns a timer ID that can be passed to `remove_timer`.
    pub fn register_timer(&mut self, callback: Value, delay: f64, interval: Option<f64>) -> u64 {
        let vm: &mut VM = unsafe { &mut *self.raw_vm };
        let id = vm.timer_id_counter;
        vm.timer_id_counter += 1;
        let fire_time = vm.time + delay.max(0.0);
        vm.pending_timers.push(TimerEntry { id, callback, fire_time, interval });
        id
    }

    /// Cancel a timer previously created with [`register_timer`](VMContext::register_timer).
    pub fn remove_timer(&mut self, id: u64) {
        let vm: &mut VM = unsafe { &mut *self.raw_vm };
        vm.timers.retain(|t| t.id != id);
        vm.pending_timers.retain(|t| t.id != id);
    }

    /// Call a script function or closure from a native function.
    ///
    /// This is the safe entry point for calling back into Zenlang from Rust
    /// native functions. It pushes a new call frame, runs the function to
    /// completion, and returns the result value.
    ///
    /// Reentrancy: this calls `VM::execute()` recursively from within the
    /// execution loop. A `return_to_depth` field on `VM` ensures the inner
    /// `execute()` returns once the callback frame is popped, without
    /// consuming the outer function's remaining instructions.
    pub fn call_value(&mut self, callee: &Value, args: &[Value]) -> Result<Value> {
        let vm: &mut VM = unsafe { &mut *self.raw_vm };
        let frame_count = vm.frames.len();
        let saved_count = vm.instruction_count;

        vm.return_to_depth = Some(frame_count);

        let result = match callee {
            Value::Function(idx) => {
                let fn_def = &vm.functions[*idx];
                if fn_def.is_generator {
                    return Err(vm.runtime_error("cannot call generator via call_value"));
                }
                let bp = vm.stack.len();
                vm.frames.push(CallFrame {
                    function_idx: *idx, ip: 0, bp,
                    is_method: false, is_closure: true,
                });
                for arg in args {
                    vm.stack.push(arg.clone());
                }
                let slot_count = fn_def.chunk.locals as usize;
                while vm.stack.len() < bp + slot_count {
                    vm.stack.push(Value::Nil);
                }
                vm.execute()?;
                vm.stack.pop().unwrap_or(Value::Nil)
            }
            Value::Closure(h) => {
                let data = vm.closures.get(*h);
                let fn_idx = data.fn_idx;
                let fn_def = &vm.functions[fn_idx];
                let bp = vm.stack.len();
                vm.frames.push(CallFrame {
                    function_idx: fn_idx, ip: 0, bp,
                    is_method: false, is_closure: true,
                });
                for uv in &data.upvalues {
                    vm.stack.push(uv.clone());
                }
                for arg in args {
                    vm.stack.push(arg.clone());
                }
                let slot_count = fn_def.chunk.locals as usize;
                while vm.stack.len() < bp + slot_count {
                    vm.stack.push(Value::Nil);
                }
                vm.execute()?;
                vm.stack.pop().unwrap_or(Value::Nil)
            }
            _ => return Err(vm.runtime_error(format!("cannot call {}", callee.type_name()))),
        };

        vm.return_to_depth = None;
        vm.instruction_count = saved_count;
        Ok(result)
    }
}

#[derive(Debug, Clone)]
struct CallFrame {
    function_idx: usize,
    ip: usize,
    bp: usize,
    is_method: bool,
    is_closure: bool,
}

impl CallFrame {
    fn new(function_idx: usize, bp: usize) -> Self {
        Self { function_idx, ip: 0, bp, is_method: false, is_closure: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugStepMode { None, StepInto, StepOver, StepOut }

#[derive(Debug, Clone)]
pub struct DebugFrameInfo {
    pub depth: usize,
    pub function: String,
    pub source_location: crate::span::SourceLocation,
}

#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub function: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct DebugState {
    pub enabled: bool,
    pub paused: bool,
    pub step_mode: DebugStepMode,
    pub step_start_depth: usize,
    pub breakpoints: Vec<Breakpoint>,
    pub resolved_breakpoints: Vec<(usize, usize)>,
    pub skip_offset: Option<(usize, usize)>,
}

fn source_loc_from_frame(
    functions: &[BytecodeFn], function_idx: usize, ip: usize,
) -> crate::span::SourceLocation {
    let line = functions.get(function_idx).map(|f| f.chunk.get_line(ip.saturating_sub(1))).unwrap_or(0);
    crate::span::SourceLocation::new(None, crate::span::Span::new(0, 0), line, 0)
}

struct TimerEntry {
    id: u64, callback: Value, fire_time: f64, interval: Option<f64>,
}

pub struct VM {
    pub stack: Vec<Value>,
    frames: Vec<CallFrame>,
    pub globals: Vec<Value>,
    pub functions: Vec<BytecodeFn>,
    pub global_names: Vec<String>,
    pub function_name_map: HashMap<String, usize>,
    natives: HashMap<String, usize>,
    native_fns: Vec<(String, NativeFn)>,
    pub foreign_registry: Rc<ForeignTypeRegistry>,
    instruction_limit: u64,
    instruction_count: u64,
    active_generator: Option<(Handle, usize)>, // (GeneratorHandle, saved_frame_count)
    time: f64,
    timers: Vec<TimerEntry>,
    pending_timers: Vec<TimerEntry>,
    frame_callbacks: Vec<Value>,
    timer_id_counter: u64,
    pub debug_state: DebugState,
    return_to_depth: Option<usize>,

    // ── Slabs for heap-allocated objects ──
    pub arrays: Slab<ArrayData>,
    pub structs: Slab<StructData>,
    pub enums: Slab<EnumData>,
    pub maps: Slab<MapData>,
    pub closures: Slab<ClosureData>,
    pub generators: Slab<GeneratorState>,
    pub foreigns: Slab<ForeignObject>,
    pub weaks: Slab<WeakData>,
}

impl VM {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(), frames: Vec::new(),
            globals: Vec::new(), functions: Vec::new(),
            global_names: Vec::new(), function_name_map: HashMap::new(),
            natives: HashMap::new(), native_fns: Vec::new(),
            foreign_registry: Rc::new(ForeignTypeRegistry::new()),
            instruction_limit: 0, instruction_count: 0,
            active_generator: None, time: 0.0,
            timers: Vec::new(), pending_timers: Vec::new(),
            frame_callbacks: Vec::new(), timer_id_counter: 1,
            debug_state: DebugState {
                enabled: false, paused: false, step_mode: DebugStepMode::None,
                step_start_depth: 0, breakpoints: Vec::new(),
                resolved_breakpoints: Vec::new(), skip_offset: None,
            },
            return_to_depth: None,
            arrays: Slab::new(), structs: Slab::new(), enums: Slab::new(),
            maps: Slab::new(), closures: Slab::new(), generators: Slab::new(),
            foreigns: Slab::new(), weaks: Slab::new(),
        }
    }

    pub fn new_with_registry(registry: Rc<ForeignTypeRegistry>) -> Self {
        let mut vm = Self::new();
        vm.foreign_registry = registry;
        vm
    }

    pub fn set_debug(&mut self, enabled: bool) {
        self.debug_state.enabled = enabled;
        if !enabled { self.debug_state.paused = false; self.debug_state.step_mode = DebugStepMode::None; }
    }

    pub fn set_breakpoint(&mut self, function: &str, line: usize) -> bool {
        if !self.function_name_map.contains_key(function) { return false; }
        if self.debug_state.breakpoints.iter().any(|b| b.function == function && b.line == line) { return true; }
        self.debug_state.breakpoints.push(Breakpoint { function: function.to_string(), line });
        self.rebuild_breakpoints();
        true
    }

    pub fn remove_breakpoint(&mut self, function: &str, line: usize) {
        self.debug_state.breakpoints.retain(|b| b.function != function || b.line != line);
        self.rebuild_breakpoints();
    }

    pub fn clear_breakpoints(&mut self) {
        self.debug_state.breakpoints.clear();
        self.debug_state.resolved_breakpoints.clear();
    }

    pub fn set_source_breakpoint(&mut self, line: usize) -> usize {
        let mut count = 0;
        let names: Vec<String> = self.function_name_map.keys().cloned().collect();
        for name in &names {
            if let Some(&idx) = self.function_name_map.get(name) {
                if let Some(f) = self.functions.get(idx) {
                    if f.chunk.lines.iter().any(|l| *l + 1 == line) {
                        if self.set_breakpoint(name, line) { count += 1; }
                    }
                }
            }
        }
        count
    }

    fn rebuild_breakpoints(&mut self) {
        self.debug_state.resolved_breakpoints.clear();
        for bp in &self.debug_state.breakpoints {
            let Some(&fn_idx) = self.function_name_map.get(&bp.function) else { continue };
            let Some(fn_def) = self.functions.get(fn_idx) else { continue };
            for (offset, &l) in fn_def.chunk.lines.iter().enumerate() {
                if l + 1 == bp.line {
                    self.debug_state.resolved_breakpoints.push((fn_idx, offset));
                    break;
                }
            }
        }
    }

    pub fn debug_continue(&mut self) -> Result<Value> {
        if !self.debug_state.paused { return Err(self.runtime_error("not paused")); }
        self.debug_state.step_mode = DebugStepMode::None;
        self.debug_state.paused = false;
        self.execute_debug()
    }

    pub fn debug_step_into(&mut self) -> Result<Value> {
        if !self.debug_state.paused { return Err(self.runtime_error("not paused")); }
        self.debug_state.step_mode = DebugStepMode::StepInto;
        self.debug_state.step_start_depth = self.frames.len();
        self.debug_state.paused = false;
        self.execute_debug()
    }

    pub fn debug_step_over(&mut self) -> Result<Value> {
        if !self.debug_state.paused { return Err(self.runtime_error("not paused")); }
        self.debug_state.step_mode = DebugStepMode::StepOver;
        self.debug_state.step_start_depth = self.frames.len();
        self.debug_state.paused = false;
        self.execute_debug()
    }

    pub fn debug_step_out(&mut self) -> Result<Value> {
        if !self.debug_state.paused { return Err(self.runtime_error("not paused")); }
        self.debug_state.step_mode = DebugStepMode::StepOut;
        self.debug_state.step_start_depth = self.frames.len();
        self.debug_state.paused = false;
        self.execute_debug()
    }

    pub fn is_paused(&self) -> bool { self.debug_state.paused }

    pub fn debug_current_location(&self) -> Option<crate::span::SourceLocation> {
        let frame = self.frames.last()?;
        Some(source_loc_from_frame(&self.functions, frame.function_idx, frame.ip))
    }

    pub fn debug_stack_frames(&self) -> Vec<DebugFrameInfo> {
        self.frames.iter().enumerate().map(|(depth, frame)| {
            let loc = source_loc_from_frame(&self.functions, frame.function_idx, frame.ip);
            DebugFrameInfo { depth, function: self.functions[frame.function_idx].name.clone(), source_location: loc }
        }).collect()
    }

    pub fn debug_locals(&self, depth: usize) -> Vec<(String, Value)> {
        if depth >= self.frames.len() { return Vec::new(); }
        let frame = &self.frames[self.frames.len() - 1 - depth];
        let fn_def = &self.functions[frame.function_idx];
        let local_count = fn_def.chunk.locals as usize;
        let mut locals = Vec::with_capacity(local_count);
        for i in 0..local_count {
            let name = if i < fn_def.arity as usize { format!("param_{}", i) } else { format!("local_{}", i - fn_def.arity as usize) };
            let val = self.stack.get(frame.bp + i).cloned().unwrap_or(Value::Nil);
            locals.push((name, val));
        }
        locals
    }

    fn execute_debug(&mut self) -> Result<Value> {
        loop {
            self.execute()?;
            if self.debug_state.paused { return Ok(Value::Nil); }
            return Ok(self.stack.pop().unwrap_or(Value::Nil));
        }
    }

    fn debug_check(&mut self) -> bool {
        if !self.debug_state.enabled || self.debug_state.paused { return false; }
        let Some(frame) = self.frames.last() else { return false };
        let (fn_idx, ip) = (frame.function_idx, frame.ip);
        if self.debug_state.skip_offset == Some((fn_idx, ip)) {
            self.debug_state.skip_offset = None;
            return false;
        }
        self.debug_state.skip_offset = None;
        for &(bp_fn, bp_off) in &self.debug_state.resolved_breakpoints {
            if bp_fn == fn_idx && bp_off == ip {
                self.debug_state.skip_offset = Some((fn_idx, ip));
                return true;
            }
        }
        match self.debug_state.step_mode {
            DebugStepMode::None => {}
            DebugStepMode::StepInto => {
                self.debug_state.step_mode = DebugStepMode::None;
                self.debug_state.skip_offset = Some((fn_idx, ip));
                return true;
            }
            DebugStepMode::StepOver => {
                if self.frames.len() <= self.debug_state.step_start_depth {
                    self.debug_state.step_mode = DebugStepMode::None;
                    self.debug_state.skip_offset = Some((fn_idx, ip));
                    return true;
                }
            }
            DebugStepMode::StepOut => {
                if self.frames.len() < self.debug_state.step_start_depth {
                    self.debug_state.step_mode = DebugStepMode::None;
                    self.debug_state.skip_offset = Some((fn_idx, ip));
                    return true;
                }
            }
        }
        false
    }

    pub fn set_instruction_limit(&mut self, limit: u64) { self.instruction_limit = limit; }

    pub fn register_type<T: 'static>(&mut self, name: &'static str) -> &mut ForeignTypeDef {
        let def = ForeignTypeDef::new(name);
        let type_id = TypeId::of::<T>();
        let registry = Rc::make_mut(&mut self.foreign_registry);
        registry.register_typed(type_id, def);
        registry.get_mut(&type_id).unwrap()
    }

    pub fn native_names(&self) -> Vec<String> {
        self.native_fns.iter().map(|(n, _)| n.clone()).collect()
    }

    /// Load compiled bytecode into the VM. Converts any Rc<str> based constant
    /// pool strings and heap Values to the handle-based representation.
    pub fn load_bytecode(&mut self, fns: Vec<BytecodeFn>, global_names: Vec<String>) {
        let offset = self.functions.len();
        for (i, f) in fns.into_iter().enumerate() {
            let idx = offset + i;
            self.function_name_map.insert(f.name.clone(), idx);
            self.functions.push(f);
            if i == 0 { self.natives.insert("__main__".into(), idx); }
        }
        self.global_names = global_names;
        self.populate_globals();
    }

    fn populate_globals(&mut self) {
        self.globals.clear();
        for name in &self.global_names {
            let val = if let Some(&idx) = self.natives.get(name.as_str()) {
                if idx < self.native_fns.len() && self.native_fns[idx].0 == *name {
                    Value::NativeFunction(self.native_fns[idx].1.clone())
                } else { Value::Nil }
            } else { Value::Nil };
            self.globals.push(val);
        }
        self.globals.resize(self.global_names.len(), Value::Nil);
    }

    /// Register a native function that can be called from Zenlang scripts.
    ///
    /// The function receives a [`VMContext`] and a slice of argument [`Value`]s,
    /// and must return a `Result<Value>`.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::rc::Rc;
    /// # use zenlang::vm::{VM, VMContext};
    /// # use zenlang::error::Result;
    /// # use zenlang::value::Value;
    /// let mut vm = VM::new();
    /// vm.register_native("increment", Rc::new(|_ctx: &mut VMContext, args: &[Value]| -> Result<Value> {
    ///     match args.get(0) {
    ///         Some(Value::Int(n)) => Ok(Value::Int(n + 1)),
    ///         _ => Ok(Value::Nil),
    ///     }
    /// }));
    /// ```
    pub fn register_native(&mut self, name: &str, f: NativeFn) {
        let idx = self.native_fns.len();
        self.natives.insert(name.to_string(), idx);
        self.native_fns.push((name.to_string(), f));
    }

    pub fn snapshot_globals_by_name(&self) -> HashMap<String, Value> {
        let mut snapshot = HashMap::new();
        for (i, name) in self.global_names.iter().enumerate() {
            if let Some(val) = self.globals.get(i) {
                snapshot.insert(name.clone(), val.clone());
            }
        }
        snapshot
    }

    /// Build a `Value::Struct` from a [`StructBuilder`].
    ///
    /// Registers the struct in the VM's intern table and returns a
    /// `Value::Struct(handle, name)`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use zenlang::value::StructBuilder;
    /// # use zenlang::vm::VM;
    /// let mut vm = VM::new();
    /// let val = vm.make_struct(
    ///     StructBuilder::new("Point")
    ///         .field("x", 10i64)
    ///         .field("y", 20i64)
    /// );
    /// assert_eq!(val.type_name(), "struct");
    /// ```
    pub fn make_struct(&mut self, builder: StructBuilder) -> Value {
        let name = builder.name().to_string();
        let h = self.structs.insert(builder.build());
        Value::Struct(h, name)
    }

    pub fn restore_globals_by_name(&mut self, snapshot: &HashMap<String, Value>) {
        for (i, name) in self.global_names.iter().enumerate() {
            if let Some(val) = snapshot.get(name) {
                if i < self.globals.len() { self.globals[i] = val.clone(); }
                else { self.globals.push(val.clone()); }
            }
        }
    }

    pub fn reload_functions(&mut self, fns: Vec<BytecodeFn>, new_global_names: Vec<String>) -> Result<()> {
        let old_name_to_idx: HashMap<String, usize> = self.functions.iter().enumerate().map(|(i, f)| (f.name.clone(), i)).collect();
        let new_name_to_idx: HashMap<String, usize> = fns.iter().enumerate().map(|(i, f)| (f.name.clone(), i)).collect();

        let mut snapshot = self.snapshot_globals_by_name();
        for val in snapshot.values_mut() {
            self.remap_function_value(val, &old_name_to_idx, &new_name_to_idx);
        }

        self.functions = fns;
        self.global_names = new_global_names;

        self.function_name_map = self.functions.iter().enumerate().map(|(i, f)| (f.name.clone(), i)).collect();
        self.populate_globals();
        self.restore_globals_by_name(&snapshot);
        self.natives.insert("__main__".into(), 0);

        self.stack.clear();
        self.frames.clear();
        Ok(())
    }

    pub fn call_if_exists(&mut self, name: &str) -> Result<Option<Value>> {
        let Some(&idx) = self.function_name_map.get(name) else { return Ok(None); };
        let fn_def = &self.functions[idx];
        let bp = self.stack.len();
        self.frames.push(CallFrame::new(idx, bp));
        let slot_count = fn_def.chunk.locals as usize;
        while self.stack.len() < bp + slot_count { self.stack.push(Value::Nil); }
        self.execute()?;
        Ok(Some(self.stack.pop().unwrap_or(Value::Nil)))
    }

    pub fn add_timer(&mut self, callback: Value, delay: f64, interval: Option<f64>) -> u64 {
        let id = self.timer_id_counter;
        self.timer_id_counter += 1;
        let fire_time = self.time + delay.max(0.0);
        self.timers.push(TimerEntry { id, callback, fire_time, interval });
        id
    }

    pub fn remove_timer(&mut self, id: u64) { self.timers.retain(|t| t.id != id); }

    pub fn add_frame_callback(&mut self, callback: Value) { self.frame_callbacks.push(callback); }

    pub fn remove_frame_callback(&mut self, callback: &Value) {
        self.frame_callbacks.retain(|c| !std::ptr::eq(c, callback));
    }

    fn flush_pending_timers(&mut self) {
        while let Some(t) = self.pending_timers.pop() { self.timers.push(t); }
    }

    pub fn tick(&mut self, dt: f64) -> Result<()> {
        self.time += dt;
        loop {
            let idx = match self.timers.iter().position(|t| self.time >= t.fire_time) { Some(i) => i, None => break };
            let timer = self.timers.remove(idx);
            if matches!(timer.callback, Value::Function(_) | Value::Closure(_)) {
                self.call_value(&timer.callback, &[])?;
                self.flush_pending_timers();
            }
            if let Some(interval) = timer.interval {
                let next = timer.fire_time + interval;
                let fire_time = if next <= self.time { self.time + interval } else { next };
                self.timers.push(TimerEntry { id: timer.id, callback: timer.callback, fire_time, interval: Some(interval) });
            }
        }
        let callbacks = std::mem::take(&mut self.frame_callbacks);
        for cb in &callbacks {
            if matches!(cb, Value::Function(_) | Value::Closure(_)) {
                self.call_value(cb, &[])?;
                self.flush_pending_timers();
            }
        }
        Ok(())
    }

    fn call_value(&mut self, callee: &Value, args: &[Value]) -> Result<Value> {
        match callee {
            Value::Function(idx) => {
                let fn_def = &self.functions[*idx];
                if fn_def.is_generator { return Err(self.runtime_error("cannot call generator via timer")); }
                let frame = CallFrame::new(*idx, 0);
                self.frames.push(frame);
                for arg in args { self.stack.push(arg.clone()); }
                let slot_count = fn_def.chunk.locals as usize;
                while self.stack.len() < slot_count { self.stack.push(Value::Nil); }
                self.execute()?;
                Ok(self.stack.pop().unwrap_or(Value::Nil))
            }
            Value::Closure(h) => {
                let data = self.closures.get(*h);
                let fn_idx = data.fn_idx;
                let fn_def = &self.functions[fn_idx];
                let mut frame = CallFrame::new(fn_idx, 0);
                frame.is_closure = true;
                self.frames.push(frame);
                for uv in &data.upvalues { self.stack.push(uv.clone()); }
                for arg in args { self.stack.push(arg.clone()); }
                let slot_count = fn_def.chunk.locals as usize;
                while self.stack.len() < slot_count { self.stack.push(Value::Nil); }
                self.execute()?;
                Ok(self.stack.pop().unwrap_or(Value::Nil))
            }
            _ => Err(self.runtime_error(format!("cannot call {}", callee.type_name()))),
        }
    }

    pub fn run_main(&mut self) -> Result<Value> {
        let main_idx = match self.natives.get("__main__") { Some(&idx) => idx, None => return Err(self.runtime_error("no main function found")) };
        let fn_def = &self.functions[main_idx];
        self.globals.resize(self.globals.len().max(1), Value::Nil);
        let frame = CallFrame::new(main_idx, 0);
        self.frames.push(frame);
        let local_count = fn_def.chunk.locals as usize;
        while self.stack.len() < local_count { self.stack.push(Value::Nil); }
        self.execute_debug()
    }

    fn runtime_error(&self, msg: impl Into<String>) -> Error {
        let mut stack_trace: Vec<crate::span::SourceLocation> = self.frames.iter().map(|frame| source_loc_from_frame(&self.functions, frame.function_idx, frame.ip)).collect();
        stack_trace.reverse();
        let msg = msg.into();
        let trace_str: String = stack_trace.iter().enumerate().map(|(i, loc)| {
            let fn_name = if i < self.frames.len() { let idx = self.frames[self.frames.len() - 1 - i].function_idx; &self.functions[idx].name } else { "?" };
            format!("  {}: at {} (in {})", i, loc, fn_name)
        }).collect::<Vec<_>>().join("\n");
        Error::Runtime { msg: if stack_trace.is_empty() { msg } else { format!("{}\nstack trace:\n{}", msg, trace_str) }, stack_trace }
    }

    fn chunk(&self) -> &Chunk { let idx = self.frames.last().unwrap().function_idx; &self.functions[idx].chunk }

    fn read_byte(&mut self) -> u8 {
        let ip = { let frame = self.frames.last().unwrap(); frame.ip };
        let b = self.chunk().code[ip];
        self.frames.last_mut().unwrap().ip += 1;
        b
    }

    fn read_u16(&mut self) -> u16 {
        let ip = { let frame = self.frames.last().unwrap(); frame.ip };
        let val = Chunk::read_u16_static(&self.chunk().code, ip);
        self.frames.last_mut().unwrap().ip += 2;
        val
    }

    // ── Structural equality helpers (deep compare for heap values) ──

    pub fn values_equal(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Array(ha), Value::Array(hb)) if ha == hb => true,
            (Value::Array(ha), Value::Array(hb)) => {
                let va = self.arrays.get(*ha);
                let vb = self.arrays.get(*hb);
                va.values.len() == vb.values.len()
                    && va.values.iter().zip(vb.values.iter()).all(|(a, b)| self.values_equal(a, b))
            }
            (Value::Struct(ha, an), Value::Struct(hb, bn)) => {
                an == bn && {
                    let va = self.structs.get(*ha);
                    let vb = self.structs.get(*hb);
                    va.values.len() == vb.values.len()
                        && va.values.iter().zip(vb.values.iter()).all(|(a, b)| self.values_equal(a, b))
                }
            }
            (Value::Enum(ha), Value::Enum(hb)) if ha == hb => true,
            (Value::Enum(ha), Value::Enum(hb)) => {
                let ea = self.enums.get(*ha);
                let eb = self.enums.get(*hb);
                ea.tag == eb.tag
                    && ea.fields.len() == eb.fields.len()
                    && ea.fields.iter().zip(eb.fields.iter()).all(|(a, b)| self.values_equal(a, b))
            }
            (Value::Map(ha), Value::Map(hb)) => ha == hb,
            (Value::Closure(ha), Value::Closure(hb)) => ha == hb,
            (Value::Generator(ha), Value::Generator(hb)) => ha == hb,
            _ => a == b,
        }
    }

    // ── The execute loop ──

    fn execute(&mut self) -> Result<()> {
        self.instruction_count = 0;
        loop {
            if self.debug_check() { self.debug_state.paused = true; return Ok(()); }
            let frame = self.frames.last().unwrap();
            if frame.ip >= self.chunk().code.len() { break; }
            self.instruction_count += 1;
            if self.instruction_limit > 0 && self.instruction_count > self.instruction_limit {
                return Err(self.runtime_error(format!("script timeout: executed {} instructions (limit: {})", self.instruction_count, self.instruction_limit)));
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
                    self.stack.push(self.stack[bp + idx].clone());
                }
                Opcode::StoreLocal(_) => {
                    let idx = self.read_u16() as usize;
                    let bp = self.frames.last().unwrap().bp;
                    self.stack[bp + idx] = self.stack.pop().unwrap();
                }
                Opcode::LoadGlobal(_) => {
                    let idx = self.read_u16() as usize;
                    if idx >= self.globals.len() { self.globals.resize(idx + 1, Value::Nil); }
                    self.stack.push(self.globals[idx].clone());
                }
                Opcode::StoreGlobal(_) => {
                    let idx = self.read_u16() as usize;
                    if idx >= self.globals.len() { self.globals.resize(idx + 1, Value::Nil); }
                    self.globals[idx] = self.stack.pop().unwrap();
                }
                Opcode::Pop => { self.stack.pop(); }
                Opcode::Dup => { self.stack.push(self.stack.last().unwrap().clone()); }
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
                            let mut result = as_.to_string(); result.push_str(bs);
                            self.stack.push(Value::Str(result.into()));
                        }
                        _ => return Err(self.runtime_error(format!("cannot add {} and {}", a.type_name(), b.type_name()))),
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
                        _ => return Err(self.runtime_error(format!("cannot subtract {} and {}", a.type_name(), b.type_name()))),
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
                        _ => return Err(self.runtime_error(format!("cannot multiply {} and {}", a.type_name(), b.type_name()))),
                    }
                }
                Opcode::Div => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => {
                            if *bi == 0 { return Err(self.runtime_error("division by zero")); }
                            self.stack.push(Value::Int(ai / bi));
                        }
                        (Value::Float(af), Value::Float(bf)) => self.stack.push(Value::Float(af / bf)),
                        (Value::Int(ai), Value::Float(bf)) => self.stack.push(Value::Float(*ai as f64 / bf)),
                        (Value::Float(af), Value::Int(bi)) => {
                            if *bi == 0 { return Err(self.runtime_error("division by zero")); }
                            self.stack.push(Value::Float(af / *bi as f64));
                        }
                        _ => return Err(self.runtime_error(format!("cannot divide {} and {}", a.type_name(), b.type_name()))),
                    }
                }
                Opcode::Mod => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => {
                            if *bi == 0 { return Err(self.runtime_error("modulo by zero")); }
                            self.stack.push(Value::Int(ai % bi));
                        }
                        _ => return Err(self.runtime_error(format!("cannot mod {} and {}", a.type_name(), b.type_name()))),
                    }
                }
                Opcode::Neg => {
                    let a = self.stack.pop().unwrap();
                    match a {
                        Value::Int(n) => self.stack.push(Value::Int(-n)),
                        Value::Float(n) => self.stack.push(Value::Float(-n)),
                        _ => return Err(self.runtime_error(format!("cannot negate {}", a.type_name()))),
                    }
                }
                Opcode::Not => { let a = self.stack.pop().unwrap(); self.stack.push(Value::Bool(!a.is_truthy())); }
                Opcode::Eq => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(self.values_equal(&a, &b)));
                }
                Opcode::Ne => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(!self.values_equal(&a, &b)));
                }
                Opcode::Lt => {
                    let b = self.stack.pop().unwrap(); let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(compare_lt(&a, &b)));
                }
                Opcode::Le => {
                    let b = self.stack.pop().unwrap(); let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(!compare_lt(&b, &a)));
                }
                Opcode::Gt => {
                    let b = self.stack.pop().unwrap(); let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(compare_lt(&b, &a)));
                }
                Opcode::Ge => {
                    let b = self.stack.pop().unwrap(); let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Bool(!compare_lt(&a, &b)));
                }
                Opcode::Jump(_) => { let target = self.read_u16() as usize; self.frames.last_mut().unwrap().ip = target; }
                Opcode::JumpIfFalse(_) => {
                    let target = self.read_u16() as usize;
                    let cond = self.stack.pop().unwrap();
                    if !cond.is_truthy() { self.frames.last_mut().unwrap().ip = target; }
                }
                Opcode::Loop(_) => { let target = self.read_u16() as usize; self.frames.last_mut().unwrap().ip = target; }
                Opcode::Call(_) => {
                    let arg_count = self.read_u16() as usize;
                    let args_start = self.stack.len() - arg_count;
                    let callee = &self.stack[args_start - 1].clone();
                    match callee {
                        Value::Function(idx) => {
                            let fn_def = &self.functions[*idx];
                            if fn_def.is_generator {
                                let g_handle = self.generators.insert(GeneratorState {
                                    function_idx: *idx, ip: 0, first_call: true, exhausted: false, locals: Vec::new(),
                                });
                                self.stack.truncate(args_start - 1);
                                self.stack.push(Value::Generator(g_handle));
                            } else {
                                let bp = args_start;
                                let frame = CallFrame::new(*idx, bp);
                                self.frames.push(frame);
                                let slot_count = fn_def.chunk.locals as usize;
                                while self.stack.len() < bp + slot_count { self.stack.push(Value::Nil); }
                            }
                        }
                        Value::Closure(h) => {
                            let data = self.closures.get(*h);
                            let fn_idx = data.fn_idx;
                            let up_count = data.upvalues.len();
                            let args: Vec<Value> = self.stack.drain(args_start..).collect();
                            self.stack.pop();
                            for uv in &data.upvalues { self.stack.push(uv.clone()); }
                            for arg in &args { self.stack.push(arg.clone()); }
                            let bp = self.stack.len() - up_count - args.len();
                            let mut frame = CallFrame::new(fn_idx, bp);
                            frame.is_closure = true;
                            self.frames.push(frame);
                            let fn_def = &self.functions[fn_idx];
                            let slot_count = fn_def.chunk.locals as usize;
                            while self.stack.len() < bp + slot_count { self.stack.push(Value::Nil); }
                        }
                        Value::NativeFunction(f) => {
                            let args: Vec<Value> = self.stack.drain(args_start..).collect();
                            self.stack.pop();
                            let mut ctx = VMContext { registry: self.foreign_registry.clone(), raw_vm: self as *mut VM };
                            let result = f(&mut ctx, &args)?;
                            self.stack.push(result);
                        }
                        _ => return Err(self.runtime_error(format!("cannot call {}", callee.type_name()))),
                    }
                }
                Opcode::CallMethod(_, _) => {
                    let method_idx = self.read_u16() as usize;
                    let arg_count = self.read_u16() as usize;
                    let args_start = self.stack.len() - arg_count;
                    let obj = &self.stack[args_start - 1].clone();
                    match obj {
                        Value::Foreign(h) => {
                            let method_name = self.chunk().method_names.get(method_idx).cloned().unwrap_or_default();
                            let fo = self.foreigns.get(*h);
                            let type_id = fo.type_id;
                            let type_name = fo.type_name;
                            let args: Vec<Value> = self.stack.drain(args_start - 1..).collect();
                            let mut ctx = VMContext { registry: self.foreign_registry.clone(), raw_vm: self as *mut VM };
                            match self.foreign_registry.call_method(&type_id, &method_name, &mut ctx, &args) {
                                Some(Ok(result)) => self.stack.push(result),
                                Some(Err(e)) => return Err(e),
                                None => return Err(self.runtime_error(format!("foreign type '{}' has no method '{}'", type_name, method_name))),
                            }
                        }
                        Value::Function(idx) => {
                            let fn_def = &self.functions[*idx];
                            let bp = args_start;
                            self.frames.push(CallFrame::new(*idx, bp));
                            let slot_count = fn_def.chunk.locals as usize;
                            while self.stack.len() < bp + slot_count { self.stack.push(Value::Nil); }
                        }
                        Value::Struct(_h, type_name) => {
                            let method_name = self.chunk().method_names.get(method_idx).cloned().unwrap_or_default();
                            let qualified = format!("{}::{}", type_name, method_name);
                            match self.function_name_map.get(&qualified).copied() {
                                Some(fn_idx) => {
                                    let fn_def = &self.functions[fn_idx];
                                    let bp = args_start - 1;
                                    let mut frame = CallFrame::new(fn_idx, bp);
                                    frame.is_method = true;
                                    self.frames.push(frame);
                                    let slot_count = fn_def.chunk.locals as usize;
                                    while self.stack.len() < bp + slot_count { self.stack.push(Value::Nil); }
                                }
                                None => return Err(self.runtime_error(format!("type '{}' has no method '{}'", type_name, method_name))),
                            }
                        }
                        _ => return Err(self.runtime_error(format!("cannot call method on {}", obj.type_name()))),
                    }
                }
                Opcode::Return => {
                    let result = self.stack.pop().unwrap_or(Value::Nil);
                    let frame = self.frames.pop().unwrap();
                    if let Some((gen_h, _)) = &self.active_generator {
                        let gen_state = self.generators.get_mut(*gen_h);
                        gen_state.exhausted = true;
                    }
                    let gen_active = self.active_generator.is_some();
                    if gen_active {
                        self.stack.truncate(frame.bp);
                        if self.frames.len() <= 0 { // generator_base_frame_count is implicit
                            self.stack.push(result);
                            break;
                        }
                    } else if frame.is_method || frame.is_closure {
                        self.stack.truncate(frame.bp);
                    } else if frame.bp > 0 {
                        self.stack.truncate(frame.bp - 1);
                    } else {
                        self.stack.truncate(frame.bp);
                    }
                    if self.frames.is_empty() {
                        self.stack.push(result);
                        self.flush_pending_timers();
                        return Ok(());
                    }
                    if Some(self.frames.len()) == self.return_to_depth {
                        self.stack.push(result);
                        self.return_to_depth = None;
                        return Ok(());
                    }
                    self.stack.push(result);
                }
                Opcode::MakeStruct(_, _) => {
                    let type_name_idx = self.read_u16() as usize;
                    let field_count = self.read_u16() as usize;
                    let type_name = match self.chunk().constants.get(type_name_idx) {
                        Some(Value::Str(s)) => s.to_string(),
                        _ => String::new(),
                    };
                    let mut values = Vec::with_capacity(field_count);
                    let mut field_names_vec = Vec::with_capacity(field_count);
                    for _ in 0..field_count {
                        let val = self.stack.pop().unwrap();
                        let name = self.stack.pop().unwrap();
                        if let Value::Str(s) = name { field_names_vec.push(s.to_string()); values.push(val); }
                    }
                    field_names_vec.reverse(); values.reverse();
                    let h = self.structs.insert(StructData { values, field_names: field_names_vec });
                    self.stack.push(Value::Struct(h, type_name));
                }
                Opcode::MakeArray(_) => {
                    let count = self.read_u16() as usize;
                    let mut elems = Vec::with_capacity(count);
                    for _ in 0..count { elems.push(self.stack.pop().unwrap()); }
                    elems.reverse();
                    let h = self.arrays.insert(ArrayData { values: elems });
                    self.stack.push(Value::Array(h));
                }
                Opcode::MakeRange => {
                    let inclusive = self.stack.pop().unwrap();
                    let end = self.stack.pop().unwrap();
                    let start = self.stack.pop().unwrap();
                    match (&start, &end, &inclusive) {
                        (Value::Int(s), Value::Int(e), Value::Bool(inc)) => self.stack.push(Value::Range(*s, *e, *inc)),
                        _ => return Err(self.runtime_error(format!("range requires integer bounds, got {} and {}", start.type_name(), end.type_name()))),
                    }
                }
                Opcode::MakeEnum(_, _) => {
                    let tag = self.read_u16();
                    let data_count = self.read_u16() as usize;
                    let mut data = Vec::new();
                    for _ in 0..data_count { data.push(self.stack.pop().unwrap()); }
                    data.reverse();
                    let h = self.enums.insert(EnumData { tag, fields: data });
                    self.stack.push(Value::Enum(h));
                }
                Opcode::LoadField(_) => {
                    let field_idx = self.read_u16() as usize;
                    let field_name = self.chunk().field_names.get(field_idx).cloned().unwrap_or_default();
                    let obj = self.stack.pop().unwrap();
                    match &obj {
                        Value::Struct(h, _) => {
                            let d = self.structs.get(*h);
                            let val = if field_idx < d.values.len() { d.values[field_idx].clone() }
                            else { d.get_field(&field_name).cloned().unwrap_or(Value::Nil) };
                            self.stack.push(val);
                        }
                        Value::Foreign(h) => {
                            let fo = self.foreigns.get(*h);
                            let type_id = fo.type_id;
                            match self.foreign_registry.get_field(self, &type_id, &field_name, &obj) {
                                Some(Ok(val)) => self.stack.push(val),
                                Some(Err(e)) => return Err(e),
                                None => return Err(self.runtime_error(format!("foreign type '{}' has no field '{}'", fo.type_name, field_name))),
                            }
                        }
                        _ => return Err(self.runtime_error(format!("cannot access field on {}", obj.type_name()))),
                    }
                }
                Opcode::StoreField(_) => {
                    let field_idx = self.read_u16() as usize;
                    let field_name = self.chunk().field_names.get(field_idx).cloned().unwrap_or_default();
                    let val = self.stack.pop().unwrap();
                    let mut obj = self.stack.pop().unwrap();
                    let result_val = val.clone();
                    match obj {
                        Value::Struct(h, _) => {
                            let d = self.structs.get_mut(h);
                            if field_idx < d.values.len() { d.values[field_idx] = val; }
                            else if let Some(field) = d.get_field_mut(&field_name) { *field = val; }
                            self.stack.push(result_val);
                        }
                        Value::Foreign(h) => {
                            let reg = self.foreign_registry.clone();
                            let type_id = self.foreigns.get(h).type_id;
                            match reg.set_field(self, &type_id, &field_name, &mut obj, val) {
                                Some(Ok(())) => self.stack.push(result_val),
                                Some(Err(e)) => return Err(e),
                                None => return Err(self.runtime_error(format!("foreign type has no field '{}'", field_name))),
                            }
                        }
                        _ => return Err(self.runtime_error(format!("cannot set field on {}", obj.type_name()))),
                    }
                }
                Opcode::LoadIndex => {
                    let index = self.stack.pop().unwrap();
                    let obj = self.stack.pop().unwrap();
                    match (&obj, &index) {
                        (Value::Array(h), Value::Int(i)) => {
                            let idx = *i as usize;
                            let arr = self.arrays.get(*h);
                            let val = arr.values.get(idx).cloned().unwrap_or(Value::Nil);
                            self.stack.push(val);
                        }
                        (Value::Str(s), Value::Int(i)) => {
                            let idx = *i as usize;
                            let c = s.chars().nth(idx).map(|c| c.to_string()).unwrap_or_default();
                            self.stack.push(Value::Str(c.into()));
                        }
                        (Value::Range(start, end, inclusive), Value::Int(i)) => {
                            let val = start + i;
                            if (!*inclusive && val >= *end) || (*inclusive && val > *end) || val < *start.min(end) {
                                return Err(self.runtime_error("index out of range bounds"));
                            }
                            self.stack.push(Value::Int(val));
                        }
                        _ => return Err(self.runtime_error(format!("cannot index {} with {}", obj.type_name(), index.type_name()))),
                    }
                }
                Opcode::StoreIndex => {
                    let val = self.stack.pop().unwrap();
                    let index = self.stack.pop().unwrap();
                    let obj = self.stack.pop().unwrap();
                    let result_val = val.clone();
                    match (&obj, &index) {
                        (Value::Array(h), Value::Int(i)) => {
                            let idx = *i as usize;
                            let arr = self.arrays.get_mut(*h);
                            if idx < arr.values.len() { arr.values[idx] = val; }
                            self.stack.push(result_val);
                        }
                        _ => return Err(self.runtime_error(format!("cannot index {} with {}", obj.type_name(), index.type_name()))),
                    }
                }
                Opcode::Len => {
                    let val = self.stack.pop().unwrap();
                    match val {
                        Value::Str(s) => self.stack.push(Value::Int(s.len() as i64)),
                        Value::Array(h) => self.stack.push(Value::Int(self.arrays.get(h).values.len() as i64)),
                        Value::Range(start, end, inclusive) => {
                            let len = if inclusive { end - start + 1 } else { end - start };
                            self.stack.push(Value::Int(len.max(0)));
                        }
                        Value::Map(h) => self.stack.push(Value::Int(self.maps.get(h).entries.len() as i64)),
                        _ => return Err(self.runtime_error(format!("cannot get length of {}", val.type_name()))),
                    }
                }
                Opcode::NewClosure(_, _) => {
                    let fn_idx = self.read_u16() as usize;
                    let up_count = self.read_u16() as usize;
                    let mut upvalues = Vec::with_capacity(up_count);
                    for _ in 0..up_count { upvalues.push(self.stack.pop().unwrap()); }
                    upvalues.reverse();
                    let h = self.closures.insert(ClosureData { fn_idx, upvalues });
                    self.stack.push(Value::Closure(h));
                }
                Opcode::BitAnd => {
                    let b = self.stack.pop().unwrap(); let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai & bi)),
                        _ => return Err(self.runtime_error(format!("cannot bitwise-and {} and {}", a.type_name(), b.type_name()))),
                    }
                }
                Opcode::BitOr => {
                    let b = self.stack.pop().unwrap(); let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai | bi)),
                        _ => return Err(self.runtime_error(format!("cannot bitwise-or {} and {}", a.type_name(), b.type_name()))),
                    }
                }
                Opcode::BitXor => {
                    let b = self.stack.pop().unwrap(); let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai ^ bi)),
                        _ => return Err(self.runtime_error(format!("cannot bitwise-xor {} and {}", a.type_name(), b.type_name()))),
                    }
                }
                Opcode::Shl => {
                    let b = self.stack.pop().unwrap(); let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai << bi)),
                        _ => return Err(self.runtime_error(format!("cannot shift left {} and {}", a.type_name(), b.type_name()))),
                    }
                }
                Opcode::Shr => {
                    let b = self.stack.pop().unwrap(); let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai >> bi)),
                        _ => return Err(self.runtime_error(format!("cannot shift right {} and {}", a.type_name(), b.type_name()))),
                    }
                }
                Opcode::BitNot => {
                    let a = self.stack.pop().unwrap();
                    match a { Value::Int(n) => self.stack.push(Value::Int(!n)), _ => return Err(self.runtime_error(format!("cannot bitwise-not {}", a.type_name()))), }
                }
                Opcode::LoadEnumTag => {
                    let val = self.stack.pop().unwrap();
                    match val {
                        Value::Enum(h) => self.stack.push(Value::Int(self.enums.get(h).tag as i64)),
                        _ => return Err(self.runtime_error("LoadEnumTag on non-enum value")),
                    }
                }
                Opcode::LoadEnumField(_) => {
                    let idx = self.read_u16() as usize;
                    let val = self.stack.pop().unwrap();
                    match val {
                        Value::Enum(h) => {
                            let field = self.enums.get(h).fields.get(idx).cloned().unwrap_or(Value::Nil);
                            self.stack.push(field);
                        }
                        _ => return Err(self.runtime_error("LoadEnumField on non-enum value")),
                    }
                }
                Opcode::Yield => {
                    let val = self.stack.pop().unwrap();
                    if let Some((gen_h, _)) = self.active_generator.as_ref().copied() {
                        let saved_frame = self.frames.last().unwrap();
                        let fn_idx = saved_frame.function_idx;
                        let bp = saved_frame.bp;
                        let ip = saved_frame.ip;
                        let fn_def = &self.functions[fn_idx];
                        let local_count = fn_def.chunk.locals as usize;
                        let state = self.generators.get_mut(gen_h);
                        state.ip = ip;
                        state.locals = self.stack[bp..bp + local_count].to_vec();
                        state.first_call = false;
                        self.frames.pop();
                        self.stack.truncate(bp);
                        self.stack.push(val);
                        break;
                    } else {
                        return Err(self.runtime_error("yield outside generator function"));
                    }
                }
                Opcode::Halt => break,
            }
        }
        self.flush_pending_timers();
        Ok(())
    }

    /// Resume a generator. Returns the yielded value or `None` if exhausted.
    pub fn resume_generator(&mut self, gen_handle: Handle) -> Result<Option<Value>> {
        let state = self.generators.get(gen_handle);
        if state.exhausted { return Ok(None); }
        let fn_idx = state.function_idx;
        let first_call = state.first_call;
        let saved_locals = state.locals.clone();
        let saved_ip = state.ip;
        let _ = state;

        let fn_def = &self.functions[fn_idx];
        let bp = self.stack.len();
        self.frames.push(CallFrame::new(fn_idx, bp));

        if first_call {
            let local_count = fn_def.chunk.locals as usize;
            while self.stack.len() < bp + local_count { self.stack.push(Value::Nil); }
        } else {
            self.stack.extend(saved_locals);
        }
        self.frames.last_mut().unwrap().ip = saved_ip;

        let saved_frame_count = self.frames.len();
        self.active_generator = Some((gen_handle, saved_frame_count));
        let result_val = self.execute_debug();
        self.active_generator = None;

        match result_val {
            Ok(val) => {
                if self.debug_state.paused { return Ok(None); }
                let state = self.generators.get(gen_handle);
                if state.exhausted { Ok(None) } else { Ok(Some(val)) }
            }
            Err(e) => { self.generators.get_mut(gen_handle).exhausted = true; Err(e) }
        }
    }
}

impl Default for VM {
    fn default() -> Self { Self::new() }
}

// ── Helper functions ──

fn compare_lt(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(ai), Value::Int(bi)) => ai < bi,
        (Value::Float(af), Value::Float(bf)) => af < bf,
        (Value::Int(ai), Value::Float(bf)) => (*ai as f64) < *bf,
        (Value::Float(af), Value::Int(bi)) => *af < (*bi as f64),
        _ => false,
    }
}

/// Recursively remap `Value::Function(old_idx)` references using name-based
/// lookup, so that global values containing function references survive hot reload.
impl VM {
    fn remap_function_value(
        &mut self,
        val: &mut Value,
        old_name_to_idx: &HashMap<String, usize>,
        new_name_to_idx: &HashMap<String, usize>,
    ) {
        let remap_idx = |idx: &mut usize| {
            let name = old_name_to_idx.iter().find(|&(_, &v)| v == *idx).map(|(k, _)| k.clone());
            if let Some(name) = name {
                if let Some(&new_idx) = new_name_to_idx.get(&name) {
                    *idx = new_idx;
                }
            }
        };
        match val {
            Value::Function(idx) => remap_idx(idx),
            Value::Closure(h) => {
                let fn_idx = self.closures.get(*h).fn_idx;
                let mut new_fn_idx = fn_idx;
                remap_idx(&mut new_fn_idx);
                let mut upvalues = std::mem::take(&mut self.closures.get_mut(*h).upvalues);
                self.closures.get_mut(*h).fn_idx = new_fn_idx;
                for uv in &mut upvalues {
                    self.remap_function_value(uv, old_name_to_idx, new_name_to_idx);
                }
                self.closures.get_mut(*h).upvalues = upvalues;
            }
            Value::Array(h) => {
                let mut values = std::mem::take(&mut self.arrays.get_mut(*h).values);
                for v in &mut values {
                    self.remap_function_value(v, old_name_to_idx, new_name_to_idx);
                }
                self.arrays.get_mut(*h).values = values;
            }
            Value::Struct(h, _) => {
                let mut values = std::mem::take(&mut self.structs.get_mut(*h).values);
                for v in &mut values {
                    self.remap_function_value(v, old_name_to_idx, new_name_to_idx);
                }
                self.structs.get_mut(*h).values = values;
            }
            Value::Enum(h) => {
                let mut fields = std::mem::take(&mut self.enums.get_mut(*h).fields);
                for v in &mut fields {
                    self.remap_function_value(v, old_name_to_idx, new_name_to_idx);
                }
                self.enums.get_mut(*h).fields = fields;
            }
            Value::Map(h) => {
                let mut entries = std::mem::take(&mut self.maps.get_mut(*h).entries);
                for v in entries.values_mut() {
                    self.remap_function_value(v, old_name_to_idx, new_name_to_idx);
                }
                self.maps.get_mut(*h).entries = entries;
            }
            _ => {}
        }
    }
}

// ── Tests ──

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::compiler;
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

    fn try_run(source: &str) -> crate::error::Result<Value> {
        let tokens = Lexer::new(source).tokenize()?;
        let parser = Parser::new(source, &tokens);
        let mut program = parser.parse()?;
        let native_names = crate::stdlib::native_names();
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names)?;
        let types = crate::typeck::check(&program, &mut symbols)?;
        let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source)?;
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        vm.run_main()
    }

    pub fn run_program(source: &str) -> crate::error::Result<Value> {
        let tokens = Lexer::new(source).tokenize()?;
        let parser = Parser::new(source, &tokens);
        let mut program = parser.parse()?;
        let native_names = crate::stdlib::native_names();
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names)?;
        let types = crate::typeck::check(&program, &mut symbols)?;
        let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source)?;
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        vm.run_main()
    }

    #[test] fn test_nil() { assert_eq!(run(""), Value::Nil); }
    #[test] fn test_int_literal() { assert_eq!(run("42"), Value::Int(42)); }
    #[test] fn test_float_literal() { assert_eq!(run("3.14"), Value::Float(3.14)); }
    #[test] fn test_bool_literal() { assert_eq!(run("true"), Value::Bool(true)); }
    #[test] fn test_string_literal() { assert_eq!(run("\"hello\""), Value::Str("hello".into())); }
    #[test] fn test_string_interpolation_basic() { assert_eq!(run("let name = \"world\"; \"hello {name}\""), Value::Str("hello world".into())); }
    #[test] fn test_string_interpolation_int() { assert_eq!(run("\"the answer is {42}\""), Value::Str("the answer is 42".into())); }
    #[test] fn test_string_interpolation_multiple() { assert_eq!(run("let a = 1; let b = 2; \"{a} + {b} = {a + b}\""), Value::Str("1 + 2 = 3".into())); }
    #[test] fn test_string_interpolation_no_interp() { assert_eq!(run("\"hello world\""), Value::Str("hello world".into())); }
    #[test] fn test_string_interpolation_escaped_brace() { assert_eq!(run("\"hello {{name}}\""), Value::Str("hello {name}".into())); }
    #[test] fn test_add_ints() { assert_eq!(run("1 + 2"), Value::Int(3)); }
    #[test] fn test_sub_ints() { assert_eq!(run("10 - 3"), Value::Int(7)); }
    #[test] fn test_mul_ints() { assert_eq!(run("3 * 4"), Value::Int(12)); }
    #[test] fn test_div_ints() { assert_eq!(run("10 / 3"), Value::Int(3)); }
    #[test] fn test_let_binding() { assert_eq!(run("let x = 42; x"), Value::Int(42)); }
    #[test] fn test_if_true() { assert_eq!(run("if true { 1 } else { 2 }"), Value::Int(1)); }
    #[test] fn test_if_false() { assert_eq!(run("if false { 1 } else { 2 }"), Value::Int(2)); }
    #[test] fn test_while_loop() { assert_eq!(run("let i = 0; while i < 5 { i = i + 1 }; i"), Value::Int(5)); }
    #[test] fn test_comparison() { assert_eq!(run("3 < 5"), Value::Bool(true)); assert_eq!(run("5 < 3"), Value::Bool(false)); }
    #[test] fn test_equality() { assert_eq!(run("3 == 3"), Value::Bool(true)); assert_eq!(run("3 == 4"), Value::Bool(false)); }
    #[test] fn test_block_expr() { assert_eq!(run("{ let x = 10; x + 5 }"), Value::Int(15)); }
    #[test] fn test_negation() { assert_eq!(run("-5"), Value::Int(-5)); }
    #[test] fn test_boolean_not() { assert_eq!(run("!true"), Value::Bool(false)); }
    #[test] fn test_for_loop() { assert_eq!(run("let s = 0; for i in 0..3 { s = s + i }; s"), Value::Int(3)); }
    #[test] fn test_match_int() { assert_eq!(run("match 2 { 1 => 10, 2 => 20, 3 => 30 }"), Value::Int(20)); }
    #[test] fn test_match_wildcard() { assert_eq!(run("match 99 { 1 => 10, _ => 99 }"), Value::Int(99)); }
    #[test] fn test_function_call() { assert_eq!(run("fn add(a: int, b: int) -> int { a + b } add(3, 4)"), Value::Int(7)); }
    #[test] fn test_function_return() { assert_eq!(run("fn make(n: int) -> int { return n * 2 } make(5)"), Value::Int(10)); }
    #[test] fn test_nested_scopes() { assert_eq!(run("let x = 1; { let x = 2; x } + x"), Value::Int(3)); }
    #[test] fn test_closure() { assert_eq!(run("let f = |x| x + 1; f(41)"), Value::Int(42)); }
    #[test] fn test_closures_share_upvalue() { assert_eq!(run("let x = 0; let f = || { x = x + 1; x }; f(); f()"), Value::Int(2)); }
    #[test] fn test_trait_impl_pipeline() {
        let source = r#"struct Circle { radius: f64 } trait Shape { fn area(&self) -> f64; } impl Shape for Circle { fn area(&self) -> f64 { self.radius * self.radius * 3.14159 } } let c = Circle { radius: 2.0 }; c.area()"#;
        let result = run(source);
        assert!((result.as_float().unwrap() - 12.56636).abs() < 0.001);
    }
    #[test] fn test_array() { assert_eq!(run("let a = [1, 2, 3]; a[1]"), Value::Int(2)); }
    #[test] fn test_struct() { assert_eq!(run("struct P { x: int, y: int } let p = P { x: 10, y: 20 }; p.x + p.y"), Value::Int(30)); }
    #[test] fn test_enum_match() {
        assert_eq!(run("enum O { Some(int), None } let v = O::Some(42); match v { O::Some(n) => n, O::None => 0 }"), Value::Int(42));
    }
    #[test] fn test_range_for() { assert_eq!(run("let s = 0; for i in 0..=3 { s = s + i }; s"), Value::Int(6)); }
    #[test] fn test_map_operations() { assert_eq!(run("let m = map_new(); map_set(m, \"k\", 42); map_get(m, \"k\")"), run("Option::Some(42)")); }

    #[test]
    fn test_call_value_calls_script_function_from_native() {
        let source = r#"
            fn double(x: int) -> int { x * 2 }
            fn main() -> int {
                call_with_42(double)
            }
        "#;
        let tokens = Lexer::new(source).tokenize().unwrap();
        let parser = Parser::new(source, &tokens);
        let mut program = parser.parse().unwrap();
        let mut native_names = crate::stdlib::native_names();
        native_names.push("call_with_42".into());
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
        let mut vm = VM::new();
        vm.register_native("call_with_42", Rc::new(|ctx: &mut VMContext, args: &[Value]| -> Result<Value> {
            let closure = &args[0];
            ctx.call_value(closure, &[Value::Int(42)])
        }));
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        let result = vm.run_main().unwrap();
        assert_eq!(result, Value::Int(84));
    }

    #[test]
    fn test_call_value_calls_closure_from_native() {
        let source = r#"
            fn main() -> int {
                call_with_42(|x| x * 3)
            }
        "#;
        let tokens = Lexer::new(source).tokenize().unwrap();
        let parser = Parser::new(source, &tokens);
        let mut program = parser.parse().unwrap();
        let mut native_names = crate::stdlib::native_names();
        native_names.push("call_with_42".into());
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
        let mut vm = VM::new();
        vm.register_native("call_with_42", Rc::new(|ctx: &mut VMContext, args: &[Value]| -> Result<Value> {
            let closure = &args[0];
            ctx.call_value(closure, &[Value::Int(42)])
        }));
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        let result = vm.run_main().unwrap();
        assert_eq!(result, Value::Int(126));
    }

    #[test]
    fn test_call_value_multiple_args() {
        let source = r#"
            fn add(a: int, b: int) -> int { a + b }
            fn main() -> int {
                call_with_2(add)
            }
        "#;
        let tokens = Lexer::new(source).tokenize().unwrap();
        let parser = Parser::new(source, &tokens);
        let mut program = parser.parse().unwrap();
        let mut native_names = crate::stdlib::native_names();
        native_names.push("call_with_2".into());
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
        let mut vm = VM::new();
        vm.register_native("call_with_2", Rc::new(|ctx: &mut VMContext, args: &[Value]| -> Result<Value> {
            let closure = &args[0];
            ctx.call_value(closure, &[Value::Int(100), Value::Int(23)])
        }));
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        let result = vm.run_main().unwrap();
        assert_eq!(result, Value::Int(123));
    }

    // ── JSON serialisation tests ──

    #[test]
    fn test_json_bool() {
        assert_eq!(run(r#"to_json(true)"#), run(r#""true""#));
        assert_eq!(run(r#"to_json(false)"#), run(r#""false""#));
    }

    #[test]
    fn test_json_int() {
        assert_eq!(run(r#"to_json(42)"#), run(r#""42""#));
    }

    #[test]
    fn test_json_float() {
        assert_eq!(run(r#"to_json(3.14)"#), run(r#""3.14""#));
    }

    #[test]
    fn test_json_string() {
        assert_eq!(run(r#"to_json("hello")"#), run(r#""\"hello\"""#));
    }

    #[test]
    fn test_json_array() {
        let result = run(r#"to_json([1, 2, 3])"#);
        assert_eq!(result.as_str(), Some(r#"[1,2,3]"#.into()));
    }

    #[test]
    fn test_json_nested_array() {
        let result = run(r#"to_json([[1, 2], [3, 4]])"#);
        assert_eq!(result.as_str(), Some(r#"[[1,2],[3,4]]"#.into()));
    }

    #[test]
    fn test_json_roundtrip_int() {
        assert_eq!(run(r#"let s = to_json(42); from_json(s)"#), run("42"));
    }

    #[test]
    fn test_json_roundtrip_bool() {
        assert_eq!(run(r#"let s = to_json(true); from_json(s)"#), run("true"));
    }

    #[test]
    fn test_json_roundtrip_string() {
        assert_eq!(run(r#"let s = to_json("hi"); from_json(s)"#), run(r#""hi""#));
    }

    #[test]
    fn test_json_roundtrip_array() {
        let result = run(r#"to_json(from_json(to_json([1, 2, 3])))"#);
        assert_eq!(result, run(r#""[1,2,3]""#));
    }

    #[test]
    fn test_json_struct() {
        let source = r#"
            struct Point { x: int, y: int }
            let p = Point { x: 10, y: 20 };
            to_json(p)
        "#;
        let result = run(source);
        let s = result.as_str().unwrap();
        assert!(s.contains(r#""__type":"Point""#));
        assert!(s.contains(r#""x":10"#));
        assert!(s.contains(r#""y":20"#));
    }

    #[test]
    fn test_json_enum() {
        let source = r#"
            enum Opt { Some(int), None }
            let v = Opt::Some(42);
            to_json(v)
        "#;
        let result = run(source);
        let s = result.as_str().unwrap();
        assert!(s.contains(r#""__tag":0"#));
        assert!(s.contains(r#""fields":[42]"#));
    }

    #[test]
    fn test_json_map() {
        let source = r#"
            let m = map_new();
            map_set(m, "a", 1);
            map_set(m, "b", 2);
            to_json(m)
        "#;
        let result = run(source);
        let s = result.as_str().unwrap();
        assert!(s.contains(r#""a":1"#));
        assert!(s.contains(r#""b":2"#));
    }

    #[test]
    fn test_json_from_json_creates_array() {
        let source = r#"from_json("[10, 20, 30]")"#;
        let result = run(source);
        assert_eq!(result, run("[10, 20, 30]"));
    }

    #[test]
    fn test_json_closure_becomes_null() {
        let result = run(r#"to_json(|x| x)"#);
        assert_eq!(result, run(r#""null""#));
    }

    #[test]
    fn test_make_struct() {
        let mut vm = VM::new();
        let val = vm.make_struct(
            crate::value::StructBuilder::new("Point")
                .field("x", 10i64)
                .field("y", 20i64)
        );
        let (h, name) = match &val {
            Value::Struct(h, name) => (*h, name.clone()),
            _ => panic!("expected Struct"),
        };
        assert_eq!(name, "Point");
        let sd = vm.structs.get(h);
        assert_eq!(sd.get_field("x"), Some(&Value::Int(10)));
        assert_eq!(sd.get_field("y"), Some(&Value::Int(20)));
    }

    #[test]
    fn test_json_nil() {
        assert_eq!(run(r#"to_json(from_json("null"))"#), run(r#""null""#));
        assert_eq!(run(r#"from_json("null")"#), Value::Nil);
    }

    #[test]
    fn test_json_from_json_invalid() {
        let result = run_program(r#"from_json("not valid json")"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_roundtrip_nested_struct() {
        let source = r#"
            struct Inner { v: int }
            struct Outer { inner: Inner }
            let o = Outer { inner: Inner { v: 42 } };
            let s = to_json(o);
            s
        "#;
        let result = run(source);
        let s = result.as_str().unwrap();
        assert!(s.contains(r#""__type":"Inner""#));
        assert!(s.contains(r#""v":42"#));
    }

    #[test]
    fn test_json_roundtrip_empty_array() {
        assert_eq!(run(r#"to_json([])"#), run(r#""[]""#));
        assert_eq!(run(r#"from_json("[]")"#), run("[]"));
    }

    #[test]
    fn test_stdlib_len() {
        assert_eq!(run(r#"len("hello")"#), Value::Int(5));
    }

    #[test]
    fn test_stdlib_contains() {
        assert_eq!(run(r#"contains("hello world", "world")"#), Value::Bool(true));
        assert_eq!(run(r#"contains("hello world", "xyz")"#), Value::Bool(false));
    }

    #[test]
    fn test_stdlib_trim() {
        assert_eq!(run(r#"trim("  hi  ")"#), run(r#""hi""#));
    }

    #[test]
    fn test_stdlib_to_upper_lower() {
        assert_eq!(run(r#"to_upper("abc")"#), run(r#""ABC""#));
        assert_eq!(run(r#"to_lower("ABC")"#), run(r#""abc""#));
    }

    #[test]
    fn test_stdlib_substring() {
        assert_eq!(run(r#"substring("hello", 1, 3)"#), run(r#""el""#));
    }

    #[test]
    fn test_stdlib_abs() {
        assert_eq!(run(r#"abs(-5)"#), Value::Int(5));
    }

    #[test]
    fn test_stdlib_min_max() {
        assert_eq!(run(r#"min(3, 7)"#), Value::Int(3));
        assert_eq!(run(r#"max(3, 7)"#), Value::Int(7));
    }

    #[test]
    fn test_stdlib_sqrt() {
        let result = run(r#"sqrt(9.0)"#);
        assert!((result.as_float().unwrap() - 3.0).abs() < 1e-10);
    }

    // ── Phase 2: Structural typing + opaque type ──────────────────────

    #[test]
    fn test_structural_compatibility_same_fields() {
        // Two structs with identical fields should be structurally compatible
        let source = r#"
            struct A { x: int, y: int }
            struct B { x: int, y: int }
            fn accept(a: A) -> int { a.x }
            let b = B { x: 10, y: 20 };
            accept(b)
        "#;
        assert_eq!(run(source), Value::Int(10));
    }

    #[test]
    fn test_structural_compatibility_extra_fields() {
        // B has extra fields — width subtyping allows passing B where A expected
        let source = r#"
            struct A { x: int }
            struct B { x: int, y: int }
            fn accept(a: A) -> int { a.x }
            let b = B { x: 10, y: 20 };
            accept(b)
        "#;
        assert_eq!(run(source), Value::Int(10));
    }

    #[test]
    fn test_structural_compatibility_missing_fields_fails() {
        // B missing a field that A requires — should fail
        let source = r#"
            struct A { x: int, y: int }
            struct B { x: int }
            fn accept(a: A) -> int { a.x + a.y }
            let b = B { x: 10 };
            accept(b)
        "#;
        let result = try_run(source);
        assert!(result.is_err(), "should fail: B missing field 'y' required by A");
    }

    #[test]
    fn test_structural_incompatible_field_types() {
        // Same field name but different types — should fail
        let source = r#"
            struct A { x: int }
            struct B { x: str }
            fn accept(a: A) -> int { a.x }
            let b = B { x: "hi" };
            accept(b)
        "#;
        let result = try_run(source);
        assert!(result.is_err(), "should fail: field 'x' has incompatible types");
    }

    #[test]
    fn test_opaque_type_isolation() {
        // Opaque type should NOT be compatible with its base
        let source = r#"
            struct Point { x: int, y: int }
            opaque type Distance = Point
            fn accept(d: Distance) -> int { d.x }
            let p = Point { x: 10, y: 20 };
            accept(p)
        "#;
        let result = try_run(source);
        assert!(result.is_err(), "should fail: opaque type Distance is not compatible with Point");
    }

    #[test]
    fn test_opaque_type_not_transparent() {
        // Opaque type should not be transparent through aliases
        let source = r#"
            struct Base { value: int }
            opaque type Wrapped = Base
            let w = Wrapped { value: 42 };
            w.value
        "#;
        let result = try_run(source);
        assert!(result.is_err(), "should fail: opaque type fields not accessible");
    }

    #[test]
    fn test_type_alias_transparent() {
        // Regular (non-opaque) type alias should be transparent
        let source = r#"
            struct Point { x: int, y: int }
            type Coordinates = Point;
            let c = Coordinates { x: 5, y: 10 };
            c.x + c.y
        "#;
        assert_eq!(run(source), Value::Int(15));
    }

    #[test]
    fn test_excess_property_check() {
        // Struct literal with unknown field should fail
        let source = r#"
            struct P { x: int }
            let p = P { x: 10, z: 99 };
            p.x
        "#;
        let result = try_run(source);
        assert!(result.is_err(), "should fail: excess property 'z'");
    }
}
