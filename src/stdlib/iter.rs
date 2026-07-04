use std::rc::Rc;

use crate::error::{Error, Result};
use crate::slab::Handle;
use crate::value::{ArrayData, ForeignObject, Value};
use crate::vm::{VM, VMContext};

use super::{option_none_vm, option_some_vm};

// ---------------------------------------------------------------------------
// Helper: extract the inner value from an Option<T> enum (tag 0 = Some)
// ---------------------------------------------------------------------------

fn extract_option_value(vm: &VM, val: &Value) -> Option<Value> {
    match val {
        Value::Enum(h) => {
            let data = vm.enums.get(*h);
            if data.tag == 0 {
                data.fields.first().cloned()
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Call `source.next()` and return the raw `Option<T>` Value.
fn call_source_next(ctx: &mut VMContext, source_h: Handle) -> Result<Value> {
    let vm: &VM = unsafe { &*ctx.raw_vm };
    let fo = vm.foreigns.get(source_h);
    let type_id = fo.type_id;
    let registry = ctx.registry.clone();
    match registry.call_method(&type_id, "next", ctx, &[Value::Foreign(source_h)]) {
        Some(Ok(val)) => Ok(val),
        Some(Err(e)) => Err(e),
        None => Err(Error::Runtime {
            msg: "iterator source has no 'next' method".into(),
            stack_trace: Vec::new(),
        }),
    }
}

/// Normalize any value into a foreign iterator handle.
/// Arrays, ranges, strings, and maps are wrapped via `iter()`.
/// Foreign values are assumed to already implement `.next()` and passed through.
fn ensure_iterator(ctx: &mut VMContext, val: &Value) -> Result<Handle> {
    match val {
        Value::Foreign(h) => Ok(*h),
        _ => {
            let result = iter_impl(ctx, &[val.clone()])?;
            match &result {
                Value::Foreign(h) => Ok(*h),
                _ => Err(Error::Runtime {
                    msg: "iter() did not return a foreign iterator".into(),
                    stack_trace: Vec::new(),
                }),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Existing iterator state types (kept from original)
// ---------------------------------------------------------------------------

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
    entries: Vec<(Value, Value)>,
    idx: usize,
}

// ---------------------------------------------------------------------------
// Lazy adapter state types
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct LazyMapIter {
    source: Handle,
    f: Value,
}

#[derive(Clone)]
struct LazyFilterIter {
    source: Handle,
    pred: Value,
}

#[derive(Clone)]
struct LazyTakeIter {
    source: Handle,
    remaining: usize,
}

#[derive(Clone)]
struct LazySkipIter {
    source: Handle,
    remaining: usize,
}

#[derive(Clone)]
struct LazyChainIter {
    first: Handle,
    second: Handle,
    on_first: bool,
}

#[derive(Clone)]
struct LazyZipIter {
    a: Handle,
    b: Handle,
}

#[derive(Clone)]
struct LazyEnumerateIter {
    source: Handle,
    idx: usize,
}

#[derive(Clone)]
struct LazyStepByIter {
    source: Handle,
    step: usize,
}

#[derive(Clone)]
enum CyclePhase {
    Collecting,
    Replaying { idx: usize },
}

#[derive(Clone)]
struct LazyCycleIter {
    source: Handle,
    saved: Vec<Value>,
    phase: CyclePhase,
}

#[derive(Clone)]
struct LazyInspectIter {
    source: Handle,
    f: Value,
}

#[derive(Clone)]
struct LazyFlattenIter {
    source: Handle,
    current: Option<Handle>,
}

#[derive(Clone)]
struct LazyFlatMapIter {
    source: Handle,
    f: Value,
    current: Option<Handle>,
}

#[derive(Clone)]
struct LazyScanIter {
    source: Handle,
    f: Value,
    acc: Value,
}

// ---------------------------------------------------------------------------
// Existing .next() methods
// ---------------------------------------------------------------------------

fn array_iter_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Foreign(h)) => {
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            let fo = vm.foreigns.get_mut(*h);
            let state: &mut ArrayIterState =
                fo.downcast_mut()
                    .ok_or_else(|| Error::Script {
                        msg: "ArrayIter.next called on wrong foreign type".into(),
                    })?;
            let item = vm.arrays.get(state.data).values.get(state.idx).cloned();
            match item {
                Some(v) => {
                    state.idx += 1;
                    Ok(option_some_vm(vm, v))
                }
                None => Ok(option_none_vm(vm)),
            }
        }
        _ => Err(Error::Script {
            msg: "ArrayIter.next called without a receiver".into(),
        }),
    }
}

fn range_iter_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Foreign(h)) => {
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            let fo = vm.foreigns.get_mut(*h);
            let state: &mut RangeIterState =
                fo.downcast_mut()
                    .ok_or_else(|| Error::Script {
                        msg: "RangeIter.next called on wrong foreign type".into(),
                    })?;
            let has_next = if state.inclusive {
                state.cur <= state.end
            } else {
                state.cur < state.end
            };
            if has_next {
                let v = state.cur;
                state.cur += 1;
                Ok(option_some_vm(vm, Value::Int(v)))
            } else {
                Ok(option_none_vm(vm))
            }
        }
        _ => Err(Error::Script {
            msg: "RangeIter.next called without a receiver".into(),
        }),
    }
}

fn str_iter_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Foreign(h)) => {
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            let fo = vm.foreigns.get_mut(*h);
            let state: &mut StrIterState =
                fo.downcast_mut()
                    .ok_or_else(|| Error::Script {
                        msg: "StrIter.next called on wrong foreign type".into(),
                    })?;
            let item = state.chars.get(state.idx).copied();
            match item {
                Some(c) => {
                    state.idx += 1;
                    Ok(option_some_vm(vm, Value::Str(c.to_string().into())))
                }
                None => Ok(option_none_vm(vm)),
            }
        }
        _ => Err(Error::Script {
            msg: "StrIter.next called without a receiver".into(),
        }),
    }
}

