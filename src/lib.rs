pub mod error;
pub mod span;
pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod symbol;
pub mod resolver;
pub mod typeck;
pub mod ir;
pub mod compiler;
pub mod value;
pub mod vm;
pub mod interop;
pub mod hotreload;
pub mod lsp;
pub mod formatter;
pub mod stdlib;
pub mod parser_test;

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
