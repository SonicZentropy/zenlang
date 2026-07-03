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

/// Saved execution state for a suspended generator (coroutine).
#[derive(Debug, Clone)]
pub struct GeneratorState {
    /// Which function this generator is executing.
    pub function_idx: usize,
    /// Next instruction to execute (saved on yield).
    pub ip: usize,
    /// If `true`, first_call — no saved locals yet.
    pub first_call: bool,
    /// If `true`, the generator has finished (returned, not yielded).
    pub exhausted: bool,
    /// Saved local variables (populated after first yield).
    pub locals: Vec<Value>,
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
    /// Wrap a Rust value as a foreign object with the given type name.
    pub fn new<T: 'static>(type_name: &'static str, data: T) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            type_name,
            data: Rc::new(RefCell::new(data)),
        }
    }

    /// Try to borrow the inner data as `T`. Returns `None` if the type doesn't match.
    pub fn downcast<T: 'static>(&self) -> Option<std::cell::Ref<'_, T>> {
        let r = self.data.borrow();
        if (*r).is::<T>() {
            Some(std::cell::Ref::map(r, |d| d.downcast_ref::<T>().unwrap()))
        } else {
            None
        }
    }

    /// Try to mutably borrow the inner data as `T`. Returns `None` if the type doesn't match.
    pub fn downcast_mut<T: 'static>(&self) -> Option<std::cell::RefMut<'_, T>> {
        let r = self.data.borrow_mut();
        if (*r).is::<T>() {
            Some(std::cell::RefMut::map(r, |d| {
                d.downcast_mut::<T>().unwrap()
            }))
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

/// A hashable key for `Value::Map`. Only a subset of `Value` variants can be
/// used as map keys (ints, strings, bools) — floats, arrays, structs, etc.
/// don't have stable hashing/equality suitable for map keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MapKey {
    Int(i64),
    Str(Rc<str>),
    Bool(bool),
}

impl MapKey {
    /// Convert a `Value` into a `MapKey`, if it's a supported key type.
    pub fn from_value(v: &Value) -> Option<MapKey> {
        match v {
            Value::Int(n) => Some(MapKey::Int(*n)),
            Value::Str(s) => Some(MapKey::Str(s.clone())),
            Value::Bool(b) => Some(MapKey::Bool(*b)),
            _ => None,
        }
    }

    /// Convert this key back into a `Value` (e.g. for `map_keys`/iteration).
    pub fn to_value(&self) -> Value {
        match self {
            MapKey::Int(n) => Value::Int(*n),
            MapKey::Str(s) => Value::Str(s.clone()),
            MapKey::Bool(b) => Value::Bool(*b),
        }
    }
}

/// A runtime value in the Zenlang VM.
///
/// All values are boxed — integers, floats, strings, arrays, structs, enums,
/// functions, foreign objects, and closures. The VM operates uniformly on `Value`,
/// which enables type-erased generics without monomorphization.
///
/// ```rust
/// use zenlang::Value;
///
/// let v = Value::Int(42);
/// assert_eq!(v.as_int(), Some(42));
/// assert_eq!(v.type_name(), "int");
/// assert!(v.is_truthy());
///
/// let s: Value = "hello".into();
/// assert_eq!(s.as_str(), Some("hello".to_string()));
/// ```
pub enum Value {
    /// `nil` — the absence of a value.
    Nil,
    /// `true` or `false`.
    Bool(bool),
    /// 64-bit signed integer.
    Int(i64),
    /// 64-bit float (also stores `f32` values from the source).
    Float(f64),
    /// Reference-counted string. Strings are immutable.
    Str(Rc<str>),
    /// A mutable array of values.
    Array(Rc<RefCell<Vec<Value>>>),
    /// A struct with named fields. The `String` holds the struct type name for method dispatch.
    Struct(Rc<RefCell<HashMap<String, Value>>>, String),
    /// An enum value with a tag (variant index) and field data.
    Enum {
        tag: u16,
        data: Rc<RefCell<Vec<Value>>>,
    },
    /// A compiled script function identified by its index into the VM's function table.
    Function(usize),
    /// A native Rust function registered via the interop system.
    NativeFunction(NativeFn),
    /// A foreign (Rust) object registered via the type registry.
    Foreign(Rc<RefCell<ForeignObject>>),
    /// A closure with captured upvalues.
    Closure(Rc<RefCell<ClosureData>>),
    /// A range `start..end` (exclusive) or `start..=end` (inclusive).
    Range(i64, i64, bool),
    /// A mutable key-value map. Keys are restricted to `int`, `str`, and
    /// `bool` (see `MapKey`) since `float`/`array`/`struct` etc. don't have
    /// stable hashing/equality suitable for map keys.
    Map(Rc<RefCell<HashMap<MapKey, Value>>>),
    /// A suspended generator (coroutine) that yields values.
    Generator(Rc<RefCell<GeneratorState>>),
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
            Value::Struct(_, name) => write!(f, "Struct({})", name),
            Value::Enum { tag, .. } => write!(f, "Enum({})", tag),
            Value::Function(idx) => write!(f, "Function({})", idx),
            Value::NativeFunction(_) => write!(f, "NativeFunction(...)"),
            Value::Foreign(obj) => write!(f, "{:?}", obj.borrow()),
            Value::Closure(c) => write!(
                f,
                "Closure(fn={}, up_count={})",
                c.borrow().fn_idx,
                c.borrow().upvalues.len()
            ),
            Value::Range(s, e, inc) => write!(f, "Range({}, {}, {})", s, e, inc),
            Value::Map(m) => write!(f, "Map({} entries)", m.borrow().len()),
            Value::Generator(g) => {
                let state = g.borrow();
                if state.exhausted {
                    write!(f, "Generator(exhausted)")
                } else {
                    write!(f, "Generator(fn={}, suspended)", state.function_idx)
                }
            }
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
            Value::Struct(s, name) => Value::Struct(s.clone(), name.clone()),
            Value::Enum { tag, data } => Value::Enum {
                tag: *tag,
                data: data.clone(),
            },
            Value::Function(idx) => Value::Function(*idx),
            Value::NativeFunction(f) => Value::NativeFunction(f.clone()),
            Value::Foreign(r) => Value::Foreign(r.clone()),
            Value::Closure(c) => Value::Closure(c.clone()),
            Value::Range(s, e, inc) => Value::Range(*s, *e, *inc),
            Value::Map(m) => Value::Map(m.clone()),
            Value::Generator(g) => Value::Generator(g.clone()),
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
            (Value::Struct(a, an), Value::Struct(b, bn)) => an == bn && *a.borrow() == *b.borrow(),
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
            (Value::Range(a, b, c), Value::Range(d, e, f)) => a == d && b == e && c == f,
            (Value::Map(a), Value::Map(b)) => *a.borrow() == *b.borrow(),
            (Value::Generator(a), Value::Generator(b)) => {
                let ga = a.borrow();
                let gb = b.borrow();
                ga.function_idx == gb.function_idx && ga.ip == gb.ip && ga.exhausted == gb.exhausted
            }
            _ => false,
        }
    }
}

impl Value {
    /// Return a human-readable name for this value's runtime type.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "str",
            Value::Array(_) => "array",
            Value::Struct(_, _) => "struct",
            Value::Enum { .. } => "enum",
            Value::Function(_) => "function",
            Value::NativeFunction(_) => "native_function",
            Value::Foreign(obj) => obj.borrow().type_name,
            Value::Closure(_) => "closure",
            Value::Range(..) => "range",
            Value::Map(_) => "map",
            Value::Generator(_) => "generator",
        }
    }

    /// Returns `true` if this value is truthy: `nil` is false, `bool` defers to the value, everything else is true.
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Nil => false,
            Value::Bool(b) => *b,
            _ => true,
        }
    }

    /// Extract the value as an `i64`. Returns `None` if it's not an `Int`.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// Extract the value as an `f64`. Ints are implicitly convertible to float. Returns `None` otherwise.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(n) => Some(*n),
            Value::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    /// Extract the value as a `bool`. Returns `None` if it's not a `Bool`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Extract the value as a `String`. Returns `None` if it's not a `Str`.
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
