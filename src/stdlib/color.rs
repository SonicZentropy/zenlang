//! Color utilities for Zenlang.
//!
//! All colors are stored as 32-bit integers (0xAARRGGBB format where AA
//! is alpha in the most significant byte).
//!
//! # Functions
//! - `rgba(r, g, b, a)` — create color from 0-255 components
//! - `hsla(h, s, l, a)` — create color from HSL + alpha (h: 0-360, s/l/a: 0.0-1.0)
//! - `hex_color(hex)` — parse "#RRGGBB" or "#AARRGGBB" hex string (returns None on failure)
//! - `lerp_color(a, b, t)` — linear interpolation between two colors
//! - `color_r(color)` — extract red component (0-255)
//! - `color_g(color)` — extract green component
//! - `color_b(color)` — extract blue component
//! - `color_a(color)` — extract alpha component
//!
//! # Example
//! ```zen
//! let c = rgba(255, 128, 64, 255);
//! assert(color_r(c) == 255);
//! let c2 = hex_color("#ff8040");
//! assert(is_some(c2));
//! ```

use std::rc::Rc;

use crate::error::Error;
use crate::value::Value;
use crate::vm::{VM, VMContext};

fn rgba_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let r = args.get(0).and_then(Value::as_int).unwrap_or(0).clamp(0, 255) as u32;
    let g = args.get(1).and_then(Value::as_int).unwrap_or(0).clamp(0, 255) as u32;
    let b = args.get(2).and_then(Value::as_int).unwrap_or(0).clamp(0, 255) as u32;
    let a = args.get(3).and_then(Value::as_int).unwrap_or(255).clamp(0, 255) as u32;
    Ok(Value::Int(((a << 24) | (r << 16) | (g << 8) | b) as i64))
}

fn hsla_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = args.get(0).and_then(Value::as_float).unwrap_or(0.0);
    let s = args.get(1).and_then(Value::as_float).unwrap_or(0.0).clamp(0.0, 1.0);
    let l = args.get(2).and_then(Value::as_float).unwrap_or(0.0).clamp(0.0, 1.0);
    let a = args.get(3).and_then(Value::as_int).unwrap_or(255).clamp(0, 255) as u32;

    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = match h as i64 % 360 {
        h if h < 60 => (c, x, 0.0),
        h if h < 120 => (x, c, 0.0),
        h if h < 180 => (0.0, c, x),
        h if h < 240 => (0.0, x, c),
        h if h < 300 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    let ri = ((r + m) * 255.0).round().clamp(0.0, 255.0) as u32;
    let gi = ((g + m) * 255.0).round().clamp(0.0, 255.0) as u32;
    let bi = ((b + m) * 255.0).round().clamp(0.0, 255.0) as u32;
    Ok(Value::Int(((a << 24) | (ri << 16) | (gi << 8) | bi) as i64))
}

fn hex_color_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let s = match args.first() {
        Some(Value::Str(s)) => s.as_ref().to_string(),
        _ => return Err(Error::Script { msg: "hex_color() expects a string".into() }),
    };
    let s = s.trim_start_matches('#');
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };

    let parse = |start, end| u32::from_str_radix(&s[start..end], 16).ok();

    let val = if s.len() == 6 {
        let r = match parse(0, 2) { Some(v) => v, None => return Ok(crate::stdlib::option_none_vm(vm)) };
        let g = match parse(2, 4) { Some(v) => v, None => return Ok(crate::stdlib::option_none_vm(vm)) };
        let b = match parse(4, 6) { Some(v) => v, None => return Ok(crate::stdlib::option_none_vm(vm)) };
        (255u32 << 24) | (r << 16) | (g << 8) | b
    } else if s.len() == 8 {
        let a = match parse(0, 2) { Some(v) => v, None => return Ok(crate::stdlib::option_none_vm(vm)) };
        let r = match parse(2, 4) { Some(v) => v, None => return Ok(crate::stdlib::option_none_vm(vm)) };
        let g = match parse(4, 6) { Some(v) => v, None => return Ok(crate::stdlib::option_none_vm(vm)) };
        let b = match parse(6, 8) { Some(v) => v, None => return Ok(crate::stdlib::option_none_vm(vm)) };
        (a << 24) | (r << 16) | (g << 8) | b
    } else {
        return Ok(crate::stdlib::option_none_vm(vm));
    };
    Ok(crate::stdlib::option_some_vm(vm, Value::Int(val as i64)))
}

