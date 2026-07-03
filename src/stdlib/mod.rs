use std::rc::Rc;

use crate::Result;
use crate::ast::Type;
use crate::symbol::FnSignature;
use crate::value::Value;
use crate::vm::{VM, VMContext};

mod fs;
mod iter;
mod log;
mod map;
mod math;

/// Register all built-in stdlib functions with the given VM.
pub fn register_builtins(vm: &mut VM) {
    // Debug / I/O
    vm.register_native("print", Rc::new(print_impl));
    vm.register_native("assert", Rc::new(assert_impl));
    vm.register_native("assert_eq", Rc::new(assert_eq_impl));
    vm.register_native("type_of", Rc::new(type_of_impl));

    // File system, path, directory operations
    fs::register(vm);

    // Logging
    log::register(vm);

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

    // Iterators
    vm.register_native("iter", Rc::new(iter::iter_impl));
    iter::register(vm);

    // Maps / dictionaries
    map::register(vm);

    // Vector/scalar math for games (Vec2/Vec3, lerp/clamp, trig, RNG)
    math::register(vm);

    // Array operations
    vm.register_native("push", Rc::new(push_impl));
    vm.register_native("pop", Rc::new(pop_impl));
    vm.register_native("insert", Rc::new(insert_impl));
    vm.register_native("remove", Rc::new(remove_impl));

    // Conversion
    vm.register_native("to_int", Rc::new(to_int_impl));
    vm.register_native("to_float", Rc::new(to_float_impl));
    vm.register_native("to_str", Rc::new(to_str_impl));

    // Generator/Coroutine
    vm.register_native("next", Rc::new(next_impl));

    // Timer / scheduling
    vm.register_native("set_timeout", Rc::new(set_timeout_impl));
    vm.register_native("set_interval", Rc::new(set_interval_impl));
    vm.register_native("clear_timer", Rc::new(clear_timer_impl));

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
    native_fn_sigs().into_iter().map(|s| s.name).collect()
}

