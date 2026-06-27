Progress
### Done
- **Phases 0-5 complete**: Project scaffolding, lexer, parser, name resolver, type checker, bytecode compiler — all tests pass
- **Value/IR updates**: Opcode with And/Or, Value with full runtime types (Str, Array, Struct, Enum, Function, NativeFunction, Foreign), Chunk with read_u16_static
- **VM fixes**: JumpIfFalse pops condition; read_byte/read_u16 borrow issues fixed; functions referenced via constants not globals; calling convention fixed (bp = args_start, correct local slot allocation, arity propagated from params)
- **Parser fixes**: Optional `;` after for/while/loop/if/block; return_stmt uses `r#match`
- **Type checker fixes**: Uses `&mut SymbolTable`, properly enters/exits scopes; `let` bindings update inferred types (via `remove_from_current_scope` + `insert_into_current_scope` to handle shadowing); `Named("int"/"float"/"bool"/"str")` aliased to native types; for-loop variable type inferred from range start
- **SymbolTable additions**: `update_variable_type`, `remove_from_current_scope`, `insert_into_current_scope`
- **Compiler fixes**: Jump patch offset points to operand (not opcode); `chunk.locals` starts at 0; arity stored and propagated to `BytecodeFn`
- **Phase 6 (VM)**: **64/64 tests pass** — all VM tests, type checker tests, resolver tests, parser tests, lexer tests pass

### Next Steps
1. **Phase 7 — Rust Interop**: ForeignType registry, FieldAccessor, bind macro
2. **Phase 8 — Hot Reload**: File watcher, bytecode recompilation, state migration
3. **Phase 9 — Std Library**: print, assert, type_of, len, math
4. **Phase 10 — Tooling**: REPL, CLI, disassembler, stack traces
5. Phase 11 — LSP** builds on top of that:
- LSP server binary (`zenlang-lsp` or `zenlang lsp`)
- `textDocument/didChange` → re-lex/re-parse/resolve/typeck (no compile needed)
- `textDocument/hover` — show type info from TypeMap
- `textDocument/completion` — symbol table lookups
- `textDocument/definition` — source locations from AST spans
- `textDocument/diagnostic` — push errors from pipeline


## User

I'm interested in building my own basic programming language using Rust.  The idea is that it is specifically designed to be used as an embedded scripting language within a Rust game engine.  My goals are to make interop with Rust itself painless and some mechanism to ensure  I can hot-reload the scripting language without restarting my game/engine.  The syntax should be as close to Rust syntax as possible and there should be no sort of borrow checking or lifetime tracking.  Please develop a skeleton plan ffor this language divided into discrete modules that a LLM can follow to implement it

## Assistant

<thinking>
The user wants me to develop a skeleton plan for a programming language called "Zenlang" (based on the directory name) that is:
1. An embedded scripting language for a Rust game engine
2. Painless interop with Rust
3. Hot-reloadable without restarting the game/engine
4. Syntax close to Rust
5. No borrow checking or lifetime tracking

They want this divided into discrete modules that an LLM can follow to implement.

Let me first explore the existing codebase to understand what's already been built, if anything.
</thinking>

**Tool Call: read**
Status: Completed

.git/
.gitignore
Cargo.toml
src/

## Assistant

<thinking>
Let me explore the existing structure more deeply.
</thinking>

**Tool Call: Cargo.toml**
Status: Completed

[package]
name = "zenlang"
version = "0.1.0"
edition = "2024"

[dependencies]

**Tool Call: src**
Status: Completed

main.rs

## Assistant

<thinking>
The codebase is essentially empty - just a fresh Cargo project. So I need to develop a comprehensive plan from scratch.

Let me think carefully about the architecture of an embedded scripting language in Rust. The key requirements are:
1. Rust-like syntax (but simpler - no borrow checker, no lifetimes)
2. Embedded scripting language for a Rust game engine
3. Painless Rust interop
4. Hot-reload support (script changes without restarting the engine)
5. No borrow checking or lifetime tracking

Let me think about the major phases:

**Phase 0: Foundation**
- Project structure, modules, error types, source locations (Span)

**Phase 1: Lexer** - Tokenizer
- Input: source text
- Output: stream of tokens
- Handle identifiers, literals (int, float, string, bool), keywords, operators, delimiters

**Phase 2: Parser** - AST construction
- Input: token stream
- Output: Abstract Syntax Tree
- Functions, if/else, while, for, let bindings, expressions, blocks
- Syntax close to Rust: `fn`, `let mut`, `if`, `while`, `for`, `return`, `match`

**Phase 3: Name Resolution / Semantic Analysis**
- Build symbol tables
- Resolve names to declarations
- No borrow checking - just basic scope tracking

