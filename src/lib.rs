pub mod ast;
pub mod compiler;
pub mod error;
pub mod formatter;
pub mod hotreload;
pub mod interop;
pub mod ir;
pub mod lexer;
pub mod lsp;
pub mod mod_resolver;
pub mod parser;
pub mod parser_test;
pub mod prelude;
pub mod resolver;
pub mod span;
pub mod stdlib;
pub mod symbol;
pub mod token;
pub mod typeck;
pub mod value;
pub mod vm;

pub use error::{Error, Result};
pub use span::{SourceLocation, Span, Spanned};
pub use token::Token;
pub use value::Value;
pub use vm::VM;

/// Initialise tracing with sensible defaults for an embedded scripting language.
pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "zenlang=info".into()),
        )
        .init();
}
