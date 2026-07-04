// Internal compiler pipeline — hidden from public API.
pub(crate) mod ast;
pub(crate) mod compiler;
pub(crate) mod ir;
pub(crate) mod lexer;
pub(crate) mod mod_resolver;
pub(crate) mod parser;
#[cfg(test)]
pub(crate) mod parser_test;
pub(crate) mod prelude;
pub(crate) mod resolver;
pub(crate) mod slab;
pub(crate) mod symbol;
pub(crate) mod token;
pub(crate) mod typeck;

// Public API modules
pub mod dap;
pub mod error;
pub mod formatter;
pub mod hotreload;
pub mod interop;
pub mod lsp;
pub mod span;
pub mod stdlib;
pub mod value;
pub mod vm;

pub use error::{Error, Result};
pub use span::{SourceLocation, Span, Spanned};
pub use value::Value;
pub use vm::{CompileConfig, VM};
pub use zenlang_macros::ZenForeign;
pub use zenlang_macros::foreign_type;
pub use zenlang_macros::zen_methods;
pub use zenlang_macros::zen_native_fn;

/// One-shot: create a temporary VM, compile, execute, and return the result.
///
/// Useful for quick scripts that don't need persistent state or registered natives.
pub fn run(source: &str) -> Result<Value> {
    let mut vm = VM::new();
    vm.exec(source)
}

/// Initialise tracing with sensible defaults for an embedded scripting language.
pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "zenlang=info".into()),
        )
        .init();
}
