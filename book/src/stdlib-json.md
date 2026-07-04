# JSON

The `std::json` module provides JSON encoding and decoding.

```rust
use std::json::*;
```

## Functions

```rust
encode(value)    // Convert Zen value to JSON string
decode(string)   // Parse JSON string to Zen value
```

## Example

```rust
let data = map_new();
map_set(data, "name", "Zen");
map_set(data, "version", 1);
map_set(data, "features", ["hot_reload", "embed"]);

let json_str = encode(data);
// {"name":"Zen","version":1,"features":["hot_reload","embed"]}

let decoded = decode(json_str);
assert(decoded == data);
```