fn lerp_color_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let a = args.get(0).and_then(Value::as_int).unwrap_or(0) as u32;
    let b = args.get(1).and_then(Value::as_int).unwrap_or(0) as u32;
    let t = args.get(2).and_then(Value::as_float).unwrap_or(0.0).clamp(0.0, 1.0);

    let lerp_comp = |ca: u32, cb: u32| -> u32 {
        let fa = (ca & 0xFF) as f64;
        let fb = (cb & 0xFF) as f64;
        (fa + (fb - fa) * t).round() as u32
    };

    let ar = (a >> 16) & 0xFF;
    let ag = (a >> 8) & 0xFF;
    let ab = a & 0xFF;
    let aa = (a >> 24) & 0xFF;
    let br = (b >> 16) & 0xFF;
    let bg = (b >> 8) & 0xFF;
    let bb = b & 0xFF;
    let ba = (b >> 24) & 0xFF;

    let r = lerp_comp(ar, br);
    let g = lerp_comp(ag, bg);
    let bv = lerp_comp(ab, bb);
    let av = lerp_comp(aa, ba);

    Ok(Value::Int((((av << 24) | (r << 16) | (g << 8) | bv)) as i64))
}

fn color_r_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let c = args.first().and_then(Value::as_int).unwrap_or(0) as u32;
    Ok(Value::Int(((c >> 16) & 0xFF) as i64))
}

fn color_g_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let c = args.first().and_then(Value::as_int).unwrap_or(0) as u32;
    Ok(Value::Int(((c >> 8) & 0xFF) as i64))
}

fn color_b_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let c = args.first().and_then(Value::as_int).unwrap_or(0) as u32;
    Ok(Value::Int((c & 0xFF) as i64))
}

fn color_a_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let c = args.first().and_then(Value::as_int).unwrap_or(0) as u32;
    Ok(Value::Int(((c >> 24) & 0xFF) as i64))
}

pub fn register(vm: &mut VM) {
    vm.register_native("rgba", Rc::new(rgba_impl));
    vm.register_native("hsla", Rc::new(hsla_impl));
    vm.register_native("hex_color", Rc::new(hex_color_impl));
    vm.register_native("lerp_color", Rc::new(lerp_color_impl));
    vm.register_native("color_r", Rc::new(color_r_impl));
    vm.register_native("color_g", Rc::new(color_g_impl));
    vm.register_native("color_b", Rc::new(color_b_impl));
    vm.register_native("color_a", Rc::new(color_a_impl));
}

pub fn signatures() -> Vec<crate::symbol::FnSignature> {
    use crate::ast::Type;
    vec![
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "rgba".into(),
            params: vec![("r".into(), Type::I64), ("g".into(), Type::I64), ("b".into(), Type::I64), ("a".into(), Type::I64)],
            return_type: Some(Type::I64),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "hsla".into(),
            params: vec![("h".into(), Type::F64), ("s".into(), Type::F64), ("l".into(), Type::F64), ("a".into(), Type::I64)],
            return_type: Some(Type::I64),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "hex_color".into(),
            params: vec![("hex".into(), Type::Str)],
            return_type: Some(Type::I64),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "lerp_color".into(),
            params: vec![("a".into(), Type::I64), ("b".into(), Type::I64), ("t".into(), Type::F64)],
            return_type: Some(Type::I64),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "color_r".into(),
            params: vec![("color".into(), Type::I64)],
            return_type: Some(Type::I64),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "color_g".into(),
            params: vec![("color".into(), Type::I64)],
            return_type: Some(Type::I64),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "color_b".into(),
            params: vec![("color".into(), Type::I64)],
            return_type: Some(Type::I64),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "color_a".into(),
            params: vec![("color".into(), Type::I64)],
            return_type: Some(Type::I64),
        },
    ]
}
