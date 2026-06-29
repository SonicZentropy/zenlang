# Zenlang Zed Extension

Provides [Zenlang](https://github.com/your-org/zenlang) support for Zed:

- Syntax highlighting (Tree-sitter)
- LSP integration: diagnostics, completions, hover, document symbols, semantic tokens
- Bracket matching, auto-indentation, code outline

## Prerequisites

- The `zenc` binary must be on your `PATH`:
  ```
  cargo install --path /path/to/zenlang
  ```

## Installation (Development)

1. Open Zed's extensions view (`zed: extensions`)
2. Click **Install Dev Extension**
3. Select the `zed-extension/` directory
4. Open any `.zen` file

## Building the Tree-sitter Grammar

If you modify `grammars/zenlang/grammar.js`, rebuild the grammar:

```
npm install -g tree-sitter-cli
cd grammars/zenlang
tree-sitter generate
```

The compiled grammar is loaded from a published git repository when the
extension is installed via the marketplace. During development, use a
`file://` URL in `extension.toml` to reference the local grammar.
