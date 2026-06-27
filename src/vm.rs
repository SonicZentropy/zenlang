use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::error::{Error, Result};
use crate::ir::{BytecodeFn, Chunk, Opcode};
use crate::value::{NativeFn, Value};

/// Execution context provided to native functions.
pub struct VMContext;

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
pub struct VM {
    stack: Vec<Value>,
    frames: Vec<CallFrame>,
    globals: Vec<Value>,
    functions: Vec<BytecodeFn>,
    natives: HashMap<String, usize>,
    native_fns: Vec<(String, NativeFn)>,
}

impl VM {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            frames: Vec::new(),
            globals: Vec::new(),
            functions: Vec::new(),
            natives: HashMap::new(),
            native_fns: Vec::new(),
        }
    }

    pub fn load_bytecode(&mut self, fns: Vec<BytecodeFn>) {
        let offset = self.functions.len();
        for (i, f) in fns.into_iter().enumerate() {
            let idx = offset + i;
            self.functions.push(f);
            if i == 0 {
                self.natives.insert("__main__".into(), idx);
            }
        }
    }

    pub fn register_native(&mut self, name: &str, f: NativeFn) {
        let idx = self.native_fns.len();
        self.natives.insert(name.to_string(), idx);
        self.native_fns.push((name.to_string(), f));
    }

    /// Run the main function.
    pub fn run_main(&mut self) -> Result<Value> {
        let main_idx = match self.natives.get("__main__") {
            Some(&idx) => idx,
            None => return Err(Error::Runtime {
                msg: "no main function found".into(),
                stack_trace: Vec::new(),
            }),
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
            let op = Opcode::from_byte(byte).ok_or_else(|| Error::Runtime {
                msg: format!("unknown opcode: {}", byte),
                stack_trace: Vec::new(),
            })?;

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
                        _ => {
                            return Err(Error::Runtime {
                                msg: format!("cannot add {} and {}", a.type_name(), b.type_name()),
                                stack_trace: Vec::new(),
                            });
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
                            return Err(Error::Runtime {
                                msg: format!("cannot subtract {} and {}", a.type_name(), b.type_name()),
                                stack_trace: Vec::new(),
                            });
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
                            return Err(Error::Runtime {
                                msg: format!("cannot multiply {} and {}", a.type_name(), b.type_name()),
                                stack_trace: Vec::new(),
                            });
                        }
                    }
                }

                Opcode::Div => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => {
                            if *bi == 0 {
                                return Err(Error::Runtime {
                                    msg: "division by zero".into(),
                                    stack_trace: Vec::new(),
                                });
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
                                return Err(Error::Runtime {
                                    msg: "division by zero".into(),
                                    stack_trace: Vec::new(),
                                });
                            }
                            self.stack.push(Value::Float(af / *bi as f64));
                        }
                        _ => {
                            return Err(Error::Runtime {
                                msg: format!("cannot divide {} and {}", a.type_name(), b.type_name()),
                                stack_trace: Vec::new(),
                            });
                        }
                    }
                }

                Opcode::Mod => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => {
                            if *bi == 0 {
                                return Err(Error::Runtime {
                                    msg: "modulo by zero".into(),
                                    stack_trace: Vec::new(),
                                });
                            }
                            self.stack.push(Value::Int(ai % bi));
                        }
                        _ => {
                            return Err(Error::Runtime {
                                msg: format!("cannot mod {} and {}", a.type_name(), b.type_name()),
                                stack_trace: Vec::new(),
                            });
                        }
                    }
                }

                Opcode::Neg => {
                    let a = self.stack.pop().unwrap();
                    match a {
                        Value::Int(n) => self.stack.push(Value::Int(-n)),
                        Value::Float(n) => self.stack.push(Value::Float(-n)),
                        _ => {
                            return Err(Error::Runtime {
                                msg: format!("cannot negate {}", a.type_name()),
                                stack_trace: Vec::new(),
                            });
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
                        Value::NativeFunction(f) => {
                            let args: Vec<Value> = self.stack.drain(args_start..).collect();
                            self.stack.pop(); // pop callee
                            let mut ctx = VMContext;
                            let result = f(&mut ctx, &args)?;
                            self.stack.push(result);
                        }
                        _ => {
                            return Err(Error::Runtime {
                                msg: format!("cannot call {}", callee.type_name()),
                                stack_trace: Vec::new(),
                            });
                        }
                    }
                }

                Opcode::CallMethod(_, _) => {
                    let _method_idx = self.read_u16() as usize;
                    let arg_count = self.read_u16() as usize;
                    // For now, treat as regular call — method dispatch will come later
                    let args_start = self.stack.len() - arg_count;
                    let callee = &self.stack[args_start - 1].clone();

                    match callee {
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
                            let mut ctx = VMContext;
                            let result = f(&mut ctx, &args)?;
                            self.stack.push(result);
                        }
                        _ => {
                            return Err(Error::Runtime {
                                msg: format!("cannot call method on {}", callee.type_name()),
                                stack_trace: Vec::new(),
                            });
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
                        // Field names are currently ignored — use sequential keys
                        let key = format!("_{}", field_count - map.len() - 1);
                        map.insert(key, val);
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
                    let _field_idx = self.read_u16() as usize;
                    let obj = self.stack.pop().unwrap();
                    match &obj {
                        Value::Struct(map) => {
                            let key = format!("_{}", _field_idx);
                            let val = map.borrow().get(&key).unwrap().clone();
                            self.stack.push(val);
                        }
                        Value::Foreign(_r) => {
                            // TODO: field access on foreign objects
                            self.stack.push(Value::Nil);
                        }
                        _ => {
                            return Err(Error::Runtime {
                                msg: format!("cannot access field on {}", obj.type_name()),
                                stack_trace: Vec::new(),
                            });
                        }
                    }
                }

                Opcode::StoreField(_) => {
                    let _field_idx = self.read_u16() as usize;
                    let val = self.stack.pop().unwrap();
                    let obj = self.stack.pop().unwrap();
                    match &obj {
                        Value::Struct(map) => {
                            let key = format!("_{}", _field_idx);
                            map.borrow_mut().insert(key, val);
                            self.stack.push(obj);
                        }
                        Value::Foreign(_r) => {
                            // TODO: field set on foreign objects
                            self.stack.push(Value::Nil);
                        }
                        _ => {
                            return Err(Error::Runtime {
                                msg: format!("cannot set field on {}", obj.type_name()),
                                stack_trace: Vec::new(),
                            });
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
                        _ => {
                            return Err(Error::Runtime {
                                msg: format!("cannot index {} with {}", obj.type_name(), index.type_name()),
                                stack_trace: Vec::new(),
                            });
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
                            return Err(Error::Runtime {
                                msg: format!("cannot index {} with {}", obj.type_name(), index.type_name()),
                                stack_trace: Vec::new(),
                            });
                        }
                    }
                }

                Opcode::NewClosure(_, _) => {
                    let _fn_idx = self.read_u16() as usize;
                    let _up_count = self.read_u16() as usize;
                    self.stack.push(Value::Function(_fn_idx));
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
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn run(source: &str) -> Value {
        let tokens = Lexer::new(source).tokenize().unwrap();
        let parser = Parser::new(&tokens);
        let mut program = parser.parse().unwrap();
        let mut symbols = crate::resolver::resolve(&mut program).unwrap();
        let types = crate::typeck::check(&program, &mut symbols).unwrap();
        let fns = compiler::compile(&program, &types, &symbols).unwrap();
        let mut vm = VM::new();
        vm.load_bytecode(fns);
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
