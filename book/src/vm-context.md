# Calling Back into Script

From a native function, you can call back into the script by getting the `VmContext`.

```rust
use zenlang::vm::VmContext;

fn register_hooks(vm: &mut Vm) {
    vm.register_fn("on_update", |args, ctx: &mut VmContext| {
        // Call a Zenlang function from Rust
        let result = ctx.call("player_update", &[args[0].clone()])?;
        Ok(result)
    });
}
```

## Calling a Named Function

```rust
// Script defines: fn greet(name) { "Hello, {name}!" }
let result = vm.call("greet", &[Value::from("World")])?;
println!("{}", result.as_str().unwrap());  // "Hello, World!"
```

## Global Variable Access

```rust
// Script sets: let player_name = "Alice";
if let Some(name) = vm.get_global("player_name") {
    println!("Player: {:?}", name);
}

vm.set_global("player_name", Value::from("Bob"));
```

## Full End-to-End Example

### Rust Host

```rust
use zenlang::vm::{Vm, VmConfig};
use zenlang::value::Value;

struct Engine {
    vm: Vm,
    dt: f64,
}

impl Engine {
    fn new() -> Self {
        let mut vm = Vm::new();

        vm.register_fn("engine_spawn", |args| {
            let kind = args[0].as_str()?;
            let x = args[1].as_f64()?;
            let y = args[2].as_f64()?;
            println!("spawn {kind} at ({x}, {y})");
            Ok(Value::from("entity_001"))
        });

        vm.register_fn("engine_get_dt", |_| Ok(Value::Float(0.016)));

        vm.eval_file("game.zen").unwrap();
        Engine { vm, dt: 0.016 }
    }

    fn update(&mut self) {
        let _ = self.vm.call("on_update", &[Value::Float(self.dt)]);
    }

    fn on_event(&mut self, event: &str) {
        let _ = self.vm.call("on_event", &[Value::from(event)]);
    }
}
```

### Script (game.zen)

```rust
struct Player { name: str, hp: i64, x: f64, y: f64 }

let mut player = Player { name: "Hero", hp: 100, x: 0.0, y: 0.0 };

fn on_update(dt) {
    if is_key_down("right") { player.x = player.x + 100.0 * dt; }
    if is_key_down("up")    { player.y = player.y - 100.0 * dt; }
}

fn on_event(event) {
    if event == "collision" {
        player.hp = player.hp - 10;
        if player.hp <= 0 {
            engine_spawn("explosion", player.x, player.y);
        }
    }
}
```

## Why This Matters

This enables patterns like:

- **Callbacks** — Rust calls into script-defined event handlers
- **Overridable behavior** — Scripts define functions that the engine calls
- **Plugin systems** — Allow scripts to define game object behavior
