# Functions and Closures

## Functions

Functions are defined with the `fn` keyword. The last expression is the return value (implicit return).

```rust
fn add(a, b) {
    a + b
}

fn greet(name: str) -> str {
    "Hello, " + name + "!"
}

fn factorial(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}
```

### Type Annotations

Parameters and return types can be annotated with `:` followed by a type.

```rust
fn distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    sqrt((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1))
}
```

### Default Parameters

Parameters can have default values. Omitted arguments use the default.

```rust
fn greet(name: str, greeting: str = "Hello") {
    print("{greeting}, {name}!");
}

greet("World");               // "Hello, World!"
greet("World", "Hi");         // "Hi, World!"
```

### Expression Bodies

Single-expression functions can omit the braces:

```rust
fn double(x) -> x * 2;
```

## Closures (Lambdas)

Closures are anonymous functions defined with pipe syntax `|params| body`.

```rust
let double = |x| x * 2;
let add = |a, b| a + b;
```

Closures capture variables from their enclosing scope by reference (via `Rc` clone).

```rust
let base = 10;
let adder = |x| x + base;
assert(adder(5) == 15);
```

Closures can be passed to iterator adapters:

```rust
let doubled = map([1, 2, 3], |x| x * 2);
```
