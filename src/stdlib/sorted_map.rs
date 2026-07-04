use std::collections::BTreeMap;
use std::rc::Rc;

use crate::error::Error;
use crate::value::{ArrayData, ForeignObject, MapKey, Value};
use crate::vm::{VM, VMContext};

type SortedMapType = BTreeMap<MapKey, Value>;

fn get_sm_handle(args: &[Value]) -> crate::Result<crate::slab::Handle> {
    match args.first() {
        Some(Value::Foreign(h)) => Ok(*h),
        _ => Err(Error::Script { msg: "expected SortedMap".into() }),
    }
}

fn key_from_arg(args: &[Value], idx: usize) -> crate::Result<MapKey> {
    args.get(idx)
        .and_then(MapKey::from_value)
        .ok_or_else(|| Error::Script {
            msg: "expected int, str, or bool key".into(),
        })
}

fn sm_new_impl(ctx: &mut VMContext, _args: &[Value]) -> crate::Result<Value> {
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let h = vm.foreigns.insert(ForeignObject::new("SortedMap", SortedMapType::new()));
    Ok(Value::Foreign(h))
}

fn sorted_map_insert_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = get_sm_handle(args)?;
    let key = key_from_arg(args, 1)?;
    let val = args.get(2).cloned().unwrap_or(Value::Nil);
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let fo = vm.foreigns.get_mut(h);
    if let Some(sm) = fo.downcast_mut::<SortedMapType>() {
        sm.insert(key, val);
        Ok(Value::Nil)
    } else {
        Err(Error::Script { msg: "corrupt SortedMap".into() })
    }
}

fn sorted_map_get_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = get_sm_handle(args)?;
    let key = key_from_arg(args, 1)?;
    let found = {
        let vm: &VM = unsafe { &*ctx.raw_vm };
        let fo = vm.foreigns.get(h);
        let sm = fo.downcast::<SortedMapType>().ok_or_else(|| Error::Script { msg: "corrupt SortedMap".into() })?;
        sm.get(&key).cloned()
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    match found {
        Some(v) => Ok(crate::stdlib::option_some_vm(vm, v)),
        None => Ok(crate::stdlib::option_none_vm(vm)),
    }
}

fn sorted_map_remove_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = get_sm_handle(args)?;
    let key = key_from_arg(args, 1)?;
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let fo = vm.foreigns.get_mut(h);
    if let Some(sm) = fo.downcast_mut::<SortedMapType>() {
        Ok(sm.remove(&key).unwrap_or(Value::Nil))
    } else {
        Err(Error::Script { msg: "corrupt SortedMap".into() })
    }
}

fn sorted_map_contains_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = get_sm_handle(args)?;
    let key = key_from_arg(args, 1)?;
    let vm: &VM = unsafe { &*ctx.raw_vm };
    let fo = vm.foreigns.get(h);
    if let Some(sm) = fo.downcast::<SortedMapType>() {
        Ok(Value::Bool(sm.contains_key(&key)))
    } else {
        Err(Error::Script { msg: "corrupt SortedMap".into() })
    }
}

fn sorted_map_len_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = get_sm_handle(args)?;
    let vm: &VM = unsafe { &*ctx.raw_vm };
    let fo = vm.foreigns.get(h);
    if let Some(sm) = fo.downcast::<SortedMapType>() {
        Ok(Value::Int(sm.len() as i64))
    } else {
        Err(Error::Script { msg: "corrupt SortedMap".into() })
    }
}

fn sorted_map_keys_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = get_sm_handle(args)?;
    let keys = {
        let vm: &VM = unsafe { &*ctx.raw_vm };
        let fo = vm.foreigns.get(h);
        let sm = fo.downcast::<SortedMapType>().ok_or_else(|| Error::Script { msg: "corrupt SortedMap".into() })?;
        sm.keys().map(|k| k.to_value()).collect::<Vec<_>>()
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(Value::Array(vm.arrays.insert(ArrayData { values: keys })))
}

fn sorted_map_values_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = get_sm_handle(args)?;
    let values = {
        let vm: &VM = unsafe { &*ctx.raw_vm };
        let fo = vm.foreigns.get(h);
        let sm = fo.downcast::<SortedMapType>().ok_or_else(|| Error::Script { msg: "corrupt SortedMap".into() })?;
        sm.values().cloned().collect::<Vec<_>>()
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(Value::Array(vm.arrays.insert(ArrayData { values })))
}

