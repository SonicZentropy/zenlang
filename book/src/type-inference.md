# Type Inference

Zenlang has full type inference — type annotations are usually optional.

```rust
fn add(a, b) = a + b;                // inferred: fn(i64, i64) -> i64
let items = [1, "hello", true];      // inferred: [{i64, str, bool}]
let mut x = 42;                      // inferred: i64
```

Type annotations can be added for clarity or to override inference:

```rust
fn distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    sqrt((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1))
}
```

Inference works across function boundaries, so function return types are inferred from their body.
