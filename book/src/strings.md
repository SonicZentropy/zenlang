# Strings

Strings are reference-counted, immutable text values.

```rust
let s = "hello";
let name = "Zen";
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

Method-call style is also supported:

```rust
"hello".len();              // 5
"hello".contains("ell");    // true
"  hi  ".trim();            // "hi"
"abc".to_upper();           // "ABC"
"XYZ".to_lower();           // "xyz"
"hello".substring(0, 2);    // "he"
"".is_empty();              // true
"hello".starts_with("he");  // true
"hello".ends_with("lo");    // true
```

## Iteration

Strings can be iterated character by character:

```rust
for c in "hello" {
    print(c);
}
```
