// Phase 7: Rust interop layer
// TODO: Foreign type registry, field accessors, native function binding

use crate::value::NativeFn;

pub struct FieldAccessor {
    pub get: NativeFn,
    pub set: NativeFn,
}

pub struct ForeignType {
    pub name: &'static str,
    pub fields: Vec<(&'static str, FieldAccessor)>,
    pub methods: Vec<(&'static str, NativeFn)>,
}
