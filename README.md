# Zenlang

A lightweight, embeddable Rust-like scripting language designed for game engines and
real-time applications. No borrow checker, no GC — just `Rc`-based reference counting
and a tight bytecode VM.

## Features

- **Rust-like syntax**: `let`, `fn`, `if`/`else`, `while`, `for`, `match`, `struct`, `enum`, `impl`.
- **Expression-oriented**: blocks (`{ ... }`) return values; `if`/`match` are expressions.
- **Bytecode VM**: register-based stack VM with ~50 opcodes. No IR, single-pass codegen.
- **Rust interop**: register foreign types with fields and methods, call Rust from scripts
  and scripts from Rust.
- **Hot reload**: watch source files for changes, recompile, and reload while preserving
  global state.
- **REPL**: interactive prompt with multi-line input detection.
- **LSP server**: `zenlang lsp` — diagnostics, completions, hover, document symbols,
  semantic tokens.
- **Disassembler**: `zenlang disasm <file>` — dump bytecode with opcodes, lines, constants.
- **No GC**: deterministic `Rc`-based ownership; no stop-the-world pauses.
- **No async**: synchronous single-threaded design; trivially embeddable.

## CLI Usage

```text
zenlang [FILE] [COMMAND]

Commands:
  run     Run a script file (with hot reload)
  repl    Start an interactive REPL
  disasm  Disassemble a compiled script
  check   Type-check only (no execution)
  lsp     Start the LSP language server (stdin/stdout)

Arguments:
  [FILE]  Path to a script file to run (equivalent to `zenlang run <file>`)
```

### Examples

**Run a script:**

```console
$ zenlang hello.zen
```

**REPL:**

```console
$ zenlang repl
> fn greet(name) { print("hello " + name); }
... greet("world");
=> nil
> _
```

**Type-check only (no execution):**

```console
$ zenlang check my_script.zen
type check passed
```

**Disassemble bytecode:**

```console
$ zenlang disasm my_script.zen
=== main ===
-- Constants --
[0] "hello"
-- Bytecode --
0000:    1  OpConstant        0
0002:    1  OpPrint
0003:    1  OpNil
0004:    1  OpReturn
```

**LSP server (editor integration):**

Start on stdin/stdout — compatible with Neovim's built-in LSP, VS Code, etc.

```console
$ zenlang lsp
```

> The LSP provides text-sync diagnostics, completions, hover type info, document
> symbols, and semantic token coloring.

## Language Tour

### Bindings

```rust
let x = 42;
let mut y = 10;
y = y + 1;
```

### Functions

```rust
fn add(a, b) -> a + b
fn greet(name: str) {
    print("Hello, " + name + "!");
}
```

Closures capture by reference:

```rust
fn make_counter(start) {
    let count = start;
    fn inc() -> count = count + 1;  // assignment returns the value
    inc
}
let c = make_counter(0);
print(c()); // 1
```

### Control Flow

```rust
let x = if cond { 1 } else { 2 };

while i < 10 {
    i = i + 1;
}

for i in 0..5 {
    print(i);
}
```

### Match

```rust
let val = match x {
    1 => "one",
    2 => "two",
    _ => "other",
};
```

### Structs, Enums, Impl

```rust
struct Point { x, y }

let p = Point { x: 1, y: 2 };
p.x = 10;

enum Option { None, Some(val) }

impl Point {
    fn magnitude(self) -> (self.x * self.x + self.y * self.y).sqrt()
}
```

### Arrays & Strings

```rust
let arr = [1, 2, 3];
arr.push(4);
print(arr.len());     // 4
print(arr.contains(2)); // true

let s = "hello";
print(s.len());       // 5
print(s.to_upper());  // "HELLO"
print(s.substring(0, 2)); // "he"
```

## Architecture

### Pipeline

```
Source ──► Lexer ──► Parser ──► Resolver ──► Type Checker ──► Compiler ──► VM
                                    │                            │
                                    ▼                            ▼
                            SymbolTable                  BytecodeFn[]
```

| Phase | Module | Output |
|-------|--------|--------|
| Lexer | `lexer.rs` | `Vec<Spanned<Token>>` |
| Parser | `parser.rs` | `Program` (AST) |
| Resolver | `resolver.rs` | `SymbolTable` (scoped name resolution) |
| Type Checker | `typeck.rs` | `TypeMap` (expression → type) |
| Compiler | `compiler.rs` | `(Vec<BytecodeFn>, Vec<String>)` — bytecode + global names |
| VM | `vm.rs` | Executes bytecode, returns `Value` |

### Modules

| Module | Responsibility |
|--------|---------------|
| `lexer.rs` | Tokenizer — produces `Spanned<Token>` with source positions |
| `parser.rs` | Recursive-descent parser — expressions, statements, declarations |
| `ast.rs` | AST node types (`Expr`, `Stmt`, `Program`, `Type`, `Param`, etc.) |
| `span.rs` | `Span(usize, usize)`, `SourceLocation`, `Spanned<T>` wrapper |
| `symbol.rs` | `SymbolTable` — scoped variable/function/type name resolution |
| `resolver.rs` | Name resolution pass — populates `SymbolTable`, detects shadowing |
| `typeck.rs` | Type checker — infers types, validates assignments/calls |
| `ir.rs` | `Chunk` / `BytecodeFn` — bytecode format, emit/read, disassembly |
| `compiler.rs` | Single-pass bytecode compiler — emits `Chunk` per function |
| `value.rs` | `Value` enum — all runtime values (int, float, bool, string, array, fn, foreign, nil) |
| `vm.rs` | Stack-based VM — executes `BytecodeFn[]`, manages call frames |
| `interop.rs` | `ForeignTypeRegistry`, `ForeignObject`, `FieldAccessor` — Rust type binding |
| `hotreload.rs` | `HotReloader` — mtime-based file watching, global snapshot/restore |
| `stdlib/mod.rs` | Built-in functions (`print`, `assert_eq`, `type_of`, `len`, math, string ops) |
| `lsp.rs` | LSP server — text sync, diagnostics, completions, hover, symbols, semantic tokens |
| `error.rs` | `Error` enum — typed errors for each phase with `Snafu` |
| `token.rs` | `TokenKind` enum — all token types with `CompactString` lexemes |
| `span.rs` | Position tracking types |

