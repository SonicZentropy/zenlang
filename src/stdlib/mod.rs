use std::rc::Rc;

use crate::value::Value;
use crate::vm::{VM, VMContext};
use crate::Result;

/// Register all built-in stdlib functions with the given VM.
pub fn register_builtins(vm: &mut VM) {
    // Debug / I/O
    vm.register_native("print", Rc::new(print_impl));
    vm.register_native("assert", Rc::new(assert_impl));
    vm.register_native("assert_eq", Rc::new(assert_eq_impl));
    vm.register_native("type_of", Rc::new(type_of_impl));

    // String operations
    vm.register_native("len", Rc::new(len_impl));
    vm.register_native("contains", Rc::new(contains_impl));
    vm.register_native("trim", Rc::new(trim_impl));
    vm.register_native("to_upper", Rc::new(to_upper_impl));
    vm.register_native("to_lower", Rc::new(to_lower_impl));
    vm.register_native("substring", Rc::new(substring_impl));

    // Math
    vm.register_native("abs", Rc::new(abs_impl));
    vm.register_native("min", Rc::new(min_impl));
    vm.register_native("max", Rc::new(max_impl));
    vm.register_native("sqrt", Rc::new(sqrt_impl));

    // Array operations
    vm.register_native("push", Rc::new(push_impl));
    vm.register_native("pop", Rc::new(pop_impl));
    vm.register_native("insert", Rc::new(insert_impl));
    vm.register_native("remove", Rc::new(remove_impl));

    // Conversion
    vm.register_native("to_int", Rc::new(to_int_impl));
    vm.register_native("to_float", Rc::new(to_float_impl));
    vm.register_native("to_str", Rc::new(to_str_impl));

    // Option/Result helpers
    vm.register_native("is_some", Rc::new(is_some_impl));
    vm.register_native("is_none", Rc::new(is_none_impl));
    vm.register_native("is_ok", Rc::new(is_ok_impl));
    vm.register_native("is_err", Rc::new(is_err_impl));
    vm.register_native("unwrap", Rc::new(unwrap_impl));
    vm.register_native("unwrap_or", Rc::new(unwrap_or_impl));
    vm.register_native("expect", Rc::new(expect_impl));
}

/// Return the list of all built-in native function names.
pub fn native_names() -> Vec<String> {
    vec![
        "print".into(),
        "assert".into(),
        "assert_eq".into(),
        "type_of".into(),
        "len".into(),
        "contains".into(),
        "trim".into(),
        "to_upper".into(),
        "to_lower".into(),
        "substring".into(),
        "abs".into(),
        "min".into(),
        "max".into(),
        "sqrt".into(),
        "push".into(),
        "pop".into(),
        "insert".into(),
        "remove".into(),
        "to_int".into(),
        "to_float".into(),
        "to_str".into(),
        "is_some".into(),
        "is_none".into(),
        "is_ok".into(),
        "is_err".into(),
        "unwrap".into(),
        "unwrap_or".into(),
        "expect".into(),
    ]
}

// --- Debug / I/O ---

fn print_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    for arg in args {
        print!("{:?}", arg);
    }
    println!();
    Ok(Value::Nil)
}

fn assert_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    if args.first().map(|v| !v.is_truthy()).unwrap_or(true) {
        let msg = args.get(1).and_then(|v| v.as_str()).unwrap_or_else(|| "assertion failed".into());
        return Err(crate::error::Error::Script { msg: format!("assert failed: {msg}") });
    }
    Ok(Value::Nil)
}

fn assert_eq_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    if args.len() < 2 {
        return Ok(Value::Nil);
    }
    if args[0] != args[1] {
        panic!("assert_eq failed: {:?} != {:?}", args[0], args[1]);
    }
    Ok(Value::Nil)
}

fn type_of_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let name = args.first().map(|v| v.type_name()).unwrap_or("nil");
    Ok(Value::Str(name.into()))
}

// --- String operations ---

fn len_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Str(s)) => Ok(Value::Int(s.len() as i64)),
        Some(Value::Array(arr)) => Ok(Value::Int(arr.borrow().len() as i64)),
        _ => Ok(Value::Int(0)),
    }
}

fn contains_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Str(s)), Some(Value::Str(sub))) => Ok(Value::Bool(s.contains(sub.as_ref()))),
        _ => Ok(Value::Bool(false)),
    }
}

fn trim_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Str(s)) => Ok(Value::Str(s.trim().into())),
        _ => Ok(Value::Nil),
    }
}

fn to_upper_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Str(s)) => {
            let upper: String = s.chars().flat_map(|c| c.to_uppercase()).collect();
            Ok(Value::Str(upper.into()))
        }
        _ => Ok(Value::Nil),
    }
}

fn to_lower_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Str(s)) => {
            let lower: String = s.chars().flat_map(|c| c.to_lowercase()).collect();
            Ok(Value::Str(lower.into()))
        }
        _ => Ok(Value::Nil),
    }
}

fn substring_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1), args.get(2)) {
        (Some(Value::Str(s)), Some(Value::Int(start)), Some(Value::Int(end))) => {
            let s = s.as_ref();
            let start = *start as usize;
            let end = (*end as usize).min(s.len());
            if start >= s.len() || start >= end {
                return Ok(Value::Str("".into()));
            }
            Ok(Value::Str(s[start..end].into()))
        }
        _ => Ok(Value::Nil),
    }
}

