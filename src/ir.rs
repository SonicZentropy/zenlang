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
            Halt => 36,
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
            36 => Halt,
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

/// A chunk of bytecode with a constant pool.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    pub locals: u32,
    /// Field name for each field index (used by LoadField/StoreField).
    pub field_names: Vec<String>,
    /// Method name for each method index (used by CallMethod).
    pub method_names: Vec<String>,
}

impl Chunk {
    pub fn new() -> Self {
        Self { code: Vec::new(), constants: Vec::new(), locals: 0, field_names: Vec::new(), method_names: Vec::new() }
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

    pub fn emit_byte(&mut self, byte: u8) {
        self.code.push(byte);
    }

    pub fn emit_u16(&mut self, val: u16) {
        self.code.extend_from_slice(&val.to_le_bytes());
    }

    pub fn emit_op(&mut self, op: Opcode) {
        self.emit_byte(op.to_byte());
        match op {
            Opcode::LoadConst(v) | Opcode::LoadLocal(v) | Opcode::StoreLocal(v)
            | Opcode::LoadGlobal(v) | Opcode::StoreGlobal(v)
            | Opcode::Jump(v) | Opcode::JumpIfFalse(v) | Opcode::Loop(v)
            | Opcode::Call(v) | Opcode::MakeStruct(v) | Opcode::MakeArray(v)
            | Opcode::LoadField(v) | Opcode::StoreField(v) => self.emit_u16(v),
            Opcode::And | Opcode::Or => {}
            Opcode::MakeEnum(t, f) => { self.emit_u16(t); self.emit_u16(f); }
            Opcode::CallMethod(m, a) => { self.emit_u16(m); self.emit_u16(a); }
            Opcode::NewClosure(f, n) => { self.emit_u16(f); self.emit_u16(n); }
            _ => {}
        }
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
}