fn map_iter_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    match args.first() {
        Some(Value::Foreign(h)) => {
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            let fo = vm.foreigns.get_mut(*h);
            let state: &mut MapIterState =
                fo.downcast_mut()
                    .ok_or_else(|| Error::Script {
                        msg: "MapIter.next called on wrong foreign type".into(),
                    })?;
            let item = state.entries.get(state.idx).cloned();
            match item {
                Some((k, v)) => {
                    state.idx += 1;
                    let arr = vm.arrays.insert(ArrayData { values: vec![k, v] });
                    Ok(option_some_vm(vm, Value::Array(arr)))
                }
                None => Ok(option_none_vm(vm)),
            }
        }
        _ => Err(Error::Script {
            msg: "MapIter.next called without a receiver".into(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Lazy adapter .next() methods
// ---------------------------------------------------------------------------

fn lazy_map_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let self_h = match args.first() {
        Some(Value::Foreign(h)) => *h,
        _ => {
            return Err(Error::Script {
                msg: "LazyMapIter.next called without a receiver".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let (source_h, f) = {
        let fo = vm.foreigns.get_mut(self_h);
        let state: &mut LazyMapIter = fo.downcast_mut().ok_or_else(|| Error::Script {
            msg: "LazyMapIter.next called on wrong foreign type".into(),
        })?;
        (state.source, state.f.clone())
    };
    let next = call_source_next(ctx, source_h)?;
    match extract_option_value(vm, &next) {
        Some(inner) => {
            let result = ctx.call_value(&f, &[inner])?;
            Ok(option_some_vm(vm, result))
        }
        None => Ok(option_none_vm(vm)),
    }
}

fn lazy_filter_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let self_h = match args.first() {
        Some(Value::Foreign(h)) => *h,
        _ => {
            return Err(Error::Script {
                msg: "LazyFilterIter.next called without a receiver".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = {
        let fo = vm.foreigns.get_mut(self_h);
        let state: &mut LazyFilterIter = fo.downcast_mut().ok_or_else(|| Error::Script {
            msg: "LazyFilterIter.next called on wrong foreign type".into(),
        })?;
        state.source
    };
    let pred = {
        let fo = vm.foreigns.get_mut(self_h);
        fo.downcast_mut::<LazyFilterIter>().ok_or_else(|| Error::Script {
            msg: "LazyFilterIter.next called on wrong foreign type".into(),
        })?
        .pred
        .clone()
    };
    loop {
        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(v) => {
                let ok = ctx.call_value(&pred, &[v.clone()])?;
                if ok.is_truthy() {
                    return Ok(option_some_vm(vm, v));
                }
            }
            None => return Ok(option_none_vm(vm)),
        }
    }
}

fn lazy_take_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let self_h = match args.first() {
        Some(Value::Foreign(h)) => *h,
        _ => {
            return Err(Error::Script {
                msg: "LazyTakeIter.next called without a receiver".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = {
        let fo = vm.foreigns.get_mut(self_h);
        let state: &mut LazyTakeIter = fo.downcast_mut().ok_or_else(|| Error::Script {
            msg: "LazyTakeIter.next called on wrong foreign type".into(),
        })?;
        if state.remaining == 0 {
            return Ok(option_none_vm(vm));
        }
        state.remaining -= 1;
        state.source
    };
    let next = call_source_next(ctx, source_h)?;
    match extract_option_value(vm, &next) {
        Some(inner) => Ok(option_some_vm(vm, inner)),
        None => Ok(option_none_vm(vm)),
    }
}

fn lazy_skip_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let self_h = match args.first() {
        Some(Value::Foreign(h)) => *h,
        _ => {
            return Err(Error::Script {
                msg: "LazySkipIter.next called without a receiver".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };

    // Drain remaining skip count
    let source_h = {
        let fo = vm.foreigns.get_mut(self_h);
        let state: &mut LazySkipIter = fo.downcast_mut().ok_or_else(|| Error::Script {
            msg: "LazySkipIter.next called on wrong foreign type".into(),
        })?;
        state.source
    };
    let skip_count = {
        let fo = vm.foreigns.get_mut(self_h);
        let state: &mut LazySkipIter = fo.downcast_mut().ok_or_else(|| Error::Script {
            msg: "LazySkipIter.next called on wrong foreign type".into(),
        })?;
        state.remaining
    };
    if skip_count > 0 {
        for _ in 0..skip_count {
            let next = call_source_next(ctx, source_h)?;
            if extract_option_value(vm, &next).is_none() {
                break;
            }
        }
        {
            let fo = vm.foreigns.get_mut(self_h);
            let state: &mut LazySkipIter = fo.downcast_mut().ok_or_else(|| Error::Script {
                msg: "LazySkipIter.next called on wrong foreign type".into(),
            })?;
            state.remaining = 0;
        }
    }

    let next = call_source_next(ctx, source_h)?;
    match extract_option_value(vm, &next) {
        Some(inner) => Ok(option_some_vm(vm, inner)),
        None => Ok(option_none_vm(vm)),
    }
}

fn lazy_chain_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let self_h = match args.first() {
        Some(Value::Foreign(h)) => *h,
        _ => {
            return Err(Error::Script {
                msg: "LazyChainIter.next called without a receiver".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };

    // Get source handles
    let (first_h, second_h) = {
        let fo = vm.foreigns.get_mut(self_h);
        let state: &mut LazyChainIter = fo.downcast_mut().ok_or_else(|| Error::Script {
            msg: "LazyChainIter.next called on wrong foreign type".into(),
        })?;
        (state.first, state.second)
    };

    // Check which phase we're in
    let on_first = {
        let fo = vm.foreigns.get_mut(self_h);
        let state: &mut LazyChainIter = fo.downcast_mut().ok_or_else(|| Error::Script {
            msg: "LazyChainIter.next called on wrong foreign type".into(),
        })?;
        state.on_first
    };

    if on_first {
        let next = call_source_next(ctx, first_h)?;
        match extract_option_value(vm, &next) {
            Some(v) => Ok(option_some_vm(vm, v)),
            None => {
                {
                    let fo = vm.foreigns.get_mut(self_h);
                    let state: &mut LazyChainIter =
                        fo.downcast_mut().ok_or_else(|| Error::Script {
                            msg: "LazyChainIter.next called on wrong foreign type".into(),
                        })?;
                    state.on_first = false;
                }
                let next = call_source_next(ctx, second_h)?;
                match extract_option_value(vm, &next) {
                    Some(v) => Ok(option_some_vm(vm, v)),
                    None => Ok(option_none_vm(vm)),
                }
            }
        }
    } else {
        let next = call_source_next(ctx, second_h)?;
        match extract_option_value(vm, &next) {
            Some(v) => Ok(option_some_vm(vm, v)),
            None => Ok(option_none_vm(vm)),
        }
    }
}

fn lazy_zip_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let self_h = match args.first() {
        Some(Value::Foreign(h)) => *h,
        _ => {
            return Err(Error::Script {
                msg: "LazyZipIter.next called without a receiver".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let (a_h, b_h) = {
        let fo = vm.foreigns.get_mut(self_h);
        let state: &mut LazyZipIter = fo.downcast_mut().ok_or_else(|| Error::Script {
            msg: "LazyZipIter.next called on wrong foreign type".into(),
        })?;
        (state.a, state.b)
    };
    let a_next = call_source_next(ctx, a_h)?;
    let b_next = call_source_next(ctx, b_h)?;
    match (extract_option_value(vm, &a_next), extract_option_value(vm, &b_next)) {
        (Some(av), Some(bv)) => {
            let arr = vm.arrays.insert(ArrayData { values: vec![av, bv] });
            Ok(option_some_vm(vm, Value::Array(arr)))
        }
        _ => Ok(option_none_vm(vm)),
    }
}

fn lazy_enumerate_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let self_h = match args.first() {
        Some(Value::Foreign(h)) => *h,
        _ => {
            return Err(Error::Script {
                msg: "LazyEnumerateIter.next called without a receiver".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = {
        let fo = vm.foreigns.get_mut(self_h);
        let state: &mut LazyEnumerateIter = fo.downcast_mut().ok_or_else(|| Error::Script {
            msg: "LazyEnumerateIter.next called on wrong foreign type".into(),
        })?;
        state.source
    };
    let next = call_source_next(ctx, source_h)?;
    match extract_option_value(vm, &next) {
        Some(inner) => {
            let idx = {
                let fo = vm.foreigns.get_mut(self_h);
                let state: &mut LazyEnumerateIter =
                    fo.downcast_mut().ok_or_else(|| Error::Script {
                        msg: "LazyEnumerateIter.next called on wrong foreign type".into(),
                    })?;
                let i = state.idx;
                state.idx += 1;
                i
            };
            let arr = vm.arrays.insert(ArrayData {
                values: vec![Value::Int(idx as i64), inner],
            });
            Ok(option_some_vm(vm, Value::Array(arr)))
        }
        None => Ok(option_none_vm(vm)),
    }
}

fn lazy_step_by_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let self_h = match args.first() {
        Some(Value::Foreign(h)) => *h,
        _ => {
            return Err(Error::Script {
                msg: "LazyStepByIter.next called without a receiver".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let (source_h, step) = {
        let fo = vm.foreigns.get_mut(self_h);
        let state: &mut LazyStepByIter = fo.downcast_mut().ok_or_else(|| Error::Script {
            msg: "LazyStepByIter.next called on wrong foreign type".into(),
        })?;
        (state.source, state.step)
    };
    // Return first element
    let first = call_source_next(ctx, source_h)?;
    let first_val = match extract_option_value(vm, &first) {
        Some(v) => v,
        None => return Ok(option_none_vm(vm)),
    };
    // Skip step-1 elements
    if step > 1 {
        for _ in 0..(step - 1) {
            let next = call_source_next(ctx, source_h)?;
            if extract_option_value(vm, &next).is_none() {
                break;
            }
        }
    }
    Ok(option_some_vm(vm, first_val))
}

fn lazy_cycle_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let self_h = match args.first() {
        Some(Value::Foreign(h)) => *h,
        _ => {
            return Err(Error::Script {
                msg: "LazyCycleIter.next called without a receiver".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };

    // Get current phase info
    let (source_h, phase, saved_len) = {
        let fo = vm.foreigns.get_mut(self_h);
        let state: &mut LazyCycleIter = fo.downcast_mut().ok_or_else(|| Error::Script {
            msg: "LazyCycleIter.next called on wrong foreign type".into(),
        })?;
        let phase = state.phase.clone();
        let saved_len = state.saved.len();
        (state.source, phase, saved_len)
    };

    match phase {
        CyclePhase::Collecting => {
            let next = call_source_next(ctx, source_h)?;
            match extract_option_value(vm, &next) {
                Some(v) => {
                    // Save and return
                    {
                        let fo = vm.foreigns.get_mut(self_h);
                        let state: &mut LazyCycleIter =
                            fo.downcast_mut().ok_or_else(|| Error::Script {
                                msg: "LazyCycleIter.next called on wrong foreign type".into(),
                            })?;
                        state.saved.push(v.clone());
                    }
                    Ok(option_some_vm(vm, v))
                }
                None => {
                    // Source exhausted, switch to replaying
                    if saved_len == 0 {
                        return Ok(option_none_vm(vm));
                    }
                    {
                        let fo = vm.foreigns.get_mut(self_h);
                        let state: &mut LazyCycleIter =
                            fo.downcast_mut().ok_or_else(|| Error::Script {
                                msg: "LazyCycleIter.next called on wrong foreign type".into(),
                            })?;
                        state.phase = CyclePhase::Replaying { idx: 0 };
                    }
                    // Return first saved element
                    {
                        let fo = vm.foreigns.get_mut(self_h);
                        let state: &mut LazyCycleIter =
                            fo.downcast_mut().ok_or_else(|| Error::Script {
                                msg: "LazyCycleIter.next called on wrong foreign type".into(),
                            })?;
                        let idx = match &state.phase {
                            CyclePhase::Replaying { idx } => *idx,
                            _ => unreachable!(),
                        };
                        state.phase = CyclePhase::Replaying { idx: idx + 1 };
                        Ok(option_some_vm(vm, state.saved[idx].clone()))
                    }
                }
            }
        }
        CyclePhase::Replaying { idx } => {
            {
                let fo = vm.foreigns.get_mut(self_h);
                let state: &mut LazyCycleIter =
                    fo.downcast_mut().ok_or_else(|| Error::Script {
                        msg: "LazyCycleIter.next called on wrong foreign type".into(),
                    })?;
                if idx < state.saved.len() {
                    let v = state.saved[idx].clone();
                    state.phase = CyclePhase::Replaying { idx: idx + 1 };
                    Ok(option_some_vm(vm, v))
                } else {
                    // Wrap around
                    state.phase = CyclePhase::Replaying { idx: 1 };
                    Ok(option_some_vm(vm, state.saved[0].clone()))
                }
            }
        }
    }
}

fn lazy_inspect_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let self_h = match args.first() {
        Some(Value::Foreign(h)) => *h,
        _ => {
            return Err(Error::Script {
                msg: "LazyInspectIter.next called without a receiver".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = {
        let fo = vm.foreigns.get_mut(self_h);
        let state: &mut LazyInspectIter = fo.downcast_mut().ok_or_else(|| Error::Script {
            msg: "LazyInspectIter.next called on wrong foreign type".into(),
        })?;
        state.source
    };
    let f = {
        let fo = vm.foreigns.get_mut(self_h);
        fo.downcast_mut::<LazyInspectIter>()
            .ok_or_else(|| Error::Script {
                msg: "LazyInspectIter.next called on wrong foreign type".into(),
            })?
            .f
            .clone()
    };
    let next = call_source_next(ctx, source_h)?;
    match extract_option_value(vm, &next) {
        Some(v) => {
            let _ = ctx.call_value(&f, &[v.clone()]);
            Ok(option_some_vm(vm, v))
        }
        None => Ok(option_none_vm(vm)),
    }
}

fn lazy_flatten_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let self_h = match args.first() {
        Some(Value::Foreign(h)) => *h,
        _ => {
            return Err(Error::Script {
                msg: "LazyFlattenIter.next called without a receiver".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };

    loop {
        // Try to get next element from current inner iterator
        let maybe_inner = {
            let fo = vm.foreigns.get_mut(self_h);
            let state: &mut LazyFlattenIter = fo.downcast_mut().ok_or_else(|| Error::Script {
                msg: "LazyFlattenIter.next called on wrong foreign type".into(),
            })?;
            state.current
        };

        if let Some(inner_h) = maybe_inner {
            let inner_next = call_source_next(ctx, inner_h)?;
            match extract_option_value(vm, &inner_next) {
                Some(v) => return Ok(option_some_vm(vm, v)),
                None => {
                    // Inner exhausted, clear it and get next from source
                    {
                        let fo = vm.foreigns.get_mut(self_h);
                        let state: &mut LazyFlattenIter =
                            fo.downcast_mut().ok_or_else(|| Error::Script {
                                msg: "LazyFlattenIter.next called on wrong foreign type".into(),
                            })?;
                        state.current = None;
                    }
                }
            }
        }

        // Get next element from source
        let source_h = {
            let fo = vm.foreigns.get_mut(self_h);
            let state: &mut LazyFlattenIter = fo.downcast_mut().ok_or_else(|| Error::Script {
                msg: "LazyFlattenIter.next called on wrong foreign type".into(),
            })?;
            state.source
        };

        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(v) => {
                // Call iter() on the element to get an inner iterator
                let inner_result = iter_impl(ctx, &[v])?;
                match inner_result {
                    Value::Foreign(inner_h) => {
                        {
                            let fo = vm.foreigns.get_mut(self_h);
                            let state: &mut LazyFlattenIter =
                                fo.downcast_mut().ok_or_else(|| Error::Script {
                                    msg: "LazyFlattenIter.next called on wrong foreign type"
                                        .into(),
                                })?;
                            state.current = Some(inner_h);
                        }
                        // Loop back to try getting from inner
                    }
                    _ => {
                        return Err(Error::Script {
                            msg: "flatten: element is not iterable".into(),
                        })
                    }
                }
            }
            None => return Ok(option_none_vm(vm)),
        }
    }
}

fn lazy_flat_map_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let self_h = match args.first() {
        Some(Value::Foreign(h)) => *h,
        _ => {
            return Err(Error::Script {
                msg: "LazyFlatMapIter.next called without a receiver".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };

    loop {
        // Try to get next element from current inner iterator
        let maybe_inner = {
            let fo = vm.foreigns.get_mut(self_h);
            let state: &mut LazyFlatMapIter = fo.downcast_mut().ok_or_else(|| Error::Script {
                msg: "LazyFlatMapIter.next called on wrong foreign type".into(),
            })?;
            state.current
        };

        if let Some(inner_h) = maybe_inner {
            let inner_next = call_source_next(ctx, inner_h)?;
            match extract_option_value(vm, &inner_next) {
                Some(v) => return Ok(option_some_vm(vm, v)),
                None => {
                    {
                        let fo = vm.foreigns.get_mut(self_h);
                        let state: &mut LazyFlatMapIter =
                            fo.downcast_mut().ok_or_else(|| Error::Script {
                                msg: "LazyFlatMapIter.next called on wrong foreign type".into(),
                            })?;
                        state.current = None;
                    }
                }
            }
        }

        // Get next element from source, apply f
        let source_h = {
            let fo = vm.foreigns.get_mut(self_h);
            let state: &mut LazyFlatMapIter = fo.downcast_mut().ok_or_else(|| Error::Script {
                msg: "LazyFlatMapIter.next called on wrong foreign type".into(),
            })?;
            state.source
        };
        let f = {
            let fo = vm.foreigns.get_mut(self_h);
            fo.downcast_mut::<LazyFlatMapIter>()
                .ok_or_else(|| Error::Script {
                    msg: "LazyFlatMapIter.next called on wrong foreign type".into(),
                })?
                .f
                .clone()
        };

        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(v) => {
                let mapped = ctx.call_value(&f, &[v])?;
                let inner_result = iter_impl(ctx, &[mapped])?;
                match inner_result {
                    Value::Foreign(inner_h) => {
                        {
                            let fo = vm.foreigns.get_mut(self_h);
                            let state: &mut LazyFlatMapIter =
                                fo.downcast_mut().ok_or_else(|| Error::Script {
                                    msg: "LazyFlatMapIter.next called on wrong foreign type"
                                        .into(),
                                })?;
                            state.current = Some(inner_h);
                        }
                    }
                    _ => {
                        return Err(Error::Script {
                            msg: "flat_map: f did not return an iterable value".into(),
                        })
                    }
                }
            }
            None => return Ok(option_none_vm(vm)),
        }
    }
}

fn lazy_scan_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let self_h = match args.first() {
        Some(Value::Foreign(h)) => *h,
        _ => {
            return Err(Error::Script {
                msg: "LazyScanIter.next called without a receiver".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = {
        let fo = vm.foreigns.get_mut(self_h);
        let state: &mut LazyScanIter = fo.downcast_mut().ok_or_else(|| Error::Script {
            msg: "LazyScanIter.next called on wrong foreign type".into(),
        })?;
        state.source
    };
    let next = call_source_next(ctx, source_h)?;
    match extract_option_value(vm, &next) {
        Some(v) => {
            let f = {
                let fo = vm.foreigns.get_mut(self_h);
                fo.downcast_mut::<LazyScanIter>()
                    .ok_or_else(|| Error::Script {
                        msg: "LazyScanIter.next called on wrong foreign type".into(),
                    })?
                    .f
                    .clone()
            };
            let acc = {
                let fo = vm.foreigns.get_mut(self_h);
                fo.downcast_mut::<LazyScanIter>()
                    .ok_or_else(|| Error::Script {
                        msg: "LazyScanIter.next called on wrong foreign type".into(),
                    })?
                    .acc
                    .clone()
            };
            let new_acc = ctx.call_value(&f, &[acc, v])?;
            {
                let fo = vm.foreigns.get_mut(self_h);
                let state: &mut LazyScanIter =
                    fo.downcast_mut().ok_or_else(|| Error::Script {
                        msg: "LazyScanIter.next called on wrong foreign type".into(),
                    })?;
                state.acc = new_acc.clone();
            }
            Ok(option_some_vm(vm, new_acc))
        }
        None => Ok(option_none_vm(vm)),
    }
}

// ---------------------------------------------------------------------------
// The native `iter(x)` function (normalizes any iterable)
// ---------------------------------------------------------------------------

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
                RangeIterState {
                    cur: *s,
                    end: *e,
                    inclusive: *inc,
                },
            ));
            Ok(Value::Foreign(fh))
        }
        Some(Value::Str(s)) => {
            let fh = vm.foreigns.insert(ForeignObject::new(
                "StrIter",
                StrIterState {
                    chars: s.chars().collect(),
                    idx: 0,
                },
            ));
            Ok(Value::Foreign(fh))
        }
        Some(Value::Map(h)) => {
            let entries: Vec<(Value, Value)> = vm
                .maps
                .get(*h)
                .entries
                .iter()
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
        Some(other) => Err(Error::Script {
            msg: format!("cannot iterate over value of type '{}'", other.type_name()),
        }),
        None => Err(Error::Script {
            msg: "iter() requires an argument".into(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Lazy adapter constructors (native functions)
// ---------------------------------------------------------------------------

pub fn map_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, f) = match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => (a.clone(), b.clone()),
        _ => {
            return Err(Error::Script {
                msg: "map requires 2 arguments: iterable and function".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let fh = vm.foreigns.insert(ForeignObject::new(
        "LazyMapIter",
        LazyMapIter {
            source: source_h,
            f,
        },
    ));
    Ok(Value::Foreign(fh))
}

pub fn filter_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, pred) = match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => (a.clone(), b.clone()),
        _ => {
            return Err(Error::Script {
                msg: "filter requires 2 arguments: iterable and predicate".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let fh = vm.foreigns.insert(ForeignObject::new(
        "LazyFilterIter",
        LazyFilterIter {
            source: source_h,
            pred,
        },
    ));
    Ok(Value::Foreign(fh))
}

pub fn take_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, n) = match (args.first(), args.get(1)) {
        (Some(a), Some(Value::Int(n))) => (a.clone(), *n),
        (Some(_), _) => {
            return Err(Error::Script {
                msg: "take requires an integer count as second argument".into(),
            })
        }
        _ => {
            return Err(Error::Script {
                msg: "take requires 2 arguments: iterable and count".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let fh = vm.foreigns.insert(ForeignObject::new(
        "LazyTakeIter",
        LazyTakeIter {
            source: source_h,
            remaining: n as usize,
        },
    ));
    Ok(Value::Foreign(fh))
}

pub fn skip_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, n) = match (args.first(), args.get(1)) {
        (Some(a), Some(Value::Int(n))) => (a.clone(), *n),
        (Some(_), _) => {
            return Err(Error::Script {
                msg: "skip requires an integer count as second argument".into(),
            })
        }
        _ => {
            return Err(Error::Script {
                msg: "skip requires 2 arguments: iterable and count".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let fh = vm.foreigns.insert(ForeignObject::new(
        "LazySkipIter",
        LazySkipIter {
            source: source_h,
            remaining: n as usize,
        },
    ));
    Ok(Value::Foreign(fh))
}

pub fn chain_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (a, b) = match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => (a.clone(), b.clone()),
        _ => {
            return Err(Error::Script {
                msg: "chain requires 2 arguments".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let first_h = ensure_iterator(ctx, &a)?;
    let second_h = ensure_iterator(ctx, &b)?;
    let fh = vm.foreigns.insert(ForeignObject::new(
        "LazyChainIter",
        LazyChainIter {
            first: first_h,
            second: second_h,
            on_first: true,
        },
    ));
    Ok(Value::Foreign(fh))
}

pub fn zip_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (a, b) = match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => (a.clone(), b.clone()),
        _ => {
            return Err(Error::Script {
                msg: "zip requires 2 arguments".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let a_h = ensure_iterator(ctx, &a)?;
    let b_h = ensure_iterator(ctx, &b)?;
    let fh = vm.foreigns.insert(ForeignObject::new(
        "LazyZipIter",
        LazyZipIter { a: a_h, b: b_h },
    ));
    Ok(Value::Foreign(fh))
}

pub fn enumerate_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let iterable = match args.first() {
        Some(a) => a.clone(),
        _ => {
            return Err(Error::Script {
                msg: "enumerate requires 1 argument".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let fh = vm.foreigns.insert(ForeignObject::new(
        "LazyEnumerateIter",
        LazyEnumerateIter {
            source: source_h,
            idx: 0,
        },
    ));
    Ok(Value::Foreign(fh))
}

pub fn step_by_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, step) = match (args.first(), args.get(1)) {
        (Some(a), Some(Value::Int(step))) => (a.clone(), *step),
        (Some(_), _) => {
            return Err(Error::Script {
                msg: "step_by requires an integer step as second argument".into(),
            })
        }
        _ => {
            return Err(Error::Script {
                msg: "step_by requires 2 arguments: iterable and step".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let fh = vm.foreigns.insert(ForeignObject::new(
        "LazyStepByIter",
        LazyStepByIter {
            source: source_h,
            step: step as usize,
        },
    ));
    Ok(Value::Foreign(fh))
}

pub fn cycle_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let iterable = match args.first() {
        Some(a) => a.clone(),
        _ => {
            return Err(Error::Script {
                msg: "cycle requires 1 argument".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let fh = vm.foreigns.insert(ForeignObject::new(
        "LazyCycleIter",
        LazyCycleIter {
            source: source_h,
            saved: Vec::new(),
            phase: CyclePhase::Collecting,
        },
    ));
    Ok(Value::Foreign(fh))
}

pub fn inspect_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, f) = match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => (a.clone(), b.clone()),
        _ => {
            return Err(Error::Script {
                msg: "inspect requires 2 arguments: iterable and function".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let fh = vm.foreigns.insert(ForeignObject::new(
        "LazyInspectIter",
        LazyInspectIter {
            source: source_h,
            f,
        },
    ));
    Ok(Value::Foreign(fh))
}

pub fn flatten_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let iterable = match args.first() {
        Some(a) => a.clone(),
        _ => {
            return Err(Error::Script {
                msg: "flatten requires 1 argument".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let fh = vm.foreigns.insert(ForeignObject::new(
        "LazyFlattenIter",
        LazyFlattenIter {
            source: source_h,
            current: None,
        },
    ));
    Ok(Value::Foreign(fh))
}

pub fn flat_map_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, f) = match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => (a.clone(), b.clone()),
        _ => {
            return Err(Error::Script {
                msg: "flat_map requires 2 arguments: iterable and function".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let fh = vm.foreigns.insert(ForeignObject::new(
        "LazyFlatMapIter",
        LazyFlatMapIter {
            source: source_h,
            f,
            current: None,
        },
    ));
    Ok(Value::Foreign(fh))
}

pub fn scan_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, init, f) = match (args.first(), args.get(1), args.get(2)) {
        (Some(a), Some(init), Some(f)) => (a.clone(), init.clone(), f.clone()),
        _ => {
            return Err(Error::Script {
                msg: "scan requires 3 arguments: iterable, initial value, and function".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let fh = vm.foreigns.insert(ForeignObject::new(
        "LazyScanIter",
        LazyScanIter {
            source: source_h,
            f,
            acc: init,
        },
    ));
    Ok(Value::Foreign(fh))
}

// ---------------------------------------------------------------------------
// Terminal operations (eagerly consume an iterator)
// ---------------------------------------------------------------------------

pub fn count_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let iterable = match args.first() {
        Some(a) => a.clone(),
        _ => {
            return Err(Error::Script {
                msg: "count requires 1 argument".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let mut count: i64 = 0;
    loop {
        let next = call_source_next(ctx, source_h)?;
        if extract_option_value(vm, &next).is_none() {
            break;
        }
        count += 1;
    }
    Ok(Value::Int(count))
}

pub fn all_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, pred) = match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => (a.clone(), b.clone()),
        _ => {
            return Err(Error::Script {
                msg: "all requires 2 arguments: iterable and predicate".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    loop {
        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(v) => {
                let ok = ctx.call_value(&pred, &[v])?;
                if !ok.is_truthy() {
                    return Ok(Value::Bool(false));
                }
            }
            None => return Ok(Value::Bool(true)),
        }
    }
}

pub fn any_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, pred) = match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => (a.clone(), b.clone()),
        _ => {
            return Err(Error::Script {
                msg: "any requires 2 arguments: iterable and predicate".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    loop {
        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(v) => {
                let ok = ctx.call_value(&pred, &[v])?;
                if ok.is_truthy() {
                    return Ok(Value::Bool(true));
                }
            }
            None => return Ok(Value::Bool(false)),
        }
    }
}

pub fn find_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, pred) = match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => (a.clone(), b.clone()),
        _ => {
            return Err(Error::Script {
                msg: "find requires 2 arguments: iterable and predicate".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    loop {
        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(v) => {
                let ok = ctx.call_value(&pred, &[v.clone()])?;
                if ok.is_truthy() {
                    return Ok(option_some_vm(vm, v));
                }
            }
            None => return Ok(option_none_vm(vm)),
        }
    }
}

pub fn position_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, pred) = match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => (a.clone(), b.clone()),
        _ => {
            return Err(Error::Script {
                msg: "position requires 2 arguments: iterable and predicate".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let mut idx: i64 = 0;
    loop {
        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(v) => {
                let ok = ctx.call_value(&pred, &[v])?;
                if ok.is_truthy() {
                    return Ok(option_some_vm(vm, Value::Int(idx)));
                }
                idx += 1;
            }
            None => return Ok(option_none_vm(vm)),
        }
    }
}

pub fn min_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let iterable = match args.first() {
        Some(a) => a.clone(),
        _ => {
            return Err(Error::Script {
                msg: "min requires 1 argument".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let mut best: Option<i64> = None;
    loop {
        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(Value::Int(n)) => {
                best = Some(best.map_or(n, |b| b.min(n)));
            }
            Some(_) => {
                return Err(Error::Script {
                    msg: "min requires an iterable of integers".into(),
                })
            }
            None => break,
        }
    }
    match best {
        Some(n) => Ok(option_some_vm(vm, Value::Int(n))),
        None => Ok(option_none_vm(vm)),
    }
}

pub fn max_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let iterable = match args.first() {
        Some(a) => a.clone(),
        _ => {
            return Err(Error::Script {
                msg: "max requires 1 argument".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let mut best: Option<i64> = None;
    loop {
        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(Value::Int(n)) => {
                best = Some(best.map_or(n, |b| b.max(n)));
            }
            Some(_) => {
                return Err(Error::Script {
                    msg: "max requires an iterable of integers".into(),
                })
            }
            None => break,
        }
    }
    match best {
        Some(n) => Ok(option_some_vm(vm, Value::Int(n))),
        None => Ok(option_none_vm(vm)),
    }
}

pub fn sum_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let iterable = match args.first() {
        Some(a) => a.clone(),
        _ => {
            return Err(Error::Script {
                msg: "sum requires 1 argument".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let mut total: i64 = 0;
    loop {
        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(Value::Int(n)) => total += n,
            Some(_) => {
                return Err(Error::Script {
                    msg: "sum requires an iterable of integers".into(),
                })
            }
            None => break,
        }
    }
    Ok(Value::Int(total))
}

pub fn product_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let iterable = match args.first() {
        Some(a) => a.clone(),
        _ => {
            return Err(Error::Script {
                msg: "product requires 1 argument".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let mut total: i64 = 1;
    loop {
        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(Value::Int(n)) => total *= n,
            Some(_) => {
                return Err(Error::Script {
                    msg: "product requires an iterable of integers".into(),
                })
            }
            None => break,
        }
    }
    Ok(Value::Int(total))
}

pub fn join_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, sep) = match (args.first(), args.get(1)) {
        (Some(a), Some(Value::Str(s))) => (a.clone(), s.clone()),
        (Some(_), _) => {
            return Err(Error::Script {
                msg: "join requires a string separator as second argument".into(),
            })
        }
        _ => {
            return Err(Error::Script {
                msg: "join requires 2 arguments: iterable and separator".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let mut parts: Vec<String> = Vec::new();
    loop {
        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(Value::Str(s)) => parts.push(s.to_string()),
            Some(other) => parts.push(format!("{:?}", other)),
            None => break,
        }
    }
    Ok(Value::Str(parts.join(&sep).into()))
}

pub fn partition_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, pred) = match (args.first(), args.get(1)) {
        (Some(a), Some(b)) => (a.clone(), b.clone()),
        _ => {
            return Err(Error::Script {
                msg: "partition requires 2 arguments: iterable and predicate".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let mut passed: Vec<Value> = Vec::new();
    let mut failed: Vec<Value> = Vec::new();
    loop {
        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(v) => {
                let ok = ctx.call_value(&pred, &[v.clone()])?;
                if ok.is_truthy() {
                    passed.push(v);
                } else {
                    failed.push(v);
                }
            }
            None => break,
        }
    }
    let passed_arr = vm.arrays.insert(ArrayData { values: passed });
    let failed_arr = vm.arrays.insert(ArrayData { values: failed });
    let result_arr = vm.arrays.insert(ArrayData {
        values: vec![Value::Array(passed_arr), Value::Array(failed_arr)],
    });
    Ok(Value::Array(result_arr))
}

pub fn fold_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let (iterable, init, f) = match (args.first(), args.get(1), args.get(2)) {
        (Some(a), Some(init), Some(f)) => (a.clone(), init.clone(), f.clone()),
        _ => {
            return Err(Error::Script {
                msg: "fold requires 3 arguments: iterable, initial value, and function".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let mut acc = init;
    loop {
        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(v) => {
                acc = ctx.call_value(&f, &[acc, v])?;
            }
            None => break,
        }
    }
    Ok(acc)
}

pub fn collect_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let iterable = match args.first() {
        Some(a) => a.clone(),
        _ => {
            return Err(Error::Script {
                msg: "collect requires 1 argument".into(),
            })
        }
    };
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let source_h = ensure_iterator(ctx, &iterable)?;
    let mut values: Vec<Value> = Vec::new();
    loop {
        let next = call_source_next(ctx, source_h)?;
        match extract_option_value(vm, &next) {
            Some(v) => values.push(v),
            None => break,
        }
    }
    let arr_h = vm.arrays.insert(ArrayData { values });
    Ok(Value::Array(arr_h))
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all iterator foreign types and their `next` methods with the VM.
pub fn register(vm: &mut VM) {
    // Existing iterators
    vm.register_type::<ArrayIterState>("ArrayIter")
        .method("next", Rc::new(array_iter_next));
    vm.register_type::<RangeIterState>("RangeIter")
        .method("next", Rc::new(range_iter_next));
    vm.register_type::<StrIterState>("StrIter")
        .method("next", Rc::new(str_iter_next));
    vm.register_type::<MapIterState>("MapIter")
        .method("next", Rc::new(map_iter_next));

    // Lazy adapters
    vm.register_type::<LazyMapIter>("LazyMapIter")
        .method("next", Rc::new(lazy_map_next));
    vm.register_type::<LazyFilterIter>("LazyFilterIter")
        .method("next", Rc::new(lazy_filter_next));
    vm.register_type::<LazyTakeIter>("LazyTakeIter")
        .method("next", Rc::new(lazy_take_next));
    vm.register_type::<LazySkipIter>("LazySkipIter")
        .method("next", Rc::new(lazy_skip_next));
    vm.register_type::<LazyChainIter>("LazyChainIter")
        .method("next", Rc::new(lazy_chain_next));
    vm.register_type::<LazyZipIter>("LazyZipIter")
        .method("next", Rc::new(lazy_zip_next));
    vm.register_type::<LazyEnumerateIter>("LazyEnumerateIter")
        .method("next", Rc::new(lazy_enumerate_next));
    vm.register_type::<LazyStepByIter>("LazyStepByIter")
        .method("next", Rc::new(lazy_step_by_next));
    vm.register_type::<LazyCycleIter>("LazyCycleIter")
        .method("next", Rc::new(lazy_cycle_next));
    vm.register_type::<LazyInspectIter>("LazyInspectIter")
        .method("next", Rc::new(lazy_inspect_next));
    vm.register_type::<LazyFlattenIter>("LazyFlattenIter")
        .method("next", Rc::new(lazy_flatten_next));
    vm.register_type::<LazyFlatMapIter>("LazyFlatMapIter")
        .method("next", Rc::new(lazy_flat_map_next));
    vm.register_type::<LazyScanIter>("LazyScanIter")
        .method("next", Rc::new(lazy_scan_next));
}
