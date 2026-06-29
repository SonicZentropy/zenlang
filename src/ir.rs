use crate::value::Value;

/// Bytecode opcodes for the Zenlang VM.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Opcode {
    LoadConst(u16),
    LoadLocal(u16),
    StoreLocal(u16),
    LoadGlobal(u16),
    StoreGlobal(u16),
    Pop,
    Dup,
    And,
    Or,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Not,
    Jump(u16),
    JumpIfFalse(u16),
    Loop(u16),
    Call(u16),
    CallMethod(u16, u16),
    Return,
    MakeStruct(u16),
    MakeArray(u16),
    MakeEnum(u16, u16),
    LoadField(u16),
    StoreField(u16),
    LoadIndex,
    StoreIndex,
    NewClosure(u16, u16),
    Len,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    BitNot,
    Halt,
}

impl Opcode {
    pub fn to_byte(&self) -> u8 {
        use Opcode::*;
        match self {
            LoadConst(_) => 0,
            LoadLocal(_) => 1,
            StoreLocal(_) => 2,
            LoadGlobal(_) => 3,
            StoreGlobal(_) => 4,
            Pop => 5,
            Dup => 6,
            And => 7,
            Or => 8,
            Add => 9,
            Sub => 10,
            Mul => 11,
            Div => 12,
            Mod => 13,
            Neg => 14,
            Eq => 15,
            Ne => 16,
            Lt => 17,
            Le => 18,
            Gt => 19,
            Ge => 20,
            Not => 21,
            Jump(_) => 22,
            JumpIfFalse(_) => 23,
            Loop(_) => 24,
            Call(_) => 25,
            CallMethod(_, _) => 26,
            Return => 27,
            MakeStruct(_) => 28,
            MakeArray(_) => 29,
            MakeEnum(_, _) => 30,
            LoadField(_) => 31,
            StoreField(_) => 32,
            LoadIndex => 33,
            StoreIndex => 34,
            NewClosure(_, _) => 35,
            Len => 36,
            BitAnd => 37,
            BitOr => 38,
            BitXor => 39,
            Shl => 40,
            Shr => 41,
            BitNot => 42,
            Halt => 43,
        }
    }

    pub fn from_byte(b: u8) -> Option<Opcode> {
        use Opcode::*;
        Some(match b {
            0 => LoadConst(0),
            1 => LoadLocal(0),
            2 => StoreLocal(0),
            3 => LoadGlobal(0),
            4 => StoreGlobal(0),
            5 => Pop,
            6 => Dup,
            7 => And,
            8 => Or,
            9 => Add,
            10 => Sub,
            11 => Mul,
            12 => Div,
            13 => Mod,
            14 => Neg,
            15 => Eq,
            16 => Ne,
            17 => Lt,
            18 => Le,
            19 => Gt,
            20 => Ge,
            21 => Not,
            22 => Jump(0),
            23 => JumpIfFalse(0),
            24 => Loop(0),
            25 => Call(0),
            26 => CallMethod(0, 0),
            27 => Return,
            28 => MakeStruct(0),
            29 => MakeArray(0),
            30 => MakeEnum(0, 0),
            31 => LoadField(0),
            32 => StoreField(0),
            33 => LoadIndex,
            34 => StoreIndex,
            35 => NewClosure(0, 0),
            36 => Len,
            37 => BitAnd,
            38 => BitOr,
            39 => BitXor,
            40 => Shl,
            41 => Shr,
            42 => BitNot,
            43 => Halt,
            _ => return None,
        })
    }

    pub fn operand_count(&self) -> usize {
        match self {
            Opcode::LoadConst(_) | Opcode::LoadLocal(_) | Opcode::StoreLocal(_)
            | Opcode::LoadGlobal(_) | Opcode::StoreGlobal(_)
            | Opcode::Jump(_) | Opcode::JumpIfFalse(_) | Opcode::Loop(_)
            | Opcode::Call(_) | Opcode::MakeStruct(_) | Opcode::MakeArray(_)
            | Opcode::LoadField(_) | Opcode::StoreField(_)
            | Opcode::NewClosure(_, _) => 1,
            Opcode::MakeEnum(_, _) | Opcode::CallMethod(_, _) => 2,
            _ => 0,
        }
    }
}

/// A chunk of bytecode with a constant pool and line number mapping.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    pub locals: u32,
    pub field_names: Vec<String>,
    pub method_names: Vec<String>,
    /// Line number for each byte of code (parallel to `code`).
    pub lines: Vec<usize>,
}

impl Chunk {
    pub fn new() -> Self {
        Self { code: Vec::new(), constants: Vec::new(), locals: 0, field_names: Vec::new(), method_names: Vec::new(), lines: Vec::new() }
    }

