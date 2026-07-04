# Structural Typing

Zenlang uses **structural typing** for nominal types (structs). Two struct types are compatible if they have the same field names and types, regardless of their declaration names.

```rust
struct Point { x: i64, y: i64 }
struct Coord { x: i64, y: i64 }

fn dist(p: Point) { /* ... */ }

let c = Coord { x: 3, y: 4 };
dist(c);  // OK — structurally compatible
```

This allows flexibility when working with different modules that define compatible shapes.

## How It Works

During type checking, struct types are compared by their field layout (name + type pairs), not by their declaration identity. Enum variants are identified by the enum type's declaration identity (nominal).
