# Debug Adapter Protocol

Zenlang supports the Debug Adapter Protocol for step-through debugging.

```bash
zenc dap
```

## Features

- **Breakpoints** — Set, remove, enable, disable breakpoints by line
- **Step Over / Into / Out** — Navigate execution
- **Stack Traces** — Call stack with source locations
- **Variables** — Inspect local and global variables
- **Scopes** — View variable scopes hierarchically
- **Pause** — Pause execution
- **Continue** — Resume execution

## Source Maps

Breakpoints work by source line. The bytecode VM maintains source location information that maps bytecode offsets back to source positions.
