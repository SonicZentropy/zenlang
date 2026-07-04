# CLI Reference

## `zenc run`

Run a script with optional hot reload.

```bash
zenc run <file>
zenc run                  # Uses zenc.json entry point
zenc run --watch <file>   # Watch for changes, hot reload
```

## `zenc repl`

Start an interactive REPL session with multi-line input detection.

```bash
zenc repl
```

## `zenc check`

Type-check a script without executing it.

```bash
zenc check <file>
```

## `zenc build`

Type-check a project via `zenc.json`.

```bash
zenc build [path]
```

## `zenc test`

Discover and run all `.zen` test files.

```bash
zenc test [paths...]
```

## `zenc disasm`

Dump bytecode with opcodes, source lines, and constants table.

```bash
zenc disasm <file>
```

## `zenc lsp`

Start the LSP language server (stdin/stdout). Used by editors.

```bash
zenc lsp
```

## `zenc dap`

Start the Debug Adapter Protocol server (stdin/stdout).

```bash
zenc dap
```

## `zenc new`

Scaffold a new project.

```bash
zenc new <project_name>
```
