# Changelog

## [0.2.0] -
- Compound assignment operators (`+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`)
- Bitwise operators (`&`, `|`, `^`, `~`, `<<`, `>>`) + compound assigns
- Numeric separator support (`1_000_000`)
- `run_script` graceful exit via Ctrl+C
- Unicode identifier support (`is_alphabetic()` instead of `is_ascii_alphabetic()`)

## [0.1.0]

### 🔴 Critical Bugs

**1. All error reporting methods lose span information** ✅ FIXED
- Parser now stores `source` and computes line/col from byte offset.
- Compiler, Resolver, TypeChecker all track `current_span`.

**2. `Break` and `Continue` are broken** ✅ FIXED
- `Break` emits `Jump(0)` with placeholder patched at loop exit (like a real break).
- `Loop` no longer emits `JumpIfFalse(0)` (was causing stack underflow).
- `loop_end` changed to `loop_end_jumps: Vec<Vec<usize>>` for multiple break jumps.

**3. `Loop` expression is broken** ✅ FIXED
- Removed the invalid `JumpIfFalse(0)` on loop entry.

**4. `for` loop now supports arrays and strings** ✅ FIXED
- Non-range iterables compile to index-based iteration using `Len` and `LoadIndex` opcodes.
- Added `Len` opcode to VM.
- Added string indexing to `LoadIndex`.

**5. `Match` pattern with `Ident` now binds in body** ✅ FIXED
- `Pattern::Ident` creates a local variable slot and stores the matched value.

### 🟡 Design Issues

**6. `I32` type name is misleading** ✅ FIXED
- `ast.rs`: `Type::I32` renamed to `Type::I64`. Parser accepts both `"i32"` and `"i64"`. Resolver maps `"int"`/`"i32"` → `I64`.

**7. No `f32` support for a game engine language** ✅ FIXED
- Added `Type::F32` variant. Parser accepts `"f32"`. Type checker handles `f32` coercion with `f64`/`i64`.
- Runtime stores all floats as `f64` (existing `Value::Float`).

**8. `Value::PartialEq` uses reference identity for arrays/structs/enums** ✅ FIXED
- Changed from `Rc::ptr_eq` to `*a.borrow() == *b.borrow()` (structural comparison).

**9. `SymbolTable` maintains triplicate state** ✅ FIXED
- Removed `scope_map` field; all lookups use `scopes[scope].symbols` directly.

**10. String interning is inconsistent**
- Lexer uses `CompactString`, AST uses `String`, causing repeated heap allocations. The AST types (`Expr::Str`, `Stmt::Let::name`, etc.) should use `CompactString` or `Rc<str>`.

**11. Constant dedup is O(n²)** ✅ FIXED
- Added `const_map: HashMap<u64, u16>` for O(1) constant lookup with collision fallback to linear scan.

**12. Parser `pub` keyword is silently consumed but does nothing** ✅ FIXED
- Added clarifying `// TODO: visibility tracking not yet implemented` comment to make the behavior explicit.

**13. No string concatenation at runtime** ✅ FIXED
- Added `Str + Str` case in VM's `Opcode::Add` handler.

### 📋 Missing Features

**14. No module/import system** — `Use`/`Mod` tokens defined but unused.

**15. No closures** — `Lambda` AST nodes are parsed but produce a compile error.

**16. No `goto_definition` in LSP** — Stubbed to `None` at `lsp.rs:763`.

**17. No compound assignment operators** — `+=`, `-=`, etc. unsupported.

**18. No bitwise operators** — `&` and `|` are boolean-only.

**19. `self` in `impl` blocks is not special** — It's parsed as a parameter name but there's no implicit receiver passing mechanism.

**20. No numeric separator support** — `1_000_000` is not lexed as a single number.

### 🔧 Code Quality

**21. `run_script` enters an infinite loop with no graceful exit** (`main.rs:84-90`).

**22. Stdlib functions silently return `Nil` on type mismatch** — `trim_impl`, `to_upper_impl`, etc. should likely produce runtime errors instead of silently returning `Nil`.

**23. The test helper in `vm.rs:748-760` is copy-pasted into every test** — Each test recompiles from scratch with no caching, making the test suite slow.

**24. Unused variables sprinkled through the code** — Most prefixed with `_` but some (like `_user_var_slot`, `_tag`, `_data_count`) indicate incomplete implementations.

**25. No Unicode identifier support** — `is_ident_start` only accepts ASCII letters, excluding non-English developers.

---

### Priority Recommendations (completed items removed)

1. ~~Fix error span reporting~~ ✅
2. ~~Fix Break/Continue/Loop~~ ✅
3. ~~Change Type::I32 to Type::I64~~ ✅
4. ~~Remove triplicate state from SymbolTable~~ ✅
5. ~~Add f32 support~~ ✅
6. ~~Fix Pattern::Ident in match compilation~~ ✅
