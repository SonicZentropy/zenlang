//! The Zenlang prelude: a small set of standard functions written in
//! Zenlang itself (see `prelude.zen`), rather than as native Rust functions.
//!
//! Iterator adapters like `map`/`filter`/`fold` need to call back into a
//! script-provided closure for each element. Native Rust functions have no
//! way to do that (`VMContext` doesn't hold a handle back into the running
//! VM), but Zenlang closures can already call each other directly — so
//! writing these in Zenlang sidesteps the limitation entirely.
//!
//! Callers that build and run a `Program` (the CLI, the REPL, hot reload)
//! should call [`inject`] right after parsing so the prelude's functions
//! are visible to the rest of the pipeline (resolver, typeck, compiler)
//! exactly like any other top-level function in the script.

use crate::ast::Program;
use crate::error::Result;
use crate::lexer::Lexer;
use crate::parser::Parser;

/// Source of the built-in prelude functions (`map`, `filter`, `fold`,
/// `enumerate`, `take`, `zip`, `collect`).
pub const SOURCE: &str = include_str!("prelude.zen");

/// Parse the prelude and prepend its top-level declarations to `program`.
///
/// Prepending (rather than appending) means a script can still shadow a
/// prelude function by declaring its own top-level function with the same
/// name — the resolver reports the more useful "already defined" error at
/// the user's declaration, not the prelude's.
pub fn inject(program: &mut Program) -> Result<()> {
    let tokens = Lexer::new(SOURCE).tokenize()?;
    let prelude_program = Parser::new(SOURCE, &tokens).parse()?;
    let mut stmts = prelude_program.stmts;
    stmts.append(&mut program.stmts);
    program.stmts = stmts;
    Ok(())
}
