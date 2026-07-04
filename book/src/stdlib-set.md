# Set

An unordered collection of unique values, backed by a map.

```rust
set_new() -> Set                          // Create an empty set
set_add(set, value)                       // Add a value (no-op if already present)
set_remove(set, value)                    // Remove a value
set_contains(set, value) -> bool          // Check membership
set_len(set) -> i64                       // Number of elements
set_to_array(set) -> Array                // Convert to array
set_from_array(array) -> Set              // Create set from array (deduplicates)
```

```rust
let s = set_new();
set_add(s, "apple");
set_add(s, "banana");
set_add(s, "apple"); // duplicate — ignored

assert(set_len(s) == 2);
assert(set_contains(s, "apple"));
assert(!set_contains(s, "cherry"));

let arr = set_to_array(s);
let s2 = set_from_array(["x", "y", "x"]);
```
