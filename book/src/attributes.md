# Attributes

Attributes provide metadata about functions, types, and other items.

## `#[allow(unused)]`

Suppress warnings about unused items.

```rust
#[allow(unused)]
fn helper() {
    // ...
}
```

## `#[test]`

Mark a function as a test case for the test runner.

```rust
#[test]
fn my_test() {
    assert(1 + 1 == 2);
}

#[test]
fn test_fail() {
    assert(false);  // test runner reports failure
}
```

Tests can be discovered and run automatically with `zenc test`.
