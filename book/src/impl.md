# Impl Blocks and Methods

Methods are defined inside `impl` blocks.

```rust
struct Point { x: i64, y: i64 }

impl Point {
    fn area(&self) -> i64 {
        self.x * self.y
    }

    fn double(&mut self) {
        self.x *= 2;
        self.y *= 2;
    }

    fn new(x: i64, y: i64) -> Point {
        Point { x, y }
    }
}
```

## Method Calls

```rust
let p = Point { x: 3, y: 4 };
assert(p.area() == 12);

let mut p = Point::new(3, 4);
p.double();
assert(p.x == 6);
```

## Self Parameter

- `&self` — immutable reference to the receiver
- `&mut self` — mutable reference to the receiver
- Without `self` — static method (called with `Type::method()`)
