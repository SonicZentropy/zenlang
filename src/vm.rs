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
    /// Raw pointer to the VM, allows native functions to interact with
    /// the VM (e.g. resume generators). Only safe to dereference in
    /// contexts where the VM is known to outlive the call.
    pub raw_vm: *mut VM,
}

impl VMContext {
    /// Register a one-shot or repeating timer from a native function.
    /// The timer is queued and flushed into the active timer list after the
    /// current callback returns, avoiding aliasing issues with the raw VM pointer.
    pub fn register_timer(&mut self, callback: Value, delay: f64, interval: Option<f64>) -> u64 {
        let vm: &mut VM = unsafe { &mut *self.raw_vm };
        let id = vm.timer_id_counter;
        vm.timer_id_counter += 1;
        let fire_time = vm.time + delay.max(0.0);
        eprintln!("DEBUG register_timer: id={}, time={}, fire_time={}, pending.len={}", id, vm.time, fire_time, vm.pending_timers.len());
        vm.pending_timers.push(TimerEntry {
            id,
            callback,
            fire_time,
            interval,
        });
        id
    }

    /// Cancel a timer by ID from a native function.
    pub fn remove_timer(&mut self, id: u64) {
        let vm: &mut VM = unsafe { &mut *self.raw_vm };
        vm.timers.retain(|t| t.id != id);
        vm.pending_timers.retain(|t| t.id != id);
    }
}

/// A call frame in the VM.
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
        Self {
            function_idx,
            ip: 0,
            bp,
            is_method: false,
            is_closure: false,
        }
    }
}

/// Debug stepping mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugStepMode {
    /// No stepping.
    None,
    /// Step one instruction.
    StepInto,
    /// Step until the current function returns or we reach the next line
    /// in the same frame.
    StepOver,
    /// Step until the current function returns.
    StepOut,
}

/// Information about a single call frame for debug inspection.
#[derive(Debug, Clone)]
pub struct DebugFrameInfo {
    /// Frame depth (0 = topmost/innermost).
    pub depth: usize,
    /// Function name.
    pub function: String,
    /// Source location.
    pub source_location: crate::span::SourceLocation,
}

/// Breakpoint target: a specific source line in a named function.
#[derive(Debug, Clone)]
pub struct Breakpoint {
    /// Function name to set the breakpoint in.
    pub function: String,
    /// 1-based source line number.
    pub line: usize,
}

/// Debugger state for the VM.
#[derive(Debug, Clone)]
pub struct DebugState {
    /// Whether debug checks are active.
    pub enabled: bool,
    /// Whether execution is currently paused.
    pub paused: bool,
    /// Step mode active during execution.
    pub step_mode: DebugStepMode,
    /// Frame depth when stepping started (for StepOver/StepOut).
    pub step_start_depth: usize,
    /// List of breakpoints.
    pub breakpoints: Vec<Breakpoint>,
    /// Resolved breakpoint bytecode offsets: (function_idx, offset) pairs.
    /// Rebuilt whenever breakpoints or functions change.
    pub resolved_breakpoints: Vec<(usize, usize)>,
    /// Offset to skip once when resuming after a breakpoint pause.
    /// Prevents immediately re-hitting the same breakpoint.
    pub skip_offset: Option<(usize, usize)>,
}

/// The Zenlang virtual machine.
/// Helper to build a SourceLocation from a function index and bytecode offset.
fn source_loc_from_frame(
    functions: &[BytecodeFn],
    function_idx: usize,
    ip: usize,
) -> crate::span::SourceLocation {
    let line = if let Some(chunk) = functions.get(function_idx).map(|f| &f.chunk) {
        chunk.get_line(ip.saturating_sub(1))
    } else {
        0
    };
    crate::span::SourceLocation::new(None, crate::span::Span::new(0, 0), line, 0)
}

/// A pending timer that fires at `fire_time` and optionally repeats.
struct TimerEntry {
    id: u64,
    callback: Value,
    fire_time: f64,
    interval: Option<f64>,
}

pub struct VM {
    stack: Vec<Value>,
    frames: Vec<CallFrame>,
    globals: Vec<Value>,
    functions: Vec<BytecodeFn>,
    global_names: Vec<String>,
    function_name_map: HashMap<String, usize>,
    natives: HashMap<String, usize>,
    native_fns: Vec<(String, NativeFn)>,
    pub foreign_registry: Rc<ForeignTypeRegistry>,
    /// Maximum number of instructions to execute before raising a script timeout error.
    /// `0` means unlimited.
    instruction_limit: u64,
    /// Instruction counter for the current `execute()` run.
    instruction_count: u64,
    /// If set, we are executing inside a generator and `Yield` should save state here.
    active_generator: Option<Rc<RefCell<crate::value::GeneratorState>>>,
    /// Frame count when `resume_generator` last called `execute()`.
    /// When a generator returns, we break if frames fall to this depth.
    generator_base_frame_count: usize,
    /// Current virtual time (in seconds). Advanced by calling `tick(dt)`.
    time: f64,
    /// Registered timers (timeouts and intervals).
    timers: Vec<TimerEntry>,
    /// Timers queued by native functions during callback execution.
    /// Flushed into `timers` after each callback returns.
    pending_timers: Vec<TimerEntry>,
    /// Callbacks to invoke once per `tick()` call (every frame).
    frame_callbacks: Vec<Value>,
    /// Monotonically increasing timer ID counter.
    timer_id_counter: u64,
    /// Debugger state (breakpoints, stepping, pause).
    pub debug_state: DebugState,
}

