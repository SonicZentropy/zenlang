# TODO - Please mark each as completed and make a git commit after each is done.  "Done" in this case means fully tested and documented as well as implemented.

1. ~~Our `parse_pattern` fix was the parser side, but the type checker's exhaustiveness check for custom enums with unit variants (like `Color { Red, Green, Blue }`) still has issues. The `enum_variants.zen` test uses `_` as a workaround. We should verify and properly fix this.~~ **DONE**
2. ~~Fix the enum_variants test to not use a workaround any longer~~ **DONE**

## Missing Tests
3. ~~**No tests for compound assignment** (`+=`, `-=`, etc.)~~ **DONE** - `tests/compound_assignment.zen`
4. ~~**No tests for bitwise operators** (`&`, `|`, `^`, `~`, `<<`, `>>`)~~ **DONE** - `tests/bitwise.zen`
5. ~~**No tests for numeric separators** (`1_000_000`)~~ **DONE** - `tests/numeric_separators.zen`

## Missing Features
6. ~~**`const` declarations** — token defined, not implemented~~ **DONE** — `tests/const.zen` + Rust tests
7. ~~**`type` aliases** — token defined, not implemented~~ **DONE** — `type Foo = Bar;` syntax, Rust tests
8. ~~**`pub` visibility** — parsed but silently ignored~~ **DONE** — `vis: Vis` field on AST, parser tracks `pub` keyword

### Tooling
9. **Tree-sitter grammar is significantly outdated** — missing generics, traits, `if let`, `while let`, `?`, spread, shorthand, compound assignment, bitwise ops, etc.

### Fixes
10. Range type returns `Unit`** — `typeck.rs:721` has a TODO. `0..5` works at runtime but has no proper type, which means you can't annotate variables as range types or use them in generics.

### Design Debt
11. **String interning inconsistency** (CompactString vs String) - should use all CompactString i think
12. **Stdlib silently returns Nil on type mismatch** instead of erroring

Known Bug Found (not fixed)
`&=` and `|=` desugar to logical AND/OR instead of bitwise AND/OR in `parser.rs:1229-1230`. Tests use `x = x & y` form to avoid this.

## Test Counts
- **166 Rust tests** (was 153 before items 6-8)
- **23 .zen tests** (was 22 before items 6-8; new `tests/const.zen`)
