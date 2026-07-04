# File I/O

The `std::fs` module provides file system operations (requires the `fs` cargo feature).

```rust
use std::fs::*;
```

## Functions

```rust
read(path)        // Read file to string
write(path, str)  // Write string to file
exists(path)      // Check if path exists
```
