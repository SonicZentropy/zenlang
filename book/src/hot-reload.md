# Hot Reload

Zenlang can watch source files and recompile them at runtime while preserving global state.

## CLI Usage

```bash
zenc run --watch game.zen
```

## Programmatic Usage

```rust
let mut vm = Vm::new();
vm.enable_hot_reload("scripts/", |vm: &mut Vm| {
    println!("Scripts reloaded! FPS: {:.1}", current_fps);
})?;

// Keep the host running — the watcher thread handles changes
loop {
    engine.update();
    std::thread::sleep(Duration::from_millis(16));
}
```

## What Gets Preserved

- **Global variables** — Values are carried across recompilations by name
- **Foreign types and functions** — Rust-side registrations remain

## What Gets Replaced

- **Function bodies** — Updated to new compiled bytecode
- **New globals** — Added after recompile
- **Removed globals** — Cleared after recompile

## Architecture

The watcher runs in a separate thread using `notify`. When a change is detected:

1. Reads the source file
2. Compiles it
3. Diffs the global variable table against the current VM state
4. Preserves matching global values
5. Replaces the compiled bytecode atomically
