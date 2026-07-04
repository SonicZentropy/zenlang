# UUID

Generate random UUID v4 identifiers.

```rust
uuid_v4() -> str    // Generate a UUID v4 string
```

```rust
let id = uuid_v4();
assert(type_of(id) == "str");
assert(len(id) == 36);
// Format: xxxxxxxx-xxxx-4xxx-xxxx-xxxxxxxxxxxx
assert(id[14] == '4'); // version nibble
```