/// Return accurate type signatures for all native functions.
pub fn native_fn_sigs() -> Vec<FnSignature> {
    // Unit param type = compatible with everything (acts as type variable)
    let mut sigs = vec![
        FnSignature {
            type_params: vec![],
            name: "print".into(),
            params: vec![],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "assert".into(),
            params: vec![("cond".into(), Type::Unit)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "assert_eq".into(),
            params: vec![("a".into(), Type::Unit), ("b".into(), Type::Unit)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "type_of".into(),
            params: vec![("val".into(), Type::Unit)],
            return_type: Some(Type::Str),
        },
        FnSignature {
            type_params: vec![],
            name: "len".into(),
            params: vec![("val".into(), Type::Unit)],
            return_type: Some(Type::I64),
        },
        FnSignature {
            type_params: vec![],
            name: "contains".into(),
            params: vec![("s".into(), Type::Str), ("sub".into(), Type::Str)],
            return_type: Some(Type::Bool),
        },
        FnSignature {
            type_params: vec![],
            name: "trim".into(),
            params: vec![("s".into(), Type::Str)],
            return_type: Some(Type::Str),
        },
        FnSignature {
            type_params: vec![],
            name: "to_upper".into(),
            params: vec![("s".into(), Type::Str)],
            return_type: Some(Type::Str),
        },
        FnSignature {
            type_params: vec![],
            name: "to_lower".into(),
            params: vec![("s".into(), Type::Str)],
            return_type: Some(Type::Str),
        },
        FnSignature {
            type_params: vec![],
            name: "substring".into(),
            params: vec![
                ("s".into(), Type::Str),
                ("start".into(), Type::I64),
                ("end".into(), Type::I64),
            ],
            return_type: Some(Type::Str),
        },
        FnSignature {
            type_params: vec![],
            name: "abs".into(),
            params: vec![("n".into(), Type::I64)],
            return_type: Some(Type::I64),
        },
        FnSignature {
            type_params: vec![],
            name: "min".into(),
            params: vec![("a".into(), Type::I64), ("b".into(), Type::I64)],
            return_type: Some(Type::I64),
        },
        FnSignature {
            type_params: vec![],
            name: "max".into(),
            params: vec![("a".into(), Type::I64), ("b".into(), Type::I64)],
            return_type: Some(Type::I64),
        },
        FnSignature {
            type_params: vec![],
            name: "sqrt".into(),
            params: vec![("n".into(), Type::F64)],
            return_type: Some(Type::F64),
        },
        FnSignature {
            type_params: vec![],
            name: "push".into(),
            params: vec![("arr".into(), Type::Unit), ("val".into(), Type::Unit)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "pop".into(),
            params: vec![("arr".into(), Type::Unit)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "insert".into(),
            params: vec![
                ("arr".into(), Type::Unit),
                ("idx".into(), Type::I64),
                ("val".into(), Type::Unit),
            ],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "remove".into(),
            params: vec![("arr".into(), Type::Unit), ("idx".into(), Type::I64)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "to_int".into(),
            params: vec![("val".into(), Type::Unit)],
            return_type: Some(Type::I64),
        },
        FnSignature {
            type_params: vec![],
            name: "to_float".into(),
            params: vec![("val".into(), Type::Unit)],
            return_type: Some(Type::F64),
        },
        FnSignature {
            type_params: vec![],
            name: "to_str".into(),
            params: vec![("val".into(), Type::Unit)],
            return_type: Some(Type::Str),
        },
        FnSignature {
            type_params: vec![],
            name: "is_some".into(),
            params: vec![("val".into(), Type::Unit)],
            return_type: Some(Type::Bool),
        },
        FnSignature {
            type_params: vec![],
            name: "is_none".into(),
            params: vec![("val".into(), Type::Unit)],
            return_type: Some(Type::Bool),
        },
        FnSignature {
            type_params: vec![],
            name: "is_ok".into(),
            params: vec![("val".into(), Type::Unit)],
            return_type: Some(Type::Bool),
        },
        FnSignature {
            type_params: vec![],
            name: "is_err".into(),
            params: vec![("val".into(), Type::Unit)],
            return_type: Some(Type::Bool),
        },
        FnSignature {
            type_params: vec![],
            name: "unwrap".into(),
            params: vec![("val".into(), Type::Unit)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "unwrap_or".into(),
            params: vec![("val".into(), Type::Unit), ("default".into(), Type::Unit)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "expect".into(),
            params: vec![("val".into(), Type::Unit), ("msg".into(), Type::Str)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "iter".into(),
            params: vec![("val".into(), Type::Unit)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "next".into(),
            params: vec![("gen".into(), Type::Unit)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "set_timeout".into(),
            params: vec![
                ("callback".into(), Type::Unit),
                ("seconds".into(), Type::F64),
            ],
            return_type: Some(Type::I64),
        },
        FnSignature {
            type_params: vec![],
            name: "set_interval".into(),
            params: vec![
                ("callback".into(), Type::Unit),
                ("seconds".into(), Type::F64),
            ],
            return_type: Some(Type::I64),
        },
        FnSignature {
            type_params: vec![],
            name: "clear_timer".into(),
            params: vec![("id".into(), Type::I64)],
            return_type: Some(Type::Unit),
        },
    ];
    sigs.extend(fs::signatures());
    sigs.extend(log::signatures());
    sigs.extend(map::signatures());
    sigs.extend(math::signatures());
    sigs
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
        let msg = args
            .get(1)
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| "assertion failed".into());
        return Err(crate::error::Error::Script {
            msg: format!("assert failed: {msg}"),
        });
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
        Some(Value::Range(start, end, inclusive)) => {
            let len = if *inclusive {
                *end - *start + 1
            } else {
                *end - *start
            };
            Ok(Value::Int(len.max(0)))
        }
        Some(Value::Map(m)) => Ok(Value::Int(m.borrow().len() as i64)),
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
        Some(Value::Array(arr)) => Ok(arr.borrow_mut().pop().unwrap_or(Value::Nil)),
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
        Some(val) => Ok(Value::Str(match val {
            Value::Nil => "nil".into(),
            Value::Bool(b) => (if *b { "true" } else { "false" }).into(),
            Value::Int(n) => format!("{n}").into(),
            Value::Float(f) => format!("{f}").into(),
            Value::Str(s) => s.clone(),
            Value::Array(_) => "[...]".into(),
            Value::Struct(_, name) => format!("{name} {{...}}").into(),
            Value::Enum { tag, .. } => format!("Enum({tag})").into(),
            _ => format!("{val:?}").into(),
        })),
        None => Ok(Value::Str("nil".into())),
    }
}

// --- Option/Result helpers ---

/// Build a `Some(v)` value using the built-in `Option` enum's convention
/// (tag 0 = Some). Shared by any stdlib module that needs to hand a
/// script-visible `Option<T>` back (iterators, map lookups, etc).
pub(crate) fn option_some(v: Value) -> Value {
    Value::Enum {
        tag: 0,
        data: std::rc::Rc::new(std::cell::RefCell::new(vec![v])),
    }
}

/// Build a `None` value using the built-in `Option` enum's convention (tag 1 = None).
pub(crate) fn option_none() -> Value {
    Value::Enum {
        tag: 1,
        data: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
    }
}

fn enum_tag(val: &Value) -> Option<u16> {
    match val {
        Value::Enum { tag, data: _ } => Some(*tag),
        _ => None,
    }
}

fn is_some_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(
        args.first().map_or(false, |v| enum_tag(v) == Some(0)),
    ))
}

fn is_none_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(
        args.first().map_or(false, |v| enum_tag(v) == Some(1)),
    ))
}

fn is_ok_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(
        args.first().map_or(false, |v| enum_tag(v) == Some(0)),
    ))
}

fn is_err_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(
        args.first().map_or(false, |v| enum_tag(v) == Some(1)),
    ))
}