    pub fn add_constant(&mut self, val: Value) -> u16 {
        let idx = self.constants.len() as u16;
        self.constants.push(val);
        idx
    }

    pub fn add_field_name(&mut self, name: &str) -> u16 {
        for (i, n) in self.field_names.iter().enumerate() {
            if n == name {
                return i as u16;
            }
        }
        let idx = self.field_names.len() as u16;
        self.field_names.push(name.to_string());
        idx
    }

    pub fn add_method_name(&mut self, name: &str) -> u16 {
        for (i, n) in self.method_names.iter().enumerate() {
            if n == name {
                return i as u16;
            }
        }
        let idx = self.method_names.len() as u16;
        self.method_names.push(name.to_string());
        idx
    }

    pub fn emit_byte(&mut self, byte: u8, line: usize) {
        self.code.push(byte);
        self.lines.push(line);
    }

    pub fn emit_u16(&mut self, val: u16, line: usize) {
        let bytes = val.to_le_bytes();
        self.code.extend_from_slice(&bytes);
        self.lines.extend_from_slice(&[line, line]);
    }

    pub fn emit_op(&mut self, op: Opcode, line: usize) {
        self.emit_byte(op.to_byte(), line);
        match op {
            Opcode::LoadConst(v) | Opcode::LoadLocal(v) | Opcode::StoreLocal(v)
            | Opcode::LoadGlobal(v) | Opcode::StoreGlobal(v)
            | Opcode::Jump(v) | Opcode::JumpIfFalse(v) | Opcode::Loop(v)
            | Opcode::Call(v) | Opcode::MakeStruct(v) | Opcode::MakeArray(v)
            | Opcode::LoadField(v) | Opcode::StoreField(v) => self.emit_u16(v, line),
            Opcode::And | Opcode::Or | Opcode::BitAnd | Opcode::BitOr | Opcode::BitXor
            | Opcode::Shl | Opcode::Shr | Opcode::BitNot => {}
            Opcode::MakeEnum(t, f) => { self.emit_u16(t, line); self.emit_u16(f, line); }
            Opcode::CallMethod(m, a) => { self.emit_u16(m, line); self.emit_u16(a, line); }
            Opcode::NewClosure(f, n) => { self.emit_u16(f, line); self.emit_u16(n, line); }
            _ => {}
        }
    }

    /// Get the source line for a given bytecode offset.
    pub fn get_line(&self, offset: usize) -> usize {
        self.lines.get(offset).copied().unwrap_or(0)
    }

    /// Patch a u16 value at a given offset (for backpatching jumps).
    pub fn patch_u16(&mut self, offset: usize, val: u16) {
        let bytes = val.to_le_bytes();
        self.code[offset] = bytes[0];
        self.code[offset + 1] = bytes[1];
    }

    pub fn read_u16(&self, offset: usize) -> u16 {
        u16::from_le_bytes([self.code[offset], self.code[offset + 1]])
    }

    pub fn read_u16_static(code: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes([code[offset], code[offset + 1]])
    }

    /// Total size of the chunk's bytecode in bytes.
    pub fn len(&self) -> usize {
        self.code.len()
    }

    /// Decode a single instruction at `offset`, returning (opcode, next_offset).
    fn decode_at(&self, offset: usize) -> Option<(Opcode, usize)> {
        let byte = *self.code.get(offset)?;
        let op = Opcode::from_byte(byte)?;
        let mut off = offset + 1;
        match op {
            Opcode::MakeEnum(_, _) | Opcode::CallMethod(_, _) | Opcode::NewClosure(_, _) => {
                off += 4;
            }
            Opcode::LoadConst(_) | Opcode::LoadLocal(_) | Opcode::StoreLocal(_)
            | Opcode::LoadGlobal(_) | Opcode::StoreGlobal(_)
            | Opcode::Jump(_) | Opcode::JumpIfFalse(_) | Opcode::Loop(_)
            | Opcode::Call(_) | Opcode::MakeStruct(_) | Opcode::MakeArray(_)
            | Opcode::LoadField(_) | Opcode::StoreField(_) => {
                off += 2;
            }
            _ => {}
        }
        Some((op, off))
    }

