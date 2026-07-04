# Maps

Free-function and method-call styles are both supported:

| Function | Method | Description |
|----------|--------|-------------|
| `map_new()` | — | Create an empty map |
| `map_set(m, k, v)` | `m.set(k, v)` | Set key-value pair |
| `map_get(m, k)` | `m.get(k)` | Get value by key, returns `Option` |
| `map_has(m, k)` | `m.has(k)` / `m.contains_key(k)` | Check if key exists |
| `map_remove(m, k)` | `m.remove(k)` | Remove key-value pair |
| `map_keys(m)` | `m.keys()` | Return array of keys |
| `map_values(m)` | `m.values()` | Return array of values |
| `map_len(m)` | `m.len()` / `m.count()` | Return number of entries |
| `map_clear(m)` | `m.clear()` | Remove all entries |
| — | `m.is_empty()` | Check if map is empty |

## Example

```rust
let inventory = map_new();
inventory.set("gold", 100);
inventory.set("sword", "iron");

let gold = inventory.get("gold");   // Some(100)
assert(inventory.has("sword"));
assert(!inventory.has("shield"));
```