**Phase 4: Type System**
- Basic type checking (but simplified - no generics, no traits initially, no borrows)
- Types: i32, f64, bool, String, () (unit), structs, enums, arrays, function pointers
- Type inference where possible

**Phase 5: Intermediate Representation (IR) / Bytecode**
- Design a bytecode instruction set
- Compiler from AST to bytecode
- Stack-based or register-based VM

**Phase 6: Virtual Machine / Interpreter**
- Execute bytecode
- Runtime value representation
- Call stack, heap

**Phase 7: Rust Interop Layer**
- How Rust functions are registered and called from Zenlang
- How Zenlang functions are called from Rust
- Value conversion between Rust and Zenlang types

**Phase 8: Hot Reloading**
- Loading/reloading scripts at runtime
- State preservation across reloads
- Unloading old versions, loading new versions

**Phase 9: Standard Library**
- Built-in functions
- Basic I/O, math, string operations

**Phase 10: Tooling**
- REPL
- Error reporting
- Debug support

Let me think about what's realistic for an LLM to implement step by step. The plan should be modular, each module should be implementable independently with clear interfaces.

For the VM, I think a register-based VM or a stack-based VM would work. Stack-based is simpler to implement but register-based is more performant. For an embedded scripting language, I'd recommend starting with a stack-based VM (simpler) and optionally optimizing later.

For the bytecode format and VM, I'll design a simple but complete instruction set.

Let me think about the hot-reloading aspect more carefully:
- The engine loads a script, compiles it to bytecode, and runs it
- When the script file changes, the engine needs to recompile and reload
- The tricky part is preserving state (variables, objects) across reloads
- One approach: partition state into "script-owned" and "engine-owned" data. Engine-owned data persists. Script-owned data is re-initialized.
- Another approach: use a versioning mechanism where the new script can access the old script's global state

For a game engine scripting language, the typical pattern is:
1. Script defines certain callbacks (e.g., `fn on_update(dt: f64)`)
2. Engine calls these callbacks
3. The script's state is its global variables

So for hot reload:
- When a script is reloaded, we want to keep the globals that existed in the previous version
- Functions and types get updated to new versions
- Global variables that still exist in the new version keep their values
- New global variables get initialized

This is similar to how Lua handles hot reloading in games, or how GDScript works in Godot.

Let me structure the plan. I'll think about each module's responsibilities, its public interface, and how it connects to adjacent modules.

Actually, let me also think about what "Rust-like syntax" means concretely:
```
// Variables
let x = 42;
let mut y = 10;
y = y + 1;

// Functions
fn add(a: i32, b: i32) -> i32 {
    a + b
}

// If/else
if x > 0 {
    // ...
} else {
    // ...
}

// Loops
while condition {
    // ...
}

for i in 0..10 {
    // ...
}

loop {
    // ...
    break;
}

// Match
match value {
    1 => "one",
    2 => "two",
    _ => "other",
}

// Structs
struct Vec2 {
    x: f64,
    y: f64,
}

// Enums
enum Option {
    Some(i32),
    None,
}

// Methods
impl Vec2 {
    fn length(&self) -> f64 {
        // ...
    }
}

// Return
fn foo() -> i32 {
    return 42;
}
```

Key Rust features to DROP:
- Borrow checker, lifetimes
- Ownership semantics (everything is garbage collected or reference counted)
- Traits (initially)
- Generics (initially)
- Macros (initially)
- Pattern matching destructuring (initially)
- Modules/crates (single file scripts or simple multi-file)

Key Rust features to KEEP:
- Syntax: fn, let, mut, if/else, while, for, loop, match, struct, enum, impl
- Expression-oriented (last expression is return)
- Pattern matching basics
- Blocks as expressions

Let me now write the comprehensive plan.

I should be in Plan Mode - READ ONLY. So I'll just present the plan, no file modifications.

Let me also look at what's typical in the Rust game engine space for context. The most popular approach is using Lua or Rhai. Rhai is a good comparison point since it's already a Rust-native scripting language.

Let me provide a thorough, well-structured plan.
</thinking>

## Plan: Zenlang — An Embedded Rust-like Scripting Language

Based on your goals (Rust-like syntax, no borrows/lifetimes, painless Rust interop, hot-reload), here's a modular plan organized into discrete, buildable phases. Each phase lists its files, public interface, and what it depends on.

---

### Phase 0 — Project Scaffolding & Shared Types

**Files:** `src/lib.rs`, `src/error.rs`, `src/span.rs`

