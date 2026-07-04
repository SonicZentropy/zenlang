# Zenlang Cookbook

A quick-reference cheatsheet for common tasks in Zenlang.

## Basics

### Hello World

```rust
print("Hello, Zenlang!");
```

### Bindings

```rust
let x = 42;               // immutable
let mut y = 10;           // mutable
y = y + 1;
let s: str = "hello";     // with type annotation
const MAX: i64 = 100;     // compile-time constant
```

### Type Annotations

```rust
let a: i64 = 42;
let b: f64 = 3.14;
let c: bool = true;
let d: str = "hi";
```

## Functions

### Define and Call

```rust
fn add(a, b) { a + b }
fn greet(name: str) -> str { "Hello, {name}!" }

add(1, 2);
greet("World");
```

### Closures

```rust
let double = |x| x * 2;
let sum = |a, b| a + b;

double(5);             // 10
sum(3, 4);             // 7
```

### Default Parameters

```rust
fn greet(name: str, greeting: str = "Hello") {
    print("{greeting}, {name}!");
}
```

## Control Flow

### if / else

```rust
let x = if cond { 1 } else { 2 };

if x > 0 {
    print("positive");
} else if x < 0 {
    print("negative");
} else {
    print("zero");
};
```

### Loops

```rust
while i < 10 { i = i + 1; }

for i in 0..5 { print(i); }          // 0 1 2 3 4
for i in 0..=5 { print(i); }         // 0 1 2 3 4 5
for x in [1, 2, 3] { print(x); }
for c in "hello" { print(c); }
```

### break / continue

```rust
loop {
    if done { break; }
    if skip { continue; }
};
```

### Match

```rust
match x {
    1 => "one",
    2 => "two",
    _ => "other",
};

match result {
    Ok(val) => print(val),
    Err(msg) => print("error: " + msg),
};

match opt {
    Some(v) if v > 5 => "big",
    Some(v) => "small",
    None => "none",
};
```

### if let / while let

```rust
if let Some(v) = opt { print(v); }

while let Some(v) = iter { print(v); }
```

### Try Operator

```rust
fn example() -> Result<i64, str> {
    let x = risky()?;   // returns early on Err
    Ok(x)
}
```

## Data Structures

### Structs

```rust
struct Point { x: i64, y: i64 }

let p = Point { x: 1, y: 2 };
let q = Point { x: 10, ..p };   // spread
let r = Point { x, y };          // shorthand

p.x;           // access
p.x = 5;       // mutate (requires mut)
```

### Enums

```rust
enum Color { Red, Green, Blue }
enum Shape { Circle(i64), Rect(i64, i64) }

let c = Red;
let s = Circle(10);

match s {
    Circle(r) => print(r),
    Rect(w, h) => print(w * h),
};
```

### Arrays

```rust
let arr = [1, 2, 3];
push(arr, 4);
pop(arr);             // 4
len(arr);             // 3
insert(arr, 1, 99);
remove(arr, 0);       // 1
arr[0];               // first element
```

### Maps

```rust
let m = map_new();
map_set(m, "hp", 100);
map_get(m, "hp");       // Some(100)
map_has(m, "hp");       // true
map_remove(m, "hp");    // Some(100)
map_keys(m);            // keys array
map_values(m);          // values array
map_len(m);
map_clear(m);

for kv in m {
    let k = kv[0];
    let v = kv[1];
};
```

### Strings

```rust
let s = "hello";
len(s);                 // 5
contains(s, "ell");     // true
trim("  hi  ");         // "hi"
to_upper("abc");        // "ABC"
to_lower("XYZ");        // "xyz"
substring("hello", 1, 4); // "ell"

// Interpolation
let msg = "Value: {x} and {y}";
```

## Methods and Traits

### Impl Blocks

```rust
struct Vec2 { x: f64, y: f64 }

impl Vec2 {
    fn len(&self) -> f64 {
        sqrt(self.x * self.x + self.y * self.y)
    }

    fn scale(&mut self, f: f64) {
        self.x *= f;
        self.y *= f;
    }

    fn new(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }
}

let v = Vec2 { x: 3.0, y: 4.0 };
v.len();
Vec2::new(1.0, 2.0);
```

### Traits

```rust
trait Area {
    fn area(&self) -> f64;
}

impl Area for Circle {
    fn area(&self) -> f64 {
        3.14159 * self.radius * self.radius
    }
}
```

## Modules

```rust
mod math {
    pub fn add(a, b) { a + b }
}

use math::add;
add(1, 2);

// From file
mod greeting;          // loads greeting.zen
use greeting::greet;
```

