use std::fs;
use std::rc::Rc;

use crate::Result;
use crate::ast::Type;
use crate::symbol::FnSignature;
use crate::value::{ArrayData, EnumData, Value};
use crate::vm::{VM, VMContext};

// --- Helpers ---

fn ok_val(ctx: &mut VMContext, val: Value) -> Value {
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let h = vm.enums.insert(EnumData {
        tag: 0,
        fields: vec![val],
    });
    Value::Enum(h)
}

fn err_val(ctx: &mut VMContext, msg: &str) -> Value {
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let h = vm.enums.insert(EnumData {
        tag: 1,
        fields: vec![Value::Str(msg.into())],
    });
    Value::Enum(h)
}

fn result_str_str() -> Type {
    Type::Result(Box::new(Type::Str), Box::new(Type::Str))
}

fn result_array_str_str() -> Type {
    Type::Result(
        Box::new(Type::Array(Box::new(Type::Str))),
        Box::new(Type::Str),
    )
}

fn result_unit_str() -> Type {
    Type::Result(Box::new(Type::Unit), Box::new(Type::Str))
}

// --- Registration ---

pub fn register(vm: &mut VM) {
    vm.register_native("read_file", Rc::new(read_file_impl));
    vm.register_native("read_lines", Rc::new(read_lines_impl));
    vm.register_native("write_file", Rc::new(write_file_impl));
    vm.register_native("append_file", Rc::new(append_file_impl));

    vm.register_native("list_dir", Rc::new(list_dir_impl));
    vm.register_native("is_dir", Rc::new(is_dir_impl));
    vm.register_native("is_file", Rc::new(is_file_impl));
    vm.register_native("create_dir", Rc::new(create_dir_impl));
    vm.register_native("create_dirs", Rc::new(create_dirs_impl));
    vm.register_native("remove_file", Rc::new(remove_file_impl));
    vm.register_native("remove_dir", Rc::new(remove_dir_impl));

    vm.register_native("path_join", Rc::new(path_join_impl));
    vm.register_native("path_dirname", Rc::new(path_dirname_impl));
    vm.register_native("path_basename", Rc::new(path_basename_impl));
    vm.register_native("path_extension", Rc::new(path_extension_impl));
    vm.register_native("path_exists", Rc::new(path_exists_impl));
}

pub fn signatures() -> Vec<FnSignature> {
    vec![
        FnSignature {
            type_params: vec![],
            name: "read_file".into(),
            params: vec![("path".into(), Type::Str)],
            return_type: Some(result_str_str()),
        },
        FnSignature {
            type_params: vec![],
            name: "read_lines".into(),
            params: vec![("path".into(), Type::Str)],
            return_type: Some(result_array_str_str()),
        },
        FnSignature {
            type_params: vec![],
            name: "write_file".into(),
            params: vec![("path".into(), Type::Str), ("content".into(), Type::Str)],
            return_type: Some(result_unit_str()),
        },
        FnSignature {
            type_params: vec![],
            name: "append_file".into(),
            params: vec![("path".into(), Type::Str), ("content".into(), Type::Str)],
            return_type: Some(result_unit_str()),
        },
        FnSignature {
            type_params: vec![],
            name: "list_dir".into(),
            params: vec![("path".into(), Type::Str)],
            return_type: Some(result_array_str_str()),
        },
        FnSignature {
            type_params: vec![],
            name: "is_dir".into(),
            params: vec![("path".into(), Type::Str)],
            return_type: Some(Type::Bool),
        },
        FnSignature {
            type_params: vec![],
            name: "is_file".into(),
            params: vec![("path".into(), Type::Str)],
            return_type: Some(Type::Bool),
        },
        FnSignature {
            type_params: vec![],
            name: "create_dir".into(),
            params: vec![("path".into(), Type::Str)],
            return_type: Some(result_unit_str()),
        },
        FnSignature {
            type_params: vec![],
            name: "create_dirs".into(),
            params: vec![("path".into(), Type::Str)],
            return_type: Some(result_unit_str()),
        },
        FnSignature {
            type_params: vec![],
            name: "remove_file".into(),
            params: vec![("path".into(), Type::Str)],
            return_type: Some(result_unit_str()),
        },
        FnSignature {
            type_params: vec![],
            name: "remove_dir".into(),
            params: vec![("path".into(), Type::Str)],
            return_type: Some(result_unit_str()),
        },
        FnSignature {
            type_params: vec![],
            name: "path_join".into(),
            params: vec![("a".into(), Type::Str), ("b".into(), Type::Str)],
            return_type: Some(Type::Str),
        },
        FnSignature {
            type_params: vec![],
            name: "path_dirname".into(),
            params: vec![("path".into(), Type::Str)],
            return_type: Some(Type::Str),
        },
        FnSignature {
            type_params: vec![],
            name: "path_basename".into(),
            params: vec![("path".into(), Type::Str)],
            return_type: Some(Type::Str),
        },
        FnSignature {
            type_params: vec![],
            name: "path_extension".into(),
            params: vec![("path".into(), Type::Str)],
            return_type: Some(Type::Str),
        },
        FnSignature {
            type_params: vec![],
            name: "path_exists".into(),
            params: vec![("path".into(), Type::Str)],
            return_type: Some(Type::Bool),
        },
    ]
}

