# TODO - Please mark each as completed and make a git commit after each is done.  "Done" in this case means fully tested and documented as well as implemented.

1. ~~Our `parse_pattern` fix was the parser side, but the type checker's exhaustiveness check for custom enums with unit variants (like `Color { Red, Green, Blue }`) still has issues. The `enum_variants.zen` test uses `_` as a workaround. We should verify and properly fix this.~~ **DONE**
2. ~~Fix the enum_variants test to not use a workaround any longer~~ **DONE**

## Missing Tests
3. ~~**No tests for compound assignment** (`+=`, `-=`, etc.)~~ **DONE** - `tests/compound_assignment.zen`
4. ~~**No tests for bitwise operators** (`&`, `|`, `^`, `~`, `<<`, `>>`)~~ **DONE** - `tests/bitwise.zen`
5. ~~**No tests for numeric separators** (`1_000_000`)~~ **DONE** - `tests/numeric_separators.zen`

Missing Features
6. **`const` declarations** — token defined, not implemented
7. **`type` aliases** — token defined, not implemented
8. **`pub` visibility** — parsed but silently ignored

### Tooling
9. **Tree-sitter grammar is significantly outdated** — missing generics, traits, `if let`, `while let`, `?`, spread, shorthand, compound assignment, bitwise ops, etc.

### Fixes
10. Range type returns `Unit`** — `typeck.rs:721` has a TODO. `0..5` works at runtime but has no proper type, which means you can't annotate variables as range types or use them in generics.

### Design Debt
11. **String interning inconsistency** (CompactString vs String) - should use all CompactString i think
12. **Stdlib silently returns Nil on type mismatch** instead of erroring



---

## Current State

**Items 1 & 2 are DONE.**

Changes made across both sessions:
1. **Parser fix** (`parse_pattern`): Uppercase identifiers without parens now parse as `Pattern::EnumVariant` (parser.rs:987-989)
2. **Type checker fix**: Function parameter type shadowing via `remove_from_current_scope` + `define` (typeck.rs:143-146)
3. **Compiler fix**: Removed enum variant names from `register_global_stmt` globals map. Enum constructors are now compiled via `MakeEnum` (not `LoadGlobal` which returned Nil) (compiler.rs:453)
4. **Test cleanup**: `enum_variants.zen` updated to use explicit `MyNone` patterns instead of `_` workarounds. `enums.zen` expanded with comprehensive unit variant tests.
5. **New Rust tests**: `test_unit_variant_exhaustiveness_check` and `test_unit_variant_pattern_matching_compiles_and_runs` (7 sub-tests covering int/string returns, function calls, wildcards, data variants, mixed variants, enum-to-enum matching)

## ~~Plan~~ (Completed)

1. ~~**Update `enum_variants.zen`** to replace all `_` workarounds with explicit unit variant patterns~~
2. ~~**Add exhaustive match test cases** to verify the type checker catches non-exhaustive matches~~
3. ~~**Run all tests** to confirm nothing breaks~~ (153 Rust + 19 .zen all pass)
4. ~~**Mark items 1 & 2 as done** in `TODO.md`~~
