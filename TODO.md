# To tackle in order.  After completing each task, mark it as done.

Would JIT help? Yes — but there are caveats.

### What JIT would buy you

1. **10–50× on numeric loops** — The interpreter currently dispatches on 24 `Value` variants for every arithmetic op. A tracing JIT (LuaJIT-style) would observe "these are always `f64`" and emit native `addsd`/`mulsd`, skipping boxing, dispatch, and refcount traffic entirely.

2. **2–4× on general code** — A simpler method JIT (compile whole functions via Cranelift) eliminates bytecode dispatch overhead. That's decent but unspectacular.

3. **Inlining** — The interpreter has full frame-setup/teardown on every call. A JIT can inline small functions, which the current architecture cannot.

### The hard parts

| Problem | Why it's harder than LuaJIT |
|---|---|
| **24-value `Value` enum** | LuaJIT has only **2** type tags (number vs. GC object) with NaN-tagging. Zenlang needs 5+ bits just to discriminate variants. Type guards are fatter, side exits are more frequent, and trace recording is more fragile. |
| **`Rc<RefCell<>>` everywhere** | Reference counting and runtime borrow-checking dominate execution time. A JIT can't optimize these away — the semantics must be preserved. Hot paths that touch shared mutable state won't speed up much. |
| **No existing IR** | You'd need to build a CFG + SSA IR from bytecode or from the AST + TypeMap. This is non-trivial. |
| **Foreign function bridge** | All ~40 native functions take `&mut VMContext` and can mutate any VM state. The JIT must either emit a call to them (preserving the VM contract) or know the semantics of specific natives. |

### What I'd recommend in priority order

| Approach | Effort | Speedup | Best for |
|---|---|---|---|
| **1. Faster interpreter** (direct threading, better inline caching) | weeks | ~1.5–2× | General code |
| **2. Cranelift method JIT** (compile bytecode functions to native, remove dispatch) | 1–2 months | ~2–4× | General code |
| **3. Tracing JIT** (LuaJIT-style, profile + specialize) | 6+ months | ~10–50× on numeric code | Game/math-heavy scripts |

**If I were building this**, I'd start with a Cranelift-based method JIT (option 2). It's the best effort-to-reward ratio: Cranelift exists, is Rust-native, and handles register allocation and codegen for you. You'd decompile `BytecodeFn` → Cranelift IR → native. This removes the dispatch loop and gives the CPU's frontend a break.

The tracing JIT (option 3) would give bigger headline numbers but requires fundamentally redesigning `Value` (NaN-tagging at minimum) and months of work. Save it for later.

**Bottom line**: Zenlang's bytecode is actually a pretty good target for JIT — it's clean, simple, and small (~50 ops). The value representation (`Rc<RefCell<24-variant-enum>`) is the real pain point, not the bytecode format.