| Module | Contents |
|---|---|
| `lib.rs` | Top-level re-exports of all public API |
| `error.rs` | `ErrorKind` enum (ParseError, TypeError, RuntimeError, IoError, etc.), `Result<T>` alias |
| `span.rs` | `Span { start: usize, end: usize }`, `SourceLocation { file: Option<String>, span: Span }` — all AST/IR nodes carry a Span for error reporting |

**Interface:**
```rust
pub struct Span(pub usize, pub usize);
pub struct Spanned<T> { pub node: T, pub span: Span }
pub enum Error { Parse(Vec<ParseError>), Type(Vec<TypeError>), Runtime(RuntimeError), Io(std::io::Error) }
pub type Result<T> = std::result::Result<T, Error>;
```

**Dependencies:** none

---

### Phase 1 — Lexer (Tokenizer)

**Files:** `src/lexer.rs`, `src/token.rs`

**`token.rs`:** All token types:
```
TokenKind: Ident, Int(i64), Float(f64), Str(String), Bool(bool)
           Fn, Let, Mut, If, Else, While, For, Loop, Break, Continue, Return, Match, True, False
           Struct, Enum, Impl, Self_, Pub, Use, Mod, Const, Type
           Plus, Minus, Star, Slash, Percent, Eq, EqEq, Ne, Lt, Gt, Le, Ge
           AndAnd, OrOr, Bang, Dot, DotDot, DotDotEq, Comma, Semi, Colon, ColonColon
           Arrow, FatArrow, Hash, Underscore
           OpenParen, CloseParen, OpenBrace, CloseBrace, OpenBracket, CloseBracket
           Eof, Error(String)
```

**Lexer:** iterator over `(Token, Span)`:
- Single-pass, no backtracking
- Handles line/column tracking via `Span`
- String escapes, number literals (int/float), line comments `//`, block comments `/* */`
- Keywords take priority over identifiers

**Interface:**
```rust
pub struct Lexer<'a> { /* source: &'a str, position: usize */ }
impl<'a> Iterator for Lexer<'a> { type Item = Spanned<Token>; }
pub fn lex(source: &str) -> Vec<Spanned<Token>>;
```

**Dependencies:** Phase 0

---

### Phase 2 — Parser (AST Construction)

**Files:** `src/parser.rs`, `src/ast.rs`

**`ast.rs`:** Recursive AST types:
```rust
pub enum Expr {
    Literal(LiteralKind),        // Int, Float, Str, Bool, Unit
    Ident(String),
    Binary{ op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    Unary{ op: UnOp, expr: Box<Expr> },
    Assignment{ target: Box<Expr>, value: Box<Expr> },
    Call{ func: Box<Expr>, args: Vec<Expr> },
    MethodCall{ obj: Box<Expr>, method: Ident, args: Vec<Expr> },
    FieldAccess{ obj: Box<Expr>, field: Ident },
    Index{ obj: Box<Expr>, index: Box<Expr> },
    Block(Vec<Stmt>),
    If{ cond: Box<Expr>, then: Box<Expr>, else_: Option<Box<Expr>> },
    While{ cond: Box<Expr>, body: Box<Expr> },
    For{ var: Ident, iter: Box<Expr>, body: Box<Expr> },
    Loop{ body: Box<Expr> },
    Return(Option<Box<Expr>>),
    Break, Continue,
    Match{ expr: Box<Expr>, arms: Vec<MatchArm> },
    StructLiteral{ name: Ident, fields: Vec<(Ident, Expr)> },
    Lambda{ params: Vec<Param>, return_type: Option<Type>, body: Box<Expr> },
    Array(Vec<Expr>),
    Range{ start: Box<Expr>, end: Box<Expr>, inclusive: bool },
}

pub enum Stmt {
    Let{ mutable: bool, name: Ident, type_ann: Option<Type>, init: Option<Expr> },
    Expr(Expr),
    Return(Option<Expr>),
    Fn{ name: Ident, params: Vec<Param>, return_type: Option<Type>, body: Vec<Stmt> },
    Struct{ name: Ident, fields: Vec<StructField> },
    Enum{ name: Ident, variants: Vec<EnumVariant> },
    Impl{ type_name: Ident, methods: Vec<Fn> },
    Mod{ name: Ident, body: Vec<Stmt> },
}

pub enum Type { I32, F64, Bool, Str, Unit, Ident(String), Array(Box<Type>), Fn(Vec<Type>, Box<Type>), }
```

**Parser:** recursive descent, Pratt parsing for expressions:
- Precedence climbing for binary operators (following Rust precedence)
- Handles error recovery: parse statement boundaries on error, collect all errors
- Produces a `Vec<Stmt>` for the top-level program

**Interface:**
```rust
pub fn parse(tokens: &[Spanned<Token>]) -> Result<Program>;
pub struct Program { pub stmts: Vec<Spanned<Stmt>> }
```

