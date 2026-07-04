# Modules and Visibility

## Inline Modules

Define a module inline with `mod`:

```rust
mod math {
    fn add(a: i64, b: i64) -> i64 { a + b }
    fn subtract(a: i64, b: i64) -> i64 { a - b }
}
```

## File Modules

A module can be loaded from a separate file:

```rust
mod greeting;
```

This loads `greeting.zen` from the same directory as the current file.

## `use` Declarations

Import names from other modules:

```rust
use math::add;
use math::square;

use std::io;
```

## Visibility

Use `pub` to make items visible outside their module:

```rust
pub fn visible_everywhere() -> i64 { 42 }
pub struct Point { x: i64, y: i64 }
pub enum Color { Red, Green, Blue }
pub const NAME: str = "zen";
pub type MyResult = Result<i64, str>;
pub use math::add;
pub mod my_mod { /* ... */ }
```
