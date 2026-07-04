# Compiler Pipeline

```
Source Code (.zen)
    │
    ▼
┌─────────────┐
│   Lexer     │  Tokenizes source into token stream
└──────┬──────┘
       │
       ▼
┌─────────────┐
│   Parser    │  Recursive descent, Pratt for expressions
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Type Check  │  Structural type checking, inference
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Bytecode    │  Single-pass code generation
└──────┬──────┘
       │
       ▼
     VM Execution
```

## Key Design Decisions

- **Recursive descent parser** — Hand-written, no parser generator
- **Pratt parsing** — For expression precedence handling
- **Single-pass codegen** — The compiler walks the AST once and emits bytecode
- **No IR** — Skips intermediate representation for simplicity
- **No optimization** — The bytecode is a straightforward translation of the AST
