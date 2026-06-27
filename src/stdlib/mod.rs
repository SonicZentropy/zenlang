use std::rc::Rc;

use crate::error::Result;
use crate::value::Value;
use crate::vm::{VM, VMContext};

/// Register built-in standard library functions with a VM.
pub fn register_builtins(vm: &mut VM) {
    // print(...args): print values to stdout
    vm.register_native("print", Rc::new(|_ctx: &mut VMContext, args: &[Value]| -> Result<Value> {
        let mut parts = Vec::new();
        for arg in args {
            parts.push(format!("{:?}", arg));
        }
        println!("{}", parts.join(" "));
        Ok(Value::Nil)
    }));

    // assert_eq(expected, actual): panic if not equal
    vm.register_native("assert_eq", Rc::new(|_ctx: &mut VMContext, args: &[Value]| -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Nil);
        }
        let expected = &args[0];
        let actual = &args[1];
        if *expected != *actual {
            panic!("assert_eq failed: expected {:?}, got {:?}", expected, actual);
        }
        Ok(Value::Nil)
    }));

    // type_of(value): return the type name as a string
    vm.register_native("type_of", Rc::new(|_ctx: &mut VMContext, args: &[Value]| -> Result<Value> {
        let name = args.first().map(|v| v.type_name()).unwrap_or("nil");
        Ok(Value::Str(name.into()))
    }));
}
