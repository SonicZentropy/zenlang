//! Vector/scalar math helpers aimed at game scripting: 2D/3D vectors (as
//! plain `Vec2`/`Vec3` structs built from arrays), interpolation, clamping,
//! trigonometry, and random numbers.
//!
//! Vectors are represented as 2- or 3-element `Value::Array`s (`[x, y]` /
//! `[x, y, z]`) rather than a dedicated `Value` variant, so they work with
//! all existing array operations (indexing, `len`, iteration, etc.) for
//! free.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Result;
use crate::error::Error;
use crate::value::{ArrayData, Value};
use crate::vm::{VM, VMContext};

fn as_f64(v: Option<&Value>) -> Result<f64> {
    v.and_then(Value::as_float).ok_or_else(|| Error::Script {
        msg: "expected a number".into(),
    })
}

fn vec_components(vm: &VM, v: &Value) -> Result<Vec<f64>> {
    match v {
        Value::Array(h) => vm.arrays.get(*h).values
            .iter()
            .map(|c| {
                c.as_float().ok_or_else(|| Error::Script {
                    msg: "vector components must be numbers".into(),
                })
            })
            .collect(),
        other => Err(Error::Script {
            msg: format!("expected a vector (array), got '{}'", other.type_name()),
        }),
    }
}

fn make_vec(vm: &mut VM, components: Vec<f64>) -> Value {
    let h = vm.arrays.insert(ArrayData {
        values: components.into_iter().map(Value::Float).collect(),
    });
    Value::Array(h)
}

// --- Trig / scalar math ---

fn sin_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Float(as_f64(args.first())?.sin()))
}
fn cos_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Float(as_f64(args.first())?.cos()))
}
fn tan_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    Ok(Value::Float(as_f64(args.first())?.tan()))
}
fn atan2_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let y = as_f64(args.first())?;
    let x = as_f64(args.get(1))?;
    Ok(Value::Float(y.atan2(x)))
}

fn lerp_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let a = as_f64(args.first())?;
    let b = as_f64(args.get(1))?;
    let t = as_f64(args.get(2))?;
    Ok(Value::Float(a + (b - a) * t))
}

fn clamp_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let x = as_f64(args.first())?;
    let lo = as_f64(args.get(1))?;
    let hi = as_f64(args.get(2))?;
    Ok(Value::Float(x.clamp(lo, hi)))
}

// --- RNG (xorshift64*, seeded from the system clock; not cryptographic) ---

thread_local! {
    static RNG_STATE: RefCell<u64> = RefCell::new(seed_from_time());
}

fn seed_from_time() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9e3779b97f4a7c15);
    nanos ^ 0x2545F4914F6CDD1D
}

fn next_u64() -> u64 {
    RNG_STATE.with(|s| {
        let mut x = *s.borrow();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *s.borrow_mut() = x;
        x
    })
}

fn rand_impl(_ctx: &mut VMContext, _args: &[Value]) -> Result<Value> {
    // Uniform float in [0, 1).
    let bits = next_u64() >> 11; // 53 significant bits
    Ok(Value::Float(bits as f64 / (1u64 << 53) as f64))
}

fn rand_range_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let lo = args
        .first()
        .and_then(Value::as_int)
        .ok_or_else(|| Error::Script {
            msg: "rand_range expects int bounds".into(),
        })?;
    let hi = args
        .get(1)
        .and_then(Value::as_int)
        .ok_or_else(|| Error::Script {
            msg: "rand_range expects int bounds".into(),
        })?;
    if hi <= lo {
        return Err(Error::Script {
            msg: "rand_range: high bound must be greater than low bound".into(),
        });
    }
    let span = (hi - lo) as u64;
    Ok(Value::Int(lo + (next_u64() % span) as i64))
}

fn rand_seed_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let seed = args.first().and_then(Value::as_int).unwrap_or(0) as u64;
    RNG_STATE.with(|s| *s.borrow_mut() = seed | 1); // xorshift needs a nonzero state
    Ok(Value::Nil)
}

// --- Vector helpers (Vec2 = [x, y], Vec3 = [x, y, z]) ---

fn vec2_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let x = as_f64(args.first())?;
    let y = as_f64(args.get(1))?;
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(make_vec(vm, vec![x, y]))
}

fn vec3_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let x = as_f64(args.first())?;
    let y = as_f64(args.get(1))?;
    let z = as_f64(args.get(2))?;
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(make_vec(vm, vec![x, y, z]))
}

fn vec_add_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let vm: &VM = unsafe { &*ctx.raw_vm };
    let a = vec_components(vm, args.first().unwrap_or(&Value::Nil))?;
    let b = vec_components(vm, args.get(1).unwrap_or(&Value::Nil))?;
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(make_vec(vm, a.iter().zip(&b).map(|(x, y)| x + y).collect()))
}

fn vec_sub_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let vm: &VM = unsafe { &*ctx.raw_vm };
    let a = vec_components(vm, args.first().unwrap_or(&Value::Nil))?;
    let b = vec_components(vm, args.get(1).unwrap_or(&Value::Nil))?;
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(make_vec(vm, a.iter().zip(&b).map(|(x, y)| x - y).collect()))
}

fn vec_scale_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let vm: &VM = unsafe { &*ctx.raw_vm };
    let a = vec_components(vm, args.first().unwrap_or(&Value::Nil))?;
    let s = as_f64(args.get(1))?;
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(make_vec(vm, a.iter().map(|x| x * s).collect()))
}

