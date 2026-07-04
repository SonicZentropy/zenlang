use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span(pub usize, pub usize);

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self(start, end)
    }

    pub fn start(&self) -> usize {
        self.0
    }

    pub fn end(&self) -> usize {
        self.1
    }

    pub fn merge(&self, other: &Self) -> Self {
        Self(self.0.min(other.0), self.1.max(other.1))
    }
}

#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub file: Option<PathBuf>,
    pub span: Span,
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    pub fn new(file: Option<PathBuf>, span: Span, line: usize, column: usize) -> Self {
        Self {
            file,
            span,
            line,
            column,
        }
    }
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.file {
            Some(path) => write!(f, "{}:{}:{}", path.display(), self.line, self.column),
            None => write!(f, "{}:{}", self.line, self.column),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}
