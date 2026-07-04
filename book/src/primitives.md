# Primitive Types

## Numeric Types

| Type | Description | Size |
|------|-------------|------|
| `i64` | Signed 64-bit integer | 8 bytes |
| `f64` | Double-precision float | 8 bytes |

All numeric literals default to `i64` unless inferred as `f64` from context:

```rust
let a = 42;       // i64
let b = 3.14;     // f64
let c: f64 = 42;  // f64
```

## Boolean

| Type | Values |
|------|--------|
| `bool` | `true`, `false` |

## String

| Type | Description |
|------|-------------|
| `str` | Reference-counted, immutable UTF-8 string |

## Void

| Type | Description |
|------|-------------|
| `void` | No value (unit type). Used for functions with no return value |

## Dynamic Types

| Type | Description |
|------|-------------|
| `any` | Universal type — accepts any value, bypasses type checking |
| `unknown` | Safe top type — accepts any value but requires narrowing before use |

### `any`

Opts out of type checking entirely. Compatible with all types.

```rust
let x: any = 42;        // ok
x = "hello";            // ok — no type error
x = [1, 2, 3];          // ok

fn process(val: any) {   // parameter accepts any type
    print(val);
}

process(42);             // ok
process("hello");        // ok
```

The `any` type is compatible with all other types. It's useful for:
- Interfacing with dynamic/foreign data
- Writing generic code without type parameters
- Prototyping before adding type annotations

### `unknown`

A **safe** top type. You can assign any value to `unknown`, but you **cannot** use it directly — no field access, method calls, or assignment to other types. Narrow it with `match` first.

```rust
let x: unknown = 42;     // ok — any value accepted

x.value                  // TYPE ERROR — field access blocked
x.to_str()               // TYPE ERROR — method call blocked
let y: i64 = x;          // TYPE ERROR — not compatible

// Narrow via match to use the value:
fn describe(val: unknown) -> str {
    match val {
        i64  => "integer",
        str  => "string",
        _    => "other",
    }
}
```
