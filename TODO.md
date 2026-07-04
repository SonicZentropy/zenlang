Phase 1: Add New API to `VM` (before hiding internals)

### Step 1.1 — Add `CompileConfig` struct

New file or inline in `src/vm.rs`:

```rust
pub struct CompileConfig {
    pub type_check: bool,
    pub with_prelude: bool,
    pub module_path: Option<PathBuf>,
    pub source_name: String,
}

impl Default for CompileConfig {
    fn default() -> Self {
        Self {
            type_check: true,
            with_prelude: true,
            module_path: None,
            source_name: "<script>".into(),
        }
    }
}
```

### Step 1.2 — Extract private compile helper in `VM`

Add a private method `compile_to_bytecode` that encapsulates the 7-step pipeline:

```rust
impl VM {
    fn compile_to_bytecode(&self, source: &str, config: &CompileConfig)
        -> Result<(Vec<BytecodeFn>, Vec<String>)>
    {
        let tokens = Lexer::new(source).tokenize()?;
        let mut program = Parser::new(source, &tokens).parse()?;

        if let Some(path) = &config.module_path {
            mod_resolver::resolve_modules(&mut program, path)?;
        }
        if config.with_prelude {
            prelude::inject(&mut program)?;
        }

        let stdlib_names = stdlib::native_names();
        let mut symbols = resolver::resolve_with_natives(&mut program, &stdlib_names)?;

        if config.type_check {
            typeck::check(&program, &mut symbols)?;
        }

        compiler::compile(&program, &/* type_map */, &symbols, &stdlib_names, source)
    }
}
```

Wait — there's a complication. When `type_check` is false, we don't have a `TypeMap`. But the compiler needs one. Need to either:

- Option A: Pass `Option<&TypeMap>`, have compiler handle `None` by skipping type-specific optimizations
- Option B: Pass an empty `TypeMap` that returns `Type::Any` for everything

I'll go with Option B — create an `empty_type_map()` or just pass a dummy. Let me check how `TypeMap` is currently used in the compiler...

Actually, thinking more carefully: the compiler uses `types.get(expr)` to determine which opcodes to emit (e.g., `I64` vs `F64` arithmetic). If type checking is off, we can pass a `TypeMap` that defaults everything to `Type::Any`, and the compiler handles `None` or `Any` by emitting generic dynamic operations.

This might need a small change to the compiler to handle `Type::Any` gracefully (it probably already does for unannotated code). But let me just present the plan and we'll handle that during implementation.

### Step 1.3 — Add public methods on `VM`

```rust
impl VM {
    /// One-shot: compile + load + run __main__
    pub fn exec(&mut self, source: &str) -> Result<Value> {
        self.exec_with(source, &CompileConfig::default())
    }

    pub fn exec_with(&mut self, source: &str, config: &CompileConfig) -> Result<Value> {
        self.load_with(source, config)?;
        self.run_main()
    }

    /// Compile + load bytecode into VM (adds to existing scope)
    pub fn load(&mut self, source: &str) -> Result<()> {
        self.load_with(source, &CompileConfig::default())
    }

    pub fn load_with(&mut self, source: &str, config: &CompileConfig) -> Result<()> {
        let (fns, global_names) = self.compile_to_bytecode(source, config)?;
        self.load_bytecode(fns, global_names);
        Ok(())
    }

    /// Read file + compile + load
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let source = std::fs::read_to_string(path.as_ref())?;
        let config = CompileConfig {
            module_path: Some(path.as_ref().parent().unwrap().to_path_buf()),
            source_name: path.as_ref().to_string_lossy().into(),
            ..Default::default()
        };
        self.load_with(&source, &config)
    }
}
```

### Step 1.4 — Add free function `zenlang::run`

```rust
/// One-shot: create a temporary VM, compile, execute, return result.
/// Useful for quick scripts that don't need persistent state.
pub fn run(source: &str) -> Result<Value> {
    let mut vm = VM::new();
    vm.exec(source)
}
```

Re-exported from `lib.rs`.

---

## Phase 2: Make Internals `pub(crate)`

### Step 2.1 — Change module visibility in `lib.rs`

```rust
// Before:
pub mod compiler;
pub mod lexer;
pub mod parser;
pub mod resolver;
pub mod typeck;
pub mod ir;
pub mod token;
pub mod ast;
pub mod symbol;
pub mod slab;
pub mod mod_resolver;
pub mod prelude;

// After:
pub(crate) mod compiler;
pub(crate) mod lexer;
pub(crate) mod parser;
pub(crate) mod resolver;
pub(crate) mod typeck;
pub(crate) mod ir;
pub(crate) mod token;
pub(crate) mod ast;
pub(crate) mod symbol;
pub(crate) mod slab;
pub(crate) mod mod_resolver;
pub(crate) mod prelude;
```

This will break all external access to those modules. That's intentional — we'll update all internal usage during Phase 4.

### Step 2.2 — Keep tooling modules public

```rust
pub mod dap;
pub mod formatter;
pub mod hotreload;
pub mod lsp;
pub mod stdlib;
pub mod value;
pub mod span;
pub mod vm;
```

---

## Phase 3: Make VM Fields Private

### Step 3.1 — Add `pub(crate)` or private to VM fields