## Generics

```rust
fn identity<T>(x: T) -> T { x }
fn first<T>(arr: [T]) -> T { arr[0] }

struct Wrapper<T> { value: T }
```

## Generators

```rust
fn counter() {
    yield 1;
    yield 2;
    yield 3;
}

let g = counter();
next(g);    // Some(1)
next(g);    // Some(2)
next(g);    // Some(3)
next(g);    // None
```

## Standard Library

### Math

```rust
abs(-5);            // 5
sqrt(9.0);          // 3.0
sin(0.0);           // 0.0
cos(0.0);           // 1.0
floor(3.7);         // 3.0
ceil(3.2);          // 4.0
round(3.5);         // 4.0
min(1, 2);          // 1
max(1, 2);          // 2
pow(2, 10);         // 1024.0
random();           // f64 in [0, 1)
random_int(1, 6);   // i64 in [1, 6]
vec2(1.0, 2.0);
vec3(1.0, 2.0, 3.0);
vec4(1.0, 2.0, 3.0, 4.0);
```

### Iterators

```rust
use std::iter::*;

map([1, 2, 3], |x| x * 2);       // [2, 4, 6]
filter([1, 2, 3, 4], |x| x % 2 == 0);  // [2, 4]
reduce([1, 2, 3], |a, b| a + b, 0);    // 6
zip(["a", "b"], [1, 2]);          // [["a",1],["b",2]]
```

### JSON

```rust
use std::json::*;

let m = map_new();
map_set(m, "name", "Zenlang");
let json = encode(m);
let data = decode(json);
```

### File I/O

```rust
use std::fs::*;

let content = read("file.txt");
write("out.txt", content);
exists("file.txt");       // true/false
```

### Logging

```rust
use std::log::*;

info("started");
warn("low health");
error("file not found");
debug("x = {x}");
```

### Timers

```rust
use std::timer::*;

let id = set_timeout(|| print("done!"), 1000);
let id = set_interval(|| update(), 16);
clear_timer(id);
```

### Option / Result

```rust
let val = Some(42);
val == None;        // false

match val {
    Some(v) => v,
    None => 0,
};

fn safe_div(a, b) -> Result<i64, str> {
    if b == 0 { Err("div by zero") }
    else { Ok(a / b) }
}

let x = safe_div(10, 2)?;   // unwrap or return Err
```

## Operators

### Arithmetic

```rust
a + b   a - b   a * b   a / b   a % b
a += b  a -= b  a *= b  a /= b  a %= b
```

### Comparison

```rust
a == b  a != b  a < b   a > b   a <= b  a >= b
```

### Logical

```rust
a && b  a || b  !a
```

### Bitwise

```rust
a & b   a | b   a ^ b   ~a      a << b  a >> b
a &= b  a |= b  a ^= b  a <<= b a >>= b
```

### Ranges

```rust
0..5     // exclusive: 0 1 2 3 4
0..=5    // inclusive: 0 1 2 3 4 5
```

## Attributes

```rust
#[allow(unused)]
fn helper() { }

#[test]
fn my_test() {
    assert(1 + 1 == 2);
}
```

## Tooling

### CLI

```bash
zenc run script.zen
zenc run --watch script.zen   # hot reload
zenc repl
zenc check script.zen
zenc disasm script.zen
zenc test
zenc new project_name
zenc build
zenc lsp                       # start language server
zenc dap                       # start debugger
```

## Embedding in Rust

### Basic VM

```rust
use zenlang::vm::{Vm, VmConfig, VmContext};
use zenlang::value::Value;

let mut vm = Vm::new();
vm.eval("print(42)").unwrap();
```

### VM with Config

```rust
let mut vm = Vm::with_config(VmConfig {
    instruction_limit: Some(100_000),          // safety: max instructions
    module_search_paths: vec!["scripts".into()],
    ..Default::default()
});
```

### Run a File

```rust
vm.eval_file("scripts/main.zen").unwrap();
```

### Evaluate an Expression

```rust
let result = vm.eval("1 + 2 * 3").unwrap();
assert_eq!(result.as_i64(), Some(7));
```

### Error Handling

```rust
match vm.eval("bad_code") {
    Ok(val) => println!("OK: {:?}", val),
    Err(zenlang::Error::Runtime(msg)) => eprintln!("Runtime: {msg}"),
    Err(zenlang::Error::Compile(errors)) => {
        for e in errors { eprintln!("Compile: {e}"); }
    }
    Err(zenlang::Error::Panic(msg)) => eprintln!("Panic: {msg}"),
}
```

---

