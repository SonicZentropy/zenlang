# Match Expressions

`match` is a powerful pattern-matching expression.

## Matching Literals

```rust
let val = match x {
    1 => "one",
    2 => "two",
    _ => "other",   // wildcard
};
```

## Matching Enums

```rust
enum Shape {
    Circle(i64),
    Rect(i64, i64),
}

let area = match shape {
    Circle(r) => 3.14 * r * r,
    Rect(w, h) => w * h,
};
```

## Match Guards

```rust
match value {
    n if n > 5 => "big",
    n if n > 0 => "small",
    _ => "zero or negative",
};
```

## Matching Options

```rust
match result {
    Ok(val) => print(val),
    Err(msg) => print("error: " + msg),
};
```