**Dependencies:** Phase 0, Phase 1

---

### Phase 3 — Symbol Table & Name Resolution

**Files:** `src/symbol.rs`, `src/resolver.rs`

**`symbol.rs`:**
```rust
pub enum SymKind { Variable(Type), Function(FnSig), TypeDecl(TypeAlias), Struct(StructDef), Enum(EnumDef) }
pub struct Scope { /* parent: Option<Box<Scope>>, symbols: HashMap<String, SymKind> */ }
```

**`resolver.rs`:**
- Walks the AST, builds nested scopes
- Validates: no duplicate definitions in same scope, variables are declared before use, function names resolve
- Marks each `Ident` node in the AST with a resolved `SymId` (index into a global symbol table)
- Reports errors for unknown names
- No borrow checking — just resolves names

**Interface:**
```rust
pub struct SymbolTable { /* ... */ }
pub fn resolve(program: &mut Program) -> Result<SymbolTable>;
```

**Dependencies:** Phase 0, Phase 2

---

### Phase 4 — Type Checker

**Files:** `src/typeck.rs`

- Walts AST after resolution
- Infers types for literals and expressions
- Validates: function argument types, return types, assignment compatibility, operator type constraints
- Struct field access validation
- Enum variant construction validation
- Collects function signatures for IR generation
- Annotates each `Expr`/`Stmt` node with its resolved type (mutable AST or separate type map)

**Key simplification:** No generics, no traits, no type inference beyond Hindley-Milner basics (local inference). Everything is nominal.

**Interface:**
```rust
pub struct TypeMap { /* mapping from NodeId -> Type */ }
pub fn check(program: &mut Program, symbols: &SymbolTable) -> Result<TypeMap>;
```

**Dependencies:** Phase 0, Phase 2, Phase 3

---

### Phase 5 — Bytecode Compiler

**Files:** `src/ir.rs`, `src/compiler.rs`

**`ir.rs`:** Bytecode instruction set (stack-based VM):
```
Opcode:
  LoadConst(idx)     // push constant pool[idx]
  LoadVar(slot)      // push local variable
  StoreVar(slot)     // pop -> local variable
  LoadGlobal(slot)
  StoreGlobal(slot)
  LoadField(slot)    // pop obj, push obj.field
  StoreField(slot)
  LoadArrayElem
  StoreArrayElem
  Add, Sub, Mul, Div, Mod
  Neg, Not
  Eq, Ne, Lt, Le, Gt, Ge
  And, Or
  Jump(offset)
  JumpIfFalse(offset)
  Call(nargs)        // pop func + nargs args, push result
  CallMethod(nargs, method_idx)
  Return
  MakeStruct(nfields)  // pop nfields, push struct
  MakeArray(nelems)
  MakeEnum(variant_idx, nfields)
  NewClosure(func_idx, ncaptures)
  Dup, Pop, Swap
  Halt
```

**`compiler.rs`:**
- Recursively walks typed AST
- Allocates local variable slots and global slots
- Emits bytecode for each construct
- Constants deduplicated into constant pool
- Closure support with upvalue capture (list of captured variables)

**Interface:**
```rust
pub struct Chunk { pub code: Vec<u8>, pub constants: Vec<Value>, pub locals: u32, pub globals: Vec<String> }
pub struct BytecodeFn { pub chunk: Chunk, pub upvalues: Vec<UpvalueDesc>, pub name: String, pub arity: u32 }
pub fn compile(program: &Program, types: &TypeMap, symbols: &SymbolTable) -> Result<Vec<BytecodeFn>>;
```

**Dependencies:** Phase 0, Phase 2, Phase 3, Phase 4

---

### Phase 6 — Virtual Machine

**Files:** `src/vm.rs`, `src/value.rs`

**`value.rs`:** Runtime value representation:
```rust
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Gc<String>),       // garbage collected
    Array(Gc<Vec<Value>>),
    Struct(Gc<Vec<(String, Value)>>),
    Enum { tag: u32, fields: Gc<Vec<Value>> },
    Function(Gc<Closure>),    // bytecode function + upvalues
    NativeFunction(fn(&mut VM, &[Value]) -> Result<Value>),
    Foreign(RustValue),       // opaque Rust object handle
}
```

Use simple reference counting (Rc) with cycle detection, or a basic mark-sweep GC. For a game scripting language, RC is simpler and acceptable.

**`vm.rs`:** Stack-based interpreter:
- Main loop: fetch opcode, decode, execute
- Call stack: frames with `ip`, `slots[]`, `chunk`, `upvalues[]`
- Closure support: open upvalues on stack, close when going out of scope
- Native function calling: lookup `NativeFunction` in function table
- Error handling: trap runtime errors with stack trace

