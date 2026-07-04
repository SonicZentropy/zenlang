# Calling Back into Script

From a native function, you can call back into the script using `VMContext`.

## The VMContext

Every native function receives a `&mut VMContext` as its first argument.
It provides a safe entry point for calling script functions and closures.

```rust
use zenlang::vm::VMContext;

fn register_hooks(vm: &mut VM) {
    vm.register_native("on_update", Rc::new(|ctx: &mut VMContext, args: &[Value]| {
        let callback = &args[0];
        ctx.call_value(callback, &[])?;
        Ok(Value::Nil)
    }));
}
```

## Calling a Named Script Function

To call a named function from a native, pass the function value:

```rust
// Script defines: fn greet(name) { "Hello, {name}!" }

// Inside a native function:
let greet_fn = /* obtain the Value::Function for greet */;
let result = ctx.call_value(&greet_fn, &[Value::from("World")])?;
println!("{}", result.as_str().unwrap());  // "Hello, World!"
```

## Accessing the VM from a Native

For advanced use cases (creating arrays, wrapping foreign objects), you can
access the VM directly through the raw pointer:

```rust
vm.register_native("make_items", Rc::new(|ctx, args| {
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let arr = vm.make_array(vec![Value::from("a"), Value::from("b")]);
    Ok(arr)
}));
```

## Timers

Native functions can schedule script callbacks:

```rust
vm.register_native("schedule_boom", Rc::new(|ctx, _| {
    let callback = /* a script closure */;
    ctx.register_timer(callback, 3.0, None)?;        // one-shot after 3s
    Ok(Value::Nil)
}));
```

```rust
ctx.register_timer(callback, delay, interval)?;
// delay:      seconds until first fire
// interval:   Some(dur) for repeating, None for one-shot
ctx.remove_timer(id);
```

## Full End-to-End Example

### Rust Host

```rust
use std::rc::Rc;
use zenlang::{VM, Value, CompileConfig};
use zenlang::vm::VMContext;
use zenlang::error::Result;

struct Engine {
    vm: VM,
    dt: f64,
}

impl Engine {
    fn new() -> Self {
        let mut vm = VM::new();

        vm.register_native("engine_spawn", Rc::new(|_, args| {
            let kind = args.get(0).and_then(|v| v.as_str()).unwrap_or("");
            let x = args.get(1).and_then(|v| v.as_float()).unwrap_or(0.0);
            let y = args.get(2).and_then(|v| v.as_float()).unwrap_or(0.0);
            println!("spawn {kind} at ({x}, {y})");
            Ok(Value::from("entity_001"))
        }));

        vm.register_native("engine_get_dt", Rc::new(|_, _| {
            Ok(Value::Float(0.016))
        }));

        vm.load_file("game.zen").unwrap();
        Engine { vm, dt: 0.016 }
    }

    fn update(&mut self) {
        // Call the script's on_update through __main__ or a stored callback
        let _ = self.vm.run_main();
    }

    fn on_event(&mut self, event: &str) {
        // Trigger script handlers via a registered native that receives events
    }
}
```

### Script (game.zen)

```rust
struct Player { name: str, hp: i64, x: f64, y: f64 }

let mut player = Player { name: "Hero", hp: 100, x: 0.0, y: 0.0 };

fn main() {
    if is_key_down("right") { player.x = player.x + 100.0 * engine_get_dt(); }
    if is_key_down("up")    { player.y = player.y - 100.0 * engine_get_dt(); }
}
```
