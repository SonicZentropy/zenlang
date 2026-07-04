# LSP Server

Zen includes a Language Server Protocol implementation. Start it with:

```bash
zenc lsp
```

## Features

- **Diagnostics** — Real-time type errors, syntax errors, and warnings
- **Completion** — Code completion for keywords, types, and identifiers
- **Hover** — Type information on hover
- **Go to Definition** — Navigate to function/type definitions
- **References** — Find all references to a symbol
- **Document Symbols** — Outline of functions, structs, enums, traits
- **Inlay Hints** — Type hints for inferred variables

## Editor Configuration

### VS Code

Create `.vscode/settings.json`:

```json
{
    "zenlang.lsp.path": "zenc",
    "zenlang.lsp.args": ["lsp"]
}
```

### Zed

Use the [Zed extension](zed-extension.md) which ships with LSP support built in.