**Interface:**
```rust
pub struct VM {
    stack: Vec<Value>,
    frames: Vec<Frame>,
    globals: Vec<Value>,
    functions: Vec<BytecodeFn>,
    native_fns: HashMap<String, NativeFn>,
}
impl VM {
    pub fn new() -> Self;
    pub fn load_bytecode(&mut self, fns: Vec<BytecodeFn>);
    pub fn register_native<N>(&mut self, name: &str, func: N) where N: Fn(&mut VM, &[Value]) -> Result<Value> + 'static;
    pub fn call(&mut self, fn_name: &str, args: &[Value]) -> Result<Value>;
    pub fn run_main(&mut self) -> Result<Value>;  // calls the "main" function
}
```

**Dependencies:** Phase 0, Phase 5

---

### Phase 7 — Rust Interop Layer

**Files:** `src/interop.rs`, `src/ffi.rs`

This is the crux of making interop "painless." Design:

1. **Rust → Zenlang:** Engine calls `vm.call("on_update", &[Value::Float(dt)])` to invoke script functions.

2. **Zenlang → Rust:** Native functions registered via `vm.register_native("print", |vm, args| { ... })`. The `Foreign` value type holds an opaque `Box<dyn Any>` pointer with a registered type ID for type-safe downcasting.

3. **Automatic Binding Generator (macro):**
```rust
// In the engine's Rust code:
zenlang::bind! {
    struct Transform { x: f64, y: f64, rotation: f64 }
    impl Transform {
        fn new() -> Self;
        fn translate(&mut self, dx: f64, dy: f64);
    }
    // Or register free functions
    fn draw_sprite(texture: &str, x: f64, y: f64);
}
```
This macro generates:
- Type metadata registrations
- Native function wrappers
- Constructor/destructor for Foreign objects
- Field accessor generators

4. **Type Mapping:**
```
Rust i32    <-> Value::Int
Rust f64    <-> Value::Float
Rust bool   <-> Value::Bool
Rust String <-> Value::String
Rust struct -> Value::Foreign (with reflection for field access)
```

**Interface:**
```rust
pub fn register_type<T: 'static>(vm: &mut VM, type_name: &str, fields: &[&str]);
pub fn wrap_foreign<T: 'static>(value: T) -> Value;
pub fn unwrap_foreign<T: 'static>(value: &Value) -> Option<&T>;
// Macro-generated:
// register_type::<Transform>(vm, "Transform", &["x", "y", "rotation"]);
```

**Dependencies:** Phase 6, macros require `proc-macro` crate or `src/` sub-module

---

### Phase 8 — Hot Reloading

**Files:** `src/hotreload.rs`

**Design:**

1. **File watcher** (using `notify` crate): monitors script files for changes
2. **On change:**
   a. Re-lex, re-parse, re-compile the script
   b. Create new `VM` with new bytecode
   c. **State migration:** Read globals from old VM, write matching globals into new VM by name
   d. Swap the active VM reference in the engine

3. **State Preservation Rules:**
   - Global variables with the **same name** in old & new scripts → value is copied over
   - Global variables **removed** from new script → discarded
   - **New** global variables → initialized to their default expression
   - Functions are always replaced (cannot preserve function state meaningfully)
   - **Marking globals as `#[persist]`** annotation to explicitly control what survives a reload

4. **Engine Integration:**
```rust
pub struct HotReloader {
    paths: Vec<PathBuf>,
    vm: VM,
    watcher: notify::Watcher,
}
impl HotReloader {
    pub fn new(script_paths: &[PathBuf]) -> Self;
    pub fn tick(&mut self) -> Result<&mut VM>;  // returns current VM, reloads if changed
    pub fn force_reload(&mut self) -> Result<&mut VM>;
}
```

**Dependencies:** Phase 7, external crate `notify` for file watching

---

### Phase 9 — Standard Library

**Files:** `src/stdlib/` (sub-module)

Built-in functions registered on VM init:
```
print(msg)      // console output
assert(cond)
to_string(val)  // string conversion
type_of(val)    // runtime type name
int(val)        // cast to int
float(val)      // cast to float
len(arr)        // array length
push(arr, val)  // array push
remove(arr, i)  // array remove
range(start, end)  // range iterator
clock()         // time in seconds (for game dt measurement)
```

---

### Phase 10 — Tooling & Polish

**Files:** `src/repl.rs`, `src/debug.rs`

- **REPL:** read-eval-print loop using rustyline
- **Debug info:** emit `DebugChunk` alongside bytecode mapping bytecode offsets to source Spans → stack traces with line numbers
- **Disassembler:** `disassemble(&Chunk)` for debugging bytecode output
- **Formatter** (stretch): basic pretty-printer for Zenlang source

