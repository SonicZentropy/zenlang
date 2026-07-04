# Formatter

Zen's formatter is built into the LSP via `textDocument/formatting` and `textDocument/rangeFormatting`.

## Features

- Consistent indentation (2 spaces)
- Whitespace around operators and keywords
- Brace placement and alignment
- Line wrapping

The formatter is code-aware, not a simple text reflow — it understands the AST.
