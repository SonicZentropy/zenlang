# To tackle in order.  After completing each task, mark it as done.

### Phase 1: `Type::Any` split (~1 day)
Add `Type::Any` variant, replace wildcard `Unit` usages, clean up `types_compatible`.

### Phase 2: Structural typing + `opaque type` (~2-3 weeks)
- **Structural by default**: `types_compatible` falls through to `structurally_compatible(a, b)` when names differ and neither is opaque
- **Width subtyping**: extra fields in provided type OK, missing fields in provided type fail
- **Excess property checks**: struct *literals* with unknown field names → compile error
- **`opaque type Name = Base`**: creates a nominally isolated type — NOT compatible with `Base` or any other type. Name-matching only. Requires explicit conversion both ways.
- **Foreign type registration**: Rust foreign structs register their field types in the symbol table so they participate in structural comparison

### Phase 3: Local bidirectional inference (~2-3 weeks)
- `Type::Var(u64)` for local unification
- `unify()` + `resolve()` in the typechecker
- Expected-type propagation downward from context
- Let-binding, lambda, and generic call-site inference

### Phase 4: `unknown` + narrowing (~1-2 weeks)
- `Type::Unknown` — safe top type, no implicit compatibility
- Field access / method call on `unknown` → compile error
- Narrowing through match patterns and casts

please formalize all 4 phases as actionable implementation plan
