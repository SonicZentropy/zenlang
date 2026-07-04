# Hashing

Fast non-cryptographic hash functions for checksums and asset caching.

```rust
fnv1a(data: str) -> str     // 64-bit FNV-1a hash (16 hex chars)
crc32(data: str) -> str     // CRC32 checksum (8 hex chars)
hash_str(data: str) -> str  // SipHash-2-4 via DefaultHasher (16 hex chars)
```

```rust
let h = fnv1a("hello");
assert(len(h) == 16);

let c = crc32("hello world");
assert(len(c) == 8);
```
