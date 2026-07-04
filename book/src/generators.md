# Generators and Coroutines

Zen supports generator functions that can yield multiple values over time using the `yield` keyword.

## Defining a Generator

```rust
fn counter() {
    yield 1;
    yield 2;
    yield 3;
}
```

## Consuming a Generator

Use `next()` to advance the generator and get the next value:

```rust
let g = counter();
assert(next(g) == Some(1));
assert(next(g) == Some(2));
assert(next(g) == Some(3));
assert(next(g) == None);
```

## Use Cases

- **Cooperative multitasking** — interleave execution of multiple scripts
- **Lazy sequences** — generate values on demand
- **State machines** — model stateful computations with yield points
