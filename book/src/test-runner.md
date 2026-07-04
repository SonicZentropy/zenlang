# Test Runner

The built-in test runner discovers and executes `.zen` test files.

```bash
# Run all tests
zenc test

# Run specific files
zenc test tests/first.zen tests/second.zen

# Run with error-level output
zenc test 2>&1 | Out-Null  # (Windows: squelch output)
```

## Writing Tests

Tests are plain `.zen` files with `assert` calls:

```rust
// tests/math.zen
fn test_addition() {
    assert(1 + 1 == 2);
}

fn test_subtraction() {
    assert(5 - 3 == 2);
}
```

If any `assert` fails, the test runner reports the file, line, and reason. If a script panics, the runner catches the panic and reports it as a test failure.