// --- File I/O ---

fn read_file_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    match fs::read_to_string(&path) {
        Ok(content) => Ok(ok_val(ctx, Value::Str(content.into()))),
        Err(e) => Ok(err_val(ctx, &e.to_string())),
    }
}

fn read_lines_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    match fs::read_to_string(&path) {
        Ok(content) => {
            let lines: Vec<Value> = content.lines().map(|l| Value::Str(l.into())).collect();
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            let arr = vm.arrays.insert(ArrayData { values: lines });
            Ok(ok_val(ctx, Value::Array(arr)))
        }
        Err(e) => Ok(err_val(ctx, &e.to_string())),
    }
}

fn write_file_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    let content = args.get(1).and_then(|v| v.as_str()).unwrap_or_default();
    match fs::write(&path, &content) {
        Ok(()) => Ok(ok_val(ctx, Value::Nil)),
        Err(e) => Ok(err_val(ctx, &e.to_string())),
    }
}

fn append_file_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    let content = args.get(1).and_then(|v| v.as_str()).unwrap_or_default();
    use std::io::Write;
    match fs::OpenOptions::new().append(true).create(true).open(&path) {
        Ok(mut file) => {
            if let Err(e) = writeln!(file, "{}", content) {
                return Ok(err_val(ctx, &e.to_string()));
            }
            Ok(ok_val(ctx, Value::Nil))
        }
        Err(e) => Ok(err_val(ctx, &e.to_string())),
    }
}

// --- Directory operations ---

fn list_dir_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    match fs::read_dir(&path) {
        Ok(entries) => {
            let mut result = Vec::new();
            for entry in entries {
                match entry {
                    Ok(e) => {
                        if let Some(name) = e.file_name().to_str() {
                            result.push(Value::Str(name.into()));
                        }
                    }
                    Err(e) => return Ok(err_val(ctx, &e.to_string())),
                }
            }
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            let arr = vm.arrays.insert(ArrayData { values: result });
            Ok(ok_val(ctx, Value::Array(arr)))
        }
        Err(e) => Ok(err_val(ctx, &e.to_string())),
    }
}

fn is_dir_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    Ok(Value::Bool(std::path::Path::new(&path).is_dir()))
}

fn is_file_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    Ok(Value::Bool(std::path::Path::new(&path).is_file()))
}

fn create_dir_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    match fs::create_dir(&path) {
        Ok(()) => Ok(ok_val(ctx, Value::Nil)),
        Err(e) => Ok(err_val(ctx, &e.to_string())),
    }
}

fn create_dirs_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    match fs::create_dir_all(&path) {
        Ok(()) => Ok(ok_val(ctx, Value::Nil)),
        Err(e) => Ok(err_val(ctx, &e.to_string())),
    }
}

fn remove_file_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    match fs::remove_file(&path) {
        Ok(()) => Ok(ok_val(ctx, Value::Nil)),
        Err(e) => Ok(err_val(ctx, &e.to_string())),
    }
}

fn remove_dir_impl(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    match fs::remove_dir(&path) {
        Ok(()) => Ok(ok_val(ctx, Value::Nil)),
        Err(e) => Ok(err_val(ctx, &e.to_string())),
    }
}

// --- Path utilities ---

fn path_join_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let a = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    let b = args.get(1).and_then(|v| v.as_str()).unwrap_or_default();
    let joined = std::path::Path::new(&a).join(&b);
    Ok(Value::Str(joined.to_string_lossy().into()))
}

fn path_dirname_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    let parent = std::path::Path::new(&path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok(Value::Str(parent.into()))
}

fn path_basename_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    let name = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    Ok(Value::Str(name.into()))
}

fn path_extension_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    Ok(Value::Str(ext.into()))
}

fn path_exists_impl(_ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    Ok(Value::Bool(std::path::Path::new(&path).exists()))
}
