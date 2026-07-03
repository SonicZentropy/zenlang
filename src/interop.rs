use std::any::TypeId;
use std::collections::HashMap;
use std::rc::Rc;

use crate::error::Result;
use crate::value::{NativeFn, Value};
use crate::vm::VM;

/// Accessor for a field on a foreign type.
pub struct FieldAccessor {
    pub get: Rc<dyn Fn(&VM, &Value) -> Result<Value>>,
    pub set: Rc<dyn Fn(&mut VM, &mut Value, Value) -> Result<()>>,
}

impl FieldAccessor {
    pub fn new<G, S>(getter: G, setter: S) -> Self
    where
        G: Fn(&VM, &Value) -> Result<Value> + 'static,
        S: Fn(&mut VM, &mut Value, Value) -> Result<()> + 'static,
    {
        Self { get: Rc::new(getter), set: Rc::new(setter) }
    }

    pub fn get(&self, vm: &VM, obj: &Value) -> Result<Value> {
        (self.get)(vm, obj)
    }

    pub fn set(&self, vm: &mut VM, obj: &mut Value, val: Value) -> Result<()> {
        (self.set)(vm, obj, val)
    }
}

impl Clone for FieldAccessor {
    fn clone(&self) -> Self {
        Self { get: self.get.clone(), set: self.set.clone() }
    }
}

/// Definition of a registered foreign type.
#[derive(Clone)]
pub struct ForeignTypeDef {
    pub name: &'static str,
    pub fields: HashMap<String, FieldAccessor>,
    pub methods: HashMap<String, NativeFn>,
}

impl ForeignTypeDef {
    pub fn new(name: &'static str) -> Self {
        Self { name, fields: HashMap::new(), methods: HashMap::new() }
    }

    pub fn field<G, S>(&mut self, name: &str, getter: G, setter: S) -> &mut Self
    where
        G: Fn(&VM, &Value) -> Result<Value> + 'static,
        S: Fn(&mut VM, &mut Value, Value) -> Result<()> + 'static,
    {
        self.fields.insert(name.to_string(), FieldAccessor::new(getter, setter));
        self
    }

    pub fn method(&mut self, name: &str, f: NativeFn) -> &mut Self {
        self.methods.insert(name.to_string(), f);
        self
    }
}

/// Registry of foreign types, keyed by TypeId.
#[derive(Clone)]
pub struct ForeignTypeRegistry {
    types: HashMap<TypeId, ForeignTypeDef>,
}

impl ForeignTypeRegistry {
    pub fn new() -> Self {
        Self { types: HashMap::new() }
    }

    /// Register a foreign type with an explicit TypeId.
    pub fn register_typed(&mut self, type_id: TypeId, def: ForeignTypeDef) {
        self.types.insert(type_id, def);
    }

    pub fn get(&self, type_id: &TypeId) -> Option<&ForeignTypeDef> {
        self.types.get(type_id)
    }

    pub fn get_mut(&mut self, type_id: &TypeId) -> Option<&mut ForeignTypeDef> {
        self.types.get_mut(type_id)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&ForeignTypeDef> {
        self.types.values().find(|d| d.name == name)
    }

    /// Look up and call a method on a foreign type.
    pub fn call_method(
        &self,
        type_id: &TypeId,
        method: &str,
        ctx: &mut crate::vm::VMContext,
        args: &[Value],
    ) -> Option<Result<Value>> {
        self.types.get(type_id).and_then(|def| {
            def.methods.get(method).map(|f| f(ctx, args))
        })
    }

    /// Look up and get a field value from a foreign type.
    pub fn get_field(&self, vm: &VM, type_id: &TypeId, field: &str, obj: &Value) -> Option<Result<Value>> {
        self.types.get(type_id).and_then(|def| {
            def.fields.get(field).map(|accessor| accessor.get(vm, obj))
        })
    }

    /// Look up and set a field value on a foreign type.
    pub fn set_field(
        &self,
        vm: &mut VM,
        type_id: &TypeId,
        field: &str,
        obj: &mut Value,
        val: Value,
    ) -> Option<Result<()>> {
        self.types.get(type_id).and_then(|def| {
            def.fields.get(field).map(|accessor| accessor.set(vm, obj, val))
        })
    }
}

/// Helper to downcast a Value::Foreign to a concrete type and apply a closure.
pub fn with_foreign<T, R, F>(vm: &VM, val: &Value, f: F) -> Result<R>
where
    T: 'static,
    F: FnOnce(&T) -> Result<R>,
{
    match val {
        Value::Foreign(h) => {
            let fo = vm.foreigns.get(*h);
            let inner: &T = fo.downcast::<T>().ok_or_else(|| {
                crate::error::Error::Runtime {
                    msg: format!("type mismatch: expected {}, got {}", std::any::type_name::<T>(), fo.type_name),
                    stack_trace: Vec::new(),
                }
            })?;
            f(inner)
        }
        _ => Err(crate::error::Error::Runtime {
            msg: format!("expected foreign value, got {}", val.type_name()),
            stack_trace: Vec::new(),
        }),
    }
}

/// Helper to downcast a Value::Foreign to a concrete type and apply a mutating closure.
///
/// Unlike `with_foreign_mut`, this takes `&Value` (not `&mut Value`), relying on
/// interior mutability of the slab to provide a `&mut T`. This is useful in method
/// dispatchers where the receiver comes from an `&[Value]` slice.
pub fn with_foreign_value_mut<T, R, F>(vm: &VM, val: &Value, f: F) -> Result<R>
where
    T: 'static,
    F: FnOnce(&mut T) -> Result<R>,
{
    match val {
        Value::Foreign(h) => {
            let type_name = vm.foreigns.get(*h).type_name;
            let inner: &mut T = vm.foreigns.get_mut(*h).downcast_mut::<T>().ok_or_else(|| {
                crate::error::Error::Runtime {
                    msg: format!("type mismatch: expected {}, got {}", std::any::type_name::<T>(), type_name),
                    stack_trace: Vec::new(),
                }
            })?;
            f(inner)
        }
        _ => Err(crate::error::Error::Runtime {
            msg: format!("expected foreign value, got {}", val.type_name()),
            stack_trace: Vec::new(),
        }),
    }
}

/// Helper to downcast a Value::Foreign to a concrete type and apply a mutating closure.
/// Takes `&mut Value` and `&mut VM` for full mutable access.
pub fn with_foreign_mut<T, R, F>(vm: &mut VM, val: &mut Value, f: F) -> Result<R>
where
    T: 'static,
    F: FnOnce(&mut T) -> Result<R>,
{
    match val {
        Value::Foreign(h) => {
            let type_name = vm.foreigns.get(*h).type_name;
            let inner: &mut T = vm.foreigns.get_mut(*h).downcast_mut::<T>().ok_or_else(|| {
                crate::error::Error::Runtime {
                    msg: format!("type mismatch: expected {}, got {}", std::any::type_name::<T>(), type_name),
                    stack_trace: Vec::new(),
                }
            })?;
            f(inner)
        }
        _ => Err(crate::error::Error::Runtime {
            msg: format!("expected foreign value, got {}", val.type_name()),
            stack_trace: Vec::new(),
        }),
    }
}
