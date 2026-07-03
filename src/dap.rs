use crate::error::Result;
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
                            "supportsConditionalBreakpoints": false,
                            "supportsHitConditionalBreakpoints": false,
                            "supportsFunctionBreakpoints": false,
                            "supportsEvaluateForHovers": false,
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

                    let mut actual_bps = Vec::new();
                    for bp in &bps {
                        let line = bp["line"].as_i64().unwrap_or(0) as usize;
                        let count = self.vm.set_source_breakpoint(line);
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
                    let vars = self.vm.debug_locals(0);
                    let variables: Vec<JsonValue> = vars
                        .iter()
                        .map(|(name, val)| {
                            json!({
                                "name": name,
                                "value": format!("{:?}", val),
                                "type": val.type_name(),
                                "variablesReference": 0,
                            })
                        })
                        .collect();
                    self.send_response(
                        req_seq,
                        &command,
                        true,
                        Some(json!({ "variables": variables })),
                    );
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
