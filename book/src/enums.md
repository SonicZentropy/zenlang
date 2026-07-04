# Enums

Enums represent a type that can be one of several variants, optionally carrying data.

## Defining Enums

```rust
// Unit variants only
enum Color { Red, Green, Blue }

// Variants with data
enum Shape { Circle(i64), Rect(i64, i64) }

// Mixed
enum Status { Active, Inactive, Error(str) }
```

## Construction

Enum variants are constructed using call syntax:

```rust
let c = Red;
let shape = Circle(10);
let err = Error("something went wrong");
```

## Pattern Matching

Enum variants are destructured with `match`:

```rust
match shape {
    Circle(radius) => assert(radius == 10),
    Rect(w, h) => assert(w > 0 && h > 0),
    _ => assert(false),
};
```

The built-in `Option` and `Result` enums work the same way:

```rust
match result {
    Ok(val) => print(val),
    Err(msg) => print("error: " + msg),
};
```
