# JSON

The `std::json` module provides JSON encoding and decoding.

```rust
use std::json::*;
```

## Functions

```rust
encode(value)    // Convert Zenlang value to JSON string
decode(string)   // Parse JSON string to Zenlang value
```

## Example

```rust
let data = map_new();
map_set(data, "name", "Zenlang");
map_set(data, "version", 1);
map_set(data, "features", ["hot_reload", "embed"]);

let json_str = encode(data);
// {"name":"Zenlang","version":1,"features":["hot_reload","embed"]}

let decoded = decode(json_str);
assert(decoded == data);
```
