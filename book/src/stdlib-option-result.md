# Option and Result

## Option

```rust
enum Option<T> { Some(T), None }
```

```rust
let val = Some(42);
assert(val != None);

match val {
    Some(v) => print(v),
    None => print("no value"),
};
```

## Result

```rust
enum Result<T, E> { Ok(T), Err(E) }
```

```rust
fn safe_div(a, b) -> Result<i64, str> {
    if b == 0 {
        Err("division by zero")
    } else {
        Ok(a / b)
    }
}

let result = safe_div(10, 0);
match result {
    Ok(val) => print(val),
    Err(msg) => print("error: " + msg),
};
```

## Try Operator `?`

The `?` operator automatically propagates errors:

```rust
fn example() -> Result<i64, str> {
    let x = safe_div(10, 2)?;  // Ok(5) → unwraps
    let y = safe_div(x, 0)?;   // Err → returns early
    Ok(y)
}
```
