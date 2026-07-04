# Core Functions

## I/O

```rust
print(...)          // Print values to stdout with newline
print_nl(...)       // Print values without newline
read_line()         // Read a line from stdin
assert(cond)        // Panic if condition is false
assert_eq(a, b)     // Panic if a != b (deep structural comparison)
```

## Type Utilities

```rust
to_str(val)         // Convert any value to string representation
to_int(val)         // Convert to i64
to_float(val)       // Convert to f64
type_of(val)        // Return type name as string
```

## String Functions

Free-function and method-call styles are both supported:

| Function | Method | Description |
|----------|--------|-------------|
| `len(s)` | `s.len()` / `s.count()` | Length of string |
| `contains(s, sub)` | `s.contains(sub)` | Check if string contains substring |
| `trim(s)` | `s.trim()` | Trim whitespace |
| `to_upper(s)` | `s.to_upper()` | Convert to uppercase |
| `to_lower(s)` | `s.to_lower()` | Convert to lowercase |
| `substring(s, i, j)` | `s.substring(i, j)` | Extract substring from i to j |
| — | `s.starts_with(sub)` | Check prefix |
| — | `s.ends_with(sub)` | Check suffix |
| — | `s.is_empty()` | Check if empty |

## Array Functions

Free-function and method-call styles are both supported:

| Function | Method | Description |
|----------|--------|-------------|
| `len(arr)` | `arr.len()` / `arr.count()` | Array length |
| `push(arr, v)` | `arr.push(v)` | Append to array |
| `pop(arr)` | `arr.pop()` | Remove and return last element |
| `insert(arr, i, v)` | `arr.insert(i, v)` | Insert at index |
| `remove(arr, i)` | `arr.remove(i)` | Remove at index |
| `contains(arr, v)` | `arr.contains(v)` | Check if element exists |
| — | `arr.is_empty()` | Check if empty |
| — | `arr.clear()` | Remove all elements |

## Range Methods

Ranges also support method-call syntax:

| Method | Description |
|--------|-------------|
| `r.len()` / `r.count()` | Number of elements in the range |
| `r.contains(v)` | Check if value is in range |
| `r.is_empty()` | Check if range is empty |

```rust
let r = 0..5;
r.len();          // 5
r.contains(3);    // true
```

## Misc

```rust
exit(code)          // Exit process with code
next(gen)           // Advance generator, returns Option
```
