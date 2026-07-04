# Foreign Types

Expose Rust types to Zenlang with fields and methods using the `#[derive(ZenForeign)]`
and `#[zen_methods]` macros.

## Basic Usage

**Rust side:**

```rust
use zenlang::{VM, Value, ZenForeign, zen_methods, zen_native_fn};

#[derive(Clone, Debug, ZenForeign)]
struct Player {
    name: String,
    health: i32,
    max_health: i32,
}

#[zen_methods]
impl Player {
    // Constructors: no &self, returns Self
    fn new(name: &str) -> Self {
        Self { name: name.into(), health: 100, max_health: 100 }
    }

    // Instance methods: &self or &mut self
    fn heal_percent(&self) -> f64 {
        self.health as f64 / self.max_health as f64 * 100.0
    }
}

fn main() -> zenlang::Result<()> {
    let mut vm = VM::new();

    // Register the type and its methods
    Player::register_zen_foreign(&mut vm);
    Player::register_zen_methods(&mut vm);

    // Register a native constructor function from the script side
    vm.register_native(
        "create_player",
        std::rc::Rc::new(|ctx: &mut VMContext, args: &[Value]| {
            let name = args.first().and_then(|v| v.as_str()).unwrap_or_default();
            let player = Player::new(&name);
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            Ok(vm.wrap_foreign("Player", player))
        }),
    );

    // ... compile and run script
    Ok(())
}
```

**Script side:**

```zen
let p = create_player("Aria");
print("Name:", p.name);       // field access
print("Health:", p.health);
let pct = p.heal_percent();   // method call
p.health = 50;                // mutable field set
```

## How It Works

### `#[derive(ZenForeign)]`

This derive macro generates a `register_zen_foreign()` method that:

- Registers the type with the VM via `vm.register_type::<T>(name)`
- Creates getters and setters for each named field
- Returns `Value::Nil` when setting to `Nil` for `Option<T>` fields

Supported field types:

| Rust Type | Zenlang Type | Notes |
|-----------|-------------|-------|
| `String` | `Str` | |
| `i64`, `i32`, `i16`, `i8` | `Int` | Casts on set |
| `u64`, `u32`, `u16`, `u8` | `Int` | Casts on set |
| `f64`, `f32` | `Float` | Casts on set |
| `bool` | `Bool` | |
| `Value` | any | Pass-through |
| `Option<T>` | `T` or `Nil` | `None` ↔ `Nil` |

### `#[zen_methods]`

This attribute macro generates `register_zen_methods()` which registers all methods
in the impl block as callable from Zenlang.

**Methods without `&self`** that return `Self` are treated as **constructors** and
registered as native functions. They can be called like `TypeName::method(...)` in script.

**Methods with `&self` or `&mut self`** are registered as instance methods and
called with `obj.method(...)` in script.

Supported parameter/return types match the field types table above.

### `#[zen_native_fn]`

For standalone native functions (not attached to a foreign type), use this
attribute to generate a type signature:

```rust
#[zen_native_fn(name: "contains", params: [Str, Str], returns: Bool)]
fn contains_impl(vm: &mut VMContext, args: &[Value]) -> Result<Value> { ... }
```

The `name` field is optional; defaults to the Rust function name.
Generates a `<fn_name>_sig()` function returning a `FnSignature`.

## Type Name Customization

By default the Zenlang type name matches the Rust struct name. Override it with
`#[foreign(name = "...")]`:

```rust
#[derive(ZenForeign)]
#[foreign(name = "Texture")]
struct MyTexture {
    id: u32,
    width: u32,
    height: u32,
}
```

Now the type is accessible as `Texture` in scripts.

## Auto-Generated Default Constructor

Add `#[foreign(default)]` to auto-register a constructor that calls
`Default::default()`:

```rust
#[derive(Clone, Default, ZenForeign)]
#[foreign(default)]
struct Transform {
    x: f64,
    y: f64,
}
```

This registers a native function named `"Transform"` that creates instances
with default values. The struct must implement `Default`.

## `Option<T>` Fields

Fields of type `Option<T>` map `None` to `Value::Nil` and `Some(v)` to the
converted value. This works for both getters and setters:

```rust
#[derive(Clone, ZenForeign)]
struct Profile {
    name: String,
    bio: Option<String>,   // can be Nil in script
    age: Option<i32>,
}
```

```zen
let p = create_profile("Aria");
p.bio = "A programmer";     // set to Some("A programmer")
p.bio = nil;                // set to None
```

## Full Example

See [`examples/foreign_types.rs`](https://github.com/SonicZentropy/zenlang/blob/main/examples/foreign_types.rs)
for a complete working example.
