# Base64

Encode and decode data using the standard Base64 alphabet (RFC 4648).

```rust
base64_encode(data: str) -> str        // Encode a string to Base64
base64_decode(encoded: str) -> Option   // Decode Base64 to a string
```

```rust
let enc = base64_encode("Hello, World!");
print(enc); // "SGVsbG8sIFdvcmxkIQ=="

let dec = base64_decode(enc);
assert(is_some(dec));
assert(unwrap(dec) == "Hello, World!");
```