---

### Dependency Graph & Suggested Implementation Order

```
Phase 0 (Shared Types)
  └─► Phase 1 (Lexer)
       └─► Phase 2 (Parser)
            └─► Phase 3 (Resolver)
                 └─► Phase 4 (Type Checker)
                      └─► Phase 5 (Bytecode Compiler)
                           └─► Phase 6 (VM)
                                ├─► Phase 7 (Rust Interop)
                                │    └─► Phase 8 (Hot Reload)
                                └─► Phase 9 (Std Lib)
                                     └─► Phase 10 (Tooling)
```

Each phase produces a testable artifact:
- Phase 1: `cargo test lexer` — verify all tokens parse correctly
- Phase 2: `cargo test parser` — verify AST structure for sample programs
- Phase 3–4: `cargo test resolver` / `typeck`
- Phase 5–6: `cargo test vm` — execute small programs end-to-end
- Phase 7–8: integration tests running Zenlang scripts from Rust

---

### Key Design Decisions Summary

| Decision | Choice | Rationale |
|---|---|---|
| Execution model | Stack-based bytecode VM | Simpler to implement than JIT; fast enough for game scripting |
| Memory management | Reference counting + `Foreign` opaque handles | Game scripting GC pauses are unacceptable; RC + engine-owned objects avoids this |
| Type system | Nominal, no generics/traits initially | Keep complexity low; games rarely need generic scripting |
| Closure handling | Upvalue capture with open/close | Standard lua-style closures |
| Hot reload | State migration by global name match | Pragmatic; matches how game devs expect it to work |
| Error recovery | Parse errors don't abort; collect all | Better UX during development |
| Script entry point | `fn main()` or registered callbacks like `on_update(dt: f64)` | Flexible for game engine integration |

---

### Questions for You Before Implementation

1. **GC vs RC:** Do you prefer simple `Rc` with potential for cycles (acceptable for game scripting where cycles are rare), or should I plan for a full mark-sweep GC from the start?

2. **Closures:** Essential for a game scripting language (callbacks, event handlers), but they add complexity to both the compiler and VM. Include from the start or defer to post-MVP?

3. **Pattern matching extent:** Full Rust-style `match` with destructuring and guards, or simplified `match` (only literal/ident patterns)?

4. **First-class script type for interop:** Would you like the `bind!` macro to generate reflection metadata (field names, type info) automatically from Rust struct definitions, or is manual registration acceptable for v1?

5. **Struct mutability from Rust:** When a Rust struct is exposed to Zenlang as `Foreign`, should script code be able to mutate its fields directly (`obj.field = val`), or only through method calls?

## User

1. I do NOT want a GC, reference counting only please.
2. Lets save closures for post-MVP, but DO build into the architecture the necessary infrastructure to support them, as we will certainly have to add them after everything else.
3. Lets start with only the simplified
4. manual registration is totally fine for now
5. It would be nice if mutating fields directly worked, but is there a reason to use method calls instead?

I'd also like to use common rust utility crates for their purposes, such as 'snafu' /  'derive_more' / logging / tracing / tracing_subscriber / once_cell / serde / pretty_assertions / tempfile / walkdir / itertools / strum / bytes / compact_str(being able to put small strings on the stack seems particularly useful in parsing) / smallvec / rayon / camino (for paths) / clap / nanoid.  You don't have to use all of them, only the useful ones.

## Assistant

<thinking>
The user has responded to my questions and given me clear direction. Let me address each point and then refine the plan.

1. **GC vs RC:** RC only, no GC. Good, that simplifies value.rs and the VM.
2. **Closures:** Post-MVP, but build infrastructure for them. That means: in the compiler, leave room for upvalue tracking; in the VM, have the concept of closures and upvalues but don't fully implement them yet.
3. **Pattern matching:** Simplified match (literal/ident patterns only).
4. **Type binding:** Manual registration is fine.
5. **Struct mutability from Foreign objects:** They want direct field mutation to work. The concern would be: if a Foreign value wraps a Rust struct behind an Rc<RefCell<T>> or Arc<Mutex<T>>, then field mutation needs to go through the borrow checker. For "painless" interop, we'd need to use interior mutability. But actually, in a scripting context, the simplest approach is to wrap foreign objects in `Rc<RefCell<dyn Any>>` and provide field access through registered getters/setters that the compiler can generate at bind time. That way `obj.field = val` in Zenlang becomes a generated setter call internally.

Actually, the user asked "is there a reason to use method calls instead?" for mutating fields. The reason would be: directly mutating foreign fields requires runtime reflection or generated getter/setter pairs, while method calls are simpler. But since they want direct mutation, we can generate field getter/setter native functions during `register_type` and have the compiler emit `LoadField`/`StoreField` ops that call them. This is feasible and worth doing.

