# Cargo Features and Setup

## Adding Zen as a Dependency

```toml
[dependencies]
zenlang = { git = "https://github.com/SonicZentropy/zenlang" }
```

## Feature Selection

The `zenlang` crate has optional features:

| Feature | Description | Default |
|---------|-------------|---------|
| `cli` | CLI binary (`zenc`) | on |
| `fs` | File I/O stdlib module | on |
| `json` | JSON stdlib module | on |
| `lsp` | LSP server | on |
| `dap` | Debug Adapter Protocol | on |
| `lualite` | LuaLite backend (alternative) | off |

## Disable Features to Minimize Size

```toml
[dependencies]
zenlang = { git = "...", default-features = false, features = ["cli"] }
```

This disables LSP, DAP, JSON, and file I/O, giving a smaller binary.
