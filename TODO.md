# To tackle in order.  After completing each task, mark it as done.

. Here are the most impactful Rust/Zenlang interop and QoL features I'd recommend, roughly in priority order:

**1. JSON serialization (`to_json`/`from_json`)**
- `serde_json` is already a dependency (used for LSP/DAP)
- Implement `Serialize`/`Deserialize` for `Value`, `MapKey`, etc.
- Expose as native functions: `to_json(value)` → `str`, `from_json(str)` → `Value`
- Enables data persistence, config files, network communication
- Scope: contained — implement traits + 2 native functions

**2. Closure callbacks from native functions**
- Currently all iterator adapters (`map`, `filter`, `fold`) are written in Zenlang because native Rust functions can't call script closures
- `VM::call_value()` already exists — what's missing is a safe method on `VMContext`
- Would allow prelude functions to be moved to Rust (performance) and enable rich callback-based APIs
- Scope: moderate — needs reentrancy safety in the VM

**3. `ForeignObject::clone` implementation**
- Currently `unimplemented!()` — cloning a foreign value crashes at runtime
- Requires type-specific cloning via `Box<dyn Clone>` or a `Clone` trait object on `ForeignObject`
- Scope: small but enables cloning any value tree containing foreign types

**4. Auto-register constructors in `#[zen_methods]`**
- Currently `new()` and other associated functions (`Self` return, no `&self`) are NOT registered
- Must manually register them as native functions
- Scope: moderate — needs macro to detect `Self` return type and generate native fn registration

**5. `TryFrom<Value>` impls for Rust types**
- Currently manual `.as_int()`, `.as_float()`, etc. everywhere
- Standard `TryFrom<Value> for i64`, `String`, `f64`, `bool` would make interop code cleaner
- Could also add `From<T>` for constructing `Value` from common Rust types
- Scope: small

**6. Easier `Value::Struct` construction from Rust (builder API)**
- Currently must manually create `StructData { values, field_names }`
- A builder like `StructBuilder::new("Point").field("x", 10).field("y", 20).build()` would be cleaner
- Scope: small

**7. Compile-time type checking for native function signatures**
- Currently `native_fn_sigs()` uses `Type::Unit` as wildcard — no type safety
- Could add a proc macro to generate signatures from native function definitions
- Scope: moderate

Any of these catch your eye? I'd personally start with **JSON serialization** — it's contained, `serde_json` is already available, and it unlocks real-world usage (save/load game state, config files, network messaging).
