# Common Patterns

A quick-reference collection of common idioms and patterns in Zenlang.

## Hello World

```rust
print("Hello, Zenlang!");
```

## Bindings

```rust
let x = 42;               // immutable
let mut y = 10;           // mutable
y = y + 1;
let s: str = "hello";     // with type annotation
const MAX: i64 = 100;     // compile-time constant
```

## Type Annotations

```rust
let a: i64 = 42;
let b: f64 = 3.14;
let c: bool = true;
let d: str = "hi";
let e: any = "dynamic";   // any type — accepts any value
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

## Impl Blocks and Methods

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

## Traits

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

## Dynamic Typing with `any`

Use the `any` type to opt out of type checking:

```rust
let x: any = 42;        // ok
x = "hello";            // ok — no type error
x = [1, 2, 3];          // ok

fn process(val: any) {   // parameter accepts any type
    print(val);
}

process(42);             // ok
process("hello");        // ok
```

## Standard Library Patterns

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
use zenlang::vm::VM;
use zenlang::value::Value;

let mut vm = VM::new();
vm.run_main()?;
```

### Error Handling

```rust
match vm.run_main() {
    Ok(val) => println!("OK: {:?}", val),
    Err(e) => eprintln!("Error: {e}"),
}
```

## Native Functions (Rust → Script)

### Simple Native Function

```rust
vm.register_fn("add", |args: &[Value]| {
    let a = args[0].as_int().unwrap();
    let b = args[1].as_int().unwrap();
    Ok(Value::Int(a + b))
});
```

### Register Multiple Functions at Once

```rust
fn register_game_api(vm: &mut VM) {
    vm.register_fn("spawn_enemy", |args| {
        let x = args[0].as_float()?;
        let y = args[1].as_float()?;
        Ok(Value::from("enemy_01"))
    });
    vm.register_fn("play_sound", |args| {
        let name = args[0].as_str()?.to_string();
        Ok(Value::Nil)
    });
}
```

## Foreign Types (Rust Structs in Script)

### Define and Register a Foreign Type

```rust
use zenlang::foreign_type;

foreign_type! {
    name: "Texture",
    #[derive(Clone, Debug)]
    pub struct Texture {
        pub id: u32,
        pub width: u32,
        pub height: u32,
    }
    impl Texture {
        fn load(path: &str) -> Self {
            Self { id: 1, width: 64, height: 64 }
        }
        fn get_size(&self) -> (i64, i64) {
            (self.width as i64, self.height as i64)
        }
        fn is_valid(&self) -> bool {
            self.id != 0
        }
    }
}
```

### Foreign Type with Mutable State

```rust
foreign_type! {
    name: "Transform",
    #[derive(Clone, Debug, Default)]
    pub struct Transform {
        pub x: f64,
        pub y: f64,
        pub rotation: f64,
        pub scale: f64,
    }
    impl Transform {
        fn new() -> Self { Self::default() }
        fn translate(&mut self, dx: f64, dy: f64) {
            self.x += dx;
            self.y += dy;
        }
        fn get_rotation(&self) -> f64 { self.rotation }
        fn set_rotation(&mut self, r: f64) { self.rotation = r; }
    }
}
```

## Calling Into Script from Rust

### Call a Named Function

```rust
let result = vm.call("greet", &[Value::from("World")])?;
println!("{}", result.as_str().unwrap());
```

### Global Variable Access

```rust
if let Some(name) = vm.get_global("player_name") {
    println!("Player: {:?}", name);
}

vm.set_global("player_name", Value::from("Bob"));
```

## Hot Reload

```rust
vm.enable_hot_reload("scripts/", |vm: &mut VM| {
    println!("Scripts reloaded!");
    // Globals are preserved automatically
})?;
```

Hot reload preserves **global variable values** across recompilations. Function bodies, new globals, and removed globals are updated.

## Cargo.toml Setup

```toml
[dependencies]
zenlang = { git = "https://github.com/SonicZentropy/zenlang" }
```

## Game Loop

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

## Error Handling

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

## State Machine

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
