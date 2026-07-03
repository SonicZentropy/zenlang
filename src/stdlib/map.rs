use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::Result;
use crate::error::Error;
use crate::value::{MapKey, Value};
use crate::vm::{VM, VMContext};

use super::{option_none, option_some};

fn key_error(v: &Value) -> Error {
    Error::Script {
        msg: format!(
            "map keys must be int, str, or bool (got '{}')",
            v.type_name()
        ),
    }
}

fn as_map(v: &Value) -> Result<Rc<RefCell<HashMap<MapKey, Value>>>> {
    match v {
        Value::Map(m) => Ok(m.clone()),
        other => Err(Error::Script {
            msg: format!("expected a map, got '{}'", other.type_name()),
        }),
    }
}

/// `map_new()` — create a new, empty map.
fn map_new_impl(_ctx: &mut VMContext, _args: &[Value]) -> Result<Value> {
    Ok(Value::Map(Rc::new(RefCell::new(HashMap::new()))))
}

/// `map_set(m, key, val)` — insert or overwrite `key` with `val`. Returns nil.
fn map_set_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let m = as_map(args.first().unwrap_or(&Value::Nil))?;
    let key_val = args.get(1).unwrap_or(&Value::Nil);
    let key = MapKey::from_value(key_val).ok_or_else(|| key_error(key_val))?;
    let val = args.get(2).cloned().unwrap_or(Value::Nil);
    m.borrow_mut().insert(key, val);
    Ok(Value::Nil)
}

/// `map_get(m, key)` — returns `Some(val)` if present, `None` otherwise.
fn map_get_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let m = as_map(args.first().unwrap_or(&Value::Nil))?;
    let key_val = args.get(1).unwrap_or(&Value::Nil);
    let Some(key) = MapKey::from_value(key_val) else {
        return Ok(option_none());
    };
    match m.borrow().get(&key) {
        Some(v) => Ok(option_some(v.clone())),
        None => Ok(option_none()),
    }
}

/// `map_has(m, key)` — returns `true` if `key` is present.
fn map_has_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let m = as_map(args.first().unwrap_or(&Value::Nil))?;
    let key_val = args.get(1).unwrap_or(&Value::Nil);
    let Some(key) = MapKey::from_value(key_val) else {
        return Ok(Value::Bool(false));
    };
    Ok(Value::Bool(m.borrow().contains_key(&key)))
}

/// `map_remove(m, key)` — removes `key`, returning `Some(old_val)` or `None`.
fn map_remove_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let m = as_map(args.first().unwrap_or(&Value::Nil))?;
    let key_val = args.get(1).unwrap_or(&Value::Nil);
    let Some(key) = MapKey::from_value(key_val) else {
        return Ok(option_none());
    };
    match m.borrow_mut().remove(&key) {
        Some(v) => Ok(option_some(v)),
        None => Ok(option_none()),
    }
}

/// `map_keys(m)` — returns an array of all keys (unspecified order).
fn map_keys_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let m = as_map(args.first().unwrap_or(&Value::Nil))?;
    let keys: Vec<Value> = m.borrow().keys().map(|k| k.to_value()).collect();
    Ok(Value::Array(Rc::new(RefCell::new(keys))))
}

/// `map_values(m)` — returns an array of all values (unspecified order).
fn map_values_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let m = as_map(args.first().unwrap_or(&Value::Nil))?;
    let values: Vec<Value> = m.borrow().values().cloned().collect();
    Ok(Value::Array(Rc::new(RefCell::new(values))))
}

/// `map_len(m)` — number of entries. (`len(m)` also works — see `len_impl`.)
fn map_len_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let m = as_map(args.first().unwrap_or(&Value::Nil))?;
    Ok(Value::Int(m.borrow().len() as i64))
}

/// `map_clear(m)` — removes all entries. Returns nil.
fn map_clear_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let m = as_map(args.first().unwrap_or(&Value::Nil))?;
    m.borrow_mut().clear();
    Ok(Value::Nil)
}

pub fn register(vm: &mut VM) {
    vm.register_native("map_new", Rc::new(map_new_impl));
    vm.register_native("map_set", Rc::new(map_set_impl));
    vm.register_native("map_get", Rc::new(map_get_impl));
    vm.register_native("map_has", Rc::new(map_has_impl));
    vm.register_native("map_remove", Rc::new(map_remove_impl));
    vm.register_native("map_keys", Rc::new(map_keys_impl));
    vm.register_native("map_values", Rc::new(map_values_impl));
    vm.register_native("map_len", Rc::new(map_len_impl));
    vm.register_native("map_clear", Rc::new(map_clear_impl));
}

pub fn signatures() -> Vec<crate::symbol::FnSignature> {
    use crate::ast::Type;
    use crate::symbol::FnSignature;
    vec![
        FnSignature {
            type_params: vec![],
            name: "map_new".into(),
            params: vec![],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "map_set".into(),
            params: vec![
                ("m".into(), Type::Unit),
                ("key".into(), Type::Unit),
                ("val".into(), Type::Unit),
            ],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "map_get".into(),
            params: vec![("m".into(), Type::Unit), ("key".into(), Type::Unit)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "map_has".into(),
            params: vec![("m".into(), Type::Unit), ("key".into(), Type::Unit)],
            return_type: Some(Type::Bool),
        },
        FnSignature {
            type_params: vec![],
            name: "map_remove".into(),
            params: vec![("m".into(), Type::Unit), ("key".into(), Type::Unit)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "map_keys".into(),
            params: vec![("m".into(), Type::Unit)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "map_values".into(),
            params: vec![("m".into(), Type::Unit)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "map_len".into(),
            params: vec![("m".into(), Type::Unit)],
            return_type: Some(Type::I64),
        },
        FnSignature {
            type_params: vec![],
            name: "map_clear".into(),
            params: vec![("m".into(), Type::Unit)],
            return_type: Some(Type::Unit),
        },
    ]
}
