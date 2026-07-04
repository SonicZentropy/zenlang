/// Register custom Rust functions callable from Zen scripts.
///
/// Run with: cargo run --example custom_natives
use std::cell::Cell;
use std::rc::Rc;

use zenlang::vm::VMContext;
use zenlang::{VM, Value};

fn main() -> zenlang::Result<()> {
    let mut vm = VM::new();

    // Register a custom native function: double(x) -> x * 2
    vm.register_native(
        "double",
        Rc::new(|_ctx: &mut VMContext, args: &[Value]| {
            let n = args.first().and_then(|v| v.as_int()).unwrap_or(0);
            Ok(Value::Int(n * 2))
        }),
    );

    // Register another: add3(a, b, c) -> a + b + c
    vm.register_native(
        "add3",
        Rc::new(|_ctx: &mut VMContext, args: &[Value]| {
            let a = args.first().and_then(|v| v.as_int()).unwrap_or(0);
            let b = args.get(1).and_then(|v| v.as_int()).unwrap_or(0);
            let c = args.get(2).and_then(|v| v.as_int()).unwrap_or(0);
            Ok(Value::Int(a + b + c))
        }),
    );

    // Register a stateful native using Rc<Cell<i64>> for shared ownership
    let count = Rc::new(Cell::new(0i64));
    let count_clone = count.clone();
    vm.register_native(
        "tick",
        Rc::new(move |_ctx: &mut VMContext, _args: &[Value]| {
            let val = count_clone.get();
            count_clone.set(val + 1);
            Ok(Value::Int(val))
        }),
    );

    let source = "\
let a = double(21);
let b = add3(1, 2, 3);
let c0 = tick();
let c1 = tick();
let c2 = tick();
[a, b, c0, c1, c2]
";

    vm.load(source)?;
    let result = vm.run_main()?;
    println!("result: {:?}", result);
    assert_eq!(count.get(), 3);

    Ok(())
}