Let me also think about the crate choices:

**Utilities to use:**
- `snafu` - error handling (replaces manual error types)
- `derive_more` - derive macros for Display, From, etc.
- `tracing` + `tracing-subscriber` - logging throughout
- `once_cell` - lazy statics (though std::sync::LazyLock is stable now, but once_cell is fine)
- `serde` - serialization (for state persistence during hot reload)
- `pretty_assertions` - dev dependency, for test diffs
- `tempfile` - dev dependency
- `walkdir` - for finding script files
- `itertools` - general collection utilities
- `strum` - enum iteration/display
- `bytes` - for bytecode chunk manipulation
- `compact_str` - small string optimization, great for lexer/parser tokens
- `smallvec` - stack-allocated small vectors, useful in parser AST
- `rayon` - parallelism (maybe for parallel compilation of multiple scripts)
- `camino` - UTF-8 paths (good practice)
- `clap` - CLI for the REPL/compiler tool
- `nanoid` - unique IDs (maybe for hot-reload version tracking)

**Not needed or questionable:**
- `serde` could be useful for serializing the bytecode, but it's not critical
- `rayon` might be over-engineering for v1
- `camino` is nice but adds a dependency - might be worth it

Let me update the plan to incorporate these decisions and crate recommendations.

The user is in plan mode - I should present the refined plan and not implement anything.

Let me refine the plan now, addressing their 5 points and integrating the crate recommendations.

Actually, I should be careful. The system reminder says I'm in plan mode - READ ONLY. I can discuss and plan but not make any changes. So I'll present the refined plan as discussion.
</thinking>

Good answers. Let me refine the plan with those decisions and integrate the crate choices.

**Re: #5 (Foreign field mutation):** The reason to prefer methods is that it avoids needing interior mutability (`RefCell`) for every foreign object. But we can make direct mutation work by having the `register_type` macro generate getter/setter native functions internally, and the compiler's `LoadField`/`StoreField` ops call those. It's feasible — just generates a bit more boilerplate in the bind layer. We'll do it.

---

### Refined Crate Choices

| Crate | Where Used | Why |
|---|---|---|
| `snafu` | `error.rs` | Replace manual Error enum with Snafu's `derive(Snafu)` for context-rich errors |
| `derive_more` | `ast.rs`, `token.rs`, `value.rs`, `ir.rs` | Derive `Display`, `From`, `Add`, etc. on token types, opcodes, values |
| `tracing` + `tracing-subscriber` | `lib.rs`, `vm.rs`, `hotreload.rs` | Structured logging throughout pipeline (trace spans per compile pass, VM execution) |
| `once_cell` | `compiler.rs` (maybe) | Lazy static type registry; though `std::sync::LazyLock` may suffice in edition 2024 |
| `compact_str` | `lexer.rs`, `token.rs` | Store identifier/string tokens inline (no heap alloc for short strings — most of them) |
| `smallvec` | `parser.rs`, `compiler.rs` | Function parameters, call arguments, struct fields — almost all <8 elements |
| `bytes` | `ir.rs`, `vm.rs` | `BytesMut` for assembling bytecode chunks; zero-copy slicing during execution |
| `itertools` | `compiler.rs`, `stdlib/` | `unique()`, `intersperse()`, etc. for codegen patterns |
| `strum` | `token.rs`, `ir.rs` | Iterate enum variants (keyword matching, opcode display) |
| `camino` | `hotreload.rs`, `cli.rs` | Utf8Path for script paths |
| `clap` | `bin/` entry point | CLI for REPL/compiler driver |
| `nanoid` | `hotreload.rs` | Unique reload epochs to invalidate stale references |
| `pretty_assertions` | dev-dep | Test diffs |
| `tempfile` | dev-dep | Integration tests with temp directories |
| `walkdir` | `hotreload.rs` | Recursive script discovery |

**Omitted:** `serde` (not needed yet — no serialization of bytecode planned), `rayon` (overkill for v1).

---

### Updated Module Map

