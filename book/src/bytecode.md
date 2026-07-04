# Bytecode and VM Architecture

Zen compiles source code directly to bytecode (no intermediate representation)
and executes it on a register-based stack VM.

## VM Design

- **Stack-based** — Local variables, temporaries, and call arguments live on
  a `Vec<Value>` stack.
- **Globals** — Indexed by name, stored in a `Vec<Value>` parallel to
  `global_names`.
- **~50 opcodes** — Compact instruction set: constants, arithmetic, jumps,
  locals/globals, calls, closures, generators, foreign field access.
- **Single-pass codegen** — No IR, no optimization passes.
- **Source maps** — Every instruction records its source line for debug info.
- **Closures** — `Value::Function(Rc<BytecodeFn>)` with upvalue capture;
  recursive references handled via index remapping in `reload_functions()`.
- **Foreign interop** — `Value::Foreign(Rc<RefCell<ForeignObject>>)` with
  name-based field/method dispatch through string tables in the chunk.

## Key Data Structures

| Component | Type | Purpose |
|-----------|------|---------|
| Stack | `Vec<Value>` | Locals, temporaries, call args |
| Globals | `Vec<Value>` | Runtime values of global variables |
| Functions | `Vec<BytecodeFn>` | Loaded bytecode functions |
| Frames | `Vec<CallFrame>` | `{ function_idx, ip, bp }` per call |
| Slabs | `Slab<T>` | Handles for arrays, structs, enums, maps, closures, generators, foreign objects |

## Instruction Limit

The VM tracks executed instructions and stops when the limit is reached
(default: no limit). This prevents infinite loops from freezing the host.

```rust
vm.set_instruction_limit(100_000);
```

## Disassembly

```rust
vm.disassemble();
```

Outputs the constant pool and opcodes with source lines for every function:
```text
=== main ===
-- Constants --
[0] "hello"
-- Bytecode --
0000:    1  OpConstant        0
0002:    1  OpPrint
0003:    1  OpNil
0004:    1  OpReturn
```
