//! Base64 encoding/decoding for Zenlang.
//!
//! Uses standard Base64 alphabet (RFC 4648) with `=` padding.
//!
//! # Functions
//! - `base64_encode(data)` — encode a string to Base64
//! - `base64_decode(encoded)` — decode Base64 to string (returns None on failure)
//!
//! # Example
//! ```zen
//! let enc = base64_encode("Hello, World!");
//! print(enc); // "SGVsbG8sIFdvcmxkIQ=="
//! let dec = base64_decode(enc);
//! assert(is_some(dec));
//! ```

use std::rc::Rc;

use crate::error::Error;
use crate::value::Value;
use crate::vm::{VM, VMContext};

const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let input = match args.first() {
        Some(Value::Str(s)) => s.as_ref().as_bytes(),
        _ => return Err(Error::Script { msg: "base64_encode() expects a string".into() }),
    };

    let mut out = Vec::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(B64_CHARS[((triple >> 18) & 0x3F) as usize]);
        out.push(B64_CHARS[((triple >> 12) & 0x3F) as usize]);
        out.push(if chunk.len() > 1 { B64_CHARS[((triple >> 6) & 0x3F) as usize] } else { b'=' });
        out.push(if chunk.len() > 2 { B64_CHARS[(triple & 0x3F) as usize] } else { b'=' });
    }

    Ok(Value::Str(unsafe { String::from_utf8_unchecked(out) }.into()))
}

fn base64_decode_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let input = match args.first() {
        Some(Value::Str(s)) => s.as_ref().to_string(),
        _ => return Err(Error::Script { msg: "base64_decode() expects a string".into() }),
    };

    let input = input.into_bytes();
    let mut out = Vec::new();
    let mut buf = [0u8; 4];
    let mut idx = 0usize;

    for &b in &input {
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => continue,
        };
        buf[idx] = val;
        idx += 1;
        if idx == 4 {
            let triple = ((buf[0] as u32) << 18) | ((buf[1] as u32) << 12) | ((buf[2] as u32) << 6) | (buf[3] as u32);
            out.push((triple >> 16) as u8);
            out.push((triple >> 8) as u8);
            out.push(triple as u8);
            idx = 0;
        }
    }

    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    match String::from_utf8(out) {
        Ok(s) => Ok(crate::stdlib::option_some_vm(vm, Value::Str(s.into()))),
        Err(_) => Ok(crate::stdlib::option_none_vm(vm)),
    }
}

pub fn register(vm: &mut VM) {
    vm.register_native("base64_encode", Rc::new(base64_encode_impl));
    vm.register_native("base64_decode", Rc::new(base64_decode_impl));
}

pub fn signatures() -> Vec<crate::symbol::FnSignature> {
    use crate::ast::Type;
    vec![
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "base64_encode".into(),
            params: vec![("data".into(), Type::Str)],
            return_type: Some(Type::Str),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "base64_decode".into(),
            params: vec![("encoded".into(), Type::Str)],
            return_type: Some(Type::Any),
        },
    ]
}