```
src/
├── lib.rs              # re-exports, tracing init, public API facade
├── error.rs            # snafu-based error types
├── span.rs             # Span, Spanned<T>, SourceLocation
├── token.rs            # Token, TokenKind (with strum, compact_str)
├── lexer.rs            # Lexer (uses compact_str for token text)
├── ast.rs              # Expr, Stmt, Type, Program (with smallvec for fields/args)
├── parser.rs           # Recursive descent parser (uses smallvec)
├── symbol.rs           # SymKind, Scope, SymbolTable
├── resolver.rs         # Name resolution pass
├── typeck.rs           # Type checker
├── ir.rs               # Bytecode opcodes, Chunk, BytecodeFn (uses bytes::BytesMut)
├── compiler.rs         # AST -> bytecode compiler
├── value.rs            # Value enum, Upvalue, Closure stub (RC only, no GC)
├── vm.rs               # VM (call stack, frame, main loop)
├── interop.rs          # Foreign type registry, bind macro, field accessors
├── hotreload.rs        # File watcher, state migration, VM swap
├── stdlib/
│   └── mod.rs          # register_builtins(&mut VM)
├── repl.rs             # Read-eval-print loop
└── bin/
    └── zenlang.rs      # clap-based CLI
```

---

### Updated Phase Details

**Phase 0 — Scaffolding + Crates**
`Cargo.toml` with all dependencies. `lib.rs` as top-level facade. `error.rs` with snafu types:

```rust
use snafu::prelude::*;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Parse error at {location}: {msg}"))]
    Parse { location: SourceLocation, msg: String },
    // ... etc
}
```

`span.rs` with `Spanned<T>` that all AST/IR nodes will use.

**Phase 1 — Lexer**
Uses `compact_str::CompactString` for token text. Uses `strum::EnumIter` on `TokenKind` to drive keyword matching (build a `HashMap<&str, TokenKind>` from the enum). `Lexer` returns `Spanned<Token>` via an iterator interface.

**Phase 2 — Parser**
Uses `smallvec::SmallVec` for function parameters, call arguments, struct fields (default capacity 4 or 8). Single `Program { stmts: Vec<Spanned<Stmt>> }` top-level node. Error recovery: on parse error, skip to next statement boundary, record error, continue.

**Phase 3 — Resolver**
Walk AST, populate `SymbolTable` with scopes. Validate no duplicate names, variables declared before use. Leaves `SymId` back-references for later phases.

**Phase 4 — Type Checker**
Simple nominal type checking. Builds a `TypeMap: HashMap<NodeId, Type>`. Validates operator type constraints, function call arity, struct field access. No generics, no traits, no HK inference.

**Phase 5 — Compiler**
Produces `Vec<BytecodeFn>`. Uses `bytes::BytesMut` for code emission. Constant pool with dedup. **Closure infrastructure design only:** the `BytecodeFn` struct includes `upvalues: Vec<UpvalueDesc>` and the `Opcode::NewClosure` op is defined but may not be emitted yet in MVP — we ensure the opcode dispatch slot exists in the VM's main loop.

**Phase 6 — VM**
Stack-based. `Value` enum internally uses `Rc` for strings, arrays, structs. Foreign objects use `Rc<RefCell<dyn Any>>`. `CallMethod` dispatches through a method table per-type. No GC at all — cycles will leak, document this limitation.

**Phase 7 — Interop**
Manual registration API:
```rust
pub struct ForeignType {
    name: &'static str,
    fields: Vec<(&'static str, FieldAccessor)>,
    methods: Vec<(&'static str, NativeFn)>,
}

impl VM {
    pub fn register_type(&mut self, ty: ForeignType);
}
```

`FieldAccessor` is an enum `{ Get(NativeFn), Set(NativeFn) }` generated from closures at bind time.
`obj.field = val` in Zenlang → compiler emits `StoreField` → VM looks up setter in type's field table → calls native setter function.

**Phase 8 — Hot Reload**
File watcher using `notify`. On change:
1. Re-compile script
2. Snapshot globals from old VM
3. Create new VM, compile fresh
4. For each global in new VM, if name exists in old VM snapshot, migrate value
5. Atomically swap VM reference

`camino::Utf8Path` for path handling. `nanoid` for epoch IDs to tag stale function references.

**Phase 9 — Std Lib**
Register built-in functions via `vm.register_native(...)`. Covers `print`, `assert`, `type_of`, `int`, `float`, `len`, `push`, `remove`, `range`, `clock`.

**Phase 10 — Tooling**
REPL with `rustyline`. Disassembler for `Chunk` with `Display` derive via `derive_more`. Stack traces with source locations.

---

### Simplified Match Design (Phase 2 + Phase 5)

```
match expr {
    pattern => body,
    pattern => body,
    ...
}
```

Patterns limited to:
- Literal: `1`, `"hello"`, `true`
- Identifier (binds value): `x`
- Wildcard: `_`

Compiler emits: evaluate `expr`, then for each arm in order: compare with literal pattern (using `Eq`), or bind to identifier/wildcard (always matches). First match wins. No destructuring, no guards.

---

Does this refined plan look good to you? If so, I'll move to implementation mode and start with Phase 0 (project setup, Cargo.toml with all dependencies, error types, span types).
