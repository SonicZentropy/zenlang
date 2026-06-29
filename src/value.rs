use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// Data stored in a closure value: function index and captured upvalues.
#[derive(Debug, Clone)]
pub struct ClosureData {
    pub fn_idx: usize,
    pub upvalues: Vec<Value>,
}

use crate::error::Result;

/// A native Rust function that can be called from Zenlang.
pub type NativeFn = Rc<dyn Fn(&mut crate::vm::VMContext, &[Value]) -> Result<Value>>;

/// Wrapper around a foreign (Rust) object stored in the VM.
pub struct ForeignObject {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub data: Rc<RefCell<dyn Any>>,
}

impl ForeignObject {
    pub fn new<T: 'static>(type_name: &'static str, data: T) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            type_name,
            data: Rc::new(RefCell::new(data)),
        }
    }

    pub fn downcast<T: 'static>(&self) -> Option<std::cell::Ref<'_, T>> {
        let r = self.data.borrow();
        if (*r).is::<T>() {
            Some(std::cell::Ref::map(r, |d| d.downcast_ref::<T>().unwrap()))
        } else {
            None
        }
    }

    pub fn downcast_mut<T: 'static>(&self) -> Option<std::cell::RefMut<'_, T>> {
        let r = self.data.borrow_mut();
        if (*r).is::<T>() {
            Some(std::cell::RefMut::map(r, |d| d.downcast_mut::<T>().unwrap()))
        } else {
            None
        }
    }
}

impl fmt::Debug for ForeignObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Foreign({})", self.type_name)
    }
}

impl Clone for ForeignObject {
    fn clone(&self) -> Self {
        // Create a new Rc pointing to the same data
        Self {
            type_id: self.type_id,
            type_name: self.type_name,
            data: self.data.clone(),
        }
    }
}

impl PartialEq for ForeignObject {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id && Rc::ptr_eq(&self.data, &other.data)
    }
}

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
    Foreign(Rc<RefCell<ForeignObject>>),
    Closure(Rc<RefCell<ClosureData>>),
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
            Value::Foreign(obj) => write!(f, "{:?}", obj.borrow()),
            Value::Closure(c) => write!(f, "Closure(fn={}, up_count={})", c.borrow().fn_idx, c.borrow().upvalues.len()),
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
            Value::Closure(c) => Value::Closure(c.clone()),
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
            (Value::Array(a), Value::Array(b)) => *a.borrow() == *b.borrow(),
            (Value::Struct(a), Value::Struct(b)) => *a.borrow() == *b.borrow(),
            (Value::Enum { tag: ta, data: da }, Value::Enum { tag: tb, data: db }) => {
                ta == tb && *da.borrow() == *db.borrow()
            }
            (Value::Function(a), Value::Function(b)) => a == b,
            (Value::Foreign(a), Value::Foreign(b)) => *a.borrow() == *b.borrow(),
            (Value::Closure(a), Value::Closure(b)) => {
                let ca = a.borrow();
                let cb = b.borrow();
                ca.fn_idx == cb.fn_idx && ca.upvalues == cb.upvalues
            }
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
            Value::Foreign(obj) => obj.borrow().type_name,
            Value::Closure(_) => "closure",
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Nil => false,
            Value::Bool(b) => *b,
            _ => true,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(n) => Some(*n),
            Value::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<String> {
        match self {
            Value::Str(s) => Some(s.to_string()),
            _ => None,
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
