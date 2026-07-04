# Traits

Traits define shared behavior across types.

```rust
trait Shape {
    fn area(&self) -> f64;
    fn perimeter(&self) -> f64;
}
```

## Implementing Traits

```rust
struct Circle { radius: f64 }

impl Shape for Circle {
    fn area(&self) -> f64 {
        3.14159 * self.radius * self.radius
    }

    fn perimeter(&self) -> f64 {
        2.0 * 3.14159 * self.radius
    }
}
```

## Trait Methods

Trait methods are called like regular methods:

```rust
let c = Circle { radius: 5.0 };
assert(c.area() > 0.0);
```
