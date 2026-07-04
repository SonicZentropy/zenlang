//! The Zenlang prelude.
//!
//! All iterator adapters (`map`, `filter`, `fold`, `collect`, etc.) are
//! implemented as native Rust functions in `src/stdlib/iter.rs` because
//! creating `ForeignObject`s requires `VM::foreigns.insert()`, which is not
//! possible from Zen code.  The closures are invoked via `ctx.call_value()`.
//!
//! The prelude is a no-op — it exists only as a placeholder for any future
//! built-in functions that need to be written in Zen rather than Rust.

use crate::ast::Program;
use crate::error::Result;

/// Parse the prelude and prepend its top-level declarations to `program`.
///
/// Currently a no-op: all built-in functions are native Rust implementations
/// registered during VM setup.
pub fn inject(program: &mut Program) -> Result<()> {
    let _ = program;
    Ok(())
}