    /// Print the disassembly of this chunk to stdout.
    pub fn disassemble(&self, name: &str) {
        println!("== {} ==", name);
        let mut offset = 0;
        while offset < self.code.len() {
            let line = self.get_line(offset);
            let (op, next) = match self.decode_at(offset) {
                Some(v) => v,
                None => break,
            };
            print!("{:04x}  {:>4}  ", offset, line);
            match op {
                Opcode::LoadConst(idx) => {
                    let val = &self.constants[idx as usize];
                    println!("LoadConst  {:>4}  '{}'", idx, format_val(val));
                }
                Opcode::LoadLocal(idx) => println!("LoadLocal  {:>4}", idx),
                Opcode::StoreLocal(idx) => println!("StoreLocal {:>4}", idx),
                Opcode::LoadGlobal(idx) => println!("LoadGlobal {:>4}", idx),
                Opcode::StoreGlobal(idx) => println!("StoreGlobal{:>4}", idx),
                Opcode::Call(idx) => println!("Call       {:>4}", idx),
                Opcode::Jump(offset_val) => println!("Jump       {:>4} -> {:04x}", offset_val, next as u16 + offset_val),
                Opcode::JumpIfFalse(offset_val) => println!("JumpIfFalse{:>4} -> {:04x}", offset_val, next as u16 + offset_val),
                Opcode::Loop(offset_val) => println!("Loop       {:>4} -> {:04x}", offset_val, next.wrapping_sub(offset_val as usize + 1)),
                Opcode::MakeStruct(count) => println!("MakeStruct {:>4}", count),
                Opcode::MakeArray(count) => println!("MakeArray  {:>4}", count),
                Opcode::MakeEnum(tag, data) => println!("MakeEnum   tag={:>4} data={:>4}", tag, data),
                Opcode::CallMethod(method, args) => {
                    let method_name = self.method_names.get(method as usize).map(|s| s.as_str()).unwrap_or("?");
                    println!("CallMethod {:>4} '{}' args={}", method, method_name, args);
                }
                Opcode::LoadField(idx) => {
                    let field_name = self.field_names.get(idx as usize).map(|s| s.as_str()).unwrap_or("?");
                    println!("LoadField  {:>4} '{}'", idx, field_name);
                }
                Opcode::StoreField(idx) => {
                    let field_name = self.field_names.get(idx as usize).map(|s| s.as_str()).unwrap_or("?");
                    println!("StoreField {:>4} '{}'", idx, field_name);
                }
                Opcode::NewClosure(fn_idx, up_count) => println!("NewClosure fn={:>4} up={}", fn_idx, up_count),
                Opcode::Return => println!("Return"),
                Opcode::Pop => println!("Pop"),
                Opcode::Dup => println!("Dup"),
                Opcode::And => println!("And"),
                Opcode::Or => println!("Or"),
                Opcode::Add => println!("Add"),
                Opcode::Sub => println!("Sub"),
                Opcode::Mul => println!("Mul"),
                Opcode::Div => println!("Div"),
                Opcode::Mod => println!("Mod"),
                Opcode::Neg => println!("Neg"),
                Opcode::Eq => println!("Eq"),
                Opcode::Ne => println!("Ne"),
                Opcode::Lt => println!("Lt"),
                Opcode::Le => println!("Le"),
                Opcode::Gt => println!("Gt"),
                Opcode::Ge => println!("Ge"),
                Opcode::Not => println!("Not"),
                Opcode::LoadIndex => println!("LoadIndex"),
                Opcode::StoreIndex => println!("StoreIndex"),
                Opcode::Len => println!("Len"),
                Opcode::BitAnd => println!("BitAnd"),
                Opcode::BitOr => println!("BitOr"),
                Opcode::BitXor => println!("BitXor"),
                Opcode::Shl => println!("Shl"),
                Opcode::Shr => println!("Shr"),
                Opcode::BitNot => println!("BitNot"),
                Opcode::Halt => println!("Halt"),
            }
            offset = next;
        }
    }
}

/// Format a constant value for disassembly display.
fn format_val(val: &Value) -> String {
    match val {
        Value::Nil => "nil".into(),
        Value::Bool(b) => format!("{b}"),
        Value::Int(i) => format!("{i}"),
        Value::Float(f) => format!("{f}"),
        Value::Str(s) => format!("\"{s}\""),
        Value::Function(idx) => format!("fn<{idx}>"),
        Value::NativeFunction(_) => "<native>".into(),
        _ => "{..}".into(),
    }
}

/// Description of an upvalue (captured variable from enclosing scope).
#[derive(Debug, Clone)]
pub struct UpvalueDesc {
    pub is_local: bool,
    pub index: u32,
}

/// A compiled function.
#[derive(Debug, Clone)]
pub struct BytecodeFn {
    pub chunk: Chunk,
    pub upvalues: Vec<UpvalueDesc>,
    pub name: String,
    pub arity: u32,
}

impl BytecodeFn {
    pub fn new(name: String, arity: u32) -> Self {
        Self { chunk: Chunk::new(), upvalues: Vec::new(), name, arity }
    }

    /// Print the disassembly of this function to stdout.
    pub fn disassemble(&self) {
        self.chunk.disassemble(&self.name);
    }
}