fn vec_dot_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let vm: &VM = unsafe { &*ctx.raw_vm };
    let a = vec_components(vm, args.first().unwrap_or(&Value::Nil))?;
    let b = vec_components(vm, args.get(1).unwrap_or(&Value::Nil))?;
    Ok(Value::Float(a.iter().zip(&b).map(|(x, y)| x * y).sum()))
}

fn vec_len_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let vm: &VM = unsafe { &*ctx.raw_vm };
    let a = vec_components(vm, args.first().unwrap_or(&Value::Nil))?;
    Ok(Value::Float(a.iter().map(|x| x * x).sum::<f64>().sqrt()))
}

fn vec_normalize_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let vm: &VM = unsafe { &*ctx.raw_vm };
    let a = vec_components(vm, args.first().unwrap_or(&Value::Nil))?;
    let len = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    if len == 0.0 {
        return Ok(make_vec(vm, a));
    }
    Ok(make_vec(vm, a.iter().map(|x| x / len).collect()))
}

pub fn register(vm: &mut VM) {
    vm.register_native("sin", Rc::new(sin_impl));
    vm.register_native("cos", Rc::new(cos_impl));
    vm.register_native("tan", Rc::new(tan_impl));
    vm.register_native("atan2", Rc::new(atan2_impl));
    vm.register_native("lerp", Rc::new(lerp_impl));
    vm.register_native("clamp", Rc::new(clamp_impl));

    vm.register_native("rand", Rc::new(rand_impl));
    vm.register_native("rand_range", Rc::new(rand_range_impl));
    vm.register_native("rand_seed", Rc::new(rand_seed_impl));

    vm.register_native("vec2", Rc::new(vec2_impl));
    vm.register_native("vec3", Rc::new(vec3_impl));
    vm.register_native("vec_add", Rc::new(vec_add_impl));
    vm.register_native("vec_sub", Rc::new(vec_sub_impl));
    vm.register_native("vec_scale", Rc::new(vec_scale_impl));
    vm.register_native("vec_dot", Rc::new(vec_dot_impl));
    vm.register_native("vec_len", Rc::new(vec_len_impl));
    vm.register_native("vec_normalize", Rc::new(vec_normalize_impl));
}

pub fn signatures() -> Vec<crate::symbol::FnSignature> {
    use crate::ast::Type;
    use crate::symbol::FnSignature;
    vec![
        FnSignature {
            type_params: vec![],
            name: "sin".into(),
            params: vec![("x".into(), Type::F64)],
            return_type: Some(Type::F64),
        },
        FnSignature {
            type_params: vec![],
            name: "cos".into(),
            params: vec![("x".into(), Type::F64)],
            return_type: Some(Type::F64),
        },
        FnSignature {
            type_params: vec![],
            name: "tan".into(),
            params: vec![("x".into(), Type::F64)],
            return_type: Some(Type::F64),
        },
        FnSignature {
            type_params: vec![],
            name: "atan2".into(),
            params: vec![("y".into(), Type::F64), ("x".into(), Type::F64)],
            return_type: Some(Type::F64),
        },
        FnSignature {
            type_params: vec![],
            name: "lerp".into(),
            params: vec![
                ("a".into(), Type::F64),
                ("b".into(), Type::F64),
                ("t".into(), Type::F64),
            ],
            return_type: Some(Type::F64),
        },
        FnSignature {
            type_params: vec![],
            name: "clamp".into(),
            params: vec![
                ("x".into(), Type::F64),
                ("lo".into(), Type::F64),
                ("hi".into(), Type::F64),
            ],
            return_type: Some(Type::F64),
        },
        FnSignature {
            type_params: vec![],
            name: "rand".into(),
            params: vec![],
            return_type: Some(Type::F64),
        },
        FnSignature {
            type_params: vec![],
            name: "rand_range".into(),
            params: vec![("lo".into(), Type::I64), ("hi".into(), Type::I64)],
            return_type: Some(Type::I64),
        },
        FnSignature {
            type_params: vec![],
            name: "rand_seed".into(),
            params: vec![("seed".into(), Type::I64)],
            return_type: Some(Type::Unit),
        },
        FnSignature {
            type_params: vec![],
            name: "vec2".into(),
            params: vec![("x".into(), Type::F64), ("y".into(), Type::F64)],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "vec3".into(),
            params: vec![
                ("x".into(), Type::F64),
                ("y".into(), Type::F64),
                ("z".into(), Type::F64),
            ],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "vec_add".into(),
            params: vec![("a".into(), Type::Any), ("b".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "vec_sub".into(),
            params: vec![("a".into(), Type::Any), ("b".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "vec_scale".into(),
            params: vec![("a".into(), Type::Any), ("s".into(), Type::F64)],
            return_type: Some(Type::Any),
        },
        FnSignature {
            type_params: vec![],
            name: "vec_dot".into(),
            params: vec![("a".into(), Type::Any), ("b".into(), Type::Any)],
            return_type: Some(Type::F64),
        },
        FnSignature {
            type_params: vec![],
            name: "vec_len".into(),
            params: vec![("a".into(), Type::Any)],
            return_type: Some(Type::F64),
        },
        FnSignature {
            type_params: vec![],
            name: "vec_normalize".into(),
            params: vec![("a".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
    ]
}
