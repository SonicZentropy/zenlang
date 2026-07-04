# Structs

Define structured data with named fields.

```rust
struct Point {
    x: i64,
    y: i64,
}
```

## Construction

```rust
let p = Point { x: 1, y: 2 };
```

### Shorthand Initialization

When variable names match field names, you can omit the value:

```rust
let x = 1;
let y = 2;
let p = Point { x, y };
```

### Spread Operator

Create a copy with some fields overridden:

```rust
let q = Point { x: 10, ..p };
```

## Field Access

```rust
assert(p.x == 1);
p.x = 10;  // requires mutable binding
```
