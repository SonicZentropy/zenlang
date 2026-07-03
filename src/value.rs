use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

type CloneFn = Rc<dyn Fn(&dyn Any) -> Box<dyn Any>>;

use crate::error::Result;
use crate::slab::Handle;

/// A native Rust function that can be called from Zenlang.
pub type NativeFn = Rc<dyn Fn(&mut crate::vm::VMContext, &[Value]) -> Result<Value>>;

// ── Handle-based heap data types (no Rc/RefCell) ──────────────────────

/// A mutable array of values.
pub struct ArrayData {
    pub values: Vec<Value>,
}

impl Clone for ArrayData {
    fn clone(&self) -> Self {
        Self { values: self.values.clone() }
    }
}

/// A struct value with named fields.
pub struct StructData {
    pub values: Vec<Value>,
    pub field_names: Vec<String>,
}

impl Clone for StructData {
    fn clone(&self) -> Self {
        Self { values: self.values.clone(), field_names: self.field_names.clone() }
    }
}

impl StructData {
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.field_names.iter().position(|n| n == name)
    }
    pub fn get_field(&self, name: &str) -> Option<&Value> {
        self.field_index(name).map(|i| &self.values[i])
    }
    pub fn get_field_mut(&mut self, name: &str) -> Option<&mut Value> {
        self.field_index(name).map(move |i| &mut self.values[i])
    }
}

/// An enum value with tag and field data.
pub struct EnumData {
    pub tag: u16,
    pub fields: Vec<Value>,
}

impl Clone for EnumData {
    fn clone(&self) -> Self {
        Self { tag: self.tag, fields: self.fields.clone() }
    }
}

/// A closure with captured upvalues.
pub struct ClosureData {
    pub fn_idx: usize,
    pub upvalues: Vec<Value>,
}

impl Clone for ClosureData {
    fn clone(&self) -> Self {
        Self { fn_idx: self.fn_idx, upvalues: self.upvalues.clone() }
    }
}

/// Saved execution state for a suspended generator.
pub struct GeneratorState {
    pub function_idx: usize,
    pub ip: usize,
    pub first_call: bool,
    pub exhausted: bool,
    pub locals: Vec<Value>,
}

impl Clone for GeneratorState {
    fn clone(&self) -> Self {
        Self {
            function_idx: self.function_idx,
            ip: self.ip,
            first_call: self.first_call,
            exhausted: self.exhausted,
            locals: self.locals.clone(),
        }
    }
}

/// A mutable key-value map.
pub struct MapData {
    pub entries: HashMap<MapKey, Value>,
}

impl Clone for MapData {
    fn clone(&self) -> Self {
        Self { entries: self.entries.clone() }
    }
}

/// Weak reference target kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeakKind {
    Struct,
    Array,
    Map,
}

/// A weak reference to a heap-allocated value.
pub struct WeakData {
    pub kind: WeakKind,
    pub target: Handle,
    pub type_name: String,
}

impl Clone for WeakData {
    fn clone(&self) -> Self {
        Self { kind: self.kind, target: self.target, type_name: self.type_name.clone() }
    }
}

/// Wrapper around a foreign (Rust) object.
pub struct ForeignObject {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub data: Box<dyn Any>,
    clone_fn: CloneFn,
}

impl ForeignObject {
    pub fn new<T: 'static + Clone>(type_name: &'static str, data: T) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            type_name,
            data: Box::new(data),
            clone_fn: Rc::new(move |any: &dyn Any| {
                let typed = any.downcast_ref::<T>().expect("ForeignObject clone: type mismatch");
                Box::new(typed.clone())
            }),
        }
    }

    pub fn downcast<T: 'static>(&self) -> Option<&T> {
        self.data.downcast_ref::<T>()
    }

    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.data.downcast_mut::<T>()
    }
}

impl fmt::Debug for ForeignObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Foreign({})", self.type_name)
    }
}

impl Clone for ForeignObject {
    fn clone(&self) -> Self {
        Self {
            type_id: self.type_id,
            type_name: self.type_name,
            data: (self.clone_fn)(&*self.data),
            clone_fn: self.clone_fn.clone(),
        }
    }
}

// ── MapKey ────────────────────────────────────────────────────────────

/// A hashable key for `Value::Map`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MapKey {
    Int(i64),
    Str(Rc<str>),
    Bool(bool),
}

impl MapKey {
    pub fn from_value(v: &Value) -> Option<MapKey> {
        match v {
            Value::Int(n) => Some(MapKey::Int(*n)),
            Value::Str(s) => Some(MapKey::Str(s.clone())),
            Value::Bool(b) => Some(MapKey::Bool(*b)),
            _ => None,
        }
    }

    pub fn to_value(&self) -> Value {
        match self {
            MapKey::Int(n) => Value::Int(*n),
            MapKey::Str(s) => Value::Str(s.clone()),
            MapKey::Bool(b) => Value::Bool(*b),
        }
    }
}

// ── Value ─────────────────────────────────────────────────────────────

