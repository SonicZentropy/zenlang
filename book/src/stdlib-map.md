# Maps

| Function | Description |
|----------|-------------|
| `map_new()` | Create an empty map |
| `map_set(m, k, v)` | Set key-value pair |
| `map_get(m, k)` | Get value by key, returns `Option` |
| `map_has(m, k)` | Check if key exists |
| `map_remove(m, k)` | Remove key-value pair |
| `map_keys(m)` | Return array of keys |
| `map_values(m)` | Return array of values |
| `map_len(m)` | Return number of entries |
| `map_clear(m)` | Remove all entries |

## Example

```rust
let inventory = map_new();
map_set(inventory, "gold", 100);
map_set(inventory, "sword", "iron");

let gold = map_get(inventory, "gold");   // Some(100)
assert(map_has(inventory, "sword"));
assert(!map_has(inventory, "shield"));
```
