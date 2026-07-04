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

## Bugs that directly undermine "hot-reloadable" as a selling point

- [x] 1. **`HotReloader::read_source` only ever reads `script_paths.first()`** (`src/hotreload.rs`). You can pass it multiple watched files, but only the first is ever re-parsed/recompiled on change — edits to the others are silently ignored. For any real game project (multiple `.zen` files per entity/system), this needs to become "reload whichever file changed" or "reload the whole project graph." — **Fixed**: `HotReloader` now treats the first path as the project root and auto-discovers every file-backed `mod` it pulls in via `mod_resolver::resolve_modules_with_paths`, watching all of them. Changing any file in the module graph triggers a full project recompile. See `src/hotreload.rs`, tests `test_reload_picks_up_submodule_change` / `test_reload_discovers_new_module_file`.
- [x] 2. **Hot reload doesn't resolve `mod` declarations at all.** `main.rs`'s normal run/test path calls `mod_resolver::resolve_modules(...)` before compiling, but `HotReloader::do_reload` doesn't. So the moment a project splits logic across files with `mod foo;`, hot reload breaks (or silently doesn't pick up submodule changes) even though the initial load works. — **Fixed** as part of #1 above; `do_reload` now calls `mod_resolver::resolve_modules` every reload.
- [x] 3. **Known bugs already flagged in `TODO.md`** that will bite hardest in exactly the "poke a value while the game is running" workflows hot-reload is for:
   - Closures with upvalue capture crash at top level (`__main__`).
   - `let mut` reassignment before a `for`/`loop` at top level causes a stack overflow.
   These being "top-level only" bugs is suspicious — it smells like top-level (`__main__`) code reuses locals/scope handling differently than function bodies, and hot-reloaded scripts often *are* top-level-heavy (config, entity registries, event wiring). Worth root-causing before building more on top. — **Investigated, found already fixed** (by an earlier, unrelated compiler fix — see the "Bug 2 fix" `compile_assignment` `Dup`-before-store change already in git history — the existing `tests/repro_closure*.zen` and `tests/repro_mut_for.zen` regression tests already cover the originally-reported cases and pass). To be sure this wasn't just "small cases happen to work," added `tests/repro_stress.zen`: 2000-iteration top-level `for` loops after a `let mut` reassignment, nested top-level loops, a 1000-iteration top-level `loop`, and 20 top-level closures created inside a `for` loop each capturing a *different* value of the loop variable (verified individually, not just "didn't crash" — classic closure-in-loop capture bugs return the same value for every closure). All pass correctly.
- [x] 4. **No reload hook for scripts.** When a struct's shape changes across a reload (e.g. you add a field to `Player`), existing live `Value::Struct` instances (`Rc<RefCell<HashMap<String,Value>>>`) simply keep their old field set — new code accessing the new field gets a runtime error instead of a sane default. A `fn on_reload(old)` convention (or auto-filling missing fields with a declared default) would make iteration much less painful. — **Added**: `do_reload()` in `src/hotreload.rs` calls `self.vm.call_if_exists("on_reload")?;` after swapping bytecode and restoring globals. Optional — scripts without `on_reload` are unaffected. Tested in `test_reload_calls_on_reload_hook_if_defined` (uses a `get_reload_count` accessor to avoid `run_main()`'s global reset) and `test_reload_without_on_reload_hook_is_fine`.

## Stdlib/language gaps that matter specifically for games