/// A runtime value in the Zenlang VM.
///
/// Mutable heap objects (arrays, structs, enums, maps, closures, generators,
/// weak refs, foreign objects) are referenced by [`Handle`] — an index into
/// a slab owned by the VM. This eliminates `Rc`/`RefCell` overhead for all
/// mutable values.
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Rc<str>),
    Array(Handle),
    Struct(Handle, String),
    Enum(Handle),
    Function(usize),
    NativeFunction(NativeFn),
    Foreign(Handle),
    Closure(Handle),
    Range(i64, i64, bool),
    Map(Handle),
    Weak(Handle),
    Generator(Handle),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "Nil"),
            Value::Bool(b) => write!(f, "Bool({})", b),
            Value::Int(n) => write!(f, "Int({})", n),
            Value::Float(n) => write!(f, "Float({})", n),
            Value::Str(s) => write!(f, "Str({:?})", s),
            Value::Array(h) => write!(f, "Array({})", h.index),
            Value::Struct(h, name) => write!(f, "Struct({}, {})", h.index, name),
            Value::Enum(h) => write!(f, "Enum({})", h.index),
            Value::Function(idx) => write!(f, "Function({})", idx),
            Value::NativeFunction(_) => write!(f, "NativeFunction(...)"),
            Value::Foreign(h) => write!(f, "Foreign({})", h.index),
            Value::Closure(h) => write!(f, "Closure({})", h.index),
            Value::Range(s, e, inc) => write!(f, "Range({}, {}, {})", s, e, inc),
            Value::Map(h) => write!(f, "Map({})", h.index),
            Value::Weak(h) => write!(f, "Weak({})", h.index),
            Value::Generator(h) => write!(f, "Generator({})", h.index),
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
            Value::Array(h) => Value::Array(*h),
            Value::Struct(h, n) => Value::Struct(*h, n.clone()),
            Value::Enum(h) => Value::Enum(*h),
            Value::Function(idx) => Value::Function(*idx),
            Value::NativeFunction(f) => Value::NativeFunction(f.clone()),
            Value::Foreign(h) => Value::Foreign(*h),
            Value::Closure(h) => Value::Closure(*h),
            Value::Range(s, e, inc) => Value::Range(*s, *e, *inc),
            Value::Map(h) => Value::Map(*h),
            Value::Weak(h) => Value::Weak(*h),
            Value::Generator(h) => Value::Generator(*h),
        }
    }
}

/// Identity-based equality for heap values: two handles are equal only if
/// they point to the same slab slot. Primitive values compare structurally.
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a.as_ref() == b.as_ref(),
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Struct(a, an), Value::Struct(b, bn)) => an == bn && a == b,
            (Value::Enum(a), Value::Enum(b)) => a == b,
            (Value::Function(a), Value::Function(b)) => a == b,
            (Value::Foreign(a), Value::Foreign(b)) => a == b,
            (Value::Closure(a), Value::Closure(b)) => a == b,
            (Value::Range(a, b, c), Value::Range(d, e, f)) => a == d && b == e && c == f,
            (Value::Map(a), Value::Map(b)) => a == b,
            (Value::Weak(a), Value::Weak(b)) => a == b,
            (Value::Generator(a), Value::Generator(b)) => a == b,
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
            Value::Struct(_, _) => "struct",
            Value::Enum(_) => "enum",
            Value::Function(_) => "function",
            Value::NativeFunction(_) => "native_function",
            Value::Foreign(_) => "foreign",
            Value::Closure(_) => "closure",
            Value::Range(..) => "range",
            Value::Map(_) => "map",
            Value::Weak(_) => "weak",
            Value::Generator(_) => "generator",
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
        match self { Value::Int(n) => Some(*n), _ => None }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(n) => Some(*n),
            Value::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self { Value::Bool(b) => Some(*b), _ => None }
    }

    pub fn as_str(&self) -> Option<String> {
        match self { Value::Str(s) => Some(s.to_string()), _ => None }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self { Value::Str(s.into()) }
}

impl From<String> for Value {
    fn from(s: String) -> Self { Value::Str(s.into()) }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self { Value::Int(n) }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self { Value::Float(n) }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self { Value::Bool(b) }
}

impl TryFrom<Value> for i64 {
    type Error = crate::error::Error;
    fn try_from(val: Value) -> crate::error::Result<i64> {
        val.as_int().ok_or_else(|| crate::error::Error::Runtime {
            msg: format!("expected integer, got {}", val.type_name()),
            stack_trace: Vec::new(),
        })
    }
}

impl TryFrom<Value> for f64 {
    type Error = crate::error::Error;
    fn try_from(val: Value) -> crate::error::Result<f64> {
        val.as_float().ok_or_else(|| crate::error::Error::Runtime {
            msg: format!("expected float, got {}", val.type_name()),
            stack_trace: Vec::new(),
        })
    }
}

impl TryFrom<Value> for bool {
    type Error = crate::error::Error;
    fn try_from(val: Value) -> crate::error::Result<bool> {
        val.as_bool().ok_or_else(|| crate::error::Error::Runtime {
            msg: format!("expected boolean, got {}", val.type_name()),
            stack_trace: Vec::new(),
        })
    }
}

impl TryFrom<Value> for String {
    type Error = crate::error::Error;
    fn try_from(val: Value) -> crate::error::Result<String> {
        val.as_str().ok_or_else(|| crate::error::Error::Runtime {
            msg: format!("expected string, got {}", val.type_name()),
            stack_trace: Vec::new(),
        })
    }
}
