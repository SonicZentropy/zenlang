# Registering Native Functions

Register Rust functions to be callable from scripts.

## Simple Native Functions

```rust
use zenlang::native_fn;

let mut vm = Vm::new();

// Simple function: add two integers
vm.register_fn("add", |args| {
    let a = args[0].as_i64()?;
    let b = args[1].as_i64()?;
    Ok(Value::Int(a + b))
});
```

## Native Function with VmContext (Calling Back Into Script)

```rust
use zenlang::vm::VmContext;

vm.register_fn("on_collide", |args: &[Value], ctx: &mut VmContext| {
    let entity = &args[0];
    ctx.call("handle_collision", &[entity.clone()])
});
```

## Batch Registration

```rust
fn register_game_api(vm: &mut Vm) {
    vm.register_fn("spawn_enemy", |args| {
        let x = args[0].as_f64()?;
        let y = args[1].as_f64()?;
        Ok(Value::from("enemy_01"))
    });
    vm.register_fn("play_sound", |args| {
        let name = args[0].as_str()?.to_string();
        audio_engine.play(&name);
        Ok(Value::Void)
    });
    vm.register_fn("get_delta_time", |_| Ok(Value::Float(engine.dt())));
}
```

## Returning Complex Values

### Returning a Map

```rust
vm.register_fn("get_player_info", |_| {
    let info = Value::new_map();
    map_set(info, "name", Value::from("Hero"));
    map_set(info, "hp", Value::Int(100));
    Ok(info)
});
```

**Script side:**

```rust
let info = get_player_info();
print(info["name"]);
```

### Returning an Array

```rust
vm.register_fn("get_inventory", |_| {
    Ok(Value::from(vec![
        Value::from("sword"),
        Value::from("shield"),
        Value::from("potion"),
    ]))
});
```

## Accepting Callbacks from Script

**Rust side:**

```rust
// Store script callbacks and invoke them later
let mut script_callbacks: Vec<Value> = Vec::new();

vm.register_fn("on_button_click", |args: &[Value], _ctx: &mut VmContext| {
    let callback = args[0].clone();
    script_callbacks.push(callback);
    Ok(Value::Void)
});

// Later, invoke stored callbacks
for cb in &script_callbacks {
    ctx.call_value(cb, &[])?;
}
```

**Script side:**

```rust
on_button_click(|| {
    print("button clicked!");
});
```

## Using the Macro

```rust
use zenlang::{native_fn, Value, VmError};

#[native_fn]
fn greet(name: Value) -> Result<Value, VmError> {
    let name_str = name.as_str().ok_or(VmError::new("expected string"))?;
    Ok(Value::from("Hello, {name_str}!"))
}
```

## Type Conversions

The `From` trait is implemented for converting between Rust types and `Value`:

```rust
use zenlang::value::{FromValue, ToValue};

// Convert Value → Rust
let x: f64 = val.from_value()?;

// Convert Rust → Value
let val: Value = 42.0.into();
```
