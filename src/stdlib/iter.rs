use std::rc::Rc;

use crate::Result;
use crate::slab::Handle;
use crate::value::{ArrayData, ForeignObject, Value};
use crate::vm::{VM, VMContext};

use super::{option_none, option_some};

#[derive(Clone)]
struct ArrayIterState {
    data: Handle,
    idx: usize,
}

#[derive(Clone)]
struct RangeIterState {
    cur: i64,
    end: i64,
    inclusive: bool,
}

#[derive(Clone)]
struct StrIterState {
    chars: Vec<char>,
    idx: usize,
}

#[derive(Clone)]
struct MapIterState {
    // Snapshotted at `iter()` time so mutating the map mid-iteration can't
    // invalidate the cursor.
    entries: Vec<(Value, Value)>,
    idx: usize,
}

fn array_iter_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Foreign(h)) => {
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            let fo = vm.foreigns.get_mut(*h);
            let state: &mut ArrayIterState = fo.downcast_mut().ok_or_else(|| {
                crate::error::Error::Script {
                    msg: "ArrayIter.next called on wrong foreign type".into(),
                }
            })?;
            let item = vm.arrays.get(state.data).values.get(state.idx).cloned();
            match item {
                Some(v) => {
                    state.idx += 1;
                    Ok(option_some(ctx, v))
                }
                None => Ok(option_none(ctx)),
            }
        }
        _ => Err(crate::error::Error::Script {
            msg: "ArrayIter.next called without a receiver".into(),
        }),
    }
}

fn range_iter_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Foreign(h)) => {
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            let fo = vm.foreigns.get_mut(*h);
            let state: &mut RangeIterState = fo.downcast_mut().ok_or_else(|| {
                crate::error::Error::Script {
                    msg: "RangeIter.next called on wrong foreign type".into(),
                }
            })?;
            let has_next = if state.inclusive {
                state.cur <= state.end
            } else {
                state.cur < state.end
            };
            if has_next {
                let v = state.cur;
                state.cur += 1;
                Ok(option_some(ctx, Value::Int(v)))
            } else {
                Ok(option_none(ctx))
            }
        }
        _ => Err(crate::error::Error::Script {
            msg: "RangeIter.next called without a receiver".into(),
        }),
    }
}

fn str_iter_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Foreign(h)) => {
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            let fo = vm.foreigns.get_mut(*h);
            let state: &mut StrIterState = fo.downcast_mut().ok_or_else(|| {
                crate::error::Error::Script {
                    msg: "StrIter.next called on wrong foreign type".into(),
                }
            })?;
            let item = state.chars.get(state.idx).copied();
            match item {
                Some(c) => {
                    state.idx += 1;
                    Ok(option_some(ctx, Value::Str(c.to_string().into())))
                }
                None => Ok(option_none(ctx)),
            }
        }
        _ => Err(crate::error::Error::Script {
            msg: "StrIter.next called without a receiver".into(),
        }),
    }
}

fn map_iter_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Foreign(h)) => {
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            let fo = vm.foreigns.get_mut(*h);
            let state: &mut MapIterState = fo.downcast_mut().ok_or_else(|| {
                crate::error::Error::Script {
                    msg: "MapIter.next called on wrong foreign type".into(),
                }
            })?;
            let item = state.entries.get(state.idx).cloned();
            match item {
                Some((k, v)) => {
                    state.idx += 1;
                    let arr = vm.arrays.insert(ArrayData { values: vec![k, v] });
                    Ok(option_some(ctx, Value::Array(arr)))
                }
                None => Ok(option_none(ctx)),
            }
        }
        _ => Err(crate::error::Error::Script {
            msg: "MapIter.next called without a receiver".into(),
        }),
    }
}

/// The native `iter(x)` function: normalizes any iterable value into an
/// iterator that responds to `.next() -> Option<T>`.
///
/// Arrays, ranges, and strings are wrapped in a small foreign cursor object.
/// Structs and foreign values are passed through unchanged — they are
/// assumed to already implement the iterator protocol themselves (a struct
/// with a `next(&mut self) -> Option<T>` method, dispatched via the
/// existing struct-method machinery).
pub fn iter_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    match args.first() {
        Some(Value::Array(h)) => {
            let fh = vm.foreigns.insert(ForeignObject::new(
                "ArrayIter",
                ArrayIterState { data: *h, idx: 0 },
            ));
            Ok(Value::Foreign(fh))
        }
        Some(Value::Range(s, e, inc)) => {
            let fh = vm.foreigns.insert(ForeignObject::new(
                "RangeIter",
                RangeIterState { cur: *s, end: *e, inclusive: *inc },
            ));
            Ok(Value::Foreign(fh))
        }
        Some(Value::Str(s)) => {
            let fh = vm.foreigns.insert(ForeignObject::new(
                "StrIter",
                StrIterState { chars: s.chars().collect(), idx: 0 },
            ));
            Ok(Value::Foreign(fh))
        }
        Some(Value::Map(h)) => {
            let entries: Vec<(Value, Value)> = vm.maps.get(*h).entries.iter()
                .map(|(k, v)| (k.to_value(), v.clone()))
                .collect();
            let fh = vm.foreigns.insert(ForeignObject::new(
                "MapIter",
                MapIterState { entries, idx: 0 },
            ));
            Ok(Value::Foreign(fh))
        }
        Some(v @ Value::Struct(..)) => Ok(v.clone()),
        Some(v @ Value::Foreign(_)) => Ok(v.clone()),
        Some(other) => Err(crate::error::Error::Script {
            msg: format!("cannot iterate over value of type '{}'", other.type_name()),
        }),
        None => Err(crate::error::Error::Script {
            msg: "iter() requires an argument".into(),
        }),
    }
}

/// Register the built-in iterator foreign types (`ArrayIter`, `RangeIter`,
/// `StrIter`) and their `next` methods with the VM.
pub fn register(vm: &mut VM) {
    vm.register_type::<ArrayIterState>("ArrayIter")
        .method("next", Rc::new(array_iter_next));
    vm.register_type::<RangeIterState>("RangeIter")
        .method("next", Rc::new(range_iter_next));
    vm.register_type::<StrIterState>("StrIter")
        .method("next", Rc::new(str_iter_next));
    vm.register_type::<MapIterState>("MapIter")
        .method("next", Rc::new(map_iter_next));
}
