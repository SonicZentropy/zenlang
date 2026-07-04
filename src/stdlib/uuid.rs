//! UUID v4 generation for Zenlang.
//!
//! Generates random UUIDs (version 4) using the RNG from the math module.
//!
//! # Functions
//! - `uuid_v4()` — generate a UUID v4 string
//!
//! # Example
//! ```zen
//! let id = uuid_v4();
//! assert(type_of(id) == "str");
//! assert(len(id) == 36);
//! ```

use std::rc::Rc;

use crate::value::Value;
use crate::vm::{VM, VMContext};

fn uuid_v4_impl(_ctx: &mut VMContext, _args: &[Value]) -> crate::Result<Value> {
    let b1 = crate::stdlib::math::next_u64();
    let b2 = crate::stdlib::math::next_u64();

    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&b1.to_le_bytes());
    bytes[8..].copy_from_slice(&b2.to_le_bytes());

    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;

    let s: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    let uuid = format!(
        "{}-{}-{}-{}-{}",
        &s[0..8], &s[8..12], &s[12..16], &s[16..20], &s[20..32]
    );
    Ok(Value::Str(uuid.into()))
}

pub fn register(vm: &mut VM) {
    vm.register_native("uuid_v4", Rc::new(uuid_v4_impl));
}

pub fn signatures() -> Vec<crate::symbol::FnSignature> {
    use crate::ast::Type;
    vec![
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "uuid_v4".into(),
            params: vec![],
            return_type: Some(Type::Str),
        },
    ]
}
