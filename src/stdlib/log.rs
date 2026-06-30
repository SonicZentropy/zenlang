use std::rc::Rc;
use std::sync::atomic::{AtomicI32, Ordering};

use crate::ast::Type;
use crate::symbol::FnSignature;
use crate::value::Value;
use crate::vm::{VM, VMContext};
use crate::Result;

const LEVEL_TRACE: i32 = 0;
const LEVEL_DEBUG: i32 = 1;
const LEVEL_INFO: i32 = 2;
const LEVEL_WARN: i32 = 3;
const LEVEL_ERROR: i32 = 4;
const LEVEL_OFF: i32 = 5;

static LOG_LEVEL: AtomicI32 = AtomicI32::new(LEVEL_INFO);

static LEVEL_NAMES: [&str; 5] = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"];

pub fn register(vm: &mut VM) {
    vm.register_native("log_set_level", Rc::new(log_set_level_impl));
    vm.register_native("log_trace", Rc::new(log_trace_impl));
    vm.register_native("log_debug", Rc::new(log_debug_impl));
    vm.register_native("log_info", Rc::new(log_info_impl));
    vm.register_native("log_warn", Rc::new(log_warn_impl));
    vm.register_native("log_error", Rc::new(log_error_impl));
}

pub fn signatures() -> Vec<FnSignature> {
    vec![
        FnSignature { name: "log_set_level".into(), params: vec![("level".into(), Type::Str)], return_type: Some(Type::Unit) },
        FnSignature { name: "log_trace".into(), params: vec![("msg".into(), Type::Str)], return_type: Some(Type::Unit) },
        FnSignature { name: "log_debug".into(), params: vec![("msg".into(), Type::Str)], return_type: Some(Type::Unit) },
        FnSignature { name: "log_info".into(), params: vec![("msg".into(), Type::Str)], return_type: Some(Type::Unit) },
        FnSignature { name: "log_warn".into(), params: vec![("msg".into(), Type::Str)], return_type: Some(Type::Unit) },
        FnSignature { name: "log_error".into(), params: vec![("msg".into(), Type::Str)], return_type: Some(Type::Unit) },
    ]
}

fn log_internal(level: i32, msg: &str) {
    if level >= LOG_LEVEL.load(Ordering::Relaxed) && level < LEVEL_OFF {
        let name = LEVEL_NAMES.get(level as usize).unwrap_or(&"?");
        eprintln!("[{}] {}", name, msg);
    }
}

fn log_set_level_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let level_str = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    let level = match level_str.to_lowercase().as_str() {
        "trace" => LEVEL_TRACE,
        "debug" => LEVEL_DEBUG,
        "info" => LEVEL_INFO,
        "warn" | "warning" => LEVEL_WARN,
        "error" => LEVEL_ERROR,
        "off" => LEVEL_OFF,
        _ => LEVEL_INFO,
    };
    LOG_LEVEL.store(level, Ordering::Relaxed);
    Ok(Value::Nil)
}

fn log_trace_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let msg = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    log_internal(LEVEL_TRACE, &msg);
    Ok(Value::Nil)
}

fn log_debug_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let msg = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    log_internal(LEVEL_DEBUG, &msg);
    Ok(Value::Nil)
}

fn log_info_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let msg = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    log_internal(LEVEL_INFO, &msg);
    Ok(Value::Nil)
}

fn log_warn_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let msg = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    log_internal(LEVEL_WARN, &msg);
    Ok(Value::Nil)
}

fn log_error_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let msg = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    log_internal(LEVEL_ERROR, &msg);
    Ok(Value::Nil)
}
