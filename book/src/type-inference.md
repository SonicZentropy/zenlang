# Type Inference

Zen has full type inference — type annotations are usually optional.

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

## Opting Out with `any`

Use the `any` type to bypass type checking entirely. Variables and parameters typed as `any` accept any value:

```rust
let x: any = 42;        // ok
x = "hello";            // ok — no type error
x = [1, 2, 3];          // ok

fn process(val: any) {   // parameter accepts any type
    print(val);
}
```

This is useful when working with dynamic data or when you want to defer type decisions.
