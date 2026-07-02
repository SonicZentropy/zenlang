use std::cell::RefCell;
use std::rc::Rc;

use crate::Result;
use crate::value::{ForeignObject, Value};
use crate::vm::{VM, VMContext};

struct ArrayIterState {
    data: Rc<RefCell<Vec<Value>>>,
    idx: usize,
}

struct RangeIterState {
    cur: i64,
    end: i64,
    inclusive: bool,
}

struct StrIterState {
    chars: Vec<char>,
    idx: usize,
}

fn some_val(v: Value) -> Value {
    Value::Enum {
        tag: 0,
        data: Rc::new(RefCell::new(vec![v])),
    }
}

fn none_val() -> Value {
    Value::Enum {
        tag: 1,
        data: Rc::new(RefCell::new(Vec::new())),
    }
}

fn array_iter_next(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Foreign(fv)) => {
            let obj = fv.borrow();
            match obj.downcast_mut::<ArrayIterState>() {
                Some(mut state) => {
                    let item = state.data.borrow().get(state.idx).cloned();
                    match item {
                        Some(v) => {
                            state.idx += 1;
                            Ok(some_val(v))
                        }
                        None => Ok(none_val()),
                    }
                }
                None => Err(crate::error::Error::Script {
                    msg: "ArrayIter.next called on wrong foreign type".into(),
                }),
            }
        }
        _ => Err(crate::error::Error::Script {
            msg: "ArrayIter.next called without a receiver".into(),
        }),
    }
}

fn range_iter_next(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Foreign(fv)) => {
            let obj = fv.borrow();
            match obj.downcast_mut::<RangeIterState>() {
                Some(mut state) => {
                    let has_next = if state.inclusive {
                        state.cur <= state.end
                    } else {
                        state.cur < state.end
                    };
                    if has_next {
                        let v = state.cur;
                        state.cur += 1;
                        Ok(some_val(Value::Int(v)))
                    } else {
                        Ok(none_val())
                    }
                }
                None => Err(crate::error::Error::Script {
                    msg: "RangeIter.next called on wrong foreign type".into(),
                }),
            }
        }
        _ => Err(crate::error::Error::Script {
            msg: "RangeIter.next called without a receiver".into(),
        }),
    }
}

fn str_iter_next(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Foreign(fv)) => {
            let obj = fv.borrow();
            match obj.downcast_mut::<StrIterState>() {
                Some(mut state) => {
                    let item = state.chars.get(state.idx).copied();
                    match item {
                        Some(c) => {
                            state.idx += 1;
                            Ok(some_val(Value::Str(c.to_string().into())))
                        }
                        None => Ok(none_val()),
                    }
                }
                None => Err(crate::error::Error::Script {
                    msg: "StrIter.next called on wrong foreign type".into(),
                }),
            }
        }
        _ => Err(crate::error::Error::Script {
            msg: "StrIter.next called without a receiver".into(),
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
pub fn iter_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Array(arr)) => Ok(Value::Foreign(Rc::new(RefCell::new(ForeignObject::new(
            "ArrayIter",
            ArrayIterState {
                data: arr.clone(),
                idx: 0,
            },
        ))))),
        Some(Value::Range(s, e, inc)) => {
            Ok(Value::Foreign(Rc::new(RefCell::new(ForeignObject::new(
                "RangeIter",
                RangeIterState {
                    cur: *s,
                    end: *e,
                    inclusive: *inc,
                },
            )))))
        }
        Some(Value::Str(s)) => Ok(Value::Foreign(Rc::new(RefCell::new(ForeignObject::new(
            "StrIter",
            StrIterState {
                chars: s.chars().collect(),
                idx: 0,
            },
        ))))),
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
}
