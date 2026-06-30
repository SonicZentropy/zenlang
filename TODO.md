# TODO

## AI Instruction: Please commit between each Phase completed

## Phase 0 — Test runner infrastructure (COMPLETE)

Files: `src/main.rs`, `src/stdlib/mod.rs`, `tests/`

- **`src/stdlib/mod.rs`**: Add `assert(condition)` native function (panics if false). Add `assert_eq` already exists.
- **`src/main.rs`**: Add `Test` subcommand — discovers `.zen` files under `tests/`, runs each through the full pipeline, reports `PASS`/`FAIL`. Exit code = number of failures.

**Dependencies**: None — can go first.

---

## Phase 1 — General enum variant construction & pattern matching

This provides the general mechanism that Option/Result will use.

### 1a — Construction via function-call syntax

When you write `Some(42)`, it parses as a regular `Expr::Call { func: Ident("Some"), args: [42] }`. The resolver registers each enum variant as a special `EnumConstructor` name, so `Some` resolves to an enum constructor, not a function. The compiler sees it's a constructor and emits `MakeEnum(tag, count)` instead of a `Call` instruction.

| File | Change |
|---|---|
| `src/symbol.rs` | Add `SymKind::EnumConstructor { enum_name, variant_name, tag, fields }` |
| `src/resolver.rs` | In `register_top_level`, when registering an enum, also register each variant as `EnumConstructor` in the current scope |
| `src/typeck.rs` | In `check_expr` for `Expr::Call`: if callee resolves to `EnumConstructor`, validate args against field types, return `Type::Named(enum_name)`; if not a constructor, fall through to normal call-checking |
| `src/compiler.rs` | In `compile_expr` for `Expr::Call`: if callee resolves to an enum constructor (look up in symbol table), compile args and emit `MakeEnum(tag, args.len())`; also register constructor names as globals in `register_global_stmt` |

**No parser changes** — `Some(x)` reuses existing call syntax.

### 1b — Destructuring in match patterns

When you write `match x { Some(v) => ..., None => ... }`, the parser recognizes `Some(v)` as a new pattern form. The compiler emits `LoadEnumTag` + `Eq` + `JumpIfFalse` to discriminate arms, and `LoadEnumField(n)` to extract bindings.

| File | Change |
|---|---|
| `src/ast.rs` | Add `Pattern::EnumVariant { variant_name: CompactString, bindings: Vec<CompactString> }` |
| `src/ir.rs` | Add `Opcode::LoadEnumTag` (0-operand, pushes tag as `Value::Int`) and `Opcode::LoadEnumField(u16)` (field-index operand, pushes that field value). Update encoding/decoding sizes and disassembly |
| `src/parser.rs` | In `parse_pattern()`: when seeing `Ident(v)` followed by `(names...)`, create `Pattern::EnumVariant { variant_name: v, bindings: names }`. [New method `parse_enum_variant_pattern`] |
| `src/resolver.rs` | In match-arm resolution: handle `Pattern::EnumVariant` — look up `variant_name` in enum definitions found in the symbol table, extract field types. Bind each binding name with the correct type (not `Type::Unit`) |
| `src/typeck.rs` | In match type-checking: handle `Pattern::EnumVariant` — look up the enum and variant from the scrutinee type, validate field count matches bindings, infer binding types from field types. Add match-arm coverage validation (warn if not all variants matched) |
| `src/compiler.rs` | For `Pattern::EnumVariant` in match compilation: emit `Dup`, `LoadEnumTag`, load the variant tag constant, `Eq`, `JumpIfFalse(next_arm)`, `Pop`. Then for each binding, emit `Dup`, `LoadEnumField(i)`, `StoreLocal(slot)`. Finally `Pop` the original enum value. Patch jump offsets |
| `src/vm.rs` | Handle `LoadEnumTag`: push `Value::Int(tag)` from top-of-stack `Value::Enum`. Handle `LoadEnumField(n)`: clone the nth element of `data` and push it |

**Dependencies**: None on Phase 2.

---

## Phase 2 — Generic types + Option/Result

### 2a — Generic type syntax