## Native Functions (Rust → Script)

### Simple Native Function

**Rust side:**

```rust
vm.register_fn("add", |args: &[Value]| -> Result<Value, VmError> {
    let a = args[0].as_i64().ok_or(VmError::new("expected i64"))?;
    let b = args[1].as_i64().ok_or(VmError::new("expected i64"))?;
    Ok(Value::Int(a + b))
});
```

**Script side:**

```rust
print(add(3, 4));  // 7
```

### Native Function with VmContext (Call Back Into Script)

**Rust side:**

```rust
vm.register_fn("on_collide", |args: &[Value], ctx: &mut VmContext| -> Result<Value, VmError> {
    let entity = &args[0];
    // Call a Zenlang callback function
    ctx.call("handle_collision", &[entity.clone()])
});
```

**Script side:**

```rust
fn handle_collision(entity) {
    print("collision detected!");
}
```

### Register Multiple Functions at Once

```rust
fn register_game_api(vm: &mut Vm) {
    vm.register_fn("spawn_enemy", |args| {
        let x = args[0].as_f64()?;
        let y = args[1].as_f64()?;
        // ... spawn logic ...
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

---

## Foreign Types (Rust Structs in Script)

### Define and Register a Foreign Type

**Rust side:**

```rust
use zenlang::foreign_type;

pub struct Texture {
    pub id: u32,
    pub width: u32,
    pub height: u32,
}

foreign_type! {
    /// The name inside `type` is the Rust struct; the string is the Zenlang name.
    type Texture = "Texture" {
        fields {
            width: i64,
            height: i64,
        }
        methods {
            fn load(path: &str) -> Texture;
            fn get_size(&self) -> (i64, i64);
            fn is_valid(&self) -> bool;
        }
    }
}

impl ForeignTexture {
    fn load(path: &str) -> Result<Value, VmError> {
        let tex = match Texture::load_from_disk(path) {
            Ok(t) => t,
            Err(e) => return Err(VmError::new(&format!("failed: {e}"))),
        };
        Ok(ForeignTexture::new(tex).into())
    }

    fn get_size(&self) -> Result<Value, VmError> {
        let t = &self.0;
        Ok((t.width as i64, t.height as i64).into())
    }

    fn is_valid(&self) -> Result<Value, VmError> {
        Ok(Value::Bool(self.0.id != 0))
    }
}
```

**Script side:**

```rust
let tex = Texture::load("player.png");
assert(tex.is_valid());
let (w, h) = tex.get_size();
print("texture size: {w} x {h}");
```

### Foreign Type with Mutable State

**Rust side:**

```rust
pub struct Transform {
    pub x: f64, pub y: f64,
    pub rotation: f64, pub scale: f64,
}

foreign_type! {
    type Transform = "Transform" {
        fields {
            x: f64, y: f64,
        }
        methods {
            fn new() -> Transform;
            fn translate(&mut self, dx: f64, dy: f64);
            fn get_rotation(&self) -> f64;
            fn set_rotation(&mut self, r: f64);
        }
    }
}

impl ForeignTransform {
    fn new() -> Result<Value, VmError> {
        Ok(ForeignTransform::new(Transform {
            x: 0.0, y: 0.0, rotation: 0.0, scale: 1.0,
        }).into())
    }
    fn translate(&mut self, dx: f64, dy: f64) -> Result<Value, VmError> {
        self.0.x += dx;
        self.0.y += dy;
        Ok(Value::Void)
    }
    fn get_rotation(&self) -> Result<Value, VmError> {
        Ok(Value::Float(self.0.rotation))
    }
    fn set_rotation(&mut self, r: f64) -> Result<Value, VmError> {
        self.0.rotation = r;
        Ok(Value::Void)
    }
}
```

**Script side:**

```rust
let t = Transform::new();
t.translate(10.0, 5.0);
t.set_rotation(1.57);
assert(t.x == 10.0);
```

### Foreign Type with Enum-Style Variants

**Rust side:**

```rust
pub enum Shape {
    Circle { radius: f64 },
    Rect { w: f64, h: f64 },
}

foreign_type! {
    type Shape = "Shape" {
        fields {}
        methods {
            fn circle(radius: f64) -> Shape;
            fn rect(w: f64, h: f64) -> Shape;
            fn area(&self) -> f64;
        }
    }
}

