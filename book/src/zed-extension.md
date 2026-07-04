# Zed Editor Extension

A Zed extension for Zen is available at `extensions/zenlang/` in the repository.

## Features

- **Syntax Highlighting** — Via tree-sitter grammar (compiled to WASM)
- **LSP Integration** — Diagnostics, completions, hover, go-to-definition
- **Run Commands** — `zenlang: run` to execute the current file
- **Format on Save** — Via the built-in formatter

## Installation

1. Copy `extensions/zenlang/` to `~/.config/zed/extensions/`
2. Restart Zed
3. Open a `.zen` file

## Configuration

The extension looks for `zenc` on your PATH. You can configure a custom path in Zed settings:

```json
{
    "zenlang": {
        "binary": {
            "path": "zenc",
            "arguments": ["lsp"]
        }
    }
}
```
