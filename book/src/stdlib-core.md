# Core Functions

## I/O

```rust
print(...)          // Print values to stdout with newline
print_nl(...)       // Print values without newline
read_line()         // Read a line from stdin
assert(cond)        // Panic if condition is false
```

## Type Utilities

```rust
to_str(val)         // Convert any value to string representation
to_int(val)         // Convert to i64
to_float(val)       // Convert to f64
type_of(val)        // Return type name as string
```

## String Functions

```rust
len(val)            // Length of string, array, or map
contains(s, sub)    // Check if string contains substring
trim(s)             // Trim whitespace
to_upper(s)         // Convert to uppercase
to_lower(s)         // Convert to lowercase
substring(s, i, j)  // Extract substring from i to j
```

## Array Functions

```rust
push(arr, val)      // Append to array
pop(arr)            // Remove and return last element
insert(arr, i, v)   // Insert at index
remove(arr, i)      // Remove at index
len(arr)            // Array length
```

## Misc

```rust
exit(code)          // Exit process with code
next(gen)           // Advance generator, returns Option
```