| File | Change |
|---|---|
| `src/ast.rs` | Add `Type::Option(Box<Type>)` and `Type::Result(Box<Type>, Box<Type>)` |
| `src/parser.rs` | In `parse_type()`: when seeing `Ident("Option")` followed by `<T>`, return `Type::Option(inner)`. Similarly for `Result<T, E>`. The `<` lookahead must distinguish from less-than (check after `Ident("Option")`/`Ident("Result")` specifically, or add a `check_type_args_start()` helper) |
| `src/typeck.rs` | `types_compatible`: `Option(a)` is compatible with `Option(b)` iff `types_compatible(a, b)`. Same for `Result`. Type display for error messages |
| `src/formatter.rs` | Handle `Type::Option(t)` and `Type::Result(t, e)` formatting |
| `src/lsp.rs` (if wanted) | Handle new type variants in semantic tokens / hover |

### 2b — Auto-register Option/Result as compiler-known types

| File | Change |
|---|---|
| `src/resolver.rs` | Before registering user declarations, auto-insert `Option` as `SymKind::Enum` with variants `Some(T)` (tag 0, one field) and `None` (tag 1, zero fields). Same for `Result` with `Ok(T)` and `Err(E)`. The variant constructor registrations flow from Phase 1a automatically |
| `src/typeck.rs` | Auto-register the `Some`/`None`/`Ok`/`Err` constructors with their type parameters (so `Some(x)` infers `x`'s type as the inner type of the Option). For now, simplest approach: `Some`'s field type is `Type::Var` (placeholder), unified with the actual argument type during checking |
| `src/stdlib/mod.rs` | Add native helpers: `is_some(val)`, `is_none(val)`, `is_ok(val)`, `is_err(val)`, `unwrap(val)`, `unwrap_or(val, default)`, `expect(val, msg)`, `map(val, fn)`*, `and_then(val, fn)`* (* = callable argument, may need special handling). Update `native_names()` |

**Dependencies**: Phase 1a (for `Some(x)` construction) and Phase 1b (for `Some(v) =>` matching).

---

## Phase 3 — Comprehensive `.zen` tests

`tests/` files covering every feature branch:

| File | What it tests |
|---|---|
| `tests/basic.zen` | `assert(1 + 2 == 3)`, `assert("hi" != "bye")`, `assert(!false)`, variable `let`/`mut` |
| `tests/control_flow.zen` | `if`/`else`, `while`, `for`, `loop`/`break`/`continue` |
| `tests/functions.zen` | fn def/call, recursion (factorial), early `return`, return-type validation |
| `tests/closures.zen` | `\|x\| x + 1`, closures capturing outer variables, closures as arguments |
| `tests/data_types.zen` | arrays (`[1,2,3]`), structs, enums, `match` with literals and wildcards |
| `tests/enum_variants.zen` | Custom enum declaration, variant construction (`Option::Some(x)` via call syntax), pattern matching with bindings |
| `tests/option_result.zen` | `Some(x)`, `None`, `Ok(x)`, `Err(x)` construction + matching; `is_some`/`unwrap` etc. |
| `tests/modules.zen` | `mod name { ... }` and `use path::item;` |
| `tests/stdlib.zen` | `print`, `len`, `trim`, `to_upper`, `to_int`, `type_of`, `push`/`pop`, `min`/`max`/`abs`/`sqrt` |
| `tests/fail_ret_type.zen` | Should fail: `fn f() -> int { return "str" }` (type checker catches) |
| `tests/fail_undefined.zen` | Should fail: `x = 42` without `let`, `undefined_func()` |

**Dependencies**: Phases 0, 1, 2.

---

## Phase 4 — Native function return types (COMPLETE)

Give each native function an accurate type signature so the type checker reports correct types for stdlib calls. Currently all natives are typed as `() -> I64`, but many return `Bool`, `Str`, or `Float`.

| File | Change |
|---|---|
| `src/resolver.rs` | Change `native_names` registration to use a richer signature map: e.g. `FnSignature { name: "contains", params: vec![Type::Str, Type::Str], return_type: Some(Type::Bool) }` instead of hardcoding `Type::I64` |
| `src/typeck.rs` | `check_call` for native functions: validate argument types against declared parameter types, return the declared return type instead of `Type::I64` |
| `tests/stdlib.zen` | Enable strict type assertions like `type_of(x) == "int"`, `contains(s, "ell") == true` |

**Dependencies**: None.

---

## Phase 5 — Exhaustive match checking (COMPLETE)

Warn/error if `match` on an enum does not cover all variants. Currently partial matches compile silently at runtime (no arm matches → fall-through to next).

| File | Change |
|---|---|
| `src/typeck.rs` | In `check_expr` for `match`: when scrutinee type is an enum, collect all variant names covered and compare to all variant names in the enum def. Emit error if any variant is missing and no wildcard arm present. Also handles zero-field variants used as `Pattern::Ident` (e.g., `None`) and `Pattern::Wildcard`. |

**Dependencies**: Phase 1b (enum pattern matching).

---

## Phase 6 — `disasm` display fix (COMPLETE)

`Opcode::from_byte()` returns placeholder operands (always 0), so disassembly shows `LoadConst 0` for every constant regardless of the actual operand bytes. The operand values must be read from the byte stream during disassembly, not from the opcode placeholder.

| File | Change |
|---|---|
| `src/ir.rs` | `disassemble()` reads u16 operands from `self.code` directly at `offset + 1` (or `offset + 3` for 2-operand ops like `MakeEnum`/`CallMethod`/`NewClosure`) instead of using the opcode's placeholder values. |

**Dependencies**: None.

---

## Phase 7 — Closure support

Fully wire up parsed lambdas (`|x, y| x + y`) to the existing closure runtime (`Value::Closure`, upvalues, `NewClosure` opcode). Currently lambdas are parsed into `Expr::Lambda` but compilation may be incomplete.

| File | Change |
|---|---|
| `src/compiler.rs` | `compile_expr` for `Expr::Lambda`: compile the lambda body as a separate `BytecodeFn`, emit `NewClosure` to create a closure at runtime, handle upvalue capture for outer variables. |
| `src/vm.rs` | Test that `|x| x + 1` works, closures can capture and mutate outer `let mut` variables. |

**Dependencies**: None.

---

## Phase 8 — `if let` / `while let` syntax

Sugar for single-arm pattern matching: `if let Some(x) = val { ... } else { ... }` and `while let Some(x) = iter { ... }`.

| File | Change |
|---|---|
| `src/ast.rs` | Add `Expr::IfLet { pattern, expr, then, else_ }` and `Expr::WhileLet { pattern, expr, body }` |
| `src/parser.rs` | Parse `if let pattern = expr { ... }` and `while let pattern = expr { ... }` |
| `src/typeck.rs` | Check patterns similarly to `match` arms |
| `src/compiler.rs` | Compile similarly to `match { pattern => then, _ => else_ }` |

**Dependencies**: Phase 1b (pattern matching infrastructure).

---

## Summary of work by file

| File | Phases affected |
|---|---|
| `src/ast.rs` | 1b (`Pattern::EnumVariant`), 2a (`Type::Option/Result`) |
| `src/parser.rs` | 1b (pattern parsing), 2a (generic type syntax) |
| `src/symbol.rs` | 1a (`SymKind::EnumConstructor`) |
| `src/resolver.rs` | 1a (register constructors), 1b (resolve patterns), 2b (auto-register Option/Result) |
| `src/typeck.rs` | 1a (validate constructor call), 1b (match arm with bindings), 2a (type compatibility) |
| `src/compiler.rs` | 1a (emit `MakeEnum` for constructors), 1b (match arm with tag/field extraction) |
| `src/ir.rs` | 1b (`LoadEnumTag`, `LoadEnumField` opcodes) |
| `src/vm.rs` | 1b (execute new opcodes) |
| `src/stdlib/mod.rs` | 0 (`assert`), 2b (Option/Result helpers) |
| `src/main.rs` | 0 (`test` subcommand) |
| `src/formatter.rs` | 2a (new type variants) |
| `tests/*.zen` | 3 (all) |

---