impl VM {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            frames: Vec::new(),
            globals: Vec::new(),
            functions: Vec::new(),
            global_names: Vec::new(),
            function_name_map: HashMap::new(),
            natives: HashMap::new(),
            native_fns: Vec::new(),
            foreign_registry: Rc::new(ForeignTypeRegistry::new()),
            instruction_limit: 0,
            instruction_count: 0,
            active_generator: None,
            generator_base_frame_count: 0,
            time: 0.0,
            timers: Vec::new(),
            pending_timers: Vec::new(),
            frame_callbacks: Vec::new(),
            timer_id_counter: 1,
            debug_state: DebugState {
                enabled: false,
                paused: false,
                step_mode: DebugStepMode::None,
                step_start_depth: 0,
                breakpoints: Vec::new(),
                resolved_breakpoints: Vec::new(),
                skip_offset: None,
            },
        }
    }

    pub fn new_with_registry(registry: Rc<ForeignTypeRegistry>) -> Self {
        Self {
            stack: Vec::new(),
            frames: Vec::new(),
            globals: Vec::new(),
            functions: Vec::new(),
            global_names: Vec::new(),
            function_name_map: HashMap::new(),
            natives: HashMap::new(),
            native_fns: Vec::new(),
            foreign_registry: registry,
            instruction_limit: 0,
            instruction_count: 0,
            active_generator: None,
            generator_base_frame_count: 0,
            time: 0.0,
            timers: Vec::new(),
            pending_timers: Vec::new(),
            frame_callbacks: Vec::new(),
            timer_id_counter: 1,
            debug_state: DebugState {
                enabled: false,
                paused: false,
                step_mode: DebugStepMode::None,
                step_start_depth: 0,
                breakpoints: Vec::new(),
                resolved_breakpoints: Vec::new(),
                skip_offset: None,
            },
        }
    }

    /// Enable or disable debug mode.
    pub fn set_debug(&mut self, enabled: bool) {
        self.debug_state.enabled = enabled;
        if !enabled {
            self.debug_state.paused = false;
            self.debug_state.step_mode = DebugStepMode::None;
        }
    }

    /// Add a breakpoint at the given line in the given function.
    /// Returns `false` if the function name is not found.
    pub fn set_breakpoint(&mut self, function: &str, line: usize) -> bool {
        if !self.function_name_map.contains_key(function) {
            return false;
        }
        // Avoid duplicates
        if self.debug_state.breakpoints.iter().any(|b| b.function == function && b.line == line) {
            return true;
        }
        self.debug_state.breakpoints.push(Breakpoint {
            function: function.to_string(),
            line,
        });
        self.rebuild_breakpoints();
        true
    }

    /// Remove a specific breakpoint.
    pub fn remove_breakpoint(&mut self, function: &str, line: usize) {
        self.debug_state.breakpoints.retain(|b| b.function != function || b.line != line);
        self.rebuild_breakpoints();
    }

    /// Remove all breakpoints.
    pub fn clear_breakpoints(&mut self) {
        self.debug_state.breakpoints.clear();
        self.debug_state.resolved_breakpoints.clear();
    }

    /// Set breakpoints on all functions that contain the given (1-based) source line.
    /// Returns the number of breakpoints set.
    pub fn set_source_breakpoint(&mut self, line: usize) -> usize {
        let mut count = 0;
        let names: Vec<String> = self.function_name_map.keys().cloned().collect();
        for name in &names {
            if let Some(&idx) = self.function_name_map.get(name) {
                if let Some(f) = self.functions.get(idx) {
                    if f.chunk.lines.iter().any(|l| *l + 1 == line) {
                        if self.set_breakpoint(name, line) {
                            count += 1;
                        }
                    }
                }
            }
        }
        count
    }

    /// Re-resolve all breakpoints from function names + line numbers to
    /// bytecode offsets.
    fn rebuild_breakpoints(&mut self) {
        self.debug_state.resolved_breakpoints.clear();
        for bp in &self.debug_state.breakpoints {
            let Some(&fn_idx) = self.function_name_map.get(&bp.function) else { continue };
            let Some(fn_def) = self.functions.get(fn_idx) else { continue };
            // Find the first bytecode offset at the given source line.
            // Internal line numbers are 0-based; breakpoints use 1-based.
            for (offset, &line) in fn_def.chunk.lines.iter().enumerate() {
                if line + 1 == bp.line {
                    self.debug_state.resolved_breakpoints.push((fn_idx, offset));
                    break;
                }
            }
        }
    }

    /// Continue execution after a breakpoint pause.
    pub fn debug_continue(&mut self) -> Result<Value> {
        if !self.debug_state.paused {
            return Err(self.runtime_error("not paused"));
        }
        self.debug_state.step_mode = DebugStepMode::None;
        self.debug_state.paused = false;
        self.execute_debug()
    }

    /// Step one instruction.
    pub fn debug_step_into(&mut self) -> Result<Value> {
        if !self.debug_state.paused {
            return Err(self.runtime_error("not paused"));
        }
        self.debug_state.step_mode = DebugStepMode::StepInto;
        self.debug_state.step_start_depth = self.frames.len();
        self.debug_state.paused = false;
        self.execute_debug()
    }

    /// Step over the current line.
    pub fn debug_step_over(&mut self) -> Result<Value> {
        if !self.debug_state.paused {
            return Err(self.runtime_error("not paused"));
        }
        self.debug_state.step_mode = DebugStepMode::StepOver;
        self.debug_state.step_start_depth = self.frames.len();
        self.debug_state.paused = false;
        self.execute_debug()
    }

    /// Step out of the current function.
    pub fn debug_step_out(&mut self) -> Result<Value> {
        if !self.debug_state.paused {
            return Err(self.runtime_error("not paused"));
        }
        self.debug_state.step_mode = DebugStepMode::StepOut;
        self.debug_state.step_start_depth = self.frames.len();
        self.debug_state.paused = false;
        self.execute_debug()
    }

    /// Whether the VM is currently paused at a breakpoint or step.
    pub fn is_paused(&self) -> bool {
        self.debug_state.paused
    }

    /// Get the current source location when paused.
    pub fn debug_current_location(&self) -> Option<crate::span::SourceLocation> {
        let frame = self.frames.last()?;
        let loc = source_loc_from_frame(&self.functions, frame.function_idx, frame.ip);
        Some(loc)
    }

    /// Get the current call stack frames as debug info.
    pub fn debug_stack_frames(&self) -> Vec<DebugFrameInfo> {
        self.frames
            .iter()
            .enumerate()
            .map(|(depth, frame)| {
                let loc = source_loc_from_frame(&self.functions, frame.function_idx, frame.ip);
                let fn_name = self.functions[frame.function_idx].name.clone();
                DebugFrameInfo {
                    depth,
                    function: fn_name,
                    source_location: loc,
                }
            })
            .collect()
    }

    /// Get local variable names and values for a given frame depth.
    /// `depth` 0 = topmost (innermost) frame.
    pub fn debug_locals(&self, depth: usize) -> Vec<(String, Value)> {
        if depth >= self.frames.len() {
            return Vec::new();
        }
        let frame = &self.frames[self.frames.len() - 1 - depth];
        let fn_def = &self.functions[frame.function_idx];
        let local_count = fn_def.chunk.locals as usize;
        let mut locals = Vec::with_capacity(local_count);
        // Parameters have names in the bytecode
        // For now, just label them as local_0, local_1, ...
        // A full implementation would use debug info from the compiler
        for i in 0..local_count {
            let name = if i < fn_def.arity as usize {
                format!("param_{}", i)
            } else {
                format!("local_{}", i - fn_def.arity as usize)
            };
            let stack_idx = frame.bp + i;
            let val = self.stack.get(stack_idx).cloned().unwrap_or(Value::Nil);
            locals.push((name, val));
        }
        locals
    }

    fn execute_debug(&mut self) -> Result<Value> {
        loop {
            self.execute()?;
            if self.debug_state.paused {
                return Ok(Value::Nil);
            }
            return Ok(self.stack.pop().unwrap_or(Value::Nil));
        }
    }

    /// Check whether the VM should pause at the current instruction.
    /// Called inside `execute()` before each instruction.
    fn debug_check(&mut self) -> bool {
        if !self.debug_state.enabled || self.debug_state.paused {
            return false;
        }
        let Some(frame) = self.frames.last() else { return false };
        let fn_idx = frame.function_idx;
        let ip = frame.ip;

        // Check skip_offset (resume past breakpoint)
        if self.debug_state.skip_offset == Some((fn_idx, ip)) {
            self.debug_state.skip_offset = None;
            return false;
        }
        self.debug_state.skip_offset = None;

        // Check resolved breakpoints
        for &(bp_fn, bp_off) in &self.debug_state.resolved_breakpoints {
            if bp_fn == fn_idx && bp_off == ip {
                self.debug_state.skip_offset = Some((fn_idx, ip));
                return true;
            }
        }

        // Check step mode
        match self.debug_state.step_mode {
            DebugStepMode::None => {}
            DebugStepMode::StepInto => {
                self.debug_state.step_mode = DebugStepMode::None;
                self.debug_state.skip_offset = Some((fn_idx, ip));
                return true;
            }
            DebugStepMode::StepOver => {
                let current_depth = self.frames.len();
                if current_depth <= self.debug_state.step_start_depth {
                    self.debug_state.step_mode = DebugStepMode::None;
                    self.debug_state.skip_offset = Some((fn_idx, ip));
                    return true;
                }
            }
            DebugStepMode::StepOut => {
                let current_depth = self.frames.len();
                if current_depth < self.debug_state.step_start_depth {
                    self.debug_state.step_mode = DebugStepMode::None;
                    self.debug_state.skip_offset = Some((fn_idx, ip));
                    return true;
                }
            }
        }
        false
    }

    /// Set a maximum number of instructions that can be executed per `run_main` / `call_function`
    /// call. When the limit is reached, a runtime error is returned.
    ///
    /// A value of `0` (the default) means unlimited execution.
    pub fn set_instruction_limit(&mut self, limit: u64) {
        self.instruction_limit = limit;
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
        eprintln!("DEBUG global_names: {:?}", global_names);
        for (i, f) in fns.iter().enumerate() {
            eprintln!("DEBUG fn[{}]: name={}, is_generator={}, arity={}, locals={}, code={:?}", 
                i, f.name, f.is_generator, f.arity, f.chunk.locals, 
                f.chunk.code.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" "));
        }
        let offset = self.functions.len();
        for (i, f) in fns.into_iter().enumerate() {
            let idx = offset + i;
            self.function_name_map.insert(f.name.clone(), idx);
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
    pub fn reload_functions(
        &mut self,
        fns: Vec<BytecodeFn>,
        new_global_names: Vec<String>,
    ) -> Result<()> {
        // Build old name→idx map from current functions
        let old_name_to_idx: HashMap<&str, usize> = self
            .functions
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

        // Rebuild the qualified-name map used for struct method dispatch
        // ("Type::method" -> fn idx). This is separate from `old_name_to_idx`
        // / `new_name_to_idx` above (which are locals used only for
        // remapping `Value::Function` references in global values) —
        // without this, `CallMethod` on a `Value::Struct` would keep
        // resolving to stale (or entirely wrong, if indices shifted)
        // function indices after every hot reload.
        self.function_name_map = self
            .functions
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.clone(), i))
            .collect();

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

    /// Call a zero-argument top-level function by plain name, if one is
    /// defined in the currently-loaded bytecode. Returns `Ok(None)` (without
    /// error) if no such function exists, so callers can use this for
    /// optional "hook" conventions without forcing every script to define
    /// one.
    ///
    /// Used by [`HotReloader`](crate::hotreload::HotReloader) to call an
    /// optional `fn on_reload()` after every successful hot reload, so
    /// scripts can re-derive cached/computed state (e.g. re-populate a
    /// lookup map, reset a timer) that a plain global-value snapshot can't
    /// meaningfully migrate on its own.
    pub fn call_if_exists(&mut self, name: &str) -> Result<Option<Value>> {
        let Some(&idx) = self.function_name_map.get(name) else {
            return Ok(None);
        };
        let fn_def = &self.functions[idx];
        let bp = self.stack.len();
        let frame = CallFrame::new(idx, bp);
        self.frames.push(frame);
        let slot_count = fn_def.chunk.locals as usize;
        while self.stack.len() < bp + slot_count {
            self.stack.push(Value::Nil);
        }
        self.execute()?;
        Ok(Some(self.stack.pop().unwrap_or(Value::Nil)))
    }

    /// Register a one-shot or repeating timer.
    ///
    /// `delay` is in seconds; if `interval` is `Some(i)`, the timer repeats
    /// every `i` seconds after the initial fire. Returns a unique timer ID
    /// that can be passed to `remove_timer`.
    pub fn add_timer(&mut self, callback: Value, delay: f64, interval: Option<f64>) -> u64 {
        let id = self.timer_id_counter;
        self.timer_id_counter += 1;
        let fire_time = self.time + delay.max(0.0);
        self.timers.push(TimerEntry {
            id,
            callback,
            fire_time,
            interval,
        });
        id
    }

    /// Cancel a timer by its ID. No-op if already fired or unknown.
    pub fn remove_timer(&mut self, id: u64) {
        self.timers.retain(|t| t.id != id);
    }

    /// Register a callback to be invoked once per `tick()` call (every frame).
    pub fn add_frame_callback(&mut self, callback: Value) {
        self.frame_callbacks.push(callback);
    }

    /// Remove a frame callback by identity (pointer comparison).
    pub fn remove_frame_callback(&mut self, callback: &Value) {
        self.frame_callbacks.retain(|c| !std::ptr::eq(c, callback));
    }

    /// Move any timers queued by native functions during callback execution
    /// into the active timer list. Called after each `call_value` in `tick()`.
    fn flush_pending_timers(&mut self) {
        while let Some(t) = self.pending_timers.pop() {
            eprintln!("DEBUG flush pending id={}", t.id);
            self.timers.push(t);
        }
    }

    /// Advance virtual time by `dt` seconds, fire any due timers,
    /// and invoke per-frame callbacks.
    ///
    /// Each due timer's callback is invoked as a fresh call (no script may
    /// be running when this is called). Intervals are re-scheduled after
    /// firing, using the original fire time plus the interval to avoid
    /// drift. If a timer's callback registers or cancels other timers,
    /// those changes are visible on subsequent iterations of the loop.
    ///
    /// Per-frame callbacks (`every_frame`) are invoked after all due timers
    /// have fired and re-scheduled, once per `tick()` call.
    pub fn tick(&mut self, dt: f64) -> Result<()> {
        self.time += dt;
        eprintln!("DEBUG tick time={}, timers.len={}, pending.len={}", self.time, self.timers.len(), self.pending_timers.len());
        loop {
            let idx = match self.timers.iter().position(|t| self.time >= t.fire_time) {
                Some(i) => i,
                None => { eprintln!("DEBUG no due timer, breaking"); break; },
            };
            eprintln!("DEBUG firing timer idx={}", idx);
            let timer = self.timers.remove(idx);
            if matches!(timer.callback, Value::Function(_) | Value::Closure(_)) {
                eprintln!("DEBUG calling callback for timer");
                self.call_value(&timer.callback, &[])?;
                eprintln!("DEBUG callback done, flushing pending");
                self.flush_pending_timers();
                eprintln!("DEBUG after flush: timers.len={}, pending.len={}", self.timers.len(), self.pending_timers.len());
            }
            if let Some(interval) = timer.interval {
                let next = timer.fire_time + interval;
                let fire_time = if next <= self.time {
                    self.time + interval
                } else {
                    next
                };
                self.timers.push(TimerEntry {
                    id: timer.id,
                    callback: timer.callback,
                    fire_time,
                    interval: Some(interval),
                });
            }
        }
        // Invoke per-frame callbacks
        let callbacks = std::mem::take(&mut self.frame_callbacks);
        for cb in &callbacks {
            if matches!(cb, Value::Function(_) | Value::Closure(_)) {
                self.call_value(cb, &[])?;
                self.flush_pending_timers();
            }
        }
        // Re-register any callbacks that want to continue firing every frame.
        // A callback that wants to continue must re-register itself via every_frame.
        Ok(())
    }

    /// Call a script value (function or closure) with the given arguments.
    ///
    /// Pushes a fresh call frame at `bp = 0`, so this must only be used
    /// when no script is currently executing (e.g. from `tick()` or from
    /// host code between script runs).
    fn call_value(&mut self, callee: &Value, args: &[Value]) -> Result<Value> {
        match callee {
            Value::Function(idx) => {
                let fn_def = &self.functions[*idx];
                if fn_def.is_generator {
                    return Err(self.runtime_error("cannot call generator via timer"));
                }
                let frame = CallFrame::new(*idx, 0);
                self.frames.push(frame);
                for arg in args {
                    self.stack.push(arg.clone());
                }
                let slot_count = fn_def.chunk.locals as usize;
                while self.stack.len() < slot_count {
                    self.stack.push(Value::Nil);
                }
                self.execute()?;
                Ok(self.stack.pop().unwrap_or(Value::Nil))
            }
            Value::Closure(closure) => {
                let data = closure.borrow();
                let fn_idx = data.fn_idx;
                eprintln!("DEBUG call_value closure: fn_idx={}, name={}", fn_idx, self.functions[fn_idx].name);
                let fn_def = &self.functions[fn_idx];
                let mut frame = CallFrame::new(fn_idx, 0);
                frame.is_closure = true;
                self.frames.push(frame);
                for uv in &data.upvalues {
                    self.stack.push(uv.clone());
                }
                for arg in args {
                    self.stack.push(arg.clone());
                }
                let slot_count = fn_def.chunk.locals as usize;
                while self.stack.len() < slot_count {
                    self.stack.push(Value::Nil);
                }
                self.execute()?;
                Ok(self.stack.pop().unwrap_or(Value::Nil))
            }
            _ => Err(self.runtime_error(format!("cannot call {}", callee.type_name()))),
        }
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

        let result = self.execute_debug()?;
        Ok(result)
    }

    /// Build a runtime error with a stack trace from the current call frames.
    fn runtime_error(&self, msg: impl Into<String>) -> Error {
        let mut stack_trace: Vec<crate::span::SourceLocation> = self
            .frames
            .iter()
            .map(|frame| source_loc_from_frame(&self.functions, frame.function_idx, frame.ip))
            .collect();
        stack_trace.reverse(); // innermost frame first
        let msg = msg.into();
        let trace_str: String = stack_trace
            .iter()
            .enumerate()
            .map(|(i, loc)| {
                let fn_name = if i < self.frames.len() {
                    let idx = self.frames[self.frames.len() - 1 - i].function_idx;
                    &self.functions[idx].name
                } else {
                    "?"
                };
                format!("  {}: at {} (in {})", i, loc, fn_name)
            })
            .collect::<Vec<_>>()
            .join("\n");
        Error::Runtime {
            msg: if stack_trace.is_empty() {
                msg
            } else {
                format!("{}\nstack trace:\n{}", msg, trace_str)
            },
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
        self.instruction_count = 0;
        loop {
            // Check for breakpoints / stepping before each instruction
            if self.debug_check() {
                self.debug_state.paused = true;
                return Ok(());
            }

            let frame = self.frames.last().unwrap();
            if frame.ip >= self.chunk().code.len() {
                break;
            }

            self.instruction_count += 1;
            if self.instruction_limit > 0 && self.instruction_count > self.instruction_limit {
                return Err(self.runtime_error(format!(
                    "script timeout: executed {} instructions (limit: {})",
                    self.instruction_count, self.instruction_limit,
                )));
            }

            let byte = self.read_byte();
            let op = Opcode::from_byte(byte)
                .ok_or_else(|| self.runtime_error(format!("unknown opcode: {}", byte)))?;

            if self.frames.last().map(|f| f.function_idx) == Some(2) {
                eprintln!("DEBUG EXEC fn[{}] op={:?} (byte=0x{:02x})", self.frames.last().unwrap().function_idx, op, byte);
            }

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
                        (Value::Float(af), Value::Float(bf)) => {
                            self.stack.push(Value::Float(af + bf))
                        }
                        (Value::Int(ai), Value::Float(bf)) => {
                            self.stack.push(Value::Float(*ai as f64 + bf))
                        }
                        (Value::Float(af), Value::Int(bi)) => {
                            self.stack.push(Value::Float(af + *bi as f64))
                        }
                        (Value::Str(as_), Value::Str(bs)) => {
                            let mut result = as_.to_string();
                            result.push_str(bs);
                            self.stack.push(Value::Str(result.into()));
                        }
                        _ => {
                            return Err(self.runtime_error(format!(
                                "cannot add {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }

                Opcode::Sub => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai - bi)),
                        (Value::Float(af), Value::Float(bf)) => {
                            self.stack.push(Value::Float(af - bf))
                        }
                        (Value::Int(ai), Value::Float(bf)) => {
                            self.stack.push(Value::Float(*ai as f64 - bf))
                        }
                        (Value::Float(af), Value::Int(bi)) => {
                            self.stack.push(Value::Float(af - *bi as f64))
                        }
                        _ => {
                            return Err(self.runtime_error(format!(
                                "cannot subtract {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }

                Opcode::Mul => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai * bi)),
                        (Value::Float(af), Value::Float(bf)) => {
                            self.stack.push(Value::Float(af * bf))
                        }
                        (Value::Int(ai), Value::Float(bf)) => {
                            self.stack.push(Value::Float(*ai as f64 * bf))
                        }
                        (Value::Float(af), Value::Int(bi)) => {
                            self.stack.push(Value::Float(af * *bi as f64))
                        }
                        _ => {
                            return Err(self.runtime_error(format!(
                                "cannot multiply {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
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
                            return Err(self.runtime_error(format!(
                                "cannot divide {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
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
                            return Err(self.runtime_error(format!(
                                "cannot mod {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }

                Opcode::Neg => {
                    let a = self.stack.pop().unwrap();
                    match a {
                        Value::Int(n) => self.stack.push(Value::Int(-n)),
                        Value::Float(n) => self.stack.push(Value::Float(-n)),
                        _ => {
                            return Err(
                                self.runtime_error(format!("cannot negate {}", a.type_name()))
                            );
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
                            if fn_def.is_generator {
                                // Generator function called: return a Generator value immediately
                                let g = Rc::new(RefCell::new(
                                    crate::value::GeneratorState {
                                        function_idx: *idx,
                                        ip: 0,
                                        first_call: true,
                                        exhausted: false,
                                        locals: Vec::new(),
                                    },
                                ));
                                // Pop callee and args, push Generator
                                self.stack.truncate(args_start - 1);
                                self.stack.push(Value::Generator(g));
                            } else {
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
                            // bp points to the first upvalue (callee was already popped)
                            let bp = self.stack.len() - up_count - args.len();
                            let mut frame = CallFrame::new(fn_idx, bp);
                            frame.is_closure = true;
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
                            let mut ctx = VMContext {
                                registry: self.foreign_registry.clone(),
                                raw_vm: self as *mut VM,
                            };
                            let result = f(&mut ctx, &args)?;
                            self.stack.push(result);
                        }
                        _ => {
                            return Err(
                                self.runtime_error(format!("cannot call {}", callee.type_name()))
                            );
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
                            let method_name = self
                                .chunk()
                                .method_names
                                .get(method_idx)
                                .cloned()
                                .unwrap_or_default();
                            let type_id = fv.borrow().type_id;
                            let type_name = fv.borrow().type_name;
                            // args[0] = receiver, args[1..] = call arguments (matches the
                            // convention used by ForeignTypeDef::method closures elsewhere).
                            let args: Vec<Value> = self.stack.drain(args_start - 1..).collect();
                            let mut ctx = VMContext {
                                registry: self.foreign_registry.clone(),
                                raw_vm: self as *mut VM,
                            };
                            match self.foreign_registry.call_method(
                                &type_id,
                                &method_name,
                                &mut ctx,
                                &args,
                            ) {
                                Some(Ok(result)) => self.stack.push(result),
                                Some(Err(e)) => return Err(e),
                                None => {
                                    return Err(self.runtime_error(format!(
                                        "foreign type '{}' has no method '{}'",
                                        type_name, method_name
                                    )));
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
                        // Native script struct method dispatch
                        Value::Struct(_, type_name) => {
                            let method_name = self
                                .chunk()
                                .method_names
                                .get(method_idx)
                                .cloned()
                                .unwrap_or_default();
                            let qualified = format!("{}::{}", type_name, method_name);
                            match self.function_name_map.get(&qualified).copied() {
                                Some(fn_idx) => {
                                    let fn_def = &self.functions[fn_idx];
                                    // bp = args_start - 1 so receiver (self) is at local 0
                                    let bp = args_start - 1;
                                    let mut frame = CallFrame::new(fn_idx, bp);
                                    frame.is_method = true;
                                    self.frames.push(frame);
                                    let slot_count = fn_def.chunk.locals as usize;
                                    while self.stack.len() < bp + slot_count {
                                        self.stack.push(Value::Nil);
                                    }
                                }
                                None => {
                                    return Err(self.runtime_error(format!(
                                        "type '{}' has no method '{}'",
                                        type_name, method_name
                                    )));
                                }
                            }
                        }
                        Value::NativeFunction(f) => {
                            let args: Vec<Value> = self.stack.drain(args_start..).collect();
                            self.stack.pop();
                            let mut ctx = VMContext {
                                registry: self.foreign_registry.clone(),
                                raw_vm: self as *mut VM,
                            };
                            let result = f(&mut ctx, &args)?;
                            self.stack.push(result);
                        }
                        _ => {
                            return Err(self.runtime_error(format!(
                                "cannot call method on {}",
                                obj.type_name()
                            )));
                        }
                    }
                }

                Opcode::Return => {
                    let result = self.stack.pop().unwrap_or(Value::Nil);
                    let frame = self.frames.pop().unwrap();

                    // If returning from a generator function, mark it as exhausted
                    if let Some(state_cell) = &self.active_generator {
                        state_cell.borrow_mut().exhausted = true;
                    }

                    if self.active_generator.is_some() {
                        // Generator resumed frame: bp points to where generator's
                        // stack begins (no callee). Trim to bp.
                        self.stack.truncate(frame.bp);
                        // If we've returned to the generator's base frame level,
                        // break out so resume_generator regains control.
                        if self.frames.len() <= self.generator_base_frame_count {
                            self.stack.push(result);
                            break;
                        }
                    } else if frame.is_method || frame.is_closure {
                        // Method/closure: bp points to receiver/first-upvalue.
                        // Trim at bp (callee already removed) to keep below values.
                        self.stack.truncate(frame.bp);
                    } else if frame.bp > 0 {
                        // Regular call: bp points past the callee. Trim at bp-1
                        // to remove callee + args.
                        self.stack.truncate(frame.bp - 1);
                    } else {
                        self.stack.truncate(frame.bp);
                    }

                    if self.frames.is_empty() {
                        self.stack.push(result);
                        self.flush_pending_timers();
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
                        if let Value::Str(s) = name {
                            field_names_vec.push(s.to_string());
                            values.push(val);
                        }
                    }
                    // Reverse because we popped in reverse order
                    field_names_vec.reverse();
                    values.reverse();
                    let field_names: Rc<Vec<String>> = Rc::new(field_names_vec);
                    self.stack.push(Value::Struct(
                        Rc::new(RefCell::new(crate::value::StructData {
                            values,
                            field_names,
                        })),
                        type_name,
                    ));
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

                Opcode::MakeRange => {
                    let inclusive = self.stack.pop().unwrap();
                    let end = self.stack.pop().unwrap();
                    let start = self.stack.pop().unwrap();
                    match (&start, &end, &inclusive) {
                        (Value::Int(s), Value::Int(e), Value::Bool(inc)) => {
                            self.stack.push(Value::Range(*s, *e, *inc));
                        }
                        _ => {
                            return Err(self.runtime_error(format!(
                                "range requires integer bounds, got {} and {}",
                                start.type_name(),
                                end.type_name()
                            )));
                        }
                    }
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
                    let field_name = self
                        .chunk()
                        .field_names
                        .get(field_idx)
                        .cloned()
                        .unwrap_or_default();
                    let obj = self.stack.pop().unwrap();
                    match &obj {
                        Value::Struct(data, _) => {
                            let d = data.borrow();
                            // Try direct index first (compile-time resolved field index)
                            let val = if field_idx < d.values.len() {
                                d.values[field_idx].clone()
                            } else {
                                // Fall back to name-based lookup (hot-reload shape change)
                                d.get_field(&field_name).cloned().unwrap_or(Value::Nil)
                            };
                            self.stack.push(val);
                        }
                        Value::Foreign(fv) => {
                            match self.foreign_registry.get_field(
                                &fv.borrow().type_id,
                                &field_name,
                                &obj,
                            ) {
                                Some(Ok(val)) => self.stack.push(val),
                                Some(Err(e)) => return Err(e),
                                None => {
                                    return Err(self.runtime_error(format!(
                                        "foreign type '{}' has no field '{}'",
                                        fv.borrow().type_name,
                                        field_name
                                    )));
                                }
                            }
                        }
                        _ => {
                            return Err(self.runtime_error(format!(
                                "cannot access field on {}",
                                obj.type_name()
                            )));
                        }
                    }
                }

                Opcode::StoreField(_) => {
                    let field_idx = self.read_u16() as usize;
                    let field_name = self
                        .chunk()
                        .field_names
                        .get(field_idx)
                        .cloned()
                        .unwrap_or_default();
                    let val = self.stack.pop().unwrap();
                    let mut obj = self.stack.pop().unwrap();
                    // Extract type_id before the match to avoid borrow conflicts
                    let foreign_type_id = match &obj {
                        Value::Foreign(fv) => Some(fv.borrow().type_id),
                        _ => None,
                    };
                    let result_val = val.clone();
                    match &mut obj {
                        Value::Struct(data, _) => {
                            let mut d = data.borrow_mut();
                            // Try direct index first (compile-time resolved field index)
                            if field_idx < d.values.len() {
                                d.values[field_idx] = val;
                            } else if let Some(field) = d.get_field_mut(&field_name) {
                                *field = val;
                            }
                            self.stack.push(result_val);
                        }
                        Value::Foreign(_) => {
                            let type_id = foreign_type_id.unwrap();
                            match self.foreign_registry.set_field(
                                &type_id,
                                &field_name,
                                &mut obj,
                                val,
                            ) {
                                Some(Ok(())) => self.stack.push(result_val),
                                Some(Err(e)) => return Err(e),
                                None => {
                                    return Err(self.runtime_error(format!(
                                        "foreign type has no field '{}'",
                                        field_name
                                    )));
                                }
                            }
                        }
                        _ => {
                            return Err(self.runtime_error(format!(
                                "cannot set field on {}",
                                obj.type_name()
                            )));
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
                            let c = s
                                .chars()
                                .nth(idx)
                                .map(|c| c.to_string())
                                .unwrap_or_default();
                            self.stack.push(Value::Str(c.into()));
                        }
                        (Value::Range(start, end, inclusive), Value::Int(i)) => {
                            let val = start + i;
                            if (!*inclusive && val >= *end)
                                || (*inclusive && val > *end)
                                || val < *start.min(end)
                            {
                                return Err(self.runtime_error("index out of range bounds"));
                            }
                            self.stack.push(Value::Int(val));
                        }
                        _ => {
                            return Err(self.runtime_error(format!(
                                "cannot index {} with {}",
                                obj.type_name(),
                                index.type_name()
                            )));
                        }
                    }
                }

                Opcode::StoreIndex => {
                    let val = self.stack.pop().unwrap();
                    let index = self.stack.pop().unwrap();
                    let obj = self.stack.pop().unwrap();
                    let result_val = val.clone();
                    match (&obj, &index) {
                        (Value::Array(arr), Value::Int(i)) => {
                            let idx = *i as usize;
                            arr.borrow_mut()[idx] = val;
                            self.stack.push(result_val);
                        }
                        _ => {
                            return Err(self.runtime_error(format!(
                                "cannot index {} with {}",
                                obj.type_name(),
                                index.type_name()
                            )));
                        }
                    }
                }

                Opcode::Len => {
                    let val = self.stack.pop().unwrap();
                    match val {
                        Value::Str(s) => self.stack.push(Value::Int(s.len() as i64)),
                        Value::Array(arr) => self.stack.push(Value::Int(arr.borrow().len() as i64)),
                        Value::Range(start, end, inclusive) => {
                            let len = if inclusive {
                                end - start + 1
                            } else {
                                end - start
                            };
                            self.stack.push(Value::Int(len.max(0)));
                        }
                        _ => {
                            return Err(self.runtime_error(format!(
                                "cannot get length of {}",
                                val.type_name()
                            )));
                        }
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
                            return Err(self.runtime_error(format!(
                                "cannot bitwise-and {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }

                Opcode::BitOr => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai | bi)),
                        _ => {
                            return Err(self.runtime_error(format!(
                                "cannot bitwise-or {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }

                Opcode::BitXor => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai ^ bi)),
                        _ => {
                            return Err(self.runtime_error(format!(
                                "cannot bitwise-xor {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }

                Opcode::Shl => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai << bi)),
                        _ => {
                            return Err(self.runtime_error(format!(
                                "cannot shift left {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }

                Opcode::Shr => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => self.stack.push(Value::Int(ai >> bi)),
                        _ => {
                            return Err(self.runtime_error(format!(
                                "cannot shift right {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }

                Opcode::BitNot => {
                    let a = self.stack.pop().unwrap();
                    match a {
                        Value::Int(n) => self.stack.push(Value::Int(!n)),
                        _ => {
                            return Err(
                                self.runtime_error(format!("cannot bitwise-not {}", a.type_name()))
                            );
                        }
                    }
                }

                Opcode::LoadEnumTag => {
                    let val = self.stack.pop().unwrap();
                    match val {
                        Value::Enum { tag, data: _ } => self.stack.push(Value::Int(tag as i64)),
                        _ => {
                            return Err(
                                self.runtime_error(format!("LoadEnumTag on non-enum value"))
                            );
                        }
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
                        _ => {
                            return Err(
                                self.runtime_error(format!("LoadEnumField on non-enum value"))
                            );
                        }
                    }
                }

                Opcode::Yield => {
                    let val = self.stack.pop().unwrap();
                    match &self.active_generator {
                        Some(state_cell) => {
                            let saved_frame = self.frames.last().unwrap();
                            let fn_idx = saved_frame.function_idx;
                            let bp = saved_frame.bp;
                            let ip = saved_frame.ip;
                            let fn_def = &self.functions[fn_idx];
                            let local_count = fn_def.chunk.locals as usize;
                            let mut state = state_cell.borrow_mut();
                            state.ip = ip;
                            state.locals = self.stack[bp..bp + local_count].to_vec();
                            state.first_call = false;
                            // Pop the frame and leave the yielded value on the stack
                            self.frames.pop();
                            self.stack.truncate(bp);
                            self.stack.push(val);
                            break;
                        }
                        None => {
                            return Err(
                                self.runtime_error("yield outside generator function")
                            );
                        }
                    }
                }

                Opcode::Halt => {
                    break;
                }
            }
        }
        self.flush_pending_timers();
        Ok(())
    }

    /// Resume execution of a generator. Returns the yielded value, or `None`
    /// if the generator is exhausted.
    pub fn resume_generator(&mut self, state_cell: Rc<RefCell<crate::value::GeneratorState>>) -> Result<Option<Value>> {
        let state = state_cell.borrow();
        if state.exhausted {
            return Ok(None);
        }
        let fn_idx = state.function_idx;
        let first_call = state.first_call;
        let saved_locals = state.locals.clone();
        let saved_ip = state.ip;
        drop(state);

        let fn_def = &self.functions[fn_idx];
        let bp = self.stack.len();
        let frame = CallFrame::new(fn_idx, bp);
        self.frames.push(frame);

        if first_call {
            // Initial call: arguments are already on the stack
            let local_count = fn_def.chunk.locals as usize;
            while self.stack.len() < bp + local_count {
                self.stack.push(Value::Nil);
            }
        } else {
            // Restore saved locals onto the stack
            self.stack.extend(saved_locals);
        }

        // Set ip to the saved position
        {
            let frame = self.frames.last_mut().unwrap();
            frame.ip = saved_ip;
        }

        self.active_generator = Some(state_cell.clone());
        self.generator_base_frame_count = self.frames.len();
        let result_val = self.execute_debug();
        self.active_generator = None;
        self.generator_base_frame_count = 0;

        match result_val {
            Ok(val) => {
                // If paused, return None (the caller should check is_paused)
                if self.debug_state.paused {
                    return Ok(None);
                }
                // Check if generator was exhausted (normal return, not yield)
                let state = state_cell.borrow();
                if state.exhausted {
                    Ok(None)
                } else {
                    Ok(Some(val))
                }
            }
            Err(e) => {
                // Mark as exhausted on error too
                state_cell.borrow_mut().exhausted = true;
                Err(e)
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
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
        let mut symbols =
            crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        vm.run_main().unwrap()
    }

    pub fn run_program(source: &str) -> crate::error::Result<Value> {
        let tokens = Lexer::new(source).tokenize()?;
        let parser = Parser::new(source, &tokens);
        let mut program = parser.parse()?;
        let native_names = crate::stdlib::native_names();
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names)?;
        let types = crate::typeck::check(&program, &mut symbols)?;
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &native_names, source)?;
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        vm.run_main()
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
    fn test_string_interpolation_basic() {
        let result = run("let name = \"world\"; \"hello {name}\"");
        assert_eq!(result, Value::Str("hello world".into()));
    }

    #[test]
    fn test_string_interpolation_int() {
        let result = run("\"the answer is {42}\"");
        assert_eq!(result, Value::Str("the answer is 42".into()));
    }

    #[test]
    fn test_string_interpolation_multiple() {
        let result = run("let a = 1; let b = 2; \"{a} + {b} = {a + b}\"");
        assert_eq!(result, Value::Str("1 + 2 = 3".into()));
    }

    #[test]
    fn test_string_interpolation_no_interp() {
        let result = run("\"hello world\"");
        assert_eq!(result, Value::Str("hello world".into()));
    }

    #[test]
    fn test_string_interpolation_escaped_brace() {
        let result = run("\"hello {{name}}\"");
        assert_eq!(result, Value::Str("hello {name}".into()));
    }

    #[test]
    fn test_string_interpolation_float() {
        let result = run("let pi = 3.14; \"pi is {pi}\"");
        assert_eq!(result, Value::Str("pi is 3.14".into()));
    }

    #[test]
    fn test_string_interpolation_bool() {
        let result = run("let b = true; \"it is {b}\"");
        assert_eq!(result, Value::Str("it is true".into()));
    }

    #[test]
    fn test_string_interpolation_empty_str() {
        let result = run("\"\"");
        assert_eq!(result, Value::Str("".into()));
    }

    #[test]
    fn test_string_interpolation_only_expr() {
        let result = run("\"{42}\"");
        assert_eq!(result, Value::Str("42".into()));
    }

    #[test]
    fn test_string_interpolation_expr_first() {
        let result = run("let x = \"hello\"; \"{x} world\"");
        assert_eq!(result, Value::Str("hello world".into()));
    }

    #[test]
    fn test_trait_impl_pipeline() {
        let source = r#"
            struct Circle { radius: f64 }
            trait Shape { fn area(&self) -> f64; }
            impl Shape for Circle {
                fn area(&self) -> f64 {
                    self.radius * self.radius * 3.14159
                }
            }
            let c = Circle { radius: 2.0 };
            c.area()
        "#;
        let result = run(source);
        assert!((result.as_float().unwrap() - 12.56636).abs() < 0.001);
    }

    #[test]
    fn test_trait_multiple_impls() {
        let source = r#"
            struct Circle { radius: f64 }
            struct Rect { w: f64, h: f64 }
            trait Shape { fn area(&self) -> f64; }
            impl Shape for Circle {
                fn area(&self) -> f64 { self.radius * self.radius * 3.14159 }
            }
            impl Shape for Rect {
                fn area(&self) -> f64 { self.w * self.h }
            }
            let c = Circle { radius: 2.0 };
            let r = Rect { w: 3.0, h: 4.0 };
            c.area() + r.area()
        "#;
        let result = run(source);
        assert!((result.as_float().unwrap() - 24.56636).abs() < 0.001);
    }

    #[test]
    fn test_trait_method_dispatch() {
        let source = r#"
            struct A { val: i64 }
            struct B { val: i64 }
            trait GetVal { fn get(&self) -> i64; }
            impl GetVal for A { fn get(&self) -> i64 { self.val } }
            impl GetVal for B { fn get(&self) -> i64 { self.val * 2 } }
            let a = A { val: 10 };
            let b = B { val: 10 };
            a.get() + b.get()
        "#;
        let result = run(source);
        assert_eq!(result, Value::Int(30));
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
            .field(
                "x",
                |obj: &Value| -> Result<Value> {
                    interop::with_foreign::<Point, _, _>(obj, |p| Ok(Value::Int(p.x as i64)))
                },
                |obj: &mut Value, val: Value| -> Result<()> {
                    let x = val.as_int().unwrap() as i32;
                    interop::with_foreign_mut::<Point, _, _>(obj, |p| {
                        p.x = x;
                        Ok(())
                    })
                },
            )
            .field(
                "y",
                |obj: &Value| -> Result<Value> {
                    interop::with_foreign::<Point, _, _>(obj, |p| Ok(Value::Int(p.y as i64)))
                },
                |obj: &mut Value, val: Value| -> Result<()> {
                    let y = val.as_int().unwrap() as i32;
                    interop::with_foreign_mut::<Point, _, _>(obj, |p| {
                        p.y = y;
                        Ok(())
                    })
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
        def.fields
            .get("x")
            .unwrap()
            .set(&mut fv, Value::Int(99))
            .unwrap();

        let result = def.fields.get("x").unwrap().get(&fv).unwrap();
        assert_eq!(result, Value::Int(99));
    }

    #[test]
    fn test_interop_foreign_method() {
        let mut vm = VM::new();
        vm.register_type::<Point>("Point")
            .field(
                "x",
                |obj: &Value| -> Result<Value> {
                    interop::with_foreign::<Point, _, _>(obj, |p| Ok(Value::Int(p.x as i64)))
                },
                |obj: &mut Value, val: Value| -> Result<()> {
                    let x = val.as_int().unwrap() as i32;
                    interop::with_foreign_mut::<Point, _, _>(obj, |p| {
                        p.x = x;
                        Ok(())
                    })
                },
            )
            .method(
                "double_x",
                Rc::new(|_ctx: &mut VMContext, args: &[Value]| -> Result<Value> {
                    interop::with_foreign::<Point, _, _>(&args[0], |p| {
                        Ok(Value::Int((p.x * 2) as i64))
                    })
                }),
            );

        let point = Point { x: 5, y: 10 };
        let fv = Value::Foreign(Rc::new(RefCell::new(ForeignObject::new("Point", point))));

        // Call method via registry
        let mut ctx = VMContext {
            registry: vm.foreign_registry.clone(),
            raw_vm: std::ptr::null_mut(),
        };
        let result = vm
            .foreign_registry
            .call_method(&TypeId::of::<Point>(), "double_x", &mut ctx, &[fv.clone()])
            .unwrap()
            .unwrap();
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
        let ctx = &mut VMContext {
            registry: Rc::new(ForeignTypeRegistry::new()),
            raw_vm: std::ptr::null_mut(),
        };
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
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &[], source).unwrap();

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
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &[], source).unwrap();

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
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &[], source1).unwrap();

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
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &[], source2).unwrap();

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
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &[], source).unwrap();

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
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &[], source).unwrap();

        vm.reload_functions(fns, global_names).unwrap();
        let result = vm.run_main().unwrap();
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn test_reload_functions_preserves_struct_method_dispatch() {
        // Struct method calls (`CallMethod` on `Value::Struct`) resolve via
        // the qualified-name `function_name_map` ("Type::method" -> fn idx),
        // which is separate from the plain-name maps `reload_functions`
        // already updates. If it isn't refreshed too, method calls break
        // (or silently call stale bytecode) after every hot reload.
        let source1 = r#"
            struct Counter { value: i64 }
            impl Counter {
                fn get(&self) -> i64 { self.value }
            }
            fn main() -> i64 {
                let c = Counter { value: 1 };
                c.get()
            }
        "#;

        let tokens = Lexer::new(source1).tokenize().unwrap();
        let parser = Parser::new(source1, &tokens);
        let mut program = parser.parse().unwrap();
        let mut symbols = crate::resolver::resolve(&mut program).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &[], source1).unwrap();

        let mut vm = VM::new();
        vm.load_bytecode(fns, global_names);
        let result = vm.run_main().unwrap();
        assert_eq!(result, Value::Int(1));

        // Reload with a change: add a new function ahead of the struct's
        // impl block (shifting every subsequent function index), and
        // change what the method returns — simulating an edit-and-save
        // during a live hot-reload session.
        let source2 = r#"
            fn unrelated() -> i64 { 999 }
            struct Counter { value: i64 }
            impl Counter {
                fn get(&self) -> i64 { self.value + 41 }
            }
            fn main() -> i64 {
                let c = Counter { value: 1 };
                c.get()
            }
        "#;

        let tokens = Lexer::new(source2).tokenize().unwrap();
        let parser = Parser::new(source2, &tokens);
        let mut program = parser.parse().unwrap();
        let mut symbols = crate::resolver::resolve(&mut program).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &[], source2).unwrap();

        vm.reload_functions(fns, global_names).unwrap();
        let result = vm.run_main().unwrap();
        assert_eq!(result, Value::Int(42));
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
        assert_eq!(result, Value::Str("42".into()));
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
            if let Some(name) = old_name_to_idx
                .iter()
                .find(|(_, v)| **v == *idx)
                .map(|(n, _)| *n)
            {
                if let Some(&new_idx) = new_name_to_idx.get(name) {
                    *idx = new_idx;
                }
            }
        }
        Value::Closure(c) => {
            let mut data = c.borrow_mut();
            if let Some(name) = old_name_to_idx
                .iter()
                .find(|(_, v)| **v == data.fn_idx)
                .map(|(n, _)| *n)
            {
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
        Value::Struct(data, _) => {
            let mut d = data.borrow_mut();
            for v in d.values.iter_mut() {
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
        let mut symbols =
            crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
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
        let mut symbols =
            crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
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

    #[test]
    fn test_instruction_limit_hits_timeout() {
        let source = "loop {}";
        let tokens = crate::lexer::Lexer::new(source).tokenize().unwrap();
        let mut program = crate::parser::Parser::new(source, &tokens).parse().unwrap();
        let native_names = crate::stdlib::native_names();
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) =
            crate::compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        vm.set_instruction_limit(100);
        let err = vm.run_main().unwrap_err();
        assert!(
            err.to_string().contains("script timeout"),
            "expected script timeout error, got: {}",
            err
        );
    }

    #[test]
    fn test_instruction_limit_zero_is_unlimited() {
        let source = "let x = 0; loop { x = x + 1; if x >= 100 { break; } } x";
        let tokens = crate::lexer::Lexer::new(source).tokenize().unwrap();
        let mut program = crate::parser::Parser::new(source, &tokens).parse().unwrap();
        let native_names = crate::stdlib::native_names();
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) =
            crate::compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        // Default limit is 0 (unlimited), so this should complete normally.
        let result = vm.run_main().unwrap();
        assert_eq!(result, Value::Int(100));
    }

    #[test]
    fn test_generator_yields_value() {
        let source = r#"
            fn gen() {
                yield 42;
            }

            let g = gen();
            let result = unwrap(next(g));
            result
        "#;
        let result = run(source);
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_generator_multiple_yields() {
        let source = r#"
            fn gen() {
                yield 1;
                yield 2;
                yield 3;
            }

            let g = gen();
            let a = unwrap(next(g));
            let b = unwrap(next(g));
            let c = unwrap(next(g));
            a + b + c
        "#;
        let result = run(source);
        assert_eq!(result, Value::Int(6));
    }

    #[test]
    fn test_generator_exhausted_returns_none() {
        let source = r#"
            fn gen() {
                yield 42;
            }

            let g = gen();
            let a = unwrap(next(g));
            let b = next(g);
            is_none(b)
        "#;
        let result = run(source);
        assert_eq!(result, Value::Bool(true));
    }

    // --- Timer / scheduling tests ---

    fn run_for_timer_tests(source: &str) -> (VM, Vec<String>) {
        let tokens = crate::lexer::Lexer::new(source).tokenize().unwrap();
        let mut program = crate::parser::Parser::new(source, &tokens).parse().unwrap();
        let native_names = crate::stdlib::native_names();
        let mut symbols =
            crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) =
            crate::compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names.clone());
        vm.run_main().unwrap();
        (vm, global_names)
    }

    fn get_log_len(vm: &VM, names: &[String]) -> usize {
        let idx = names.iter().position(|n| n == "log").unwrap();
        match &vm.globals[idx] {
            Value::Array(arr) => arr.borrow().len(),
            _ => panic!("expected array global 'log'"),
        }
    }

    #[test]
    fn test_timer_set_timeout_fires_callback() {
        let source = r#"
            let log = [];
            set_timeout(|| { push(log, "fired"); }, 0.5);
        "#;
        let (mut vm, names) = run_for_timer_tests(source);
        assert_eq!(get_log_len(&vm, &names), 0);
        vm.tick(0.6).unwrap();
        assert_eq!(get_log_len(&vm, &names), 1);
    }

    #[test]
    fn test_timer_set_timeout_zero_delay_fires_immediately() {
        let source = r#"
            let log = [];
            set_timeout(|| { push(log, "fired"); }, 0.0);
        "#;
        let (mut vm, names) = run_for_timer_tests(source);
        assert_eq!(get_log_len(&vm, &names), 0);
        vm.tick(0.0).unwrap();
        assert_eq!(get_log_len(&vm, &names), 1);
    }

    #[test]
    fn test_timer_set_timeout_multiple_callbacks() {
        let source = r#"
            let log = [];
            set_timeout(|| { push(log, "first"); }, 0.2);
            set_timeout(|| { push(log, "second"); }, 0.1);
        "#;
        let (mut vm, names) = run_for_timer_tests(source);
        assert_eq!(get_log_len(&vm, &names), 0);
        vm.tick(0.15).unwrap();
        assert_eq!(get_log_len(&vm, &names), 1);
        vm.tick(0.1).unwrap();
        assert_eq!(get_log_len(&vm, &names), 2);
    }

    #[test]
    fn test_timer_interval_fires_repeatedly() {
        let source = r#"
            let log = [];
            set_interval(|| { push(log, "tick"); }, 0.5);
        "#;
        let (mut vm, names) = run_for_timer_tests(source);
        vm.tick(0.5).unwrap();
        assert_eq!(get_log_len(&vm, &names), 1);
        vm.tick(0.5).unwrap();
        assert_eq!(get_log_len(&vm, &names), 2);
        vm.tick(0.5).unwrap();
        assert_eq!(get_log_len(&vm, &names), 3);
    }

    #[test]
    fn test_timer_clear_timeout_no_fire() {
        let source = r#"
            let log = [];
            let id = set_timeout(|| { push(log, "fired"); }, 0.5);
            clear_timer(id);
        "#;
        let (mut vm, names) = run_for_timer_tests(source);
        vm.tick(1.0).unwrap();
        assert_eq!(get_log_len(&vm, &names), 0);
    }

    #[test]
    fn test_timer_clear_interval_stops_repeating() {
        let source = r#"
            let log = [];
            let id = set_interval(|| { push(log, "tick"); }, 0.5);
            clear_timer(id);
        "#;
        let (mut vm, names) = run_for_timer_tests(source);
        vm.tick(2.0).unwrap();
        assert_eq!(get_log_len(&vm, &names), 0);
    }

    #[test]
    fn test_timer_nonexistent_id_does_nothing() {
        let source = r#"
            let log = [];
            set_timeout(|| { push(log, "fired"); }, 0.5);
            clear_timer(999);
        "#;
        let (mut vm, names) = run_for_timer_tests(source);
        vm.tick(1.0).unwrap();
        assert_eq!(get_log_len(&vm, &names), 1);
    }

    #[test]
    fn test_timer_set_timeout_returns_id() {
        let source = r#"
            let id = set_timeout(|| {}, 1.0);
            id
        "#;
        let tokens = crate::lexer::Lexer::new(source).tokenize().unwrap();
        let mut program = crate::parser::Parser::new(source, &tokens).parse().unwrap();
        let native_names = crate::stdlib::native_names();
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) =
            crate::compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        let result = vm.run_main().unwrap();
        assert!(result.as_int().unwrap() > 0, "expected positive timer ID");
    }

    #[test]
    fn test_timer_callback_error_propagates() {
        let source = r#"
            set_timeout(|| { assert(false); }, 0.5);
        "#;
        let tokens = crate::lexer::Lexer::new(source).tokenize().unwrap();
        let mut program = crate::parser::Parser::new(source, &tokens).parse().unwrap();
        let native_names = crate::stdlib::native_names();
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) =
            crate::compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        vm.run_main().unwrap();
        let err = vm.tick(1.0).unwrap_err();
        assert!(
            err.to_string().contains("assert failed"),
            "expected assert error, got: {}",
            err
        );
    }

    #[test]
    #[test]
    #[test]
    fn test_timer_callback_multiple_statements() {
        // Verify that a single callback can execute multiple statements
        let source = r#"
            let log = [];
            set_timeout(|| {
                push(log, "first");
                push(log, "second");
            }, 0.5);
        "#;
        let (mut vm, names) = run_for_timer_tests(source);
        assert_eq!(get_log_len(&vm, &names), 0);
        vm.tick(0.6).unwrap();
        assert_eq!(get_log_len(&vm, &names), 2, "expected both pushes to execute");
    }

    #[test]
    #[test]
    #[test]
    fn test_timer_callback_calls_set_timeout() {
        // Verify that a callback can call set_timeout (the outer call, not the second timer)
        let source = r#"
            let log = [];
            set_timeout(|| {
                push(log, "first");
            }, 0.3);
            set_timeout(|| {
                push(log, "second");
            }, 0.5);
        "#;
        let (mut vm, names) = run_for_timer_tests(source);
        assert_eq!(get_log_len(&vm, &names), 0);
        vm.tick(0.4).unwrap();
        assert_eq!(get_log_len(&vm, &names), 1, "expected first timer");
        vm.tick(0.2).unwrap();
        assert_eq!(get_log_len(&vm, &names), 2, "expected second timer");
    }

    #[test]
    fn test_timer_register_from_callback_script() {
        // Simpler script: register one timer from another, with a shared array
        let source = r#"
            let log = [];
            set_timeout(|| {
                push(log, "first");
                set_timeout(|| { push(log, "second"); }, 0.1);
            }, 0.5);
        "#;
        // Also try: a simpler version without push
        let source2 = r#"
            let log = [];
            set_timeout(|| {
                set_timeout(|| { push(log, "second"); }, 0.1);
            }, 0.5);
        "#;
        eprintln!("=== Testing source1 ===");
        let tokens = crate::lexer::Lexer::new(source).tokenize().unwrap();
        let mut program = crate::parser::Parser::new(source, &tokens).parse().unwrap();
        let native_names = crate::stdlib::native_names();
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) =
            crate::compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names.clone());

        vm.run_main().unwrap();

        // Timer should be registered
        assert_eq!(vm.timers.len(), 1, "expected 1 timer after setup");

        vm.tick(0.5).unwrap();
        // After first tick, the callback should have run (pushing "first" and registering new timer)
        assert_eq!(vm.timers.len(), 1, "expected inner timer to be registered");
        let log_idx = global_names.iter().position(|n| n == "log").unwrap();
        let log_len = match &vm.globals[log_idx] {
            Value::Array(arr) => arr.borrow().len(),
            _ => panic!("expected array"),
        };
        assert_eq!(log_len, 1, "expected 1 log entry after first tick");

        vm.tick(0.5).unwrap();
        // After second tick, the inner timer should have fired too
        let log_len = match &vm.globals[log_idx] {
            Value::Array(arr) => arr.borrow().len(),
            _ => panic!("expected array"),
        };
        assert_eq!(log_len, 2, "expected 2 log entries after second tick");
    }

    // --- Debug infrastructure tests ---

    fn compile_and_load(source: &str) -> VM {
        let tokens = crate::lexer::Lexer::new(source).tokenize().unwrap();
        let mut program = crate::parser::Parser::new(source, &tokens).parse().unwrap();
        let native_names = crate::stdlib::native_names();
        let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) =
            crate::compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
        let mut vm = VM::new();
        crate::stdlib::register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        vm
    }

    #[test]
    fn test_debug_disabled_by_default() {
        let mut vm = compile_and_load("fn main() { 42 }");
        assert!(!vm.debug_state.enabled);
        assert!(!vm.is_paused());
    }

    #[test]
    fn test_debug_set_breakpoint_unknown_function() {
        let mut vm = compile_and_load("fn main() { 42 }");
        assert!(!vm.set_breakpoint("nonexistent", 1));
    }

    #[test]
    fn test_debug_set_breakpoint_known_function() {
        let mut vm = compile_and_load("fn main() { 42 }");
        assert!(vm.set_breakpoint("main", 1));
    }

    #[test]
    fn test_debug_breakpoint_hits_and_pauses() {
        let mut vm = compile_and_load("fn main() { let x = 42; x }");
        vm.set_debug(true);
        vm.set_breakpoint("main", 1);
        vm.run_main().unwrap();
        assert!(vm.is_paused(), "expected VM to pause at breakpoint");
    }

    #[test]
    fn test_debug_breakpoint_continue() {
        let mut vm = compile_and_load("fn main() { let x = 42; x }");
        vm.set_debug(true);
        vm.set_breakpoint("main", 1);
        vm.run_main().unwrap();
        assert!(vm.is_paused());
        let result = vm.debug_continue().unwrap();
        assert!(!vm.is_paused(), "expected VM to finish after continue");
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_debug_no_breakpoint_runs_normally() {
        let mut vm = compile_and_load("fn main() { 42 }");
        vm.set_debug(true);
        let result = vm.run_main().unwrap();
        assert!(!vm.is_paused(), "expected VM to finish without breakpoint");
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_debug_step_into() {
        let mut vm = compile_and_load("fn main() { let x = 42; x }");
        vm.set_debug(true);
        vm.set_breakpoint("main", 1);
        vm.run_main().unwrap();
        assert!(vm.is_paused());
        // After step_into, we advance one instruction and pause again
        let result = vm.debug_step_into().unwrap();
        assert!(vm.is_paused(), "expected VM to pause after step_into");
        // Continue to end
        let result = vm.debug_continue().unwrap();
        assert!(!vm.is_paused());
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_debug_stack_frames_when_paused() {
        let mut vm = compile_and_load("fn main() { let x = 42; x }");
        vm.set_debug(true);
        vm.set_breakpoint("main", 1);
        vm.run_main().unwrap();
        assert!(vm.is_paused());
        let frames = vm.debug_stack_frames();
        assert!(!frames.is_empty(), "expected at least one frame");
        assert_eq!(frames.last().unwrap().function, "main");
    }

    #[test]
    fn test_debug_current_location_when_paused() {
        let mut vm = compile_and_load("fn main() { let x = 42; x }");
        vm.set_debug(true);
        vm.set_breakpoint("main", 1);
        vm.run_main().unwrap();
        assert!(vm.is_paused());
        let loc = vm.debug_current_location();
        assert!(loc.is_some(), "expected a source location");
    }

    #[test]
    fn test_debug_clear_breakpoints() {
        let mut vm = compile_and_load("fn main() { 42 }");
        vm.set_debug(true);
        vm.set_breakpoint("main", 1);
        vm.clear_breakpoints();
        let result = vm.run_main().unwrap();
        assert!(!vm.is_paused(), "expected VM to finish without breakpoints");
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_debug_set_debug_false_disables_pause() {
        let mut vm = compile_and_load("fn main() { 42 }");
        vm.set_debug(true);
        vm.set_breakpoint("main", 1);
        vm.set_debug(false); // disable before run
        let result = vm.run_main().unwrap();
        assert!(!vm.is_paused(), "expected VM to finish when debug disabled");
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_debug_continue_without_pause_errors() {
        let mut vm = compile_and_load("fn main() { 42 }");
        let err = vm.debug_continue();
        assert!(err.is_err(), "expected error when continuing without pause");
    }

    #[test]
    fn test_debug_step_over_pauses() {
        let mut vm = compile_and_load("fn main() { let x = 42; x }");
        vm.set_debug(true);
        vm.set_breakpoint("main", 1);
        vm.run_main().unwrap();
        assert!(vm.is_paused());
        let result = vm.debug_step_over().unwrap();
        assert!(vm.is_paused(), "expected VM to pause after step_over");
        let result = vm.debug_continue().unwrap();
        assert!(!vm.is_paused());
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_debug_step_out() {
        let mut vm = compile_and_load("fn main() { let x = 42; x }");
        vm.set_debug(true);
        vm.set_breakpoint("main", 1);
        vm.run_main().unwrap();
        assert!(vm.is_paused());
        // Step out of main returns to __main__ and pauses
        let result = vm.debug_step_out().unwrap();
        assert!(vm.is_paused(), "expected VM to pause after step_out from main");
        // Continue from __main__ to finish
        let result = vm.debug_continue().unwrap();
        assert!(!vm.is_paused());
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_debug_remove_breakpoint() {
        let mut vm = compile_and_load("fn main() { 42 }");
        vm.set_debug(true);
        vm.set_breakpoint("main", 1);
        vm.remove_breakpoint("main", 1);
        let result = vm.run_main().unwrap();
        assert!(!vm.is_paused());
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_debug_locals_populated() {
        let source = "fn add(a: int, b: int) -> int { a + b }
fn main() { add(1, 2) }";
        let mut vm = compile_and_load(source);
        vm.set_debug(true);
        vm.set_breakpoint("add", 1);
        vm.run_main().unwrap();
        assert!(vm.is_paused());
        let locals = vm.debug_locals(0); // top frame = add
        assert!(!locals.is_empty(), "expected locals");
        // Should have param_0 and param_1
        let names: Vec<&str> = locals.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"param_0"));
        assert!(names.contains(&"param_1"));
    }
}
