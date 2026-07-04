//! Hashing functions for Zenlang.
//!
//! Provides fast non-cryptographic hashes suitable for asset caching,
//! checksums, and hash tables.
//!
//! # Functions
//! - `fnv1a(data)` — 64-bit FNV-1a hash (returns hex string)
//! - `crc32(data)` — CRC32 checksum (returns hex string)
//! - `hash_str(data)` — SipHash-2-4 via std DefaultHasher (returns hex string)
//!
//! # Example
//! ```zen
//! let h = fnv1a("hello");
//! assert(type_of(h) == "str");
//! assert(len(h) == 16); // 64-bit = 16 hex chars
//! ```

use std::rc::Rc;
use std::hash::{Hash, Hasher};

use crate::error::Error;
use crate::value::Value;
use crate::vm::{VM, VMContext};

fn get_bytes(args: &[Value]) -> crate::Result<Vec<u8>> {
    match args.first() {
        Some(Value::Str(s)) => Ok(s.as_ref().as_bytes().to_vec()),
        _ => Err(Error::Script { msg: "expected a string".into() }),
    }
}

fn fnv1a_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let data = get_bytes(args)?;
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(Value::Str(format!("{:016x}", hash).into()))
}

fn crc32_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let data = get_bytes(args)?;
    let mut crc: u32 = 0xffffffff;
    for &b in &data {
        crc ^= b as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xedb88320;
            } else {
                crc >>= 1;
            }
        }
    }
    crc ^= 0xffffffff;
    Ok(Value::Str(format!("{:08x}", crc).into()))
}

fn hash_str_impl(_ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let data = get_bytes(args)?;
    let s = unsafe { String::from_utf8_unchecked(data) };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    let hash = hasher.finish();
    Ok(Value::Str(format!("{:016x}", hash).into()))
}

pub fn register(vm: &mut VM) {
    vm.register_native("fnv1a", Rc::new(fnv1a_impl));
    vm.register_native("crc32", Rc::new(crc32_impl));
    vm.register_native("hash_str", Rc::new(hash_str_impl));
}

pub fn signatures() -> Vec<crate::symbol::FnSignature> {
    use crate::ast::Type;
    vec![
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "fnv1a".into(),
            params: vec![("data".into(), Type::Str)],
            return_type: Some(Type::Str),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "crc32".into(),
            params: vec![("data".into(), Type::Str)],
            return_type: Some(Type::Str),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "hash_str".into(),
            params: vec![("data".into(), Type::Str)],
            return_type: Some(Type::Str),
        },
    ]
}
