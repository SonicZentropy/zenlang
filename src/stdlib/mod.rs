//! Built-in native functions for the Zenlang VM.
//!
//! `register_builtins(vm)` registers all stdlib functions (I/O, math, strings,
//! maps, iterators, JSON, file system, logging). Use `native_names()` to get
//! the list of names for the resolver.
//!
//! Individual native function implementations are in sub-modules (`json`,
//! `fs`, `iter`, `map`, `log`, `math`).

use std::rc::Rc;

use crate::Result;
use crate::ast::Type;
use crate::symbol::FnSignature;
use crate::value::{EnumData, Value, WeakData, WeakKind};
use crate::vm::{VM, VMContext};
use crate::zen_native_fn;

mod fs;
mod iter;
mod json;
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

    // JSON serialization
    json::register(vm);

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
    vm.register_native("after", Rc::new(after_impl));
    vm.register_native("every_frame", Rc::new(every_frame_impl));

    // Option/Result helpers
    vm.register_native("is_some", Rc::new(is_some_impl));
    vm.register_native("is_none", Rc::new(is_none_impl));
    vm.register_native("is_ok", Rc::new(is_ok_impl));
    vm.register_native("is_err", Rc::new(is_err_impl));
    vm.register_native("unwrap", Rc::new(unwrap_impl));
    vm.register_native("unwrap_or", Rc::new(unwrap_or_impl));
    vm.register_native("expect", Rc::new(expect_impl));

    // Weak references
    vm.register_native("make_weak", Rc::new(make_weak_impl));
    vm.register_native("upgrade", Rc::new(upgrade_impl));
}

/// Return the list of all built-in native function names.
pub fn native_names() -> Vec<String> {
    native_fn_sigs().into_iter().map(|s| s.name).collect()
}

/// Return accurate type signatures for all native functions.
pub fn native_fn_sigs() -> Vec<FnSignature> {
    // `Type::Any` param type = compatible with everything (dynamic/native values)
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
            params: vec![("cond".into(), Type::Any)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "assert_eq".into(),
            params: vec![("a".into(), Type::Any), ("b".into(), Type::Any)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "type_of".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::Str),
        },
        FnSignature {
            type_params: vec![],
            name: "len".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::I64),
        },
        contains_impl_sig(),
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
            params: vec![("arr".into(), Type::Any), ("val".into(), Type::Any)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "pop".into(),
            params: vec![("arr".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "insert".into(),
            params: vec![
                ("arr".into(), Type::Any),
                ("idx".into(), Type::I64),
                ("val".into(), Type::Any),
            ],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "remove".into(),
            params: vec![("arr".into(), Type::Any), ("idx".into(), Type::I64)],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "to_int".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::I64),
        },
        FnSignature {
            type_params: vec![],
            name: "to_float".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::F64),
        },
        FnSignature {
            type_params: vec![],
            name: "to_str".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::Str),
        },
        FnSignature {
            type_params: vec![],
            name: "is_some".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::Bool),
        },
        FnSignature {
            type_params: vec![],
            name: "is_none".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::Bool),
        },
        FnSignature {
            type_params: vec![],
            name: "is_ok".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::Bool),
        },
        FnSignature {
            type_params: vec![],
            name: "is_err".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::Bool),
        },
        FnSignature {
            type_params: vec![],
            name: "unwrap".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "unwrap_or".into(),
            params: vec![("val".into(), Type::Any), ("default".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "expect".into(),
            params: vec![("val".into(), Type::Any), ("msg".into(), Type::Str)],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "make_weak".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "upgrade".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "iter".into(),
            params: vec![("val".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "next".into(),
            params: vec![("gen".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "set_timeout".into(),
            params: vec![
                ("callback".into(), Type::Any),
                ("seconds".into(), Type::F64),
            ],
            return_type: Some(Type::I64),
        },
        FnSignature {
            type_params: vec![],
            name: "set_interval".into(),
            params: vec![
                ("callback".into(), Type::Any),
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
        FnSignature {
            type_params: vec![],
            name: "after".into(),
            params: vec![
                ("seconds".into(), Type::Any),
                ("callback".into(), Type::Any),
            ],
            return_type: Some(Type::I64),
        },
        FnSignature {
            type_params: vec![],
            name: "every_frame".into(),
            params: vec![("callback".into(), Type::Any)],
            return_type: Some(Type::Unit),
        },
    ];
    sigs.extend(fs::signatures());
    sigs.extend(json::signatures());
    sigs.extend(map::signatures());
    sigs.extend(log::signatures());
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

fn assert_eq_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    if args.len() < 2 {
        return Ok(Value::Nil);
    }
    let vm: &crate::vm::VM = unsafe { &*ctx.raw_vm };
    if !vm.values_equal(&args[0], &args[1]) {
        return Err(crate::error::Error::Script {
            msg: format!("assert_eq failed: {:?} != {:?}", args[0], args[1]),
        });
    }
    Ok(Value::Nil)
}

fn type_of_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let name = args.first().map(|v| v.type_name()).unwrap_or("nil");
    Ok(Value::Str(name.into()))
}

// --- String operations ---

fn len_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Str(s)) => Ok(Value::Int(s.len() as i64)),
        Some(Value::Array(h)) => {
            let vm: &VM = unsafe { &*ctx.raw_vm };
            Ok(Value::Int(vm.arrays.get(*h).values.len() as i64))
        }
        Some(Value::Range(start, end, inclusive)) => {
            let len = if *inclusive {
                *end - *start + 1
            } else {
                *end - *start
            };
            Ok(Value::Int(len.max(0)))
        }
        Some(Value::Map(h)) => {
            let vm: &VM = unsafe { &*ctx.raw_vm };
            Ok(Value::Int(vm.maps.get(*h).entries.len() as i64))
        }
        _ => Ok(Value::Int(0)),
    }
}

#[zen_native_fn(name: "contains", params: [Str, Str], returns: Bool)]
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

fn push_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Array(h)), Some(val)) => {
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            vm.arrays.get_mut(*h).values.push(val.clone());
            Ok(Value::Nil)
        }
        _ => Ok(Value::Nil),
    }
}

