# Introduction

**Zen** is a lightweight, embeddable Rust-like scripting language designed for **game engines and real-time applications**. It is implemented in Rust and available as a library crate (`zenlang`) with a CLI binary (`zenc`).

## Design Philosophy

Zen was created for developers who want a familiar, ergonomic scripting language that integrates tightly with Rust without the overhead of a full language runtime.

- **Rust-like syntax** — Familiar to Rust developers, reducing the learning curve for game programmers.
- **No borrow checker** — Skips Rust's ownership system to keep implementation simple and predictable at the cost of compile-time memory guarantees.
- **No GC** — Uses `Rc`-based reference counting and an arena-based slab allocator. Deterministic, no stop-the-world pauses.
- **No async** — Synchronous, single-threaded by design; trivially embeddable.
- **Tight bytecode VM** — A register-based stack VM with ~50 opcodes; single-pass codegen, no intermediate representation.
- **Rust interop as a first-class feature** — Register foreign types with fields and methods, call Rust from scripts and scripts from Rust.
- **Hot reload** — Watch source files for changes, recompile, and reload while preserving global state.

## Use Cases

- **Game scripting** — Embed Zen in your game engine for moddable, hot-reloadable game logic.
- **Real-time applications** — Configurable instruction limits prevent runaway scripts from freezing the host.
- **Prototyping** — Rapid iteration with REPL and hot reload.
- **Education** — Rust-like syntax without the borrow checker makes it approachable for learners.

## Quick Example

```rust
struct Player {
    name: str,
    health: i64,
    x: f64,
    y: f64,
}

impl Player {
    fn is_alive(&self) -> bool {
        self.health > 0
    }

    fn take_damage(&mut self, amount: i64) {
        self.health = self.health - amount;
    }
}

fn main() {
    let p = Player {
        name: "Hero",
        health: 100,
        x: 0.0,
        y: 0.0,
    };
    assert(p.is_alive());
}
```
