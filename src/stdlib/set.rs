//! Set collection for Zenlang: an unordered collection of unique values.
//!
//! Internally stored as `Value::Map` (keys are set elements, values are nil).
//!
//! # Functions
//! - `set_new()` — create an empty set
//! - `set_add(set, value)` — add a value (duplicates ignored)
//! - `set_remove(set, value)` — remove a value
//! - `set_contains(set, value)` — check membership
//! - `set_len(set)` — number of elements
//! - `set_to_array(set)` — convert to array of elements
//! - `set_from_array(arr)` — create set from array (dedup)
//!
//! Iterate over a set by converting to array first: `iter(set_to_array(s))`.
//!
//! # Example
//! ```zen
//! let s = set_new();
//! set_add(s, "apple");
//! set_add(s, "banana");
//! assert(set_contains(s, "apple"));
//! assert(!set_contains(s, "cherry"));
//! assert(set_len(s) == 2);
//! set_remove(s, "apple");
//! assert(set_len(s) == 1);
//! ```

use std::rc::Rc;

use crate::error::Error;
use crate::value::{ArrayData, MapData, MapKey, Value};
use crate::vm::{VM, VMContext};

fn set_new_impl(ctx: &mut VMContext, _args: &[Value]) -> crate::Result<Value> {
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(Value::Map(vm.maps.insert(MapData {
        entries: std::collections::HashMap::new(),
    })))
}

fn set_add_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Map(h)) => *h,
        _ => return Err(Error::Script { msg: "set_add() expects a set (map)".into() }),
    };
    let key = match args.get(1) {
        Some(v) => MapKey::from_value(v).ok_or_else(|| Error::Script {
            msg: "set_add() value must be int, str, or bool".into(),
        })?,
        None => return Err(Error::Script { msg: "set_add() requires a value".into() }),
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    vm.maps.get_mut(h).entries.insert(key, Value::Nil);
    Ok(Value::Nil)
}

fn set_remove_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Map(h)) => *h,
        _ => return Err(Error::Script { msg: "set_remove() expects a set (map)".into() }),
    };
    let key = match args.get(1) {
        Some(v) => MapKey::from_value(v).ok_or_else(|| Error::Script {
            msg: "set_remove() value must be int, str, or bool".into(),
        })?,
        None => return Err(Error::Script { msg: "set_remove() requires a value".into() }),
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    vm.maps.get_mut(h).entries.remove(&key);
    Ok(Value::Nil)
}

fn set_contains_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Map(h)) => *h,
        _ => return Err(Error::Script { msg: "set_contains() expects a set (map)".into() }),
    };
    let key = match args.get(1) {
        Some(v) => MapKey::from_value(v).ok_or_else(|| Error::Script {
            msg: "set_contains() value must be int, str, or bool".into(),
        })?,
        None => return Err(Error::Script { msg: "set_contains() requires a value".into() }),
    };
    let vm: &VM = unsafe { &*ctx.raw_vm };
    Ok(Value::Bool(vm.maps.get(h).entries.contains_key(&key)))
}

fn set_len_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Map(h)) => *h,
        _ => return Err(Error::Script { msg: "set_len() expects a set (map)".into() }),
    };
    let vm: &VM = unsafe { &*ctx.raw_vm };
    Ok(Value::Int(vm.maps.get(h).entries.len() as i64))
}

fn set_to_array_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Map(h)) => *h,
        _ => return Err(Error::Script { msg: "set_to_array() expects a set (map)".into() }),
    };
    let vm: &VM = unsafe { &*ctx.raw_vm };
    let keys: Vec<Value> = vm.maps.get(h).entries.keys().map(|k| k.to_value()).collect();
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(Value::Array(vm.arrays.insert(ArrayData { values: keys })))
}

fn set_from_array_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let arr_h = match args.first() {
        Some(Value::Array(h)) => *h,
        _ => return Err(Error::Script { msg: "set_from_array() expects an array".into() }),
    };
    let vm: &VM = unsafe { &*ctx.raw_vm };
    let values = &vm.arrays.get(arr_h).values;
    let mut entries = std::collections::HashMap::new();
    for v in values {
        if let Some(k) = MapKey::from_value(v) {
            entries.insert(k, Value::Nil);
        }
    }
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(Value::Map(vm.maps.insert(MapData { entries })))
}

pub fn register(vm: &mut VM) {
    vm.register_native("set_new", Rc::new(set_new_impl));
    vm.register_native("set_add", Rc::new(set_add_impl));
    vm.register_native("set_remove", Rc::new(set_remove_impl));
    vm.register_native("set_contains", Rc::new(set_contains_impl));
    vm.register_native("set_len", Rc::new(set_len_impl));
    vm.register_native("set_to_array", Rc::new(set_to_array_impl));
    vm.register_native("set_from_array", Rc::new(set_from_array_impl));
}

pub fn signatures() -> Vec<crate::symbol::FnSignature> {
    use crate::ast::Type;
    vec![
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "set_new".into(),
            params: vec![],
            return_type: Some(Type::Any),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "set_add".into(),
            params: vec![("set".into(), Type::Any), ("val".into(), Type::Any)],
            return_type: Some(Type::Unit),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "set_remove".into(),
            params: vec![("set".into(), Type::Any), ("val".into(), Type::Any)],
            return_type: Some(Type::Unit),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "set_contains".into(),
            params: vec![("set".into(), Type::Any), ("val".into(), Type::Any)],
            return_type: Some(Type::Bool),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "set_len".into(),
            params: vec![("set".into(), Type::Any)],
            return_type: Some(Type::I64),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "set_to_array".into(),
            params: vec![("set".into(), Type::Any)],
            return_type: Some(Type::Array(Box::new(Type::Any))),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "set_from_array".into(),
            params: vec![("arr".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
    ]
}
