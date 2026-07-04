//! Deque (double-ended queue) for Zenlang.
//!
//! Internally stored as `Value::Array`. Supports O(1) push/pop at both ends
//! (push_front uses `insert(0)`; push_back uses array append).
//!
//! # Functions
//! - `deque_new()` — create an empty deque
//! - `deque_push_front(deque, value)`
//! - `deque_push_back(deque, value)`
//! - `deque_pop_front(deque)` — remove and return front element (or nil)
//! - `deque_pop_back(deque)` — remove and return back element (or nil)
//! - `deque_peek_front(deque)` — read front without removing
//! - `deque_peek_back(deque)` — read back without removing
//! - `deque_len(deque)` — number of elements
//! - `deque_is_empty(deque)` — true if empty
//! - `deque_to_array(deque)` — convert to array (front to back)
//!
//! # Example
//! ```zen
//! let d = deque_new();
//! deque_push_back(d, 10);
//! deque_push_front(d, 5);
//! assert(deque_peek_front(d) == 5);
//! assert(deque_peek_back(d) == 10);
//! assert(deque_pop_front(d) == 5);
//! assert(deque_pop_back(d) == 10);
//! assert(deque_is_empty(d));
//! ```

use std::rc::Rc;

use crate::error::Error;
use crate::value::{ArrayData, Value};
use crate::vm::{VM, VMContext};

fn deque_new_impl(ctx: &mut VMContext, _args: &[Value]) -> crate::Result<Value> {
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(Value::Array(vm.arrays.insert(ArrayData { values: Vec::new() })))
}

fn deque_push_front_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Array(h)) => *h,
        _ => return Err(Error::Script { msg: "deque_push_front() expects a deque (array)".into() }),
    };
    let val = args.get(1).cloned().unwrap_or(Value::Nil);
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    vm.arrays.get_mut(h).values.insert(0, val);
    Ok(Value::Nil)
}

fn deque_push_back_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Array(h)) => *h,
        _ => return Err(Error::Script { msg: "deque_push_back() expects a deque (array)".into() }),
    };
    let val = args.get(1).cloned().unwrap_or(Value::Nil);
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    vm.arrays.get_mut(h).values.push(val);
    Ok(Value::Nil)
}

fn deque_pop_front_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Array(h)) => *h,
        _ => return Err(Error::Script { msg: "deque_pop_front() expects a deque (array)".into() }),
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let arr = &mut vm.arrays.get_mut(h).values;
    if arr.is_empty() {
        Ok(Value::Nil)
    } else {
        Ok(arr.remove(0))
    }
}

fn deque_pop_back_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Array(h)) => *h,
        _ => return Err(Error::Script { msg: "deque_pop_back() expects a deque (array)".into() }),
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(vm.arrays.get_mut(h).values.pop().unwrap_or(Value::Nil))
}

fn deque_peek_front_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Array(h)) => *h,
        _ => return Err(Error::Script { msg: "deque_peek_front() expects a deque (array)".into() }),
    };
    let vm: &VM = unsafe { &*ctx.raw_vm };
    Ok(vm.arrays.get(h).values.first().cloned().unwrap_or(Value::Nil))
}

fn deque_peek_back_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Array(h)) => *h,
        _ => return Err(Error::Script { msg: "deque_peek_back() expects a deque (array)".into() }),
    };
    let vm: &VM = unsafe { &*ctx.raw_vm };
    Ok(vm.arrays.get(h).values.last().cloned().unwrap_or(Value::Nil))
}

fn deque_len_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Array(h)) => *h,
        _ => return Err(Error::Script { msg: "deque_len() expects a deque (array)".into() }),
    };
    let vm: &VM = unsafe { &*ctx.raw_vm };
    Ok(Value::Int(vm.arrays.get(h).values.len() as i64))
}

fn deque_is_empty_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Array(h)) => *h,
        _ => return Err(Error::Script { msg: "deque_is_empty() expects a deque (array)".into() }),
    };
    let vm: &VM = unsafe { &*ctx.raw_vm };
    Ok(Value::Bool(vm.arrays.get(h).values.is_empty()))
}

fn deque_to_array_impl(ctx: &mut VMContext, args: &[Value]) -> crate::Result<Value> {
    let h = match args.first() {
        Some(Value::Array(h)) => *h,
        _ => return Err(Error::Script { msg: "deque_to_array() expects a deque (array)".into() }),
    };
    let vm: &VM = unsafe { &*ctx.raw_vm };
    let values = vm.arrays.get(h).values.clone();
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(Value::Array(vm.arrays.insert(ArrayData { values })))
}

pub fn register(vm: &mut VM) {
    vm.register_native("deque_new", Rc::new(deque_new_impl));
    vm.register_native("deque_push_front", Rc::new(deque_push_front_impl));
    vm.register_native("deque_push_back", Rc::new(deque_push_back_impl));
    vm.register_native("deque_pop_front", Rc::new(deque_pop_front_impl));
    vm.register_native("deque_pop_back", Rc::new(deque_pop_back_impl));
    vm.register_native("deque_peek_front", Rc::new(deque_peek_front_impl));
    vm.register_native("deque_peek_back", Rc::new(deque_peek_back_impl));
    vm.register_native("deque_len", Rc::new(deque_len_impl));
    vm.register_native("deque_is_empty", Rc::new(deque_is_empty_impl));
    vm.register_native("deque_to_array", Rc::new(deque_to_array_impl));
}

pub fn signatures() -> Vec<crate::symbol::FnSignature> {
    use crate::ast::Type;
    vec![
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "deque_new".into(),
            params: vec![],
            return_type: Some(Type::Any),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "deque_push_front".into(),
            params: vec![("deque".into(), Type::Any), ("val".into(), Type::Any)],
            return_type: Some(Type::Unit),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "deque_push_back".into(),
            params: vec![("deque".into(), Type::Any), ("val".into(), Type::Any)],
            return_type: Some(Type::Unit),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "deque_pop_front".into(),
            params: vec![("deque".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "deque_pop_back".into(),
            params: vec![("deque".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "deque_peek_front".into(),
            params: vec![("deque".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "deque_peek_back".into(),
            params: vec![("deque".into(), Type::Any)],
            return_type: Some(Type::Any),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "deque_len".into(),
            params: vec![("deque".into(), Type::Any)],
            return_type: Some(Type::I64),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "deque_is_empty".into(),
            params: vec![("deque".into(), Type::Any)],
            return_type: Some(Type::Bool),
        },
        crate::symbol::FnSignature {
            type_params: vec![],
            name: "deque_to_array".into(),
            params: vec![("deque".into(), Type::Any)],
            return_type: Some(Type::Array(Box::new(Type::Any))),
        },
    ]
}
