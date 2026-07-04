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
        Self {
            values: self.values.clone(),
        }
    }
}

/// A struct value with named fields.
pub struct StructData {
    pub values: Vec<Value>,
    pub field_names: Vec<String>,
}

impl Clone for StructData {
    fn clone(&self) -> Self {
        Self {
            values: self.values.clone(),
            field_names: self.field_names.clone(),
        }
    }
}

impl StructData {
    /// Find the index of a field by name.
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.field_names.iter().position(|n| n == name)
    }
    /// Get a reference to a field value by name.
    pub fn get_field(&self, name: &str) -> Option<&Value> {
        self.field_index(name).map(|i| &self.values[i])
    }
    /// Get a mutable reference to a field value by name.
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
        Self {
            tag: self.tag,
            fields: self.fields.clone(),
        }
    }
}

/// A closure with captured upvalues.
pub struct ClosureData {
    pub fn_idx: usize,
    pub upvalues: Vec<Value>,
}

impl Clone for ClosureData {
    fn clone(&self) -> Self {
        Self {
            fn_idx: self.fn_idx,
            upvalues: self.upvalues.clone(),
        }
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
        Self {
            entries: self.entries.clone(),
        }
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
        Self {
            kind: self.kind,
            target: self.target,
            type_name: self.type_name.clone(),
        }
    }
}

/// Wrapper around a foreign (Rust) object.
///
/// Stores a type-erased `Box<dyn Any>` together with a `TypeId` for safe
/// downcasting and a `clone_fn` closure that knows how to clone the inner
/// value without requiring `T: Clone` at the type-erased level.
///
/// # Example
/// ```ignore
/// struct Player { name: String }
/// let fo = ForeignObject::new("Player", Player { name: "Aria".into() });
/// let p: &Player = fo.downcast().unwrap();
/// assert_eq!(p.name, "Aria");
/// ```
pub struct ForeignObject {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub data: Box<dyn Any>,
    clone_fn: CloneFn,
}

impl ForeignObject {
    /// Create a new `ForeignObject` wrapping a value of type `T`.
    ///
    /// The type must implement `Clone` so the object can be cloned later.
    /// The `type_name` is a human-readable label used for diagnostics.
    pub fn new<T: 'static + Clone>(type_name: &'static str, data: T) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            type_name,
            data: Box::new(data),
            clone_fn: Rc::new(move |any: &dyn Any| {
                let typed = any
                    .downcast_ref::<T>()
                    .expect("ForeignObject clone: type mismatch");
                Box::new(typed.clone())
            }),
        }
    }

    /// Downcast the wrapped value to a concrete type `T`.
    ///
    /// Returns `None` if the type does not match.
    pub fn downcast<T: 'static>(&self) -> Option<&T> {
        self.data.downcast_ref::<T>()
    }

    /// Downcast the wrapped value to a mutable reference of type `T`.
    ///
    /// Returns `None` if the type does not match.
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
    /// Convert a `&Value` into a `MapKey` if the value is a supported key type
    /// (int, str, or bool). Returns `None` for other value variants.
    pub fn from_value(v: &Value) -> Option<MapKey> {
        match v {
            Value::Int(n) => Some(MapKey::Int(*n)),
            Value::Str(s) => Some(MapKey::Str(s.clone())),
            Value::Bool(b) => Some(MapKey::Bool(*b)),
            _ => None,
        }
    }

    /// Convert this `MapKey` back into a `Value`.
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
/// weak refs, foreign objects) are referenced by `Handle` — an index into
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
    /// Return the human-readable type name for this value.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// assert_eq!(Value::Nil.type_name(), "nil");
    /// assert_eq!(Value::Int(42).type_name(), "int");
    /// assert_eq!(Value::Str("hello".into()).type_name(), "str");
    /// ```
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

    /// Whether this value is truthy in Zenlang semantics:
    /// `nil` and `false` are falsy; everything else is truthy.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// assert!(!Value::Nil.is_truthy());
    /// assert!(!Value::Bool(false).is_truthy());
    /// assert!(Value::Bool(true).is_truthy());
    /// assert!(Value::Int(0).is_truthy());
    /// assert!(Value::Str("".into()).is_truthy());
    /// ```
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Nil => false,
            Value::Bool(b) => *b,
            _ => true,
        }
    }

    /// Extract the integer value if this is `Value::Int`.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// assert_eq!(Value::Int(42).as_int(), Some(42));
    /// assert_eq!(Value::Float(3.0).as_int(), None);
    /// ```
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// Extract the float value if this is `Value::Float`, or coerce from `Int`.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// assert_eq!(Value::Float(3.14).as_float(), Some(3.14));
    /// assert_eq!(Value::Int(42).as_float(), Some(42.0));
    /// assert_eq!(Value::Bool(true).as_float(), None);
    /// ```
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(n) => Some(*n),
            Value::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    /// Extract the boolean value if this is `Value::Bool`.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// assert_eq!(Value::Bool(true).as_bool(), Some(true));
    /// assert_eq!(Value::Int(1).as_bool(), None);
    /// ```
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Extract the string value as an owned `String` if this is `Value::Str`.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// let s: String = Value::Str("hello".into()).as_str().unwrap();
    /// assert_eq!(s, "hello");
    /// ```
    pub fn as_str(&self) -> Option<String> {
        match self {
            Value::Str(s) => Some(s.to_string()),
            _ => None,
        }
    }
}

