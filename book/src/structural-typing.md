# Structural Typing

Zen uses **structural typing** for struct types. Two struct types are compatible if they have the same field names and types, regardless of their declaration names.

```rust
struct Point { x: i64, y: i64 }
struct Coord { x: i64, y: i64 }

fn dist(p: Point) { /* ... */ }

let c = Coord { x: 3, y: 4 };
dist(c);  // OK — structurally compatible
```

This allows flexibility when working with different modules that define compatible shapes.

## How It Works

During type checking, struct types are compared by their field layout (name + type pairs), not by their declaration identity. Enum variant types are identified by their declaration identity (nominal).

## Width Subtyping

A struct with **extra fields** can be passed where a smaller struct is expected:

```rust
struct Vec2 { x: f64, y: f64 }
struct Vec3 { x: f64, y: f64, z: f64 }

fn length2d(v: Vec2) -> f64 {
    sqrt(v.x * v.x + v.y * v.y)
}

let v = Vec3 { x: 3.0, y: 4.0, z: 5.0 };
length2d(v);  // OK — Vec3 has all Vec2 fields
```

But a struct **missing** a required field is rejected:

```rust
fn accept(p: Point) { /* ... */ }
let p = Point { x: 10 };  // TYPE ERROR — missing field 'y'
```

## Excess Property Checks

Struct **literals** with unknown fields are rejected, even if the extra field matches nothing:

```rust
struct P { x: i64 }
let p = P { x: 10, z: 99 };  // TYPE ERROR — excess property 'z'
```

This catches typos and unintentional extra fields in constructors. The check applies to literals only — passing a pre-built struct with extra fields via width subtyping is allowed (see above).