fn sorted_map_entries_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = get_sm_handle(args)?;
    let pairs = {
        let vm: &VM = unsafe { &*ctx.raw_vm };
        let fo = vm.foreigns.get(h);
        let sm = fo.downcast::<SortedMapType>().ok_or_else(|| Error::Script { msg: "corrupt SortedMap".into() })?;
        let mut pairs = Vec::new();
        // Clone keys/values while holding &VM ref
        for (k, v) in sm.iter() {
            pairs.push((k.clone(), v.clone()));
        }
        pairs
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let entries: Vec<Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let pair = vec![k.to_value(), v];
            Value::Array(vm.arrays.insert(ArrayData { values: pair }))
        })
        .collect();
    Ok(Value::Array(vm.arrays.insert(ArrayData { values: entries })))
}

fn sorted_map_range_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = get_sm_handle(args)?;
    let lo = key_from_arg(args, 1)?;
    let hi = key_from_arg(args, 2)?;
    let pairs = {
        let vm: &VM = unsafe { &*ctx.raw_vm };
        let fo = vm.foreigns.get(h);
        let sm = fo.downcast::<SortedMapType>().ok_or_else(|| Error::Script { msg: "corrupt SortedMap".into() })?;
        let mut pairs = Vec::new();
        for (k, v) in sm.range(lo..=hi) {
            pairs.push((k.clone(), v.clone()));
        }
        pairs
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let entries: Vec<Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let pair = vec![k.to_value(), v];
            Value::Array(vm.arrays.insert(ArrayData { values: pair }))
        })
        .collect();
    Ok(Value::Array(vm.arrays.insert(ArrayData { values: entries })))
}

pub fn register(vm: &mut VM) {
    vm.register_native("sorted_map_new", Rc::new(sm_new_impl));
    vm.register_native("sorted_map_insert", Rc::new(sorted_map_insert_impl));
    vm.register_native("sorted_map_get", Rc::new(sorted_map_get_impl));
    vm.register_native("sorted_map_remove", Rc::new(sorted_map_remove_impl));
    vm.register_native("sorted_map_contains", Rc::new(sorted_map_contains_impl));
    vm.register_native("sorted_map_len", Rc::new(sorted_map_len_impl));
    vm.register_native("sorted_map_keys", Rc::new(sorted_map_keys_impl));
    vm.register_native("sorted_map_values", Rc::new(sorted_map_values_impl));
    vm.register_native("sorted_map_entries", Rc::new(sorted_map_entries_impl));
    vm.register_native("sorted_map_range", Rc::new(sorted_map_range_impl));
}

pub fn signatures() -> Vec<crate::symbol::FnSignature> {
    use crate::ast::Type;
    vec![
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "sorted_map_new".into(),
            params: vec![],
            return_type: Some(Type::Any),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "sorted_map_insert".into(),
            params: vec![("sm".into(), Type::Any), ("key".into(), Type::Any), ("val".into(), Type::Any)],
            return_type: Some(Type::Unit),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "sorted_map_get".into(),
            params: vec![("sm".into(), Type::Any), ("key".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "sorted_map_remove".into(),
            params: vec![("sm".into(), Type::Any), ("key".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "sorted_map_contains".into(),
            params: vec![("sm".into(), Type::Any), ("key".into(), Type::Any)],
            return_type: Some(Type::Bool),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "sorted_map_len".into(),
            params: vec![("sm".into(), Type::Any)],
            return_type: Some(Type::I64),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "sorted_map_keys".into(),
            params: vec![("sm".into(), Type::Any)],
            return_type: Some(Type::Array(Box::new(Type::Any))),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "sorted_map_values".into(),
            params: vec![("sm".into(), Type::Any)],
            return_type: Some(Type::Array(Box::new(Type::Any))),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "sorted_map_entries".into(),
            params: vec![("sm".into(), Type::Any)],
            return_type: Some(Type::Array(Box::new(Type::Any))),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "sorted_map_range".into(),
            params: vec![("sm".into(), Type::Any), ("lo".into(), Type::Any), ("hi".into(), Type::Any)],
            return_type: Some(Type::Array(Box::new(Type::Any))),
        },
    ]
}
