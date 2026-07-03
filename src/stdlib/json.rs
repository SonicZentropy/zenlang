use std::rc::Rc;
use serde_json;

use crate::error::{Error, Result};
use crate::value::{
    ArrayData, EnumData, MapData, MapKey, StructData, Value,
};
use crate::vm::{VM, VMContext};

/// Serialize a Zenlang `Value` to a JSON string.
fn to_json_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let val = args.first().cloned().unwrap_or(Value::Nil);
    let json_val = value_to_json(ctx, &val)?;
    let json_str = serde_json::to_string(&json_val)
        .map_err(|e| Error::Script { msg: format!("JSON serialization error: {e}") })?;
    Ok(Value::Str(json_str.into()))
}

/// Parse a JSON string into a Zenlang `Value`.
fn from_json_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let json_str = match args.first() {
        Some(Value::Str(s)) => s.as_ref().to_string(),
        _ => return Err(Error::Script { msg: "from_json requires a string argument".into() }),
    };
    let json_val: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| Error::Script { msg: format!("JSON parse error: {e}") })?;
    json_to_value(ctx, &json_val)
}

/// Convert a `Value` to a `serde_json::Value`, resolving handles through the VM.
fn value_to_json(ctx: &mut VMContext, val: &Value) -> Result<serde_json::Value> {
    let vm: &VM = unsafe { &*ctx.raw_vm };
    match val {
        Value::Nil => Ok(serde_json::Value::Null),
        Value::Bool(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Int(n) => Ok(serde_json::Value::Number(
            serde_json::Number::from(*n)
        )),
        Value::Float(f) => Ok(serde_json::Value::Number(
            serde_json::Number::from_f64(*f).unwrap_or(serde_json::Number::from_f64(0.0).unwrap())
        )),
        Value::Str(s) => Ok(serde_json::Value::String(s.as_ref().to_string())),
        Value::Array(h) => {
            let data = vm.arrays.get(*h);
            let elems: Result<Vec<serde_json::Value>> = data.values.iter()
                .map(|v| value_to_json(ctx, v))
                .collect();
            Ok(serde_json::Value::Array(elems?))
        }
        Value::Struct(h, type_name) => {
            let data = vm.structs.get(*h);
            let mut map = serde_json::Map::new();
            map.insert("__type".into(), serde_json::Value::String(type_name.clone()));
            for (i, name) in data.field_names.iter().enumerate() {
                let field_val = data.values.get(i).cloned().unwrap_or(Value::Nil);
                map.insert(name.clone(), value_to_json(ctx, &field_val)?);
            }
            Ok(serde_json::Value::Object(map))
        }
        Value::Enum(h) => {
            let data = vm.enums.get(*h);
            let mut map = serde_json::Map::new();
            map.insert("__tag".into(), serde_json::Value::Number(
                serde_json::Number::from(data.tag)
            ));
            let fields: Result<Vec<serde_json::Value>> = data.fields.iter()
                .map(|v| value_to_json(ctx, v))
                .collect();
            map.insert("fields".into(), serde_json::Value::Array(fields?));
            Ok(serde_json::Value::Object(map))
        }
        Value::Map(h) => {
            let data = vm.maps.get(*h);
            let mut map = serde_json::Map::new();
            for (key, v) in &data.entries {
                let key_str = match key {
                    MapKey::Int(n) => n.to_string(),
                    MapKey::Str(s) => s.as_ref().to_string(),
                    MapKey::Bool(b) => b.to_string(),
                };
                map.insert(key_str, value_to_json(ctx, v)?);
            }
            Ok(serde_json::Value::Object(map))
        }
        Value::Weak(h) => {
            let weak = vm.weaks.get(*h);
            if vm.weaks.is_valid(weak.target) {
                let inner = match weak.kind {
                    crate::value::WeakKind::Struct => Value::Struct(weak.target, weak.type_name.clone()),
                    crate::value::WeakKind::Array => Value::Array(weak.target),
                    crate::value::WeakKind::Map => Value::Map(weak.target),
                };
                let mut map = serde_json::Map::new();
                map.insert("weak".into(), value_to_json(ctx, &inner)?);
                Ok(serde_json::Value::Object(map))
            } else {
                Ok(serde_json::Value::Null)
            }
        }
        Value::Range(_, _, _) | Value::Function(_)
        | Value::NativeFunction(_) | Value::Closure(_)
        | Value::Generator(_) | Value::Foreign(_) => {
            Ok(serde_json::Value::Null)
        }
    }
}

/// Convert a `serde_json::Value` back into a Zenlang `Value`, allocating handles in the VM.
fn json_to_value(ctx: &mut VMContext, json: &serde_json::Value) -> Result<Value> {
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    match json {
        serde_json::Value::Null => Ok(Value::Nil),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Ok(Value::Int(0))
            }
        }
        serde_json::Value::String(s) => Ok(Value::Str(s.as_str().into())),
        serde_json::Value::Array(arr) => {
            let values: Result<Vec<Value>> = arr.iter()
                .map(|v| json_to_value(ctx, v))
                .collect();
            let h = vm.arrays.insert(ArrayData { values: values? });
            Ok(Value::Array(h))
        }
        serde_json::Value::Object(obj) => {
            // Check for struct serialization: {"__type": "...", ...}
            if let Some(serde_json::Value::String(type_name)) = obj.get("__type") {
                let mut field_names = Vec::new();
                let mut values = Vec::new();
                for (key, val_json) in obj.iter() {
                    if key == "__type" { continue; }
                    field_names.push(key.clone());
                    values.push(json_to_value(ctx, val_json)?);
                }
                let h = vm.structs.insert(StructData { values, field_names });
                return Ok(Value::Struct(h, type_name.clone()));
            }
            // Check for enum serialization: {"__tag": N, "fields": [...]}
            if let Some(serde_json::Value::Number(tag_num)) = obj.get("__tag") {
                if let Some(tag) = tag_num.as_u64() {
                    let fields = match obj.get("fields") {
                        Some(serde_json::Value::Array(arr)) => {
                            let mut result = Vec::new();
                            for v in arr {
                                result.push(json_to_value(ctx, v)?);
                            }
                            result
                        }
                        _ => Vec::new(),
                    };
                    let h = vm.enums.insert(EnumData { tag: tag as u16, fields });
                    return Ok(Value::Enum(h));
                }
            }
            // Default: serialize as Map
            let mut entries = std::collections::HashMap::new();
            for (key, val_json) in obj.iter() {
                let map_key = MapKey::Str(key.as_str().into());
                entries.insert(map_key, json_to_value(ctx, val_json)?);
            }
            let h = vm.maps.insert(MapData { entries });
            Ok(Value::Map(h))
        }
    }
}

pub fn register(vm: &mut crate::vm::VM) {
    vm.register_native("to_json", Rc::new(to_json_impl));
    vm.register_native("from_json", Rc::new(from_json_impl));
}

pub fn signatures() -> Vec<crate::symbol::FnSignature> {
    use crate::ast::Type;
    vec![
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "to_json".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::Str),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "from_json".into(),
            params: vec![("json".into(), Type::Str)],
            return_type: Some(Type::Any),
        },
    ]
}
