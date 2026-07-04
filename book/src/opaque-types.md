# Opaque Types

Opaque types are Rust-side types exposed to Zenlang through FFI. They are registered in Rust using [`#[derive(ZenForeign)]` and `#[zen_methods]`](./foreign-types.md), or the unified `foreign_type!` macro.

## In Script

Opaque types can be constructed and their methods called, but their internal layout is hidden:

```rust
let tex = Texture::load("player.png");
tex.set_filter(Linear);
draw_sprite(tex, x, y);
```

## Type Erasure

Because generic type parameters are erased at runtime, you can use opaque types generically and they will work correctly — all values are `Value` enum variants internally, and opaque types are stored as `Rc<dyn Any>`.
