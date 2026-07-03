# All items complete ✅

## Summary of changes

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
