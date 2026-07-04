# Memory Management

Zen uses a simple, deterministic memory strategy: no garbage collector and no borrow checker.

## Value Representation

All values are stored as a `Value` enum:

```rust
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Rc<str>),
    Array(Rc<RefCell<Vec<Value>>>),
    Map(Rc<RefCell<HashMap<Value, Value>>>),
    Function(Rc<FnValue>),
    Foreign(Rc<dyn Any>),
    Void,
}
```

## Reference Counting

- **Strings** — `Rc<str>`, cloned by incrementing the reference count
- **Arrays/Maps** — `Rc<RefCell<...>>`, shared mutable state
- **Functions** — `Rc<FnValue>`, closures capture their environment as cloned `Rc` handles

## No GC Pauses

Because Zen uses `Rc` instead of a tracing GC, there are no stop-the-world pauses. Memory is reclaimed deterministically when reference counts drop to zero.

## Arena / Slab Allocator

The compiler and runtime use slab-based allocation for internal data structures, reducing allocation overhead for short-lived objects.
