# Bindings and Constants

## Let Bindings

Use `let` to bind a value to a name. Bindings are **immutable by default**.

```rust
let x = 42;
let name = "Zen";
let pi: f64 = 3.14159;  // with type annotation
```

## Mutable Bindings

Use `let mut` to create a mutable binding.

```rust
let mut count = 0;
count = count + 1;
count += 5;
```

## Constants

Constants are defined at the top level and evaluated at compile time.

```rust
const MAX_SPEED = 100;
const PI: f64 = 3.14159;
```