// --- Math ---

fn abs_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(n.abs())),
        Some(Value::Float(n)) => Ok(Value::Float(n.abs())),
        _ => Ok(Value::Int(0)),
    }
}

fn min_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(a)), Some(Value::Int(b))) => Ok(Value::Int((*a).min(*b))),
        (Some(Value::Float(a)), Some(Value::Float(b))) => Ok(Value::Float((*a).min(*b))),
        (Some(Value::Int(a)), Some(Value::Float(b))) => Ok(Value::Float((*a as f64).min(*b))),
        (Some(Value::Float(a)), Some(Value::Int(b))) => Ok(Value::Float((*a).min(*b as f64))),
        _ => Ok(Value::Int(0)),
    }
}

fn max_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(a)), Some(Value::Int(b))) => Ok(Value::Int((*a).max(*b))),
        (Some(Value::Float(a)), Some(Value::Float(b))) => Ok(Value::Float((*a).max(*b))),
        (Some(Value::Int(a)), Some(Value::Float(b))) => Ok(Value::Float((*a as f64).max(*b))),
        (Some(Value::Float(a)), Some(Value::Int(b))) => Ok(Value::Float((*a).max(*b as f64))),
        _ => Ok(Value::Int(0)),
    }
}

fn sqrt_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Float(n)) => Ok(Value::Float(n.sqrt())),
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sqrt())),
        _ => Ok(Value::Float(0.0)),
    }
}

// --- Array operations ---

fn push_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Array(arr)), Some(val)) => {
            arr.borrow_mut().push(val.clone());
            Ok(Value::Nil)
        }
        _ => Ok(Value::Nil),
    }
}

fn pop_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Array(arr)) => {
            Ok(arr.borrow_mut().pop().unwrap_or(Value::Nil))
        }
        _ => Ok(Value::Nil),
    }
}

fn insert_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1), args.get(2)) {
        (Some(Value::Array(arr)), Some(Value::Int(idx)), Some(val)) => {
            let idx = *idx as usize;
            let mut v = arr.borrow_mut();
            if idx <= v.len() {
                v.insert(idx, val.clone());
            }
            Ok(Value::Nil)
        }
        _ => Ok(Value::Nil),
    }
}

fn remove_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Array(arr)), Some(Value::Int(idx))) => {
            let idx = *idx as usize;
            let mut v = arr.borrow_mut();
            if idx < v.len() {
                Ok(v.remove(idx))
            } else {
                Ok(Value::Nil)
            }
        }
        _ => Ok(Value::Nil),
    }
}

// --- Conversion ---

fn to_int_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Int(*n as i64)),
        Some(Value::Str(s)) => {
            let n = s.parse::<i64>().unwrap_or(0);
            Ok(Value::Int(n))
        }
        Some(Value::Bool(b)) => Ok(Value::Int(if *b { 1 } else { 0 })),
        _ => Ok(Value::Int(0)),
    }
}

fn to_float_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Float(n)) => Ok(Value::Float(*n)),
        Some(Value::Int(n)) => Ok(Value::Float(*n as f64)),
        Some(Value::Str(s)) => {
            let n = s.parse::<f64>().unwrap_or(0.0);
            Ok(Value::Float(n))
        }
        _ => Ok(Value::Float(0.0)),
    }
}

fn to_str_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(val) => Ok(Value::Str(format!("{val:?}").into())),
        None => Ok(Value::Str("nil".into())),
    }
}

// --- Option/Result helpers ---

fn enum_tag(val: &Value) -> Option<u16> {
    match val {
        Value::Enum { tag, data: _ } => Some(*tag),
        _ => None,
    }
}

fn is_some_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(args.first().map_or(false, |v| enum_tag(v) == Some(0))))
}

fn is_none_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(args.first().map_or(false, |v| enum_tag(v) == Some(1))))
}

fn is_ok_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(args.first().map_or(false, |v| enum_tag(v) == Some(0))))
}

fn is_err_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(args.first().map_or(false, |v| enum_tag(v) == Some(1))))
}

fn unwrap_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Enum { tag, data }) if *tag == 0 => {
            Ok(data.borrow().first().cloned().unwrap_or(Value::Nil))
        }
        Some(Value::Enum { tag: _, data: _ }) => {
            Err(crate::error::Error::Script { msg: "unwrap failed: got None/Err".into() })
        }
        _ => Err(crate::error::Error::Script { msg: "unwrap called on non-enum value".into() }),
    }
}

fn unwrap_or_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Enum { tag, data }), Some(default)) if *tag == 0 => {
            Ok(data.borrow().first().cloned().unwrap_or_else(|| default.clone()))
        }
        (_, Some(default)) => Ok(default.clone()),
        _ => Ok(Value::Nil),
    }
}

fn expect_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let msg = args.get(1).and_then(|v| v.as_str()).unwrap_or_else(|| "expect failed".into());
    match args.first() {
        Some(Value::Enum { tag, data }) if *tag == 0 => {
            Ok(data.borrow().first().cloned().unwrap_or(Value::Nil))
        }
        _ => Err(crate::error::Error::Script { msg: format!("expect failed: {msg}") }),
    }
}