impl ForeignShape {
    fn circle(radius: f64) -> Result<Value, VmError> {
        Ok(ForeignShape::new(Shape::Circle { radius }).into())
    }
    fn rect(w: f64, h: f64) -> Result<Value, VmError> {
        Ok(ForeignShape::new(Shape::Rect { w, h }).into())
    }
    fn area(&self) -> Result<Value, VmError> {
        match &self.0 {
            Shape::Circle { radius } => Ok(Value::Float(3.14159 * radius * radius)),
            Shape::Rect { w, h } => Ok(Value::Float(w * h)),
        }
    }
}
```

**Script side:**

```rust
let c = Shape::circle(5.0);
let r = Shape::rect(3.0, 4.0);
print(c.area());   // 78.53975
print(r.area());   // 12.0
```

---

## Passing Complicated Values

### Returning a Map from Rust

```rust
vm.register_fn("get_player_info", |_| {
    let info = map_new();
    map_set(info, "name", Value::from("Hero"));
    map_set(info, "hp", Value::Int(100));
    map_set(info, "mana", Value::Int(50));
    Ok(info)
});
```

**Script side:**

```rust
let info = get_player_info();
print(info["name"]);
```

### Returning an Array from Rust

```rust
vm.register_fn("get_inventory", |_| {
    let items = vec![
        Value::from("sword"),
        Value::from("shield"),
        Value::from("potion"),
    ];
    Ok(Value::Array(Rc::new(RefCell::new(items))))
});
```

### Accepting Callbacks from Script

**Rust side:**

```rust
vm.register_fn("on_button_click", |args: &[Value], ctx: &mut VmContext| {
    let callback = args[0].clone();
    // Store it and call later
    callbacks.push(callback);
    Ok(Value::Void)
});

// Later, invoke the stored callback
for cb in &callbacks {
    ctx.call_value(cb, &[])?;
}
```

**Script side:**

```rust
on_button_click(|| {
    print("button was clicked!");
});
```

---

## Calling Into Script from Rust

### Call a Named Function

```rust
// Script defines: fn greet(name) { "Hello, {name}!" }
let result = vm.call("greet", &[Value::from("World")])?;
println!("{}", result.as_str().unwrap());  // "Hello, World!"
```

### Call with VmContext Inside a Native Function

```rust
vm.register_fn("process_entity", |args: &[Value], ctx: &mut VmContext| {
    let entity_id = &args[0];
    // Call the script's per-entity update function
    ctx.call("entity_update", &[entity_id.clone()])
});
```

### Global Variable Access

```rust
// Script sets: let player_name = "Alice";
if let Some(name) = vm.get_global("player_name") {
    println!("Player: {:?}", name);
}

vm.set_global("player_name", Value::from("Bob"));
```

---

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

        // Register API
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
struct Player {
    name: str,
    hp: i64,
    x: f64,
    y: f64,
}

let mut player = Player { name: "Hero", hp: 100, x: 0.0, y: 0.0 };

fn on_update(dt) {
    // Move player based on input
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

---

## Hot Reload

### Rust Side

```rust
vm.enable_hot_reload("scripts/", |vm: &mut Vm| {
    println!("Scripts reloaded! FPS: {:.1}", current_fps);
    // Globals are preserved automatically
})?;

// Keep the host running — the watcher thread handles changes
loop {
    engine.update();
    std::thread::sleep(Duration::from_millis(16));
}
```

Hot reload preserves **global variable values** across recompilations. Function bodies, new globals, and removed globals are updated. Foreign registrations (`register_fn`, `foreign_type!`) survive unchanged.

---

## Cargo.tomm Setup

```toml
[dependencies]
zenlang = { git = "https://github.com/SonicZentropy/zenlang" }

# Or with specific features:
zenlang = { git = "https://github.com/SonicZentropy/zenlang", default-features = false, features = ["json", "fs", "lsp"] }
```

## Common Patterns

### Game Loop

```rust
struct Game {
    player_hp: i64,
    score: i64,
}

impl Game {
    fn new() -> Game { Game { player_hp: 100, score: 0 } }
    fn update(&mut self, dt: f64) {
        // game logic here
    }
    fn is_alive(&self) -> bool { self.player_hp > 0 }
}
```

### Error Handling

```rust
fn load_config(path: str) -> Result<map, str> {
    if !exists(path) {
        return Err("file not found");
    }
    let content = read(path);
    Ok(decode(content))
}

fn main() {
    match load_config("settings.json") {
        Ok(cfg) => print("loaded"),
        Err(e) => print("error: {e}"),
    };
}
```

### State Machine

```rust
enum State { Idle, Running, Paused }

struct Machine { state: State }

impl Machine {
    fn update(&mut self) {
        match self.state {
            Idle => { /* ... */ },
            Running => { /* ... */ },
            Paused => { /* ... */ },
        };
    }
}
```
