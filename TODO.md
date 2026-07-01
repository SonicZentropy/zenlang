Completed and committed:**
- **Phase 3 — Module system**: `mod` declarations, `use` imports, recursive resolution, multi-file support
- **Phase 4 — String interpolation**: lexer `{`/`}` escape handling, parser desugaring to concat, VM `ToString` encoding, full `.zen` test suite

**In progress:**
- Writing doc comments on new public API items → most already have them, just verifying coverage
- Integrating string interpolation with the LSP → not started yet

**Known issue**: VM `CallMethod` sets `bp = args_start - 1` and `Return` truncates to `bp - 1`, which discards values below the receiver when combining method call results with binary operators. We worked around it in tests by using separate statements.
