use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::error::Result;

/// A native Rust function that can be called from Zenlang.
pub type NativeFn = Rc<dyn Fn(&mut crate::vm::VMContext, &[Value]) -> Result<Value>>;

pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Rc<str>),
    Array(Rc<RefCell<Vec<Value>>>),
    Struct(Rc<RefCell<HashMap<String, Value>>>),
    Enum { tag: u16, data: Rc<RefCell<Vec<Value>>> },
    Function(usize),
    NativeFunction(NativeFn),
    Foreign(Rc<RefCell<dyn Any>>),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "Nil"),
            Value::Bool(b) => write!(f, "Bool({})", b),
            Value::Int(n) => write!(f, "Int({})", n),
            Value::Float(n) => write!(f, "Float({})", n),
            Value::Str(s) => write!(f, "Str({:?})", s),
            Value::Array(_) => write!(f, "Array(...)"),
            Value::Struct(_) => write!(f, "Struct(...)"),
            Value::Enum { tag, .. } => write!(f, "Enum({})", tag),
            Value::Function(idx) => write!(f, "Function({})", idx),
            Value::NativeFunction(_) => write!(f, "NativeFunction(...)"),
            Value::Foreign(_) => write!(f, "Foreign(...)"),
        }
    }
}

impl Clone for Value {
    fn clone(&self) -> Self {
        match self {
            Value::Nil => Value::Nil,
            Value::Bool(b) => Value::Bool(*b),
            Value::Int(n) => Value::Int(*n),
            Value::Float(n) => Value::Float(*n),
            Value::Str(s) => Value::Str(s.clone()),
            Value::Array(a) => Value::Array(a.clone()),
            Value::Struct(s) => Value::Struct(s.clone()),
            Value::Enum { tag, data } => Value::Enum { tag: *tag, data: data.clone() },
            Value::Function(idx) => Value::Function(*idx),
            Value::NativeFunction(f) => Value::NativeFunction(f.clone()),
            Value::Foreign(r) => Value::Foreign(r.clone()),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a.as_ref() == b.as_ref(),
            (Value::Array(a), Value::Array(b)) => Rc::ptr_eq(a, b),
            (Value::Struct(a), Value::Struct(b)) => Rc::ptr_eq(a, b),
            (Value::Enum { tag: ta, data: da }, Value::Enum { tag: tb, data: db }) => {
                ta == tb && Rc::ptr_eq(da, db)
            }
            (Value::Function(a), Value::Function(b)) => a == b,
            _ => false,
        }
    }
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "str",
            Value::Array(_) => "array",
            Value::Struct(_) => "struct",
            Value::Enum { .. } => "enum",
            Value::Function(_) => "function",
            Value::NativeFunction(_) => "native_function",
            Value::Foreign(_) => "foreign",
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Nil => false,
            Value::Bool(b) => *b,
            _ => true,
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.into())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s.into())
    }
}
