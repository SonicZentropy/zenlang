# v0.4.0 Implementation Plan

## ✅ COMPLETED: Initial fixes (Parts 1-3)

### Part 1: Tighten Expr::Index for known non-indexable types (DONE)

In typeck.rs, distinguish the _ branch:

- Type::Any → allow through (truly runtime-determined, e.g. range values)
- Type::Var(_) → allow through (not yet inferred)
- Type::Generic(_) → allow through (could be anything)
- Type::I64 | Type::F32 | Type::F64 | Type::Bool | Type::Unit | Type::Fn{..} → error: "type 'X' does not support indexing"
- Type::Unknown → error: "cannot index 'unknown'; narrow via match or cast first"
- Type::Named(name) → error: "type 'X' does not support indexing" (structs)
This alone doesn't fix the lazy adapter problem (since map returns Type::Any), but it's a necessary foundation and catches other latent bugs.

### Part 2: Add Type::Iter(Box<Type>) to the type system

New AST variant in ast.rs:
/// `Iter<T>` — a lazy iterator yielding `T`. Produced by `map`/`filter`/etc.
/// Must call `collect()` to materialize into `[T]`.
Iter(Box<Type>),
Then:

- stdlib/mod.rs native_fn_sigs(): Change map, filter, take, zip, enumerate, etc. to return Type::Iter(param_type) instead of Type::Any
- typeck.rs: Handle Type::Iter in resolve_var, unify, types_compatible, type_display

### Part 3: Error on Type::Iter in Expr::Index

In the Expr::Index handler, add before the catch-all:
Type::Iter(_) => {
    self.error(
        "cannot index lazy iterator; did you forget to call collect()?"
    );
    Type::Unit
}
This gives the user a specific, actionable error at compile time when they write map[arr, f](0).

### ✅ Test plan — ALL PASSING

- `cargo test` — 262 unit tests pass
- `cargo run -- test` — 39/39 Zen integration tests pass
- `cargo run -- run examples/tour.zen` — tour completes ("Zen tour complete!")

## Functionalization Plan

### Phase 1: Pipe Operator `|>` (8 files, ~80 lines)

**Goal:** `x |> f` desugars to `f(x)` at parse time.

| File | Change |
|---|---|
| `src/token.rs` | Add `Pipe` variant to `TokenKind` + Display (`\|>`) |
| `src/lexer.rs:195` | In `'|'` handler, check `>` first (before `=`, `|`), emit `Pipe` |
| `src/parser.rs:9-65` | Add `Pipe` precedence level between `Compare` and `Term`; update `next()` and `of()` |
| `src/parser.rs:620-649` | Add `TokenKind::Pipe` to the binary-op match; handle it: parse RHS at `Pipe.next()` prec, emit `Expr::Call { func: rhs, args: [lhs] }` |
| `tests/pipe.zen` | New test: basic chain `arr \|> map(_, f)`, multi-step `x \|> f \|> g`, precedence `x \|> f + g` |
| `examples/tour.zen` | Add pipe examples |

**No AST/compiler/typeck/VM changes** — pure parse-time desugar.

---

### Phase 2: Partial Application `_` (2 files, ~50 lines)

**Goal:** `f(_, arg2, _)` desugars to `\|__p0, __p1\| f(__p0, arg2, __p1)` at parse time.

| File | Change |
|---|---|
| `src/parser.rs:676-683` | After parsing call args, scan for `Expr::Unit` (from `_` token). If found, generate fresh param names `__p0..__pN`, replace each `Unit` with `Ident("__pN")`, wrap in `Expr::Lambda`. |
| `tests/partial_app.zen` | New test: single placeholder, multi-placeholder, mixed with real args, nested calls |

**No AST/compiler/typeck/VM changes** — `Expr::Lambda`/`Expr::Call` already fully supported.

**Edge cases handled:**
- `f(_, x)` → `|__p0| f(__p0, x)` ✓
- `f(_, _)` → `|__p0, __p1| f(__p0, __p1)` ✓
- `f(_)` → `|__p0| f(__p0)` (identity-like) ✓
- `f(x)` → unchanged (no `_`) ✓
- `let x = _` → unchanged (still `Expr::Unit` — not in call arg context) ✓

---

### ✅ DONE: Phase 3-4: Lazy Iterator Adapters (8+ files, ~900+ lines)

