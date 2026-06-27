use crate::span::SourceLocation;
use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("parse error at {location}: {msg}"))]
    Parse { location: SourceLocation, msg: String },

    #[snafu(display("type error at {location}: {msg}"))]
    TypeError { location: SourceLocation, msg: String },

    #[snafu(display("resolution error at {location}: {msg}"))]
    Resolve { location: SourceLocation, msg: String },

    #[snafu(display("runtime error: {msg}"))]
    Runtime { msg: String, stack_trace: Vec<SourceLocation> },

    #[snafu(display("I/O error: {source}"))]
    Io { source: std::io::Error },

    #[snafu(display("script error: {msg}"))]
    Script { msg: String },

    #[snafu(display("compile error at {location}: {msg}"))]
    Compile { location: SourceLocation, msg: String },

    #[snafu(display("multiple compile errors"))]
    CompileMultiple { errors: Vec<Error> },

    #[snafu(display("multiple parse errors"))]
    ParseMultiple { errors: Vec<Error> },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
