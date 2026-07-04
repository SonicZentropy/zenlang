# Generics

Zenlang supports generic type parameters with `<T>` syntax. Generics are **type-erased** — there is no monomorphization. All values are `Value` enum variants at runtime.

## Generic Functions

```rust
fn identity<T>(x: T) -> T { x }
fn first<T>(arr: [T]) -> T { arr[0] }
```

## Generic Structs

```rust
struct Wrapper<T> { value: T }

let w = Wrapper { value: 42 };
```

## Generic Enums

The built-in `Option` and `Result` types use generics:

```rust
enum Option<T> { Some(T), None }
enum Result<T, E> { Ok(T), Err(E) }
```

## Generic Impl Blocks

```rust
impl<T> Vec<T> {
    fn push(&self, val: T) { /* ... */ }
}
```

Because generics are type-erased, you don't pay monomorphization costs, but you also don't get compile-time type specialization.
