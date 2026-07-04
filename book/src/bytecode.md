# Bytecode and VM Architecture

Zen compiles source code directly to bytecode (no intermediate representation) and executes it on a register-based stack VM.

## VM Design

- **Register-based** — Uses a register array for operands, reducing stack shuffling
- **~50 opcodes** — Compact instruction set
- **Single-pass codegen** — No IR, no optimization passes
- **Source maps** — Every instruction records its source line for debug info

## Execution

```rust
struct Vm {
    registers: Vec<Value>,         // Value stack / register file
    globals: HashMap<String, Value>, // Global variables
    call_stack: Vec<Frame>,        // Function call frames
    ip: usize,                     // Instruction pointer
    instruction_limit: Option<u64>, // Safety limit
}
```

## Instruction Limit

The VM tracks executed instructions and stops when the limit is reached (default: 100,000 if not configured). This prevents infinite loops from freezing the host.

```rust
vm = Vm::with_config(VmConfig { instruction_limit: 50000, .. });
```
