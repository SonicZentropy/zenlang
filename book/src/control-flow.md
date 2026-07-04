# Control Flow

## if / else

`if` is an expression — it returns a value.

```rust
let x = if cond { 1 } else { 2 };
```

The `else` branch is required when used as an expression, unless the type is `void`.

```rust
if cond {
    do_something();
}
```

Chaining:

```rust
let label = if x < 0 {
    "negative"
} else if x == 0 {
    "zero"
} else {
    "positive"
};
```

## While Loops

```rust
let mut i = 0;
while i < 10 {
    print(i);
    i = i + 1;
}
```

`break` and `continue` are supported inside loops:

```rust
loop {
    if done { break; }
    if skip { continue; }
    process();
}
```

## For Loops

Iterate over ranges, arrays, strings, maps, and custom iterators.

```rust
// Exclusive range
for i in 0..5 { print(i); }

// Inclusive range
for i in 0..=5 { print(i); }

// Arrays
for x in [1, 2, 3] { sum = sum + x; }

// Strings (iterates characters)
for c in "hello" { print(c); }

// Maps (iterates [key, value] pairs)
for kv in my_map { print(kv); }

// Custom iterators
for x in Counter { current: 0, max: 5 } { print(x); }
```

## if let / while let

Sugar for match expressions that bind a single pattern.

```rust
if let Some(v) = opt {
    print(v);
} else {
    print("none");
}

while let Some(v) = iter {
    print(v);
}
```

## Try Operator `?`

The `?` operator unwraps `Result` values, returning early on `Err`.

```rust
fn try_unwrap() -> Result<i64, str> {
    let x = try_ok()?;
    Ok(x)
}
```

This desugars to:

```rust
match try_ok() {
    Ok(val) => val,
    Err(e) => return Err(e),
}
```
