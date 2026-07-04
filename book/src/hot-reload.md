# Hot Reload

Zen can watch source files and recompile them at runtime while preserving
global state.

## CLI Usage

```bash
zenc run --watch game.zen
```

## Programmatic Usage

Use the `HotReloader` struct:

```rust
use zenlang::hotreload::HotReloader;
use zenlang::VM;

let vm = VM::new();
// Register natives, foreign types, etc. before creating the reloader:
// vm.register_native("spawn", ...);

let mut reloader = HotReloader::new(["game.zen"], vm);

loop {
    if reloader.tick()? {
        println!("Scripts reloaded!");
    }
    // Access the VM:
    // let vm = reloader.vm_mut();
    std::thread::sleep(std::time::Duration::from_millis(16));
}
```

The first path is the entry script; additional paths are watched but don't
trigger recompilation. Files pulled in via `mod name;` are auto-discovered.

## Manual Reload

```rust
reloader.force_reload()?;
```

## Accessing the VM

```rust
let vm = reloader.vm();        // immutable reference
let vm = reloader.vm_mut();    // mutable reference (e.g. to run main)
```

## Watched Paths

```rust
for path in reloader.watched_paths() {
    println!("watching: {}", path.display());
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

The reloader polls file modification times. When a change is detected:

1. Re-reads the entire entry script (and all transitively `mod`-included files)
2. Re-lexes, re-parses, re-resolves, re-typechecks, and re-compiles
3. Diffs the global variable table against the current VM state
4. Preserves matching global values via `reload_functions()`
5. Replaces compiled bytecode atomically
