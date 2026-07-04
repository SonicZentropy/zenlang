//! Noise functions for procedural generation in Zenlang.
//!
//! Implements value noise with smoothstep interpolation. Simple and fast,
//! suitable for game content generation.
//!
//! # Functions
//! - `perlin2d(x, y, seed)` — 2D value noise in [0, 1]
//! - `simplex2d(x, y, seed)` — 2D simplex-like noise in [0, 1] (value-noise variant)
//! - `fbm2d(x, y, octaves, seed)` — fractal Brownian motion (layered noise)
//!
//! # Example
//! ```zen
//! let n = perlin2d(1.5, 2.3, 42);
//! assert(n >= 0.0 && n <= 1.0);
//! ```

use std::rc::Rc;

use crate::error::Error;
use crate::value::Value;
use crate::vm::{VM, VMContext};

fn as_f64(v: Option<&Value>) -> crate::Result<f64> {
    v.and_then(Value::as_float).ok_or_else(|| Error::Script {
        msg: "expected a number".into(),
    })
}

fn hash2(x: i64, y: i64, seed: i64) -> f64 {
    let mut h = seed.wrapping_mul(374761393)
        .wrapping_add(x.wrapping_mul(668265263))
        .wrapping_add(y.wrapping_mul(1274126177));
    h = h.wrapping_mul(h.wrapping_mul(1274126177) ^ 0x9e3779b97f4a7c15u64 as i64);
    h = h ^ (h >> 13);
    ((h & 0x7fffffff) as f64) / 2147483648.0
}

fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn value_noise2d(x: f64, y: f64, seed: i64) -> f64 {
    let ix = x.floor() as i64;
    let iy = y.floor() as i64;
    let fx = x - x.floor();
    let fy = y - y.floor();

    let sx = smoothstep(fx);
    let sy = smoothstep(fy);

    let n00 = hash2(ix, iy, seed);
    let n10 = hash2(ix + 1, iy, seed);
    let n01 = hash2(ix, iy + 1, seed);
    let n11 = hash2(ix + 1, iy + 1, seed);

    let nx0 = lerp(n00, n10, sx);
    let nx1 = lerp(n01, n11, sx);
    lerp(nx0, nx1, sy)
}

fn perlin2d_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let x = as_f64(args.first())?;
    let y = as_f64(args.get(1))?;
    let seed = args.get(2).and_then(Value::as_int).unwrap_or(0);
    Ok(Value::Float(value_noise2d(x, y, seed)))
}

fn simplex2d_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let x = as_f64(args.first())?;
    let y = as_f64(args.get(1))?;
    let seed = args.get(2).and_then(Value::as_int).unwrap_or(0);
    // Skew for simplex-like distribution
    let s = (x + y) * 0.3660254037844386; // (sqrt(3)-1)/2
    let xi = (x + s).floor() as i64;
    let yi = (y + s).floor() as i64;
    let t = (xi + yi) as f64 * 0.21132486540518713; // (3-sqrt(3))/6
    let x0 = x - (xi as f64 - t);
    let y0 = y - (yi as f64 - t);

    let (i1, j1) = if x0 > y0 { (1i64, 0i64) } else { (0i64, 1i64) };

    let x1 = x0 - i1 as f64 + 0.21132486540518713;
    let y1 = y0 - j1 as f64 + 0.21132486540518713;
    let x2 = x0 - 1.0 + 2.0 * 0.21132486540518713;
    let y2 = y0 - 1.0 + 2.0 * 0.21132486540518713;

    let h0 = hash2(xi, yi, seed);
    let h1 = hash2(xi + i1, yi + j1, seed);
    let h2 = hash2(xi + 1, yi + 1, seed);

    let t0 = 0.5 - x0 * x0 - y0 * y0;
    let v0 = if t0 > 0.0 { t0 * t0 * t0 * t0 * h0 } else { 0.0 };
    let t1 = 0.5 - x1 * x1 - y1 * y1;
    let v1 = if t1 > 0.0 { t1 * t1 * t1 * t1 * h1 } else { 0.0 };
    let t2 = 0.5 - x2 * x2 - y2 * y2;
    let v2 = if t2 > 0.0 { t2 * t2 * t2 * t2 * h2 } else { 0.0 };

    Ok(Value::Float((v0 + v1 + v2) * 70.0))
}

fn fbm2d_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let x = as_f64(args.first())?;
    let y = as_f64(args.get(1))?;
    let octaves = args.get(2).and_then(Value::as_int).unwrap_or(4);
    let seed = args.get(3).and_then(Value::as_int).unwrap_or(0);

    let mut value = 0.0f64;
    let mut amplitude = 1.0f64;
    let mut frequency = 1.0f64;
    let mut max_val = 0.0f64;

    for _ in 0..octaves {
        value += amplitude * value_noise2d(x * frequency, y * frequency, seed);
        max_val += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    Ok(Value::Float(value / max_val))
}

pub fn register(vm: &mut VM) {
    vm.register_native("perlin2d", Rc::new(perlin2d_impl));
    vm.register_native("simplex2d", Rc::new(simplex2d_impl));
    vm.register_native("fbm2d", Rc::new(fbm2d_impl));
}

pub fn signatures() -> Vec<crate::symbol::FnSignature> {
    use crate::ast::Type;
    vec![
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "perlin2d".into(),
            params: vec![("x".into(), Type::F64), ("y".into(), Type::F64), ("seed".into(), Type::I64)],
            return_type: Some(Type::F64),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "simplex2d".into(),
            params: vec![("x".into(), Type::F64), ("y".into(), Type::F64), ("seed".into(), Type::I64)],
            return_type: Some(Type::F64),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "fbm2d".into(),
            params: vec![("x".into(), Type::F64), ("y".into(), Type::F64), ("octaves".into(), Type::I64), ("seed".into(), Type::I64)],
            return_type: Some(Type::F64),
        },
    ]
}