fn pop_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Array(h)) => {
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            Ok(vm.arrays.get_mut(*h).values.pop().unwrap_or(Value::Nil))
        }
        _ => Ok(Value::Nil),
    }
}

fn insert_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1), args.get(2)) {
        (Some(Value::Array(h)), Some(Value::Int(idx)), Some(val)) => {
            let idx = *idx as usize;
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            let v = &mut vm.arrays.get_mut(*h).values;
            if idx <= v.len() {
                v.insert(idx, val.clone());
            }
            Ok(Value::Nil)
        }
        _ => Ok(Value::Nil),
    }
}

fn remove_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Array(h)), Some(Value::Int(idx))) => {
            let idx = *idx as usize;
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            let v = &mut vm.arrays.get_mut(*h).values;
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

fn to_str_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(val) => Ok(Value::Str(match val {
            Value::Nil => "nil".into(),
            Value::Bool(b) => (if *b { "true" } else { "false" }).into(),
            Value::Int(n) => format!("{n}").into(),
            Value::Float(f) => format!("{f}").into(),
            Value::Str(s) => s.clone(),
            Value::Array(_) => "[...]".into(),
            Value::Struct(_, name) => format!("{name} {{...}}").into(),
            Value::Enum(h) => {
                let vm: &VM = unsafe { &*ctx.raw_vm };
                let tag = vm.enums.get(*h).tag;
                format!("Enum({tag})").into()
            }
            _ => format!("{val:?}").into(),
        })),
        None => Ok(Value::Str("nil".into())),
    }
}

// --- Option/Result helpers ---

/// Build a `Some(v)` value using the built-in `Option` enum's convention
/// (tag 0 = Some). Shared by any stdlib module that needs to hand a
/// script-visible `Option<T>` back (iterators, map lookups, etc).
pub(crate) fn option_some(ctx: &mut VMContext, v: Value) -> Value {
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    option_some_vm(vm, v)
}

/// Build a `None` value using the built-in `Option` enum's convention (tag 1 = None).
pub(crate) fn option_none(ctx: &mut VMContext) -> Value {
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    option_none_vm(vm)
}

/// Build `Some(v)` from a `&mut VM` directly (no VMContext needed).
pub(crate) fn option_some_vm(vm: &mut VM, v: Value) -> Value {
    let h = vm.enums.insert(EnumData {
        tag: 0,
        fields: vec![v],
    });
    Value::Enum(h)
}

/// Build `None` from a `&mut VM` directly (no VMContext needed).
pub(crate) fn option_none_vm(vm: &mut VM) -> Value {
    let h = vm.enums.insert(EnumData {
        tag: 1,
        fields: vec![],
    });
    Value::Enum(h)
}

fn enum_tag(ctx: &mut VMContext, val: &Value) -> Option<u16> {
    match val {
        Value::Enum(h) => {
            let vm: &VM = unsafe { &*ctx.raw_vm };
            Some(vm.enums.get(*h).tag)
        }
        _ => None,
    }
}

fn is_some_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(
        args.first().is_some_and(|v| enum_tag(ctx, v) == Some(0)),
    ))
}

fn is_none_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(
        args.first().is_some_and(|v| enum_tag(ctx, v) == Some(1)),
    ))
}

fn is_ok_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(
        args.first().is_some_and(|v| enum_tag(ctx, v) == Some(0)),
    ))
}

fn is_err_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(
        args.first().is_some_and(|v| enum_tag(ctx, v) == Some(1)),
    ))
}