fn unwrap_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Enum { tag, data }) if *tag == 0 => {
            Ok(data.borrow().first().cloned().unwrap_or(Value::Nil))
        }
        Some(Value::Enum { tag: _, data: _ }) => Err(crate::error::Error::Script {
            msg: "unwrap failed: got None/Err".into(),
        }),
        _ => Err(crate::error::Error::Script {
            msg: "unwrap called on non-enum value".into(),
        }),
    }
}

fn unwrap_or_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Enum { tag, data }), Some(default)) if *tag == 0 => Ok(data
            .borrow()
            .first()
            .cloned()
            .unwrap_or_else(|| default.clone())),
        (_, Some(default)) => Ok(default.clone()),
        _ => Ok(Value::Nil),
    }
}

// --- Generator/Coroutine ---

fn set_timeout_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    eprintln!("DEBUG set_timeout_impl called with {} args", args.len());
    if args.len() < 2 {
        return Err(crate::error::Error::Script {
            msg: "set_timeout requires a callback and a delay in seconds".into(),
        });
    }
    let callback = args[0].clone();
    if !matches!(callback, Value::Function(_) | Value::Closure(_)) {
        return Err(crate::error::Error::Script {
            msg: "set_timeout first argument must be a function".into(),
        });
    }
    let seconds = match &args[1] {
        Value::Float(f) => *f,
        Value::Int(n) => *n as f64,
        _ => {
            return Err(crate::error::Error::Script {
                msg: "set_timeout seconds must be a number".into(),
            })
        }
    };
    let id = ctx.register_timer(callback, seconds, None);
    Ok(Value::Int(id as i64))
}

fn set_interval_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    if args.len() < 2 {
        return Err(crate::error::Error::Script {
            msg: "set_interval requires a callback and an interval in seconds".into(),
        });
    }
    let callback = args[0].clone();
    if !matches!(callback, Value::Function(_) | Value::Closure(_)) {
        return Err(crate::error::Error::Script {
            msg: "set_interval first argument must be a function".into(),
        });
    }
    let seconds = match &args[1] {
        Value::Float(f) => *f,
        Value::Int(n) => *n as f64,
        _ => {
            return Err(crate::error::Error::Script {
                msg: "set_interval seconds must be a number".into(),
            })
        }
    };
    let id = ctx.register_timer(callback, seconds, Some(seconds));
    Ok(Value::Int(id as i64))
}

fn clear_timer_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    if let Some(Value::Int(id)) = args.first() {
        ctx.remove_timer(*id as u64);
    }
    Ok(Value::Nil)
}

fn next_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Generator(cell)) => {
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            match vm.resume_generator(cell.clone())? {
                Some(val) => Ok(option_some(val)),
                None => Ok(option_none()),
            }
        }
        _ => Err(crate::error::Error::Script {
            msg: "next() requires a generator argument".into(),
        }),
    }
}

fn expect_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let msg = args
        .get(1)
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| "expect failed".into());
    match args.first() {
        Some(Value::Enum { tag, data }) if *tag == 0 => {
            Ok(data.borrow().first().cloned().unwrap_or(Value::Nil))
        }
        _ => Err(crate::error::Error::Script {
            msg: format!("expect failed: {msg}"),
        }),
    }
}