**Strategy:** Replace eager `prelude.zen` functions with lazy native Rust implementations. Each adapter is a `ForeignObject` with a `.next()` method registered via `register_type().method("next", ...)`.

**Why native not .zen:** Creating foreign objects requires Rust (`VM::foreigns.insert(...)`). The old comment "native functions can't call back into script closures" is obsolete — `ctx.call_value()` now exists.

**New types in `src/stdlib/iter.rs`:**

| Type | State | `.next()` behavior |
|---|---|---|
| `LazyMapIter` | `source: Handle, f: Value` | calls source `.next()`, if `Some(v)`, calls `ctx.call_value(f, [v])`, returns `Some(result)` |
| `LazyFilterIter` | `source: Handle, pred: Value` | loops calling source `.next()` until `pred(v)` is true or `None` |
| `LazyTakeIter` | `source: Handle, remaining: usize` | decrements `remaining`, returns `None` when 0 |
| `LazySkipIter` | `source: Handle, remaining: usize` | skips `remaining` elements on first `.next()`, then passes through |
| `LazyChainIter` | `first: Handle, second: Handle, on_first: bool` | exhausts `first`, then `second` |
| `LazyZipIter` | `a: Handle, b: Handle` | calls both `.next()`; if both `Some`, returns `[av, bv]` |
| `LazyEnumerateIter` | `source: Handle, idx: usize` | pairs each value with incrementing index |
| `LazyStepByIter` | `source: Handle, step: usize, idx: usize` | skips `step-1` elements between yields |
| `LazyCycleIter` | `source: Handle, saved: Vec<Value>, idx: usize, phase: CyclePhase` | caches on first pass, repeats forever |
| `LazyInspectIter` | `source: Handle, f: Value` | calls `f(v)` as side-effect, passes `v` through |
| `LazyFlattenIter` | `source: Handle, current: Option<Handle>` | calls `iter()` on each element, exhausts inner before outer |
| `LazyFlatMapIter` | `source: Handle, f: Value, current: Option<Handle>` | maps then flattens |
| `LazyScanIter` | `source: Handle, f: Value, acc: Value` | like fold but emits each accumulator |

**Helper functions:**
- `call_source_next(vm, registry, ctx, source_h)` → `Option<Value>` — dispatches `.next()` on source via `registry.call_method()`
- `ensure_iterator(vm, ctx, val)` → `Handle` — calls `iter()` if not already a foreign iterator

**Key implementation detail for `.next()` methods:**

```rust
fn lazy_map_next(ctx: &mut VMContext, args: &[Value]) -> Result<Value> {
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
    let self_h = match args.first() { Some(Value::Foreign(h)) => *h, _ => error };
    let fo = vm.foreigns.get_mut(self_h);
    let state: &mut LazyMapIter = fo.downcast_mut()?;

    let next = call_source_next(vm, &ctx.registry, ctx, state.source)?;
    match extract_option_value(vm, &next) {
        Some(inner) => {
            let result = ctx.call_value(&state.f, &[inner])?;
            Ok(option_some(ctx, result))
        }
        None => Ok(option_none(ctx)),
    }
}
```

| File | Change |
|---|---|
| `src/stdlib/iter.rs` | Add all lazy adapter types + `.next()` methods + native constructors (`map_impl`, `filter_impl`, etc.) + register them in `register()` |
| `src/stdlib/mod.rs` | Register new native functions (`map`, `filter`, `take`, `skip`, `chain`, `zip`, `enumerate`, `step_by`, `cycle`, `inspect`, `flatten`, `flat_map`, `scan`) in `register_builtins()` |
| `src/stdlib/mod.rs` | Add type signatures for all new functions in `native_fn_sigs()` |
| `src/prelude.zen` | Remove all iterator adapter functions (now native) — keep file empty or remove `include_str` reference |
| `src/prelude.rs` | Remove `include_str!("prelude.zen")`, remove iterator adapter doc, potentially simplify `inject()` to a no-op or remove |

**Impact:**
- `map(arr, |x| f(x))` now returns a lazy `LazyMapIter` instead of an array — **breaking change**
- To get an array, use `collect(map(arr, |x| f(x)))`
- But with pipes: `arr |> map(_, |x| f(x)) |> collect()`

---

### ✅ DONE: Phase 5: Terminal Operations (1 file, ~200 lines)

**Goal:** Functions that eagerly consume a lazy iterator and return a single value.

All go in `src/stdlib/iter.rs`:

