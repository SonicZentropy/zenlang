# Sorted Map

An ordered key-value map backed by a B-tree. Keys are kept in sorted order.

```rust
sorted_map_new() -> SortedMap                        // Create empty sorted map
sorted_map_insert(map, key, value)                   // Insert key-value pair
sorted_map_get(map, key) -> Option                   // Get value by key
sorted_map_remove(map, key)                          // Remove key-value pair
sorted_map_contains(map, key) -> bool                // Check if key exists
sorted_map_len(map) -> i64                           // Number of entries
sorted_map_keys(map) -> Array                        // All keys in sorted order
sorted_map_values(map) -> Array                      // All values (key order)
sorted_map_entries(map) -> Array                     // All [key, value] pairs
sorted_map_range(map, start, end) -> Array           // Entries in key range
```

```rust
let sm = sorted_map_new();
sorted_map_insert(sm, "zebra", 1);
sorted_map_insert(sm, "apple", 2);
sorted_map_insert(sm, "mango", 3);

let keys = sorted_map_keys(sm);
// keys is ["apple", "mango", "zebra"]
assert(keys[0] == "apple");
assert(keys[2] == "zebra");

let range = sorted_map_range(sm, "mango", "zebra");
assert(len(range) == 2);
```
