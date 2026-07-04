# Timers and Scheduling

The `std::timer` module provides one-shot and periodic timers.

```rust
use std::timer::*;
```

## Functions

```rust
set_timeout(callback, ms)      // Run callback once after ms
set_interval(callback, ms)     // Run callback every ms
clear_timer(id)                // Cancel a timer
```