fn unwrap_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Enum(h)) => {
            let vm: &VM = unsafe { &*ctx.raw_vm };
            let data = vm.enums.get(*h);
            if data.tag == 0 {
                Ok(data.fields.first().cloned().unwrap_or(Value::Nil))
            } else {
                Err(crate::error::Error::Script {
                    msg: "unwrap failed: got None/Err".into(),
                })
            }
        }
        _ => Err(crate::error::Error::Script {
            msg: "unwrap called on non-enum value".into(),
        }),
    }
}

fn unwrap_or_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Enum(h)), Some(default)) => {
            let vm: &VM = unsafe { &*ctx.raw_vm };
            let data = vm.enums.get(*h);
            if data.tag == 0 {
                Ok(data
                    .fields
                    .first()
                    .cloned()
                    .unwrap_or_else(|| default.clone()))
            } else {
                Ok(default.clone())
            }
        }
        (_, Some(default)) => Ok(default.clone()),
        _ => Ok(Value::Nil),
    }
}

// --- Generator/Coroutine ---

fn set_timeout_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
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
            });
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
            });
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

fn after_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    if args.len() < 2 {
        return Err(crate::error::Error::Script {
            msg: "after requires a delay in seconds and a callback".into(),
        });
    }
    let seconds = match &args[0] {
        Value::Float(f) => *f,
        Value::Int(n) => *n as f64,
        _ => {
            return Err(crate::error::Error::Script {
                msg: "after seconds must be a number".into(),
            });
        }
    };
    let callback = args[1].clone();
    if !matches!(callback, Value::Function(_) | Value::Closure(_)) {
        return Err(crate::error::Error::Script {
            msg: "after second argument must be a function".into(),
        });
    }
    let id = ctx.register_timer(callback, seconds, None);
    Ok(Value::Int(id as i64))
}

fn every_frame_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let callback = args.first().cloned().unwrap_or(Value::Nil);
    if !matches!(callback, Value::Function(_) | Value::Closure(_)) {
        return Err(crate::error::Error::Script {
            msg: "every_frame requires a function argument".into(),
        });
    }
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    vm.add_frame_callback(callback);
    Ok(Value::Nil)
}

fn next_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Generator(h)) => {
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            match vm.resume_generator(*h)? {
                Some(val) => Ok(option_some(ctx, val)),
                None => Ok(option_none(ctx)),
            }
        }
        _ => Err(crate::error::Error::Script {
            msg: "next() requires a generator argument".into(),
        }),
    }
}

fn expect_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let msg = args
        .get(1)
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| "expect failed".into());
    match args.first() {
        Some(Value::Enum(h)) => {
            let vm: &VM = unsafe { &*ctx.raw_vm };
            let data = vm.enums.get(*h);
            if data.tag == 0 {
                Ok(data.fields.first().cloned().unwrap_or(Value::Nil))
            } else {
                Err(crate::error::Error::Script {
                    msg: format!("expect failed: {msg}"),
                })
            }
        }
        _ => Err(crate::error::Error::Script {
            msg: format!("expect failed: {msg}"),
        }),
    }
}

// --- Weak references ---

fn make_weak_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let val = args.first().cloned().unwrap_or(Value::Nil);
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    match &val {
        Value::Struct(h, name) => {
            let w = vm.weaks.insert(WeakData {
                kind: WeakKind::Struct,
                target: *h,
                type_name: name.clone(),
            });
            Ok(Value::Weak(w))
        }
        Value::Array(h) => {
            let w = vm.weaks.insert(WeakData {
                kind: WeakKind::Array,
                target: *h,
                type_name: String::new(),
            });
            Ok(Value::Weak(w))
        }
        Value::Map(h) => {
            let w = vm.weaks.insert(WeakData {
                kind: WeakKind::Map,
                target: *h,
                type_name: String::new(),
            });
            Ok(Value::Weak(w))
        }
        _ => Err(crate::error::Error::Script {
            msg: format!("cannot create weak reference from {}", val.type_name()),
        }),
    }
}

fn upgrade_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Weak(h)) => {
            let vm: &VM = unsafe { &*ctx.raw_vm };
            let weak = vm.weaks.get(*h);
            let target_handle = weak.target;
            if vm.weaks.is_valid(target_handle) {
                let val = match weak.kind {
                    WeakKind::Struct => Value::Struct(target_handle, weak.type_name.clone()),
                    WeakKind::Array => Value::Array(target_handle),
                    WeakKind::Map => Value::Map(target_handle),
                };
                Ok(option_some(ctx, val))
            } else {
                Ok(option_none(ctx))
            }
        }
        _ => Err(crate::error::Error::Script {
            msg: "upgrade requires a weak reference argument".into(),
        }),
    }
}