- [x] **No map/dictionary type.** `Value` has `Array` and `Struct`, but no `HashMap`-like keyed collection. Games constantly need id→entity, name→asset, tag→list lookups. This is probably the single highest-value addition. — **Added**: `Value::Map(Rc<RefCell<HashMap<MapKey, Value>>>)` (`src/value.rs`), with `MapKey` supporting `int`/`str`/`bool` keys. Stdlib in `src/stdlib/map.rs`: `map_new`, `map_set`, `map_get`/`map_has`/`map_remove` (using the `Option<T>` protocol), `map_keys`, `map_values`, `map_len` (`len()` also works generically), `map_clear`. Maps are iterable via `for kv in my_map { ... }`, yielding `[key, value]` pairs (see `MapIterState` in `src/stdlib/iter.rs`). Tests: `tests/maps.zen`.
- [x] **No vector/math library.** Only `abs/min/max/sqrt` exist. No `Vec2`/`Vec3`, no `lerp`, `clamp`, `sin/cos/atan2`, no RNG (`rand`, `rand_range`). Every game script ends up hand-rolling these or bridging back to Rust constantly — defeats the point of scripting for gameplay tuning. — **Added** `src/stdlib/math.rs`: `sin/cos/tan/atan2`, `lerp`, `clamp`, a seeded xorshift64* RNG (`rand`, `rand_range`, `rand_seed`), and `Vec2`/`Vec3` helpers (`vec2`, `vec3`, `vec_add`, `vec_sub`, `vec_scale`, `vec_dot`, `vec_len`, `vec_normalize`) built on plain arrays so they interop with existing array ops. Tests: `tests/math.zen`.
- [x] **Iterator adapters.** Now that the `next()`/`Option<T>` protocol exists, `map`/`filter`/`fold`/`enumerate`/`zip`/`take`/`collect` are a natural, cheap follow-up (they can literally be written as native functions that just loop calling `.next()`), and hugely improve ergonomics for the kind of data-wrangling gameplay code does. — **Added, but as *Zenlang* functions, not native Rust ones**: native functions have no way to call back into a script-provided closure (`VMContext` doesn't hold a VM handle), so `map`/`filter`/`fold` couldn't be implemented in Rust without a bigger VM refactor. Instead, added `src/prelude.zen` (embedded via `include_str!`, injected into every compiled program right after module resolution — see `src/prelude.rs`, wired into `main.rs`'s `run`/`test`/`check`/`disasm` and `hotreload.rs`) with `map`, `filter`, `fold`, `enumerate`, `take`, `zip`, `collect`, all written in Zenlang itself using the iterator protocol. Doubles as a real-world exercise of that protocol. Tests: `tests/prelude_iterators.zen`.
- [x] **No coroutines/generators.** — **Implemented**: `yield` keyword (`Value::Generator`, `Opcode::Yield`, `GeneratorState`), `FunctionCompiler.is_generator` flag, `Call` opcode returns `Value::Generator` on first call, `Yield` saves IP+locals and suspends execution, `Return` marks exhausted. `VMContext.raw_vm` pointer lets native functions (like `next(g)`) resume generator execution via `resume_generator()`. The `next` native function returns `Option<T>` so `unwrap(next(g))` retrieves yielded values. Tested: single yield, multiple yields, exhausted returns None. All 177 lib tests + 37 `.zen` integration tests pass.
- [x] **No delta-time/scheduling primitives** if you want scripts to register timers/callbacks (`after(seconds, fn)`, `every_frame(fn)`) rather than the host driving everything — depends on how tightly you want scripts embedded in your loop. — **Implemented**: `set_timeout`/`set_interval`/`clear_timer` native functions with `VM::tick(dt)` time advancement previously existed. Added `after(seconds, callback)` as a more natural API shape (seconds-first), `every_frame(callback)` to register per-frame callbacks via `VM::tick()`, and `VM::add_frame_callback`/`remove_frame_callback` methods.

## Performance, since "fast iteration" presumably also means "fast enough at runtime"

- [x] **`Value::Struct` is `Rc<RefCell<HashMap<String, Value>>>`.** Every field access is a hash lookup, and every instance carries full hashmap overhead. For thousands of live entities/components (typical in games) this is meaningfully slower and heavier than a fixed-slot layout. A `Vec<Value>` with compile-time-resolved field→index (falling back to a name lookup only when the shape doesn't match, to preserve hot-reload flexibility) would keep the dynamic-friendliness while being much faster for the common case. — **Implemented**: Replaced `HashMap<String,Value>` with `StructData` (`Vec<Value>` + shared `Rc<Vec<String>>`). The compiler now resolves field names to struct-type-specific indices at compile time (via the `TypeMap` and `SymbolTable`) and emits those directly in `LoadField`/`StoreField` opcodes. The VM uses O(1) Vec indexing in the common case and falls back to a name→index linear scan when the shape doesn't match (e.g. after hot reload). Struct literals are compiled in declaration order so the struct-specific indices match the storage layout. Added `StructData` struct to `value.rs` with `get_field()`/`get_field_mut()` helpers for the fallback path.
- [x] **Rc reference cycles leak.** Everything is `Rc<RefCell<...>>`, no cycle collector. Long-running game sessions with cyclic references (parent/child entities, observer patterns) will leak memory slowly. At minimum, a `Weak`-reference value type would let scripters break cycles intentionally.
- [x] **No script step/time budget.** An infinite loop in a hot-reloaded script currently just hangs the game (and blocks the editor tooling too). A configurable instruction-count or wall-clock budget with a catchable "script timeout" error is standard in embedded scripting VMs and saves a lot of debugging pain. — **Added**: `VM::set_instruction_limit(limit)` sets a max instruction count per `run_main`/`call_function` call. When exceeded, a `"script timeout: executed N instructions (limit: M)"` runtime error is returned. Default `0` = unlimited. Tested in `test_instruction_limit_hits_timeout` (infinite `loop {}` with limit=100) and `test_instruction_limit_zero_is_unlimited` (bounded loop completes normally).

## Tooling/dev-experience (this is where "not waiting on Rust" really gets won or lost)

- [x] **The LSP comment in `lsp.rs` literally says Zed isn't sending `didChange`/`didSave` and the root cause looks client-side.** — **Investigated**: Updated `textDocumentSync` to the most explicit `TextDocumentSyncOptions` form (with `open_close`, `change: Full`, and `save: SaveOptions { include_text }` set). The same symptom persists — `didOpen` is received correctly but `didChange`/`didSave` are not sent. This matches the prior investigator's conclusion that it's a Zed client bug (confirmed by the fact that both `Kind(FULL)` and `Options` forms were tried with identical results). Added 13 LSP unit tests (compile_source, offset/position conversion, hover, goto-definition, completion, document symbols, semantic tokens, comment extraction) to prevent server-side regressions. The Zed client should be filed upstream; server-side has been validated to work correctly.
- [x] **No debugger (DAP) integration** — breakpoints, step-over/into, variable inspection while the game is running. This is probably the biggest "quality of life at scale" investment for a game-scripting workflow, but is also the most expensive to build. — **Added**: Full VM debug infrastructure in `src/vm.rs`: `DebugState`, `DebugStepMode`, `DebugFrameInfo`; breakpoint set/resolve/check with `skip_offset` resume; step modes (Into/Over/Out); pause/resume (`debug_continue`, `debug_step_into/over/out`); stack inspection (`debug_locals`, `debug_stack_frames`, `debug_current_location`); `DebugBreak` error variant for LSP diagnostics. Debug hook in `execute()` checks before each instruction. `execute_debug()` wrapper handles pause/resume loop. `resume_generator()` fixed for `execute_debug` compatibility. 16 unit tests cover breakpoints, stepping, stack frames, and locals. **DAP protocol server** (`src/dap.rs`): JSON-RPC 2.0 over stdin/stdout with Content-Length framing. Handles initialize, launch, setBreakpoints (source-line → all matching functions), configurationDone, continue/next/stepIn/stepOut, stackTrace/scopes/variables, threads, pause, terminate/disconnect. Print output captured via replaced native function and forwarded as DAP output events. Single-threaded design: VM runs until pause/termination, then returns to message loop. Command: `zenc Dap <script>`.
- [x] **Runtime errors report line/col but not a source snippet or "did you mean" suggestions** — small thing, but it's what you'll be staring at dozens of times a day during iteration. — **Improved**: `runtime_error()` in `src/vm.rs` now formats the full stack trace with function names into the error message, producing output like `division by zero\nstack trace:\n  0: at 10:5 (in damage)\n  1: at 25:3 (in main)`. Source snippets and "did you mean" suggestions are still future work, but the structured trace is no longer silently discarded.
- [x] **`register_type`/`ForeignTypeDef` binding boilerplate is verbose per Rust type** (see `examples/foreign_types.rs` — every field/method is a hand-written closure pair). A small derive macro (`#[zen_type]` on a struct + `#[zen_method]` on impl blocks) would cut a lot of ceremony for engine integrators, which matters a lot since your use case is "embed this in an engine." — **Added**: `#[derive(ZenForeign)]` proc-macro in new `zenlang-macros` crate (`macros/`). Generates `register_zen_foreign(vm)` with auto-generated getter/setter closures for all common field types (`String`, `i64`/`i32`/`i16`/`i8`, `u64`/`u32`/`u16`/`u8`, `f64`/`f32`, `bool`, `Value`, and `Rc<RefCell<...>>` foreign references). Example updated to use the macro. Method registration currently manual via `Rc::make_mut(&mut vm.foreign_registry)`. Remaining: `#[zen_method]` attribute support for auto-registration (see TODO.md line 30 post-fix).



- [x] 1. Fix the two `hotreload.rs` gaps (multi-file watch + `mod` resolution) — directly serves your stated goal and is a small, contained fix.
- [x] 2. Add a `Map`/`Dict` value type + stdlib functions.
- [x] 3. Add iterator adapters (`map`/`filter`/`fold`/`enumerate`) — cheap now that the protocol exists. (Delivered as a Zenlang-language prelude — see above.)
- [x] 4. Add a small math/vector stdlib module (`Vec2`/`Vec3`, lerp/clamp, trig, RNG).
- [x] 5. Investigate and fix the two known top-level closure/loop bugs, since they'll surface constantly in hot-reloaded top-level game config code. (Found already fixed; added `tests/repro_stress.zen` to lock it in.)
- [x] 6. Implement `fn on_reload()` hook for scripts — called after every successful hot reload, so scripts can re-derive caches, fix up struct shapes, etc. (`src/hotreload.rs:177`).

### Bonus fixes made along the way (not originally listed)

- [x] Two related typeck bugs found while adding maps/iterators: `Expr::For`'s loop-variable type inference and `Expr::Index`'s element-type inference both defaulted to `Type::I64` for any non-array/non-str value instead of the type-erased `Type::Unit` placeholder — this silently broke type checking for `for`-loops and indexing over ranges-as-values, maps, and custom iterators. Fixed both to default to `Type::Unit`.
- [x] `Expr::MethodCall`, `Expr::Match` (enum-variant patterns), `Expr::Call`, `Expr::If`, and `Expr::While` in `typeck.rs` all unconditionally rejected `Type::Unit` (the codebase's established "type-erased/generic" placeholder) operands — meaning calling a method on, matching an enum out of, calling, or branching on any type-erased value (e.g. an untyped closure parameter, or the result of a native function like `iter()`) was a hard type error. Fixed all five to treat `Type::Unit` as passthrough/compatible, consistent with `types_compatible`'s existing rule. This was required for the Zenlang-native prelude (`map`/`filter`/etc.) to type-check at all.
- [x] Found and fixed a real parser bug in `match_target()` (the restricted expression parser used for `match <target> { ... }` to avoid struct-literal ambiguity): it supported `ident` and `ident.field.field` chains but never applied trailing calls, so `match it.next() { ... }` silently dropped the `()` and failed to parse (previously you had to write `match (it.next()) { ... }`, wrapping in parens, which is still supported). Fixed `match_target` to apply the normal postfix chain (field access, method/function calls, indexing) after the initial struct-literal-safe prefix.
- [x] Added `VMContext.raw_vm: *mut VM` pointer to allow native functions (like `next`) to interact with the VM (e.g. resume generators). Used by `next_impl` in `src/stdlib/mod.rs`.
- [x] Verified via `cargo build`, `cargo test --lib` (177 tests), and `cargo run -- test` (37 `.zen` integration tests) that nothing regressed across all of the above.

## Phase — Iterator protocol

- General `Iterator` protocol: any value with a `.next(&mut self) -> Option<T>` method drives a `for` loop.
- `for var in iter_expr { body }` (generic case; inline range literals keep their existing fast path) desugars in the compiler to `let __for_iter = iter(iter_expr); loop { match __for_iter.next() { Some(var) => { body }, None => break } }`, reusing existing `Match`/`Loop`/`CallMethod` codegen — no new opcodes.
- New native `iter(x)`: wraps `Array`/`Range`/`Str` values in foreign cursor objects (`ArrayIter`/`RangeIter`/`StrIter`, in `src/stdlib/iter.rs`) each exposing `next()`; passes `Struct`/`Foreign` values through unchanged since they're assumed to already implement the protocol.
- User-defined structs become iterable for free by implementing `fn next(&mut self) -> Option<T>` in an `impl` block — no special-casing needed, reuses existing struct method dispatch.
- Fixed `Opcode::CallMethod`'s `Value::Foreign` dispatch in `vm.rs` to pass the receiver as `args[0]` to registered native methods (previously dropped, so foreign methods couldn't see or mutate their own receiver).
- `tests/iterators.zen` covers arrays, strings, range variables, custom struct iterators, manual `iter()`/`.next()` use, and `break`/`continue`.

## Phase — Try operator `?`

- `expr?` desugars to `match expr { Ok(val) => val, Err(e) => return Err(e) }` at parse time — no new AST nodes needed, reuses `Expr::Match` infrastructure.
- `TokenKind::Question` added to `Precedence::of()` at `Call` level so `expr?` binds as tightly as a function call.
- Fixed match-arm stack management for `Pattern::EnumVariant`: after `Dup` + `LoadEnumTag` + `Eq` + `JumpIfFalse`, the original enum value was being incorrectly `Pop`'d before the binding loop, causing "LoadEnumField on non-enum value" at runtime. Removed the extra `Pop` — the enum value survives on the stack for the `Dup`/`LoadEnumField` binding loop.
- 3 full-pipeline tests: inline `Ok(42)?`, function return `try_unwrap()?`, and `Err("fail")?` early return.

## Phase 0 — Type-erased generics

- Generic function definitions with `<T, U>` syntax — `fn identity<T>(x: T) -> T { x }`.
- Generic struct/enum definitions with type params — `struct Foo<T> { x: T }`.
- Generic impl blocks — `impl<T> Vec<T> { fn push(&self, val: T) { } }`.
- **Type erasure** strategy: no monomorphization. Generic params are `Type::Generic("T")` in the type checker, compatible with any type. The VM already boxes all values as `Value` enum, so no runtime changes needed.
- `resolve_type` helper converts `Type::Named("T")` → `Type::Generic("T")` at registration time when `"T"` matches the declaration's type params.
- All 123 tests pass including 3 generic integration tests.

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


Would JIT help? Yes — but there are caveats.

### What JIT would buy you

1. **10–50× on numeric loops** — The interpreter currently dispatches on 24 `Value` variants for every arithmetic op. A tracing JIT (LuaJIT-style) would observe "these are always `f64`" and emit native `addsd`/`mulsd`, skipping boxing, dispatch, and refcount traffic entirely.

2. **2–4× on general code** — A simpler method JIT (compile whole functions via Cranelift) eliminates bytecode dispatch overhead. That's decent but unspectacular.

3. **Inlining** — The interpreter has full frame-setup/teardown on every call. A JIT can inline small functions, which the current architecture cannot.

### The hard parts

| Problem | Why it's harder than LuaJIT |
|---|---|
| **24-value `Value` enum** | LuaJIT has only **2** type tags (number vs. GC object) with NaN-tagging. Zenlang needs 5+ bits just to discriminate variants. Type guards are fatter, side exits are more frequent, and trace recording is more fragile. |
| **`Rc<RefCell<>>` everywhere** | Reference counting and runtime borrow-checking dominate execution time. A JIT can't optimize these away — the semantics must be preserved. Hot paths that touch shared mutable state won't speed up much. |
| **No existing IR** | You'd need to build a CFG + SSA IR from bytecode or from the AST + TypeMap. This is non-trivial. |
| **Foreign function bridge** | All ~40 native functions take `&mut VMContext` and can mutate any VM state. The JIT must either emit a call to them (preserving the VM contract) or know the semantics of specific natives. |

### What I'd recommend in priority order

| Approach | Effort | Speedup | Best for |
|---|---|---|---|
| **1. Cranelift method JIT** (compile bytecode functions to native, remove dispatch) | 1–2 months | ~2–4× | General code |

**Bottom line**: Zenlang's bytecode is actually a pretty good target for JIT — it's clean, simple, and small (~50 ops). The value representation (`Rc<RefCell<24-variant-enum>`) is the real pain point, not the bytecode format.


Next Steps
- Consider adding convenience methods (e.g. `VM::alloc_array`, `VM::alloc_foreign`) to the public API for external users
- All parser tests (`test_nested_scopes`, `test_enum_match`, `test_map_operations`) now pass.

JSON serialization** — `to_json`/`from_json` native functions backed by serde_json
2. **Closure callbacks** — `VMContext::call_value()` with reentrancy-safe `return_to_depth`
3. **ForeignObject::clone** — `clone_fn` closure approach, derived Clone on iter states
4. **Auto-register constructors** — `#[zen_methods]` detects no-self + returns Self → `vm.register_native()`
5. **`TryFrom<Value>` / `From<T>` impls** — i64, f64, bool, String conversions to/from Value
6. **StructBuilder API** — builder pattern + `VM::make_struct()` helper
7. **`#[zen_native_fn]` proc macro** — generates `FnSignature` from annotated native functions

value.rs**: 52 new unit tests covering `From`/`TryFrom` impls, `ForeignObject`, `StructBuilder`, `MapKey`, `Value` methods, `StructData`, `PartialEq`, debug format; rustdoc examples on `StructBuilder::new/field/build/name`
- **vm.rs**: 20 new unit tests (stdlib fns, JSON edge cases, `make_struct`); rustdoc on `VMContext`, `register_timer`, `remove_timer`, `call_value`, `register_native`, `make_struct`
- **macros/src/lib.rs**: Added `name:` parameter to `#[zen_native_fn]` (optional; defaults to Rust fn name); doc on `#[derive(ZenForeign)]`
- **stdlib/mod.rs**: Module-level doc; `contains_impl` now uses `#[zen_native_fn(name: "contains", ...)]` so the generated sig has the correct Zenlang name
- **interop.rs**: Added rustdoc to `FieldAccessor::new`, `ForeignTypeDef::new/field/method`, `ForeignTypeRegistry::get/get_mut/get_by_name


### Phase 1: `Type::Any` split (~1 day) ✅
Add `Type::Any` variant, replace wildcard `Unit` usages, clean up `types_compatible`.

### Phase 2: Structural typing + `opaque type` (~2-3 weeks) ✅
- **Structural by default**: `types_compatible` falls through to `structurally_compatible(a, b)` when names differ and neither is opaque
- **Width subtyping**: extra fields in provided type OK, missing fields in provided type fail
- **Excess property checks**: struct *literals* with unknown field names → compile error
- **`opaque type Name = Base`**: creates a nominally isolated type — NOT compatible with `Base` or any other type. Name-matching only. Requires explicit conversion both ways.
- **Foreign type registration**: Rust foreign structs register their field types in the symbol table so they participate in structural comparison

### Phase 3: Local bidirectional inference (~2-3 weeks) ✅
- `Type::Var(u64)` for local unification
- `unify()` + `resolve()` in the typechecker
- Expected-type propagation downward from context
- Let-binding, lambda, and generic call-site inference

### Phase 4: `unknown` + narrowing (~1-2 weeks) ✅
- `Type::Unknown` — safe top type, no implicit compatibility
- Field access / method call on `unknown` → compile error
- Narrowing through match patterns and casts

### 🔴 Critical
1. **`assert_eq` panics the host process** — Changed `panic!()` to `return Err(Error::Script{..})` in `src/stdlib/mod.rs:382`. Added 2 tests.
2. **`tests/fs.zen` fails** — Fixed 5 match arms returning `bool` vs `()` mismatch by changing `Ok(_) => true` to `Ok(_) => ()`.
3. **`tests/prelude_iterators.zen` fails** — Changed default unannotated param type from `Type::Unit` to `Type::Any` in `src/resolver.rs:143,195` (function signatures), and changed default unannotated return type from `Type::Unit` to `Type::Any` in `src/typeck.rs:431` (function ident type lookup).

### 🟡 Major
4. **README CLI docs** — Updated all 19 `zenlang` → `zenc` references, added missing commands (`new`, `build`, `dap`, `test`).
5. **README broken link** — Fixed `tests/tour.zen` → `examples/tour.zen`.
6. **DAP `unwrap()`** — Replaced `unwrap()` with proper error handling in `src/dap.rs:33`.
7. **Changelog** — Updated stale reference to non-existent test failures.

### 🔵 Minor
8. **`parser_test.rs` compiled unconditionally** — Moved to `#[cfg(test)]` in `src/lib.rs:13`.
9. **Dead code** — Removed unused `fresh_var()`, `next_id` from `src/typeck.rs`.
10. **`operand_count` wrong for `NewClosure`** — Fixed from 1 to 2 in `src/ir.rs:174`.
11. **Formatter missing `Match`** — Added `TokenKind::Match` to `must_start_line()` in `src/formatter.rs:190`.
12. **JSON `from_f64` double unwrap** — Replaced with `unwrap_or_else` + `expect` in `src/stdlib/json.rs:40`.
13. **Unused `arena_b::Arena` import** — Removed from `src/main.rs:10`.

### Verifications
- `cargo build`: 0 warnings
- `cargo test --lib`: 246 passed, 0 failed
- `cargo run -- test`: 37/37 passed

| # | Item | Status | Commit |
|---|------|--------|--------|
| 1 | Unified `foreign_type!` macro | ✅ Done | `e4e4674` |
| 2 | Type name override `#[foreign(name = "...")]` | ✅ Done | `d1a4766` |
| 3 | Safe constructor helper `VM::wrap_foreign()` | ✅ Done | `1f3788f` |
| 4 | `Option<T>` field support | ✅ Done | `9b6c930` |
| 5 | Default-constructor `#[foreign(default)]` | ✅ Done | `9b6c930` |
| 6 | `ty_to_field_type` syn AST matching | ✅ Done | `7155d0a` |
| 7 | Book documentation alignment | ✅ Done | `de276b0` |


Add gh-pages workflow for building the MDBook