```rust
pub struct VM {
    stack: Vec<Value>,
    globals: Vec<Value>,
    functions: Vec<BytecodeFn>,
    global_names: Vec<String>,
    function_name_map: HashMap<String, usize>,
    natives: HashMap<String, usize>,
    native_fns: Vec<(String, NativeFn)>,
    foreign_registry: Rc<ForeignTypeRegistry>,
    // ... everything else private ...

    // Slabs — keep them at least pub(crate) or add accessors
    pub(crate) arrays: Slab<ArrayData>,
    pub(crate) structs: Slab<StructData>,
    pub(crate) enums: Slab<EnumData>,
    pub(crate) maps: Slab<MapData>,
    pub(crate) closures: Slab<ClosureData>,
    pub(crate) generators: Slab<GeneratorState>,
    pub(crate) foreigns: Slab<ForeignObject>,
    pub(crate) weaks: Slab<WeakData>,

    debug_state: DebugState,
    // ... etc ...
}
```

### Step 3.2 — Add accessor methods where needed

```rust
impl VM {
    pub fn global(&self, name: &str) -> Option<&Value> { ... }
    pub fn globals_snapshot(&self) -> HashMap<String, Value> { ... }
    // Already have: snapshot_globals_by_name, restore_globals_by_name

    pub fn current_instruction_count(&self) -> u64 { self.instruction_count }
    pub fn instruction_limit(&self) -> u64 { self.instruction_limit }
}
```

### Step 3.3 — Move `register_builtins` into `VM::new()`

```rust
impl VM {
    pub fn new() -> Self {
        let mut vm = Self::empty();
        stdlib::register_builtins(&mut vm);
        vm
    }

    /// Internal: creates VM without registering builtins
    pub(crate) fn empty() -> Self {
        Self { /* all fields default */ }
    }
}
```

Now users never call `stdlib::register_builtins` manually.

---

## Phase 4: Update Internal Call Sites

All the places that currently do the manual pipeline must switch to the new VM methods:

### Step 4.1 — `src/main.rs`

| Call site | Change |
|-----------|--------|
| `run_script()` | Replace `Lexer→Parser→Resolver→Typeck→Compiler→VM` with `VM::exec()` |
| `run_tests()` | Replace pipeline with `VM::exec()` per test |
| `run_disasm()` | Replace pipeline with `VM::load()` + disassemble from VM internals |
| `run_check()` | Replace with `VM::load_with(type_check=true)` |
| `run_repl()` | Replace with `VM::load()` per line + `call()` or direct eval |
| `cmd_build()` | Replace with `VM::load_file()` |

### Step 4.2 — `src/lsp.rs`

- Change `compile_source()` free function to use `VM::load_with()` internally
- The LSP's `compile_source` is used for hover, goto-def, completions — it needs access to `TypeMap`, `SymbolTable`, etc. Keep these `pub(crate)` so the LSP can still use them.

### Step 4.3 — Test helpers in `src/vm.rs`

- The `run()`, `try_run()`, `run_program()` helpers currently use the full pipeline. Replace with `VM::exec()`.
- Some tests may need access to internal state (e.g., checking bytecode). Those can use `pub(crate)` access since they're in the same crate.

### Step 4.4 — Examples

Update example crates (bevy, fyrox, macroquad, egor_edict integrations) to use the new API. These are the most important — they're the user-facing demonstration of the API.

---

## Phase 5: Clean Up Re-exports in `lib.rs`

```rust
pub use error::{Error, Result};
pub use span::{SourceLocation, Span, Spanned};
pub use value::Value;
pub use vm::{VM, CompileConfig};
pub use zenlang_macros::ZenForeign;
pub use zenlang_macros::foreign_type;
pub use zenlang_macros::zen_methods;
pub use zenlang_macros::zen_native_fn;

pub use crate::run;  // free function
```

Remove re-exports of internal types (`Token`, etc.).

---

## Phase 6: Test & Verify

```bash
cargo test           # All tests pass
cargo clippy         # No warnings
cargo build          # Examples build
cargo publish -p zenlang-macros  # Still publishable
cd examples/bevy_integration && cargo check  # Examples work
```

---

## Migration Guide for Users

Add to `docs/` or README a quick migration section:

| Old API | New API |
|---------|---------|
| `zenlang::Lexer::new(...)` | Hidden (internal) |
| `zenlang::Parser::new(...)` | Hidden (internal) |
| `30 lines of pipeline code` | `vm.exec(source)` or `vm.load(source)` |
| `stdlib::register_builtins(&mut vm)` | Automatic in `VM::new()` |
| `vm.xxx(CompileConfig { ... })` | Use `vm.load_with(source, config)` |

---

## Summary of All Files Changed

| File | Change |
|------|--------|
| `src/vm.rs` | Add `CompileConfig`, `compile_to_bytecode()`, `exec()`, `exec_with()`, `load()`, `load_with()`, `load_file()`, make VM fields private |
| `src/lib.rs` | Change 12 modules from `pub mod` → `pub(crate)`, update re-exports, add free `run()` |
| `src/main.rs` | Replace 6 manual pipeline call sites with VM methods |
| `src/lsp.rs` | Use VM methods internally |
| `src/compiler.rs` | Handle `None` TypeMap gracefully |
| `src/typeck.rs` | Keep `pub(crate)` as needed |

---

Ready for implementation?
