# Changelog

## [0.3.0] — 2026-07-04

### Language
- Method-call syntax for built-in types: arrays (`.push()`, `.pop()`, `.len()`, `.insert()`, `.remove()`, `.contains()`, `.is_empty()`, `.clear()`), strings (`.len()`, `.contains()`, `.trim()`, `.to_upper()`, `.to_lower()`, `.substring()`, `.is_empty()`, `.starts_with()`, `.ends_with()`), maps (`.set()`, `.get()`, `.has()`, `.contains_key()`, `.remove()`, `.keys()`, `.values()`, `.len()`, `.is_empty()`, `.clear()`), ranges (`.len()`, `.contains()`, `.is_empty()`)

### Type Checker
- `!`, `&&`, `||` operators now accept `Type::Any` (fixes type errors on `!arr.is_empty()` etc.)
- `Expr::Range` returns `Type::Any` instead of `Type::Unit` (enables method calls on range values)

### Runtime
- `assert_eq` uses `VM::values_equal()` deep structural comparison instead of `PartialEq` handle identity (fixes false failures on structurally equal enum/map/array values)

### Documentation
- Updated `collections.md`, `strings.md`, `stdlib-core.md`, `stdlib-map.md` with method-call examples
- New `examples/method_calls.zen`

---

## [0.2.0] — 2026-07-03

### Public API & Project Structure
- Renamed "Zenlang" to "Zen" across all docs and mdbook
- Privatized 12 modules (ast, compiler, ir, lexer, etc.) to `pub(crate)`
- Added `VM::load()`, `load_with()`, `exec()`, `exec_with()`, `load_file()`, `reload()`, `disassemble()` methods
- Added `CompileConfig` for `type_check`, `with_prelude`, `module_path`, `source_name`
- Added free `pub fn run(source)` one-shot convenience function
- Added `VM::make_array(values)` public helper
- Published `zenlang v0.2.0` and `zenlang-macros v0.1.0` to crates.io
- Added `.cargo/config.toml` with profile/linker optimizations
- Added `rust-toolchain.toml` pinning stable

### Type System
- `any` keyword for dynamic typing (`let x: any = 42`)
- `Type::Any` split from `Type::Unit` as universal-compatible wildcard
- Structural typing: width subtyping, excess property checks on struct literals
- `opaque type Name = Base` for nominal isolation
- `Type::Unknown` — safe top type requiring narrowing via match/pattern
- `Type::Var` for local bidirectional type inference
- `Type::Generic("T")` for type-erased generics

### Generics
- Generic function definitions (`fn identity<T>(x: T) -> T`)
- Generic structs/enums (`struct Foo<T> { x: T }`)
- Generic impl blocks (`impl<T> Vec<T> { ... }`)
- Type erasure strategy: no monomorphization, compiles once

### Traits
- Trait declarations (`trait Shape { fn area(&self) -> f64; }`)
- `impl Trait for Type` with method resolution
- Full symbol table support

### Modules
- `mod foo;` declarations with file loading from `<name>.zen`
- `use` imports with recursive resolution
- Multi-file project support

### String Interpolation
- `"Hello {name}"` desugaring at parse time
- `{{`/`}}` escape syntax for literal braces
- VM `ToString` opcode encoding

### Try Operator
- `expr?` desugars to `match expr { Ok(v) => v, Err(e) => return Err(e) }`
- Works in functions returning `Result<T, E>`

### V0.1.0 Features Completed
- `if let` / `while let` (desugar to match/loop at parse time)
- `..` spread operator in struct literals
- Named field shorthand in struct literals (`Point { x, y }`)
- `impl` block compilation (methods as `TypeName::method` entries)
- Method calls on struct types with `self` receiver
- Field access type-checking and compile-time index resolution
- Closures with upvalue capture (by-value Rc clone)
- Enum variant construction via call syntax (`Some(42)`)
- Pattern matching with `LoadEnumTag`/`LoadEnumField` opcodes
- `Option<T>` / `Result<T, E>` with auto-registered helpers (`is_some`, `unwrap`, `map`, `and_then`, etc.)
- Exhaustive match checking
- Native function signatures for accurate type-checking
- Iterator protocol + `for` loops over non-range iterables
- Prelude (`.zen` stdlib): `map`, `filter`, `fold`, `enumerate`, `take`, `zip`, `collect`

