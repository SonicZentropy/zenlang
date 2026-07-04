▼Todo
[•] Explore the type system and type checker architecture
[ ] Add `any` / `dynamic` keyword to lexer and parser
[ ] Add `Type::Any` variant to the AST type system
[ ] Make the type checker accept `any` in all positions without errors
[ ] Ensure runtime naturally errors on type mismatches
[ ] Add test coverage
[ ] Commit

CURRENT THOUGHTS:
Type::Any already exists and is fully handled in the type checker — this is much simpler than I expected. Let me verify the remaining details:

# Todos

[✓] Explore the type system and type checker architecture
[✓] Add `any` keyword to parser's parse_type()
[✓] Verify Type::Any is handled in all typeck paths
[✓] Handle unannotated let/param defaults to Any
[✓] Test and commit

- Thought: 677ms
Let me check:

1. The exact current state of parse_type() to know where to add "any"
2. How unannotated let statements and function params currently work
3. What check_stmt does with let bindings that have no type annotation
→Read src\parser.rs [offset=1410, limit=85]
→Read src\typeck.rs [offset=200, limit=150]

- Thought: 4.2s
Now I can see the full picture:

1. parse_type() in parser.rs — needs a new branch for "any" keyword. Currently any falls through to Type::Named("any").

2. check_stmt in typeck.rs:
   - Stmt::Let (line 210-248): When type_ann is None, it infers from the initializer (line 232-233). When it's present, it unifies the init type with the annotation (line 224). With any, unify already handles this since types_compatible(any, _) == true.
   - Stmt::Fn params (line 312): Unannotated params default to Type::Any already.
   - Return type mismatch (line 274-281): Uses types_compatible, so any return type would accept anything.

But there's an issue: when a let has a type annotation and the user wants to OPT OUT of type checking, they'd write let x: any = ... — but currently the type checker would unify the init type with any (which works fine since Any is compatible with everything).

However, there's another issue: what about unannotated let bindings? Currently, if you write let x = 42;, the type checker infers x: i64. If the user then does x = "hello";, it would fail. But the user might want unannotated lets to behave like TypeScript's inferred types. For minimal viability, I think the current behavior is fine — any keyword is the opt-out mechanism.

The key changes needed:

1. parser.rs parse_type(): Add "any" branch
2. Verify Type::Any works with all the check_expr branches — let me check the remaining type checker code

Let me also look at the types_compatible and unify functions more carefully:
→Read src\typeck.rs [offset=1190, limit=100]

- Thought: 2.3s
types_compatible already handles Any at line 1205. And I can see that resolve_named at line 1251 doesn't have "any" → Type::Any. It has "int" → Type::I64, "float" → Type::F64, etc. If the parser parses any as Type::Named("any") (which is what happens currently since there's no special case for "any" in parse_type), then resolve_named would see Named("any") and pass it through as Named("any"), not Any. Then types_compatible(Named("any"),_) would not match (Type::Any, _) — it would try to find it as a struct/type alias name and potentially error.

So the fix is clear:

1. Add "any" to parse_type() → return Type::Any
2. That's it! Since types_compatible already handles (Type::Any,_) → true, and unify handles it too

Let me also verify that unify handles Any correctly:
→Read src\typeck.rs [offset=136, limit=65]
