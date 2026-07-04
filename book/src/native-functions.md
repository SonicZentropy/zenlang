# Registering Native Functions

Register Rust functions to be callable from scripts.

## Simple Native Function

```rust
use std::rc::Rc;
use zenlang::{VM, Value};
use zenlang::vm::VMContext;
use zenlang::error::Result;

let mut vm = VM::new();

vm.register_native("add", Rc::new(|_ctx: &mut VMContext, args: &[Value]| -> Result<Value> {
    let a = args.first().and_then(|v| v.as_int()).unwrap_or(0);
    let b = args.get(1).and_then(|v| v.as_int()).unwrap_or(0);
    Ok(Value::Int(a + b))
}));
```

**Script side:**

```rust
print(add(3, 4));  // 7
```

The closure signature must be `Fn(&mut VMContext, &[Value]) -> Result<Value>`.
Use `Rc::new(...)` to wrap it.

## Native Function with Callback Into Script

```rust
vm.register_native("on_collide", Rc::new(|ctx: &mut VMContext, args: &[Value]| -> Result<Value> {
    let entity = &args[0];
    ctx.call_value(entity, &[])?;
    Ok(Value::Nil)
}));
```

`ctx.call_value(callee, args)` calls a script function or closure from within
a native function. The `callee` can be a `Value::Function(idx)` or a
`Value::Closure(handle)`.

## Batch Registration

```rust
fn register_game_api(vm: &mut VM) {
    vm.register_native("spawn_enemy", Rc::new(|ctx, args| {
        let x = args.get(0).and_then(|v| v.as_float()).unwrap_or(0.0);
        let y = args.get(1).and_then(|v| v.as_float()).unwrap_or(0.0);
        // ... spawn logic ...
        Ok(Value::from("enemy_01"))
    }));
    vm.register_native("get_delta_time", Rc::new(|_, _| {
        Ok(Value::Float(0.016)))
    }));
}
```

## Returning Values

### Returning a Map

Build maps from the script side via `map_new()` and `map_set()`. To create
a map from Rust, call the built-in `map_new` through the VM, or use
`Value` constructors after running script code that builds the map.

### Returning an Array

```rust
vm.register_native("get_inventory", Rc::new(|ctx, _| {
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let arr = vm.make_array(vec![
        Value::from("sword"),
        Value::from("shield"),
        Value::from("potion"),
    ]);
    Ok(arr)
}));
```

`vm.make_array(values)` is the safe way to create a `Value::Array` from a
Rust `Vec<Value>`.

### Returning a Foreign Object

```rust
vm.register_native("create_player", Rc::new(|ctx, args| {
    let name = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let player = Player::new(name);
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    Ok(vm.wrap_foreign("Player", player))
}));
```

## Accepting Callbacks from Script

**Rust side:**

```rust
let mut callbacks: Vec<Value> = Vec::new();

vm.register_native("on_button_click", Rc::new(|ctx, args| {
    let callback = args[0].clone();
    callbacks.push(callback);
    Ok(Value::Nil)
}));

// Later, invoke stored callbacks
for cb in &callbacks {
    ctx.call_value(cb, &[])?;
}
```

**Script side:**

```rust
on_button_click(|| {
    print("button clicked!");
});
```

## Using the `#[zen_native_fn]` Macro

```rust
use zenlang::{Value, zen_native_fn};

#[zen_native_fn]
fn greet(name: String) -> String {
    format!("Hello, {name}!")
}
```

The macro wraps the function with the required `Rc<dyn Fn(...)>` signature and
handles argument/value conversions automatically.

## Type Conversions

`Value` implements `From` for common Rust types:

```rust
// Rust → Value
let val: Value = 42.into();          // Value::Int(42)
let val: Value = "hello".into();     // Value::Str(...)
let val: Value = true.into();        // Value::Bool(true)

// Value → Rust (via accessors)
if let Some(n) = val.as_int() { /* i64 */ }
if let Some(s) = val.as_str() { /* &str */ }
if let Some(f) = val.as_float() { /* f64 */ }
if let Some(b) = val.as_bool() { /* bool */ }
```
