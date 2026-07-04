# Strings

Strings are reference-counted, immutable text values.

```rust
let s = "hello";
let name = "Zenlang";
```

## String Interpolation

Strings support interpolation with `{expr}` syntax:

```rust
let name = "Zen";
let msg = "Welcome to {name} v{1}!";
// Result: "Welcome to Zen v1!"
```

Use `{{` and `}}` to escape literal braces.

## String Operations

```rust
len("hello");              // 5
contains("hello", "ell");  // true
trim("  hi  ");            // "hi"
to_upper("abc");           // "ABC"
to_lower("XYZ");           // "xyz"
substring("hello", 0, 2);  // "he"
```

## Iteration

Strings can be iterated character by character:

```rust
for c in "hello" {
    print(c);
}
```