### Bytecode VM

The VM is a register-based stack machine with ~50 opcodes:

- **Stack**: local variables, temporaries, call arguments.
- **Globals**: indexed by name, stored in a `Vec<Value>` parallel to `global_names`.
- **Call frames**: `{ function_idx, ip, bp }` — stack-allocated frame list.
- **Closures**: `Value::Function(Rc<BytecodeFn>)` — recursive function refs handled
  via index remapping in `reload_functions()`.
- **Foreign interop**: `Value::Foreign(Rc<RefCell<ForeignObject>>)` with name-based
  field/method dispatch through string tables in the bytecode `Chunk`.

Key opcodes: `OpConstant`, `OpAdd`/`OpSub`/`OpMul`/`OpDiv`, `OpNegate`, `OpNot`,
`OpEq`/`OpNe`/`OpLt`/`OpGt`/`OpLe`/`OpGe`, `OpJump`/`OpJumpIfFalse`,
`OpSetGlobal`/`OpGetGlobal`, `OpSetLocal`/`OpGetLocal`, `OpCall`/`OpReturn`,
`OpMakeArray`/`OpIndex`/`OpSetIndex`, `OpPush`/`OpPop`, `OpContains`,
`OpGetField`/`OpSetField`/`OpCallMethod`.

### Error Handling

Compile-time errors carry `SourceLocation { file, span, line, column }` and are
returned as `Result` values (no panics). Runtime errors include a `Vec<SourceLocation>`
stack trace built from the bytecode line table.

### String Interning

Keywords and identifiers use `CompactString` (small-string optimisation). The lexer
produces owned tokens; no interning table is needed at this scale.

## Embedding

Add Zenlang to your `Cargo.toml`:

```toml
[dependencies]
zenlang = { git = "..." }
```

Basic usage:

```rust
use zenlang::{VM, Error};
use zenlang::compiler::compile;
use zenlang::stdlib::{native_names, register_builtins};

let source = "fn main() { print(\"hello\"); }";

// Full pipeline
let tokens = zenlang::lexer::Lexer::new(source).tokenize()?;
let mut program = zenlang::parser::Parser::new(&tokens).parse()?;
let names = native_names();
let mut symbols = zenlang::resolver::resolve_with_natives(&mut program, &names)?;
let types = zenlang::typeck::check(&program, &mut symbols)?;
let (fns, global_names) = compile(&program, &types, &symbols, &names, source)?;

let mut vm = VM::new();
register_builtins(&mut vm);
vm.load_bytecode(fns, global_names);
let result = vm.run_main()?;
println!("{:?}", result);
```

### Registering Foreign Types

```rust
use zenlang::interop::ForeignTypeDef;
use zenlang::value::Value;

vm.register_type::<MyType>("MyType")
    .field("x", |obj| Ok(Value::Int(obj.x as i64)), |obj, val| { obj.x = val.as_int()?; Ok(()) })
    .method("do_stuff", |ctx, args| { /* ... */ Ok(Value::Nil) });
```

## Examples

All examples are in the [`examples/`](./examples/) directory.

### Simple embedding examples (run with `cargo run --example <name>`)

| Example | File | What it shows |
|---------|------|---------------|
| **basic_embedding** | [`basic_embedding.rs`](./examples/basic_embedding.rs) | Full compile-and-run pipeline, reading script return values |
| **custom_natives** | [`custom_natives.rs`](./examples/custom_natives.rs) | Registering Rust functions (`double`, `add3`, stateful `tick` with `Rc<Cell>`) callable from scripts |
| **foreign_types** | [`foreign_types.rs`](./examples/foreign_types.rs) | Exposing a Rust `Player` struct with fields (`name`, `health`) and methods (`heal_percent`) to scripts |
| **cross_call** | [`cross_call.rs`](./examples/cross_call.rs) | Script calling Rust natives (`compute_stats`, `damage_formula`) and receiving structured return values |
| **hot_reload** | [`hot_reload.rs`](./examples/hot_reload.rs) | HotReloader with tempfile, mtime-based recompilation, and global state preservation |

```console
# Run any of the inline examples:
cargo run --example basic_embedding
cargo run --example custom_natives
cargo run --example foreign_types
cargo run --example cross_call
cargo run --example hot_reload
```

### Engine integration examples (standalone crates)

These are full crate directories with their own `Cargo.toml`, showing how to embed
Zenlang in real game engines.

| Example | Directory | Engine | What it shows |
|---------|-----------|--------|---------------|
| **bevy_integration** | [`bevy_integration/`](./examples/bevy_integration/) | Bevy 0.19 | Registering a `ScriptPlayer` foreign type, per-frame script execution via Bevy systems, `RefCell<VM>` as a resource |
| **fyrox_integration** | [`fyrox_integration/`](./examples/fyrox_integration/) | Fyrox 1.0 | Registering a `ScriptedEntity` foreign type, scripting plugin with per-frame updates |

```console
# Run engine examples from their own directories:
cd examples/bevy_integration && cargo run
cd examples/fyrox_integration && cargo run
```

## Running Tests

```console
$ cargo test
```

93 tests covering lexer, parser, resolver, type checker, compiler, VM, interop,
hot reload, and stdlib.