impl From<&str> for Value {
    /// Convert a string slice into a `Value::Str`.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// let v: Value = "hello".into();
    /// assert_eq!(v.as_str(), Some("hello".into()));
    /// ```
    fn from(s: &str) -> Self {
        Value::Str(s.into())
    }
}

impl From<String> for Value {
    /// Convert an owned `String` into a `Value::Str`.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// let v: Value = String::from("world").into();
    /// assert_eq!(v.as_str(), Some("world".into()));
    /// ```
    fn from(s: String) -> Self {
        Value::Str(s.into())
    }
}

impl From<i64> for Value {
    /// Convert an `i64` into a `Value::Int`.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// let v: Value = 42i64.into();
    /// assert_eq!(v.as_int(), Some(42));
    /// ```
    fn from(n: i64) -> Self {
        Value::Int(n)
    }
}

impl From<f64> for Value {
    /// Convert an `f64` into a `Value::Float`.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// let v: Value = 3.14f64.into();
    /// assert_eq!(v.as_float(), Some(3.14));
    /// ```
    fn from(n: f64) -> Self {
        Value::Float(n)
    }
}

impl From<bool> for Value {
    /// Convert a `bool` into a `Value::Bool`.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// let v: Value = true.into();
    /// assert_eq!(v.as_bool(), Some(true));
    /// ```
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl TryFrom<Value> for i64 {
    type Error = crate::error::Error;
    /// Try to extract an `i64` from a `Value`. Returns a runtime error if the
    /// value is not `Value::Int`.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// let v = Value::Int(42);
    /// assert_eq!(i64::try_from(v).unwrap(), 42);
    /// ```
    fn try_from(val: Value) -> crate::error::Result<i64> {
        val.as_int().ok_or_else(|| crate::error::Error::Runtime {
            msg: format!("expected integer, got {}", val.type_name()),
            stack_trace: Vec::new(),
        })
    }
}

impl TryFrom<Value> for f64 {
    type Error = crate::error::Error;
    /// Try to extract an `f64` from a `Value`. Allows coercion from `Int`.
    /// Returns a runtime error if the value is neither `Float` nor `Int`.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// let v = Value::Float(2.5);
    /// assert!((f64::try_from(v).unwrap() - 2.5).abs() < 1e-10);
    /// ```
    fn try_from(val: Value) -> crate::error::Result<f64> {
        val.as_float().ok_or_else(|| crate::error::Error::Runtime {
            msg: format!("expected float, got {}", val.type_name()),
            stack_trace: Vec::new(),
        })
    }
}

impl TryFrom<Value> for bool {
    type Error = crate::error::Error;
    /// Try to extract a `bool` from a `Value`. Returns a runtime error if the
    /// value is not `Value::Bool`.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// let v = Value::Bool(false);
    /// assert!(!bool::try_from(v).unwrap());
    /// ```
    fn try_from(val: Value) -> crate::error::Result<bool> {
        val.as_bool().ok_or_else(|| crate::error::Error::Runtime {
            msg: format!("expected boolean, got {}", val.type_name()),
            stack_trace: Vec::new(),
        })
    }
}

impl TryFrom<Value> for String {
    type Error = crate::error::Error;
    /// Try to extract an owned `String` from a `Value`. Returns a runtime error
    /// if the value is not `Value::Str`.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::Value;
    /// let v = Value::Str("hello".into());
    /// assert_eq!(String::try_from(v).unwrap(), "hello");
    /// ```
    fn try_from(val: Value) -> crate::error::Result<String> {
        val.as_str().ok_or_else(|| crate::error::Error::Runtime {
            msg: format!("expected string, got {}", val.type_name()),
            stack_trace: Vec::new(),
        })
    }
}

/// Builder for constructing `StructData` with named fields.
///
/// # Example
/// ```ignore
/// let builder = StructBuilder::new("Point")
///     .field("x", 10i64)
///     .field("y", 20i64)
///     .field("label", "origin");
/// let (data, name) = (builder.build(), builder.into_name());
/// let h = vm.structs.insert(data);
/// let val = Value::Struct(h, name);
/// ```
pub struct StructBuilder {
    name: String,
    values: Vec<Value>,
    field_names: Vec<String>,
}

impl StructBuilder {
    /// Create a new `StructBuilder` with the given struct type name.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::StructBuilder;
    /// let builder = StructBuilder::new("Point");
    /// assert_eq!(builder.name(), "Point");
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            values: Vec::new(),
            field_names: Vec::new(),
        }
    }

    /// Add a named field with an auto-converted value.
    ///
    /// Accepts any type that implements `Into<Value>`: `i64`, `f64`, `bool`,
    /// `&str`, `String`, or `Value` directly.
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::StructBuilder;
    /// let builder = StructBuilder::new("Point")
    ///     .field("x", 10i64)
    ///     .field("y", 20i64);
    /// assert_eq!(builder.name(), "Point");
    /// ```
    pub fn field<V: Into<Value>>(mut self, name: impl Into<String>, value: V) -> Self {
        self.field_names.push(name.into());
        self.values.push(value.into());
        self
    }

    /// Consume the builder and return the constructed [`StructData`].
    ///
    /// # Example
    /// ```
    /// # use zenlang::value::{StructBuilder, StructData};
    /// let data = StructBuilder::new("Point")
    ///     .field("x", 10i64)
    ///     .build();
    /// assert_eq!(data.field_names, vec!["x"]);
    /// assert!(data.get_field("x").is_some());
    /// ```
    pub fn build(self) -> StructData {
        StructData {
            values: self.values,
            field_names: self.field_names,
        }
    }

    /// Borrow the struct type name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Consume the builder and return the struct type name.
    pub fn into_name(self) -> String {
        self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Value type_name ─────────────────────────────────────────────────

    #[test]
    fn test_type_name_nil() {
        assert_eq!(Value::Nil.type_name(), "nil");
    }
    #[test]
    fn test_type_name_int() {
        assert_eq!(Value::Int(42).type_name(), "int");
    }
    #[test]
    fn test_type_name_float() {
        assert_eq!(Value::Float(1.5).type_name(), "float");
    }
    #[test]
    fn test_type_name_bool() {
        assert_eq!(Value::Bool(true).type_name(), "bool");
    }
    #[test]
    fn test_type_name_str() {
        assert_eq!(Value::Str("a".into()).type_name(), "str");
    }

    // ── Value is_truthy ─────────────────────────────────────────────────

    #[test]
    fn test_is_truthy_nil_is_false() {
        assert!(!Value::Nil.is_truthy());
    }
    #[test]
    fn test_is_truthy_false_is_false() {
        assert!(!Value::Bool(false).is_truthy());
    }
    #[test]
    fn test_is_truthy_true_is_true() {
        assert!(Value::Bool(true).is_truthy());
    }
    #[test]
    fn test_is_truthy_zero_int_is_true() {
        assert!(Value::Int(0).is_truthy());
    }
    #[test]
    fn test_is_truthy_empty_str_is_true() {
        assert!(Value::Str("".into()).is_truthy());
    }

    // ── Value as_int / as_float / as_bool / as_str ──────────────────────

    #[test]
    fn test_as_int_matches() {
        assert_eq!(Value::Int(42).as_int(), Some(42));
    }
    #[test]
    fn test_as_int_non_int_is_none() {
        assert_eq!(Value::Float(3.0).as_int(), None);
        assert_eq!(Value::Bool(true).as_int(), None);
        assert_eq!(Value::Nil.as_int(), None);
    }
    #[test]
    fn test_as_float_float() {
        assert!((Value::Float(2.5).as_float().unwrap() - 2.5).abs() < 1e-10);
    }
    #[test]
    fn test_as_float_coerces_int() {
        assert_eq!(Value::Int(42).as_float(), Some(42.0));
    }
    #[test]
    fn test_as_float_non_float_is_none() {
        assert_eq!(Value::Bool(true).as_float(), None);
    }
    #[test]
    fn test_as_bool_matches() {
        assert_eq!(Value::Bool(false).as_bool(), Some(false));
        assert_eq!(Value::Bool(true).as_bool(), Some(true));
    }
    #[test]
    fn test_as_bool_non_bool_is_none() {
        assert_eq!(Value::Int(0).as_bool(), None);
    }
    #[test]
    fn test_as_str_matches() {
        assert_eq!(Value::Str("hi".into()).as_str(), Some("hi".into()));
    }
    #[test]
    fn test_as_str_non_str_is_none() {
        assert_eq!(Value::Int(0).as_str(), None);
    }

    // ── From<T> for Value ───────────────────────────────────────────────

    #[test]
    fn test_from_i64() {
        let v: Value = 42i64.into();
        assert_eq!(v.as_int(), Some(42));
    }
    #[test]
    fn test_from_f64() {
        let v: Value = 2.71f64.into();
        assert!((v.as_float().unwrap() - 2.71).abs() < 1e-10);
    }
    #[test]
    fn test_from_bool() {
        let v: Value = true.into();
        assert_eq!(v.as_bool(), Some(true));
    }
    #[test]
    fn test_from_str_slice() {
        let v: Value = "hello".into();
        assert_eq!(v.as_str(), Some("hello".into()));
    }
    #[test]
    fn test_from_string() {
        let v: Value = String::from("world").into();
        assert_eq!(v.as_str(), Some("world".into()));
    }

    // ── TryFrom<Value> for Rust types ───────────────────────────────────

    #[test]
    fn test_try_from_i64_ok() {
        assert_eq!(i64::try_from(Value::Int(99)).unwrap(), 99);
    }
    #[test]
    fn test_try_from_i64_err() {
        assert!(i64::try_from(Value::Bool(true)).is_err());
    }
    #[test]
    fn test_try_from_f64_ok() {
        assert!((f64::try_from(Value::Float(1.5)).unwrap() - 1.5).abs() < 1e-10);
    }
    #[test]
    fn test_try_from_f64_coerces_int() {
        assert_eq!(f64::try_from(Value::Int(7)).unwrap(), 7.0);
    }
    #[test]
    fn test_try_from_f64_err() {
        assert!(f64::try_from(Value::Nil).is_err());
    }
    #[test]
    fn test_try_from_bool_ok() {
        assert!(bool::try_from(Value::Bool(true)).unwrap());
    }
    #[test]
    fn test_try_from_bool_err() {
        assert!(bool::try_from(Value::Int(0)).is_err());
    }
    #[test]
    fn test_try_from_string_ok() {
        assert_eq!(String::try_from(Value::Str("abc".into())).unwrap(), "abc");
    }
    #[test]
    fn test_try_from_string_err() {
        assert!(String::try_from(Value::Int(0)).is_err());
    }

    // ── ForeignObject ───────────────────────────────────────────────────

    #[derive(Clone, Debug, PartialEq)]
    struct TestObj {
        name: String,
        value: i64,
    }

    #[test]
    fn test_foreign_new_downcast() {
        let fo = ForeignObject::new(
            "TestObj",
            TestObj {
                name: "foo".into(),
                value: 42,
            },
        );
        let inner: &TestObj = fo.downcast().unwrap();
        assert_eq!(inner.name, "foo");
        assert_eq!(inner.value, 42);
    }
    #[test]
    fn test_foreign_downcast_wrong_type() {
        let fo = ForeignObject::new(
            "TestObj",
            TestObj {
                name: "x".into(),
                value: 1,
            },
        );
        let result: Option<&String> = fo.downcast();
        assert!(result.is_none());
    }
    #[test]
    fn test_foreign_downcast_mut() {
        let mut fo = ForeignObject::new(
            "TestObj",
            TestObj {
                name: "x".into(),
                value: 0,
            },
        );
        let inner: &mut TestObj = fo.downcast_mut().unwrap();
        inner.value = 99;
        assert_eq!(fo.downcast::<TestObj>().unwrap().value, 99);
    }
    #[test]
    fn test_foreign_clone() {
        let fo = ForeignObject::new(
            "TestObj",
            TestObj {
                name: "orig".into(),
                value: 7,
            },
        );
        let cloned = fo.clone();
        let a: &TestObj = fo.downcast().unwrap();
        let b: &TestObj = cloned.downcast().unwrap();
        assert_eq!(a, b);
    }
    #[test]
    fn test_foreign_type_name() {
        let fo = ForeignObject::new(
            "TestObj",
            TestObj {
                name: "".into(),
                value: 0,
            },
        );
        assert_eq!(fo.type_name, "TestObj");
    }

    // ── StructData ──────────────────────────────────────────────────────

    #[test]
    fn test_struct_data_field_index() {
        let sd = StructData {
            values: vec![Value::Int(1), Value::Int(2)],
            field_names: vec!["x".into(), "y".into()],
        };
        assert_eq!(sd.field_index("x"), Some(0));
        assert_eq!(sd.field_index("y"), Some(1));
        assert_eq!(sd.field_index("z"), None);
    }
    #[test]
    fn test_struct_data_get_field() {
        let sd = StructData {
            values: vec![Value::Int(10), Value::Bool(true)],
            field_names: vec!["a".into(), "b".into()],
        };
        assert_eq!(sd.get_field("a"), Some(&Value::Int(10)));
        assert_eq!(sd.get_field("b"), Some(&Value::Bool(true)));
        assert_eq!(sd.get_field("c"), None);
    }
    #[test]
    fn test_struct_data_get_field_mut() {
        let mut sd = StructData {
            values: vec![Value::Int(0)],
            field_names: vec!["x".into()],
        };
        *sd.get_field_mut("x").unwrap() = Value::Int(42);
        assert_eq!(sd.get_field("x"), Some(&Value::Int(42)));
    }

    // ── StructBuilder ──────────────────────────────────────────────────

    #[test]
    fn test_struct_builder_basic() {
        let data = StructBuilder::new("Point")
            .field("x", 10i64)
            .field("y", 20i64)
            .build();
        assert_eq!(data.field_names, vec!["x", "y"]);
        assert_eq!(data.get_field("x"), Some(&Value::Int(10)));
        assert_eq!(data.get_field("y"), Some(&Value::Int(20)));
    }
    #[test]
    fn test_struct_builder_name() {
        let builder = StructBuilder::new("Player");
        assert_eq!(builder.name(), "Player");
        assert_eq!(builder.into_name(), "Player");
    }
    #[test]
    fn test_struct_builder_field_types() {
        let data = StructBuilder::new("Mixed")
            .field("a", 1i64)
            .field("b", 2.5f64)
            .field("c", true)
            .field("d", "text")
            .field("e", Value::Nil)
            .build();
        assert_eq!(data.field_names.len(), 5);
        assert_eq!(data.get_field("a"), Some(&Value::Int(1)));
        assert_eq!(data.get_field("b"), Some(&Value::Float(2.5)));
        assert_eq!(data.get_field("c"), Some(&Value::Bool(true)));
    }

    // ── MapKey ──────────────────────────────────────────────────────────

    #[test]
    fn test_map_key_from_int() {
        let v = Value::Int(7);
        assert_eq!(MapKey::from_value(&v), Some(MapKey::Int(7)));
    }
    #[test]
    fn test_map_key_from_str() {
        let v = Value::Str("key".into());
        assert_eq!(MapKey::from_value(&v), Some(MapKey::Str("key".into())));
    }
    #[test]
    fn test_map_key_from_bool() {
        let v = Value::Bool(false);
        assert_eq!(MapKey::from_value(&v), Some(MapKey::Bool(false)));
    }
    #[test]
    fn test_map_key_from_invalid() {
        assert_eq!(MapKey::from_value(&Value::Nil), None);
        assert_eq!(MapKey::from_value(&Value::Int(0)), Some(MapKey::Int(0)));
    }
    #[test]
    fn test_map_key_to_value() {
        assert_eq!(MapKey::Int(3).to_value(), Value::Int(3));
        assert_eq!(MapKey::Str("k".into()).to_value(), Value::Str("k".into()));
        assert_eq!(MapKey::Bool(true).to_value(), Value::Bool(true));
    }

    // ── Value equality / PartialEq ─────────────────────────────────────

    #[test]
    fn test_value_eq_primitive() {
        assert_eq!(Value::Nil, Value::Nil);
        assert_eq!(Value::Int(1), Value::Int(1));
        assert_ne!(Value::Int(1), Value::Int(2));
        assert_eq!(Value::Bool(true), Value::Bool(true));
        assert_eq!(Value::Float(3.0), Value::Float(3.0));
        assert_eq!(Value::Str("a".into()), Value::Str("a".into()));
    }
    #[test]
    fn test_value_eq_cross_type() {
        assert_ne!(Value::Int(0), Value::Bool(false));
        assert_ne!(Value::Nil, Value::Bool(false));
    }

    // ── Value debug format ──────────────────────────────────────────────

    #[test]
    fn test_value_debug() {
        assert_eq!(format!("{:?}", Value::Nil), "Nil");
        assert_eq!(format!("{:?}", Value::Int(42)), "Int(42)");
        assert_eq!(format!("{:?}", Value::Bool(true)), "Bool(true)");
    }
}
