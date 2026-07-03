use crate::error::Result;
use crate::value::Value as ZenValue;
use crate::vm::{DebugStepMode, VM};
use serde_json::{json, Value as JsonValue};
use std::cell::RefCell;
use std::io::{self, BufRead, Read, Write};
use std::path::Path;
use std::rc::Rc;

fn read_message() -> Option<JsonValue> {
    let mut stdin = io::stdin().lock();
    let mut content_len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if stdin.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(len) = line.strip_prefix("Content-Length: ") {
            content_len = len.trim().parse().ok();
        }
    }
    let len = content_len?;
    let mut buf = vec![0u8; len];
    stdin.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn write_message(msg: &JsonValue) {
    let body = serde_json::to_string(msg).unwrap();
    let mut stdout = io::stdout().lock();
    let _ = write!(stdout, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = stdout.flush();
}

struct DapSession {
    seq: i64,
    vm: VM,
    path: Option<String>,
    output_buffer: Rc<RefCell<Vec<String>>>,
    running: bool,
    finished: bool,
    /// Maps variablesReference IDs to compound values for tree expansion.
    variable_refs: Vec<crate::value::Value>,
    /// Breakpoint line → condition expression (for conditional breakpoints).
    bp_conditions: std::collections::HashMap<usize, String>,
}

impl DapSession {
    fn next_seq(&mut self) -> i64 {
        let s = self.seq;
        self.seq += 1;
        s
    }

    fn send_event(&mut self, event: &str, body: JsonValue) {
        let mut msg = json!({
            "type": "event",
            "event": event,
            "seq": self.next_seq(),
        });
        if !body.is_null() {
            msg["body"] = body;
        }
        write_message(&msg);
    }

    fn send_response(
        &mut self,
        req_seq: i64,
        command: &str,
        success: bool,
        body: Option<JsonValue>,
    ) {
        let mut msg = json!({
            "type": "response",
            "request_seq": req_seq,
            "success": success,
            "command": command,
            "seq": self.next_seq(),
        });
        if let Some(b) = body {
            msg["body"] = b;
        }
        write_message(&msg);
    }

    fn flush_output(&mut self) {
        let lines: Vec<String> = self.output_buffer.borrow_mut().drain(..).collect();
        for line in lines {
            self.send_event(
                "output",
                json!({
                    "category": "stdout",
                    "output": line,
                }),
            );
        }
    }

    fn stopped_reason(&self) -> &str {
        if self.vm.debug_state.breakpoints.iter().any(|bp| {
            if let Some(loc) = self.vm.debug_current_location() {
                bp.line == loc.line
            } else {
                false
            }
        }) {
            "breakpoint"
        } else {
            match self.vm.debug_state.step_mode {
                DebugStepMode::None => "breakpoint",
                _ => "step",
            }
        }
    }

    fn send_stopped(&mut self) {
        let reason = self.stopped_reason();
        self.send_event(
            "stopped",
            json!({
                "reason": reason,
                "threadId": 1,
                "allThreadsStopped": true,
            }),
        );
    }

    fn resume_and_check(&mut self, action: impl FnOnce(&mut VM) -> Result<crate::value::Value>) {
        if self.finished {
            return;
        }
        let result = action(&mut self.vm);
        self.flush_output();
        match result {
            Ok(_) => {
                if self.vm.is_paused() {
                    // Check if this is a conditional breakpoint whose
                    // condition evaluates to falsy — skip it automatically.
                    if self.try_skip_conditional_breakpoint() {
                        return;
                    }
                    self.send_stopped();
                } else {
                    self.finished = true;
                    self.send_event("terminated", json!({}));
                }
            }
            Err(e) => {
                self.finished = true;
                self.send_event(
                    "output",
                    json!({
                        "category": "stderr",
                        "output": format!("error: {}\n", e),
                    }),
                );
                self.send_event("terminated", json!({}));
            }
        }
    }

    /// If the VM is stopped at a breakpoint that has a condition, evaluate the
    /// condition and return true (auto-resume) if the condition is falsy.
    fn try_skip_conditional_breakpoint(&mut self) -> bool {
        let loc = match self.vm.debug_current_location() {
            Some(l) => l,
            None => return false,
        };
        let cond = match self.bp_conditions.get(&loc.line) {
            Some(c) => c.clone(),
            None => return false,
        };

        // Simple evaluation via variable lookup in locals
        let locals = self.vm.debug_locals(0);
        let val = eval_condition(&cond, &locals);
        match val {
            Some(v) if is_truthy(&v) => false,
            Some(_) => {
                // Condition is falsy: resume
                self.resume_and_check(|vm| vm.debug_continue());
                true
            }
            None => false, // Could not evaluate; stop anyway
        }
    }

    /// Convert a Value to a DAP variable descriptor, storing compound values
    /// for tree expansion and returning a non-zero variablesReference.
    fn value_to_dap_var(&mut self, name: &str, val: crate::value::Value) -> JsonValue {
        let vr = self.store_variable(val.clone());
        json!({
            "name": name,
            "value": format!("{:?}", val),
            "type": val.type_name(),
            "variablesReference": vr,
        })
    }

    /// Store a compound value for tree expansion and return its reference ID.
    /// Returns 0 for primitive values.
    fn store_variable(&mut self, val: crate::value::Value) -> i64 {
        match &val {
            crate::value::Value::Array(_)
            | crate::value::Value::Map(_)
            | crate::value::Value::Struct(..)
            | crate::value::Value::Foreign(_) => {
                let id = self.variable_refs.len() + 2; // 1 = locals, 2+ = compound
                self.variable_refs.push(val);
                id as i64
            }
            _ => 0,
        }
    }

}

/// Expand a compound value into (name, value) pairs.
fn expand_value_raw(vm: &VM, val: &ZenValue) -> Vec<(String, ZenValue)> {
    match val {
        ZenValue::Array(h) => {
            let arr = &vm.arrays.get(*h).values;
            arr.iter()
                .enumerate()
                .map(|(i, v)| (format!("[{}]", i), v.clone()))
                .collect()
        }
        ZenValue::Map(h) => {
            let map = &vm.maps.get(*h).entries;
            map.iter()
                .map(|(k, v)| (format!("{:?}", k), v.clone()))
                .collect()
        }
        ZenValue::Struct(h, _) => {
            let data = vm.structs.get(*h);
            let names = data.field_names.clone();
            names.iter()
                .map(|name| {
                    let fv = data.get_field(name).cloned().unwrap_or(ZenValue::Nil);
                    (name.clone(), fv)
                })
                .collect()
        }
        ZenValue::Foreign(_) => {
            // Foreign expansion requires the registry; handled in the caller
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// Evaluate a condition expression for breakpoints.
fn eval_condition(cond: &str, locals: &[(String, crate::value::Value)]) -> Option<crate::value::Value> {
    let trimmed = cond.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Simple variable lookup
    if let Some((_, val)) = locals.iter().find(|(name, _)| name == trimmed) {
        return Some(val.clone());
    }
    // Binary operators: `a == b`, `a != b`, `a < b`, etc.
    if let Some(op_pos) = trimmed.find(|c: char| c == '=' || c == '!' || c == '<' || c == '>') {
        let lhs = trimmed[..op_pos].trim();
        let mut rest = trimmed[op_pos..].trim_start();
        // Handle ==, !=, <=, >=, <, >
        let op = if rest.starts_with("==") {
            rest = &rest[2..];
            "=="
        } else if rest.starts_with("!=") {
            rest = &rest[2..];
            "!="
        } else if rest.starts_with("<=") {
            rest = &rest[2..];
            "<="
        } else if rest.starts_with(">=") {
            rest = &rest[2..];
            ">="
        } else if rest.starts_with('<') {
            rest = &rest[1..];
            "<"
        } else if rest.starts_with('>') {
            rest = &rest[1..];
            ">"
        } else {
            return None;
        };
        let rhs = rest.trim();
        if let (Some(lv), Some(rv)) = (eval_simple_term(lhs, locals), eval_simple_term(rhs, locals)) {
            match op {
                "==" => Some(crate::value::Value::Bool(lv == rv)),
                "!=" => Some(crate::value::Value::Bool(lv != rv)),
                _ => {
                    // Numeric comparisons
                    let li = to_f64(&lv);
                    let ri = to_f64(&rv);
                    match (li, ri) {
                        (Some(a), Some(b)) => {
                            let result = match op {
                                "<" => a < b,
                                "<=" => a <= b,
                                ">" => a > b,
                                ">=" => a >= b,
                                _ => false,
                            };
                            Some(crate::value::Value::Bool(result))
                        }
                        _ => None,
                    }
                }
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// Evaluate a simple term (variable name or numeric literal).
fn eval_simple_term(term: &str, locals: &[(String, crate::value::Value)]) -> Option<crate::value::Value> {
    if let Ok(i) = term.parse::<i64>() {
        return Some(crate::value::Value::Int(i));
    }
    if let Ok(f) = term.parse::<f64>() {
        return Some(crate::value::Value::Float(f));
    }
    if let Some((_, val)) = locals.iter().find(|(name, _)| name == term) {
        return Some(val.clone());
    }
    None
}

fn to_f64(v: &crate::value::Value) -> Option<f64> {
    match v {
        crate::value::Value::Int(i) => Some(*i as f64),
        crate::value::Value::Float(f) => Some(*f),
        crate::value::Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn is_truthy(v: &crate::value::Value) -> bool {
    match v {
        crate::value::Value::Nil => false,
        crate::value::Value::Bool(b) => *b,
        _ => true,
    }
}

pub fn run_dap(source: &str, source_path: Option<&Path>) -> Result<()> {
    let tokens = crate::lexer::Lexer::new(source).tokenize()?;
    let parser = crate::parser::Parser::new(source, &tokens);
    let mut program = parser.parse()?;
    if let Some(path) = source_path {
        crate::mod_resolver::resolve_modules(&mut program, path)?;
    }
    crate::prelude::inject(&mut program)?;
    let native_names = crate::stdlib::native_names();
    let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names)?;
    let types = crate::typeck::check(&program, &mut symbols)?;
    let (fns, global_names) =
        crate::compiler::compile(&program, &types, &symbols, &native_names, source)?;

    let output_buffer = Rc::new(RefCell::new(Vec::new()));

    let mut vm = VM::new();
    crate::stdlib::register_builtins(&mut vm);

    {
        let out = output_buffer.clone();
        vm.register_native(
            "print",
            Rc::new(
                move |_ctx: &mut crate::vm::VMContext, args: &[crate::value::Value]| {
                    let mut s = String::new();
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            s.push(' ');
                        }
                        s.push_str(&format!("{:?}", arg));
                    }
                    out.borrow_mut().push(s);
                    Ok(crate::value::Value::Nil)
                },
            ),
        );
    }

    vm.load_bytecode(fns, global_names);
    vm.set_debug(true);

    let path_str = source_path.map(|p| p.to_string_lossy().replace('\\', "/"));

    let mut session = DapSession {
        seq: 1,
        vm,
        path: path_str,
        output_buffer,
        running: false,
        finished: false,
        variable_refs: Vec::new(),
        bp_conditions: std::collections::HashMap::new(),
    };

    session.dap_loop()
}

impl DapSession {
    fn dap_loop(&mut self) -> Result<()> {
        loop {
            let req = match read_message() {
                Some(m) => m,
                None => return Ok(()),
            };

            if req["type"].as_str() != Some("request") {
                continue;
            }

            let command = req["command"].as_str().unwrap_or("").to_string();
            let req_seq = req["seq"].as_i64().unwrap_or(0);

            match command.as_str() {
                "initialize" => {
                    self.send_response(
                        req_seq,
                        &command,
                        true,
                        Some(json!({
                            "supportsConfigurationDoneRequest": true,
                            "supportsSetVariable": false,
                            "supportsConditionalBreakpoints": true,
                            "supportsHitConditionalBreakpoints": false,
                            "supportsFunctionBreakpoints": false,
                            "supportsEvaluateForHovers": true,
                            "supportsStepBack": false,
                            "supportsDataBreakpoints": false,
                            "supportsTerminateRequest": true,
                            "supportsExceptionInfoRequest": false,
                        })),
                    );
                    self.send_event("initialized", json!({}));
                }

                "launch" => {
                    self.send_response(req_seq, &command, true, None);
                }

                "setBreakpoints" => {
                    let args = req.get("arguments");
                    let bps = args
                        .and_then(|a| a.get("breakpoints"))
                        .and_then(|b| b.as_array())
                        .cloned()
                        .unwrap_or_default();

                    self.vm.clear_breakpoints();
                    self.bp_conditions.clear();

                    let mut actual_bps = Vec::new();
                    for bp in &bps {
                        let line = bp["line"].as_i64().unwrap_or(0) as usize;
                        let count = self.vm.set_source_breakpoint(line);
                        if let Some(cond) = bp["condition"].as_str() {
                            if !cond.is_empty() {
                                self.bp_conditions.insert(line, cond.to_string());
                            }
                        }
                        actual_bps.push(json!({
                            "line": line,
                            "verified": count > 0,
                        }));
                    }

                    self.send_response(
                        req_seq,
                        &command,
                        true,
                        Some(json!({ "breakpoints": actual_bps })),
                    );
                }

                "setExceptionBreakpoints" => {
                    self.send_response(req_seq, &command, true, None);
                }

                "configurationDone" => {
                    self.send_response(req_seq, &command, true, None);
                    self.running = true;
                    self.resume_and_check(|vm| vm.run_main());
                }

                "continue" => {
                    self.resume_and_check(|vm| vm.debug_continue());
                    self.send_response(
                        req_seq,
                        &command,
                        true,
                        Some(json!({ "allThreadsContinued": true })),
                    );
                }

                "next" => {
                    self.resume_and_check(|vm| vm.debug_step_over());
                    self.send_response(req_seq, &command, true, None);
                }

                "stepIn" => {
                    self.resume_and_check(|vm| vm.debug_step_into());
                    self.send_response(req_seq, &command, true, None);
                }

                "stepOut" => {
                    self.resume_and_check(|vm| vm.debug_step_out());
                    self.send_response(req_seq, &command, true, None);
                }

                "stackTrace" => {
                    let frames = self.vm.debug_stack_frames();
                    let stack_frames: Vec<JsonValue> = frames
                        .iter()
                        .enumerate()
                        .map(|(i, frame)| {
                            let source = self.path.as_ref().map(|p| {
                                json!({
                                    "path": p,
                                    "sourceReference": 0,
                                })
                            });
                            json!({
                                "id": i,
                                "name": frame.function,
                                "source": source,
                                "line": frame.source_location.line as i64,
                                "column": 1,
                            })
                        })
                        .collect();
                    self.send_response(
                        req_seq,
                        &command,
                        true,
                        Some(json!({
                            "stackFrames": stack_frames,
                            "totalFrames": frames.len(),
                        })),
                    );
                }

                "scopes" => {
                    self.send_response(
                        req_seq,
                        &command,
                        true,
                        Some(json!({
                            "scopes": [{
                                "name": "Locals",
                                "variablesReference": 1,
                                "expensive": false,
                            }]
                        })),
                    );
                }

                "variables" => {
                    let ref_id = req["arguments"]["variablesReference"].as_i64().unwrap_or(0) as usize;
                    let variables: Vec<JsonValue> = if ref_id == 1 {
                        // Top-level locals
                        self.vm.debug_locals(0)
                            .into_iter()
                            .map(|(name, val)| self.value_to_dap_var(&name, val))
                            .collect()
                    } else {
                        // Child variables from a compound value
                        let idx = ref_id.wrapping_sub(2);
                        let mut vars = Vec::new();
                        if let Some(val) = self.variable_refs.get(idx) {
                            // Foreign needs registry access
                            if let crate::value::Value::Foreign(h) = val {
                                let fo = self.vm.foreigns.get(*h);
                                let type_name = fo.type_name.to_owned();
                                let registry = &self.vm.foreign_registry;
                                if let Some(def) = registry.get_by_name(&type_name) {
                                    let children: Vec<(String, ZenValue)> = def.fields.keys().filter_map(|name| {
                                        def.fields[name].get(&self.vm, val).ok().map(|fv| (name.clone(), fv))
                                    }).collect();
                                    for (name, fv) in children {
                                        vars.push(self.value_to_dap_var(&name, fv));
                                    }
                                }
                            } else {
                                let children = expand_value_raw(&self.vm, val);
                                for (name, fv) in children {
                                    vars.push(self.value_to_dap_var(&name, fv));
                                }
                            }
                        }
                        vars
                    };
                    self.send_response(
                        req_seq,
                        &command,
                        true,
                        Some(json!({ "variables": variables })),
                    );
                }

                "evaluate" => {
                    let expr = req["arguments"]["expression"].as_str().unwrap_or("");
                    let frame_id = req["arguments"]["frameId"].as_i64().unwrap_or(0) as usize;
                    // Simple variable lookup in the specified frame
                    let locals = self.vm.debug_locals(frame_id);
                    let result = locals.iter().find(|(name, _)| name == expr).map(|(_, val)| {
                        let var = self.value_to_dap_var(expr, val.clone());
                        json!({
                            "result": var["value"],
                            "type": var["type"],
                            "variablesReference": var["variablesReference"],
                        })
                    });
                    match result {
                        Some(body) => {
                            self.send_response(req_seq, &command, true, Some(body));
                        }
                        None => {
                            // Try to evaluate as a simple expression
                            self.send_response(
                                req_seq,
                                &command,
                                false,
                                Some(json!({
                                    "result": format!("cannot evaluate '{}'", expr),
                                })),
                            );
                        }
                    }
                }

                "source" => {
                    self.send_response(
                        req_seq,
                        &command,
                        true,
                        Some(json!({
                            "content": "",
                            "mimeType": "text/plain",
                        })),
                    );
                }

                "threads" => {
                    self.send_response(
                        req_seq,
                        &command,
                        true,
                        Some(json!({
                            "threads": [{"id": 1, "name": "main"}],
                        })),
                    );
                }

                "pause" => {
                    self.send_response(req_seq, &command, true, None);
                }

                "terminate" | "disconnect" => {
                    self.send_response(req_seq, &command, true, None);
                    return Ok(());
                }

                _ => {
                    self.send_response(req_seq, &command, false, None);
                }
            }
        }
    }
}
