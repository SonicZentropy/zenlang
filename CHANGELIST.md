# Changelist

## Phase 13 — VM dispatch for struct method calls

- `Value::Struct(Rc<..>)` → `Value::Struct(Rc<..>, String)` — stores struct type name alongside the field map for runtime method lookup.
- `MakeStruct(u16)` → `MakeStruct(u16, u16)` — first operand is the constant-pool index of the struct type name, second is field count.
- Compiler emits the type name constant when compiling struct literals.
- VM `MakeStruct` handler reads the type name from constants and attaches it to the struct value.
- Added `function_name_map: HashMap<String, usize>` to VM, populated in `load_bytecode`, mapping qualified names like `"Point::area"` to function indices.
- `CallMethod` handler now dispatches `Value::Struct` — constructs `"TypeName::method_name"`, looks it up in `function_name_map`, sets frame `bp = args_start - 1` so the receiver becomes local 0 (`self`).
- Updated all `Value::Struct` pattern matches across codebase to destructure the new tuple variant.
- `methods.zen` test now calls `p.area()` at runtime.

## Phase 12 — Type-check method calls and fields

- `Expr::Field { obj, field }` in type checker: resolves `obj`'s struct type, looks up field in `StructDef.fields`, returns its declared type (instead of `Type::Unit`).
- `Expr::MethodCall { obj, method, args }` in type checker: resolves struct type, finds method via qualified name `"Type::method"`, validates arg count and types against signature (skipping `self` param), returns method's return type.
- Fixed `self` param type in resolver and type checker for `impl` methods — `self` now resolves to `Type::Named(struct_name)` instead of `Type::Unit`.
- Both resolver and type checker now enter a scope per-method inside `Stmt::Impl`, setting `self`'s type from the enclosing struct.
- All 116 unit tests + 13 integration tests pass.

## Phase 11 — Fix `impl` block compilation

- `compile_functions` recurses into `Stmt::Impl` — each method is compiled as a standalone `BytecodeFn` with qualified name `"TypeName::method_name"`.
- `register_function_names` registers qualified method names (e.g., `"Point::area"` maps to function index).
- `register_global_stmt` registers qualified names as globals so methods are available at runtime.
- Parser: `TokenKind::Self_` prefix emits `Expr::Ident("self")` enabling `self.x` in method bodies.
- `zenc disasm tests/methods.zen` confirms `Point::area` appears as a separate `BytecodeFn`.

## Phase 10 — Named field shorthand in struct literals

- `Point { x, y }` desugars to `Point { x: x, y: y }` at parse time.
- Parser checks for `,` or `}` after field identifier to detect shorthand vs explicit `field: expr`.
- `is_struct_lit_start` updated to return true for `Foo { x }`.
- `tests/shorthand_test.zen` validates the feature.

## Phase 9 — `..` spread operator in struct literals

- `Point { x: 10, ..base }` creates a struct copying all fields from `base`, then overriding `x`.
- Compiles spread expression first, then emits `StoreField` for each explicit field.
- Added `spread: Option<Box<Expr>>` field to `Expr::StructLit`.
- Parser recognizes `..expr` inside struct literal braces.
- `tests/spread_test.zen` validates basic spread, all-fields-explicit + spread, copy-only, chained spread.

## Phase 8 — `if let` / `while let` syntax

- `if let pat = expr { then } else { else_ }` desugars to `match expr { pat => then, _ => else_ }` at parse time.
- `while let pat = expr { body }` desugars to `loop { match expr { pat => body, _ => break } }` at parse time.
- No changes needed in type checker or compiler — the AST already has `match`/`loop`.
- `else if let` chaining uses `check()` (not `r#match()`) for the `if` token so `if_stmt` can consume it.
- `tests/if_let.zen` validates `Some`/`None`/`Ok`/`Err` patterns, with/without else, `else if let` chains.

## Phase 7 — Closure support

- Lambdas `|x, y| x + y` compile as separate `BytecodeFn` entries with upvalue capture.
- Captured variables captured **by value** (Rc clone) at closure creation time.
- `NewClosure` opcode pops upvalues from stack and builds `ClosureData`.
- Closure calls push upvalues then args, set up frame.
- `tests/closures.zen` validates basic lambda, upvalue capture, multiple captures.

## Phase 6 — `disasm` display fix

- Disassembly reads u16 operands from byte stream directly instead of using `Opcode::from_byte()` placeholder values (which are always 0).

## Phase 5 — Exhaustive match checking

- Type checker reports error if `match` on an enum does not cover all variants and has no wildcard arm.
- Handles zero-field variants used as `Pattern::Ident` (e.g., `None`) and `Pattern::Wildcard`.

## Phase 4 — Native function return type accuracy

- Each native function declared with an accurate `FnSignature` (param types + return type) instead of hardcoded `() -> I64`.
- Type checker validates argument types against declared parameter types; returns the declared return type.

## Phase 3 — Comprehensive `.zen` tests

- Added `tests/basic.zen`, `tests/control_flow.zen`, `tests/functions.zen`, `tests/closures.zen`, `tests/data_types.zen`, `tests/enum_variants.zen`, `tests/option_result.zen`, `tests/modules.zen`, `tests/stdlib.zen`, `tests/fail_ret_type.zen`, `tests/fail_undefined.zen`.

## Phase 2 — Generic types + Option/Result

- 2a: `Type::Option(Box<Type>)` and `Type::Result(Box<Type>, Box<Type>)` with parser, type-compatibility, and formatting support.
- 2b: Auto-register `Option`/`Result` enums at resolver startup; native helpers (`is_some`, `is_none`, `is_ok`, `is_err`, `unwrap`, `unwrap_or`, `expect`, `map`, `and_then`).

## Phase 1 — General enum variant construction & pattern matching

- 1a: Enum variant construction via function-call syntax `Some(42)` — parser reuses `Expr::Call`, resolver registers `EnumConstructor`, compiler emits `MakeEnum(tag, field_count)`.
- 1b: Pattern matching with `LoadEnumTag`/`LoadEnumField` opcodes, `Pattern::EnumVariant` AST node, match-arm coverage validation.

## Phase 0 — Test runner infrastructure

- `assert(condition)` native function.
- `zenc test` subcommand — discovers `.zen` files under `tests/`, runs each through full pipeline, reports `PASS`/`FAIL`.
