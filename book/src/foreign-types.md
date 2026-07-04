# Foreign Types

Expose Rust types to Zenlang with fields and methods using the `foreign_type!` macro.

```rust
use zenlang::foreign_type;

struct Texture {
    id: u32,
    width: u32,
    height: u32,
}

foreign_type! {
    type Texture = "Texture" {
        fields {
            width: i64,
            height: i64,
        }
        methods {
            fn load(path: &str) -> Texture;
            fn get_size(&self) -> (i64, i64);
        }
    }
}

// Implement the methods
impl ForeignTexture {
    fn load(path: &str) -> Result<Value, VmError> {
        let tex = Texture::load_from_file(path)?;
        Ok(ForeignTexture::new(tex).into())
    }

    fn get_size(&self) -> Result<Value, VmError> {
        Ok((self.0.width as i64, self.0.height as i64).into())
    }
}
```

## In Script

```rust
let tex = Texture::load("player.png");
assert(tex.width > 0);
let (w, h) = tex.get_size();
```

## Foreign Type with Mutable State

**Rust side:**

```rust
pub struct Transform {
    pub x: f64, pub y: f64,
    pub rotation: f64, pub scale: f64,
}

foreign_type! {
    type Transform = "Transform" {
        fields { x: f64, y: f64 }
        methods {
            fn new() -> Transform;
            fn translate(&mut self, dx: f64, dy: f64);
            fn get_rotation(&self) -> f64;
            fn set_rotation(&mut self, r: f64);
        }
    }
}

impl ForeignTransform {
    fn new() -> Result<Value, VmError> {
        Ok(ForeignTransform::new(Transform {
            x: 0.0, y: 0.0, rotation: 0.0, scale: 1.0,
        }).into())
    }
    fn translate(&mut self, dx: f64, dy: f64) -> Result<Value, VmError> {
        self.0.x += dx; self.0.y += dy;
        Ok(Value::Void)
    }
    fn get_rotation(&self) -> Result<Value, VmError> {
        Ok(Value::Float(self.0.rotation))
    }
    fn set_rotation(&mut self, r: f64) -> Result<Value, VmError> {
        self.0.rotation = r;
        Ok(Value::Void)
    }
}
```

**Script side:**

```rust
let t = Transform::new();
t.translate(10.0, 5.0);
t.set_rotation(1.57);
assert(t.x == 10.0);
```

## Foreign Type with Enum-Style Variants

**Rust side:**

```rust
pub enum Shape {
    Circle { radius: f64 },
    Rect { w: f64, h: f64 },
}

foreign_type! {
    type Shape = "Shape" {
        fields {}
        methods {
            fn circle(radius: f64) -> Shape;
            fn rect(w: f64, h: f64) -> Shape;
            fn area(&self) -> f64;
        }
    }
}

impl ForeignShape {
    fn circle(radius: f64) -> Result<Value, VmError> {
        Ok(ForeignShape::new(Shape::Circle { radius }).into())
    }
    fn rect(w: f64, h: f64) -> Result<Value, VmError> {
        Ok(ForeignShape::new(Shape::Rect { w, h }).into())
    }
    fn area(&self) -> Result<Value, VmError> {
        match &self.0 {
            Shape::Circle { radius } => Ok(Value::Float(3.14159 * radius * radius)),
            Shape::Rect { w, h } => Ok(Value::Float(w * h)),
        }
    }
}
```

**Script side:**

```rust
let c = Shape::circle(5.0);
let r = Shape::rect(3.0, 4.0);
print(c.area());   // 78.53975
print(r.area());   // 12.0
```

Foreign types are stored as `Rc<dyn Any>` and accessed through thin wrappers.
