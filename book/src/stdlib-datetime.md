# Date and Time

Functions for working with dates and times.

```rust
now() -> f64                       // Current Unix timestamp (seconds since epoch)
format(timestamp: f64, fmt: str) -> str    // Format a timestamp
```

The `format` function uses `strftime`-style format specifiers:

| Specifier | Description |
|-----------|-------------|
| `%Y`      | 4-digit year |
| `%m`      | 2-digit month (01-12) |
| `%d`      | 2-digit day (01-31) |
| `%H`      | 2-digit hour (00-23) |
| `%M`      | 2-digit minute (00-59) |
| `%S`      | 2-digit second (00-59) |

```rust
let ts = now();
assert(type_of(ts) == "float");

let date = format(ts, "%Y-%m-%d");
assert(len(date) == 10);
```
