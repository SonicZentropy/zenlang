# Disassembler

The `zenc disasm` command dumps compiled bytecode for inspection.

```bash
zenc disasm script.zen
```

## Output Includes

- **Opcodes** — Each VM instruction with operands
- **Source Lines** — Original source line for each instruction
- **Constants Table** — Literal values used by the script
- **Function Table** — Function signatures and entry points
- **Global Table** — Global variable definitions

This is useful for debugging the compiler and understanding how Zenlang code maps to VM instructions.