| Function | Signature | Behavior |
|---|---|---|
| `count(it)` | `iterable → i64` | Counts elements |
| `all(it, pred)` | `iterable, (T → bool) → bool` | true if all match |
| `any(it, pred)` | `iterable, (T → bool) → bool` | true if any match |
| `find(it, pred)` | `iterable, (T → bool) → Option<T>` | first match |
| `position(it, pred)` | `iterable, (T → bool) → Option<i64>` | index of first match |
| `min(it)` | `iterable → Option<i64>` | minimum (int only) |
| `max(it)` | `iterable → Option<i64>` | maximum (int only) |
| `sum(it)` | `iterable → i64` | sum |
| `product(it)` | `iterable → i64` | product (maybe float later) |
| `join(it, sep)` | `iterable, str → str` | string join |
| `partition(it, pred)` | `iterable, (T → bool) → [passed, failed]` | split into two arrays |

Each calls `ensure_iterator()` + loops calling `.next()` until `None`.

**`fold` and `collect`:** Already exist. Keep as native implementations (move from prelude.zen to native if not already). `fold` is already eager. `collect` can stay as an eager native that drives the iterator.

---

### Phase 6: Documentation, Testing, Version Bump

| File | Change |
|---|---|
| `book/src/operators.md` | Document `\|>` pipe operator with examples |
| `book/src/functions.md` | Document `_` placeholder partial application |
| `book/src/stdlib-iter.md` | Complete rewrite — document all lazy adapters + terminal ops, show pipe chaining examples |
| `book/src/SUMMARY.md` | Update if any new pages added |
| `book/src/embedding.md` | Update version `"0.4.0"` |
| `book/src/common-patterns.md` | Update version `"0.4.0"`, add iterator patterns |
| `book/src/cargo-features.md` | Update version `"0.4.0"` |
| `Changelog.md` | Add v0.4.0 entries |
| `Cargo.toml` | Bump version to `"0.4.0"` |
| `tests/pipe.zen` | New — pipe operator tests |
| `tests/partial_app.zen` | New — partial application tests |
| `tests/prelude_iterators.zen` | ✅ Updated — now uses `collect()` before indexing |
| `tests/iterators.zen` | Update if needed |
| `examples/tour.zen` | Add pipe + partial app + lazy iterator examples |

---

### Test Plan

After each phase, run:
```
cargo test                       # Rust unit/integration tests
cargo run -- test                # Zen integration tests
cargo run -- run examples/tour.zen
cargo clippy
cargo doc --no-deps
```

### Edge Cases & Concerns

1. **`_` ambiguity:** `f(_)` currently means `f(())`. After change, it means partial application. Any existing code relying on `_` as unit in a call arg will break. Acceptable per pre-alpha policy. Could mitigate by adding `()` syntax for unit in the future.

2. **~~Lazy adapter `collect()` requirement~~ (RESOLVED):** The type-checker now catches indexing into `Type::Iter` at compile time: `"cannot index lazy iterator; call collect() to materialize it into an array"`.

3. **`next()` native function:** The existing `next(generator)` built-in works on generators, not on iterators. The lazy adapters use `.method()` dispatch (`foreign.next()`), not the `next()` native function. No collision.

4. **Closure capture in lazy adapters:** The closure `f` stored in e.g. `LazyMapIter` is a `Value` (closure handle). When `ctx.call_value()` is called, it re-enters the VM's execute loop. This is already tested and working (see `test_call_value_calls_closure_from_native`).

5. **Memory:** Each lazy adapter stores a `Handle` to the source. The source iterator stays alive as long as the adapter holds it. Cycling through a large iterator and collecting could hold all elements in memory for `LazyCycleIter`'s saved list — documented behavior.

6. **Cycle support for non-foreigns:** `LazyCycleIter` needs to cache elements because it can't replay the source after exhaustion. For array/range iterators, the first pass is cached in `saved`, then repeated.

---

### Estimated Total: ~1,250 lines across ~15 files

| Phase | Lines | Complexity |
|---|---|---|
| 1. Pipe `\|>` | ~80 | Low |
| 2. Partial app `_` | ~50 | Low |
| 3-4. Lazy adapters | ~900 | High (needs careful native fn impl) |
| 5. Terminal ops | ~200 | Medium |
| 6. Docs/tests/bump | ~200 | Medium


BONUS POINTS: add anything else you can think of to make the language's dev experience top tier
