# Known issues

- Unit-variant enum patterns (e.g., `match x { Red => ... }`) are parsed as variable bindings, not variant tests, because `Pattern::Ident` is catch-all. Data variants via `Some(v)` work correctly. To match unit variants, use a wildcard fallthrough arm for the "else" case.