### Foreign Type Interop
- Unified `foreign_type!` proc macro
- `#[derive(ZenForeign)]` for auto-generating field getters/setters
- `#[zen_methods]` attribute for auto-registering foreign methods
- `#[zen_native_fn]` for compile-time type signatures
- `VM::wrap_foreign()` safe constructor helper
- `VM::make_struct()` builder API
- `VMContext::call_value()` with reentrancy-safe `return_to_depth`
- `TryFrom<Value>` / `From<T>` impls for i64, f64, bool, String
- `ForeignObject` clone support
- JSON serialization (`to_json`/`from_json` backed by serde_json)

### Runtime Improvements
- Coroutines/generators: `yield` keyword, `GeneratorState`, `next()` native
- `VM::set_instruction_limit()` to prevent infinite loops
- Weak references (`Value::Weak`) for breaking Rc cycles
- `after(seconds, callback)` and `every_frame(callback)` scheduling
- `Value::Struct` optimized to `Vec<Value>` with compile-time field indices
- Arena/slab refactor for heap-allocated values
- `on_reload()` hook called after successful hot reload
- Runtime error stack traces with function names
- `assert_eq` converted from `panic!` to `Error::Script`

### Tooling
- DAP debug adapter server (breakpoints, step over/into/out, variable tree)
- VM debug infrastructure (breakpoints, stepping modes, stack inspection)
- LSP: goto-definition, completions, hover docs, document symbols, semantic tokens (13 unit tests)
- `zenc new` / `zenc build` project scaffolding
- Formatter: `for...in` spacing fix
- Zed extension with syntax highlighting, formatting, diagnostics

### Documentation
- Full mdbook with 30+ pages covering language guide, stdlib, embedding, tooling
- COOKBOOK.md with practical patterns
- LLM skill file for AI-assisted development
- Rustdoc examples on public API items
- Updated all examples to use new VM API

---

## [0.1.0] — 2026-06-26

Initial release. Core language features:

### Language
- Rust-like syntax: `let`, `const`, `fn`, `if`/`else`, `match`, `while`, `for`, `loop`, `break`, `continue`
- Structs with named fields, enums with data variants
- Expression-oriented: blocks return values, `if`/`match` are expressions
- Compound assignment (`+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`)
- Bitwise operators (`&`, `|`, `^`, `~`, `<<`, `>>`)
- Numeric separators (`1_000_000`)
- Unicode identifiers
- Dynamic typing with `any`-compatible type erasure

### VM
- Register-based stack VM with ~50 opcodes
- `Rc`-based reference counting (no GC)
- Bytecode compiler with single-pass codegen
- 24-variant `Value` enum (Nil, Bool, Int, Float, Str, Array, Struct, Enum, Function, NativeFunction, Foreign, Closure, Range, Map, Weak, Generator)

### Standard Library
- Core: `print`, `assert`, `to_str`, `to_int`, `to_float`, `type_of`
- Strings: `len`, `contains`, `trim`, `to_upper`, `to_lower`, `substring`
- Arrays: `push`, `pop`, `insert`, `remove`, `len`
- Math: `abs`, `min`, `max`, `sqrt`, `sin`, `cos`, `tan`, `atan2`, `lerp`, `clamp`, RNG
- Maps: `map_new`, `map_set`, `map_get`, `map_has`, `map_remove`, `map_keys`, `map_values`, `map_len`, `map_clear`
- IO: `read_file`, `read_lines`, `write_file`, `append_file`, `list_dir`, `create_dir`
- JSON: `to_json`, `from_json`
- Timers: `set_timeout`, `set_interval`, `clear_timer`
- Logging: `log_set_level`, `log_trace`, `log_info`, `log_warn`, `log_error`
- Iteration: `iter`, `next`, map/filter/fold/enumerate/take/zip/collect (prelude)
- Option/Result: `is_some`, `is_none`, `unwrap`, `unwrap_or`, `expect`, `is_ok`, `is_err`

### Embedding
- `VM` struct with full lifecycle: new, load, exec, call, disassemble
- `VMContext` for native function registration
- `ForeignTypeRegistry` for custom types
- `HotReloader` for file watching and live reload
- `register_native()` for Rust function callbacks
- `register_type()` for foreign struct definitions
- Examples: `basic_embedding`, `custom_natives`, `foreign_types`, `cross_call`, `hot_reload`

### Tooling
- `zenc run`, `zenc check`, `zenc test`, `zenc disasm`, `zenc repl`, `zenc build`, `zenc new`
- LSP server with diagnostics, hover, completion, goto-definition
- DAP debug adapter with breakpoints and stepping
- Markdown-based mdbook documentation
- Tree-sitter grammar for syntax highlighting
- Zed extension
- GitHub Actions for mdbook deployment
