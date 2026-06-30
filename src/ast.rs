
use compact_str::CompactString;

use crate::span::Spanned;
use crate::token::TokenKind;

// ---------- Types ----------

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I64,
    F32,
    F64,
    Bool,
    Str,
    Unit,
    Named(CompactString),
    Array(Box<Type>),
    Fn { params: Vec<Type>, ret: Box<Type> },
}

// ---------- Operators ----------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Assign,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
}

// ---------- Patterns (simplified match) ----------

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard,
    Ident(CompactString),
    Int(i64),
    Float(f64),
    Str(CompactString),
    Bool(bool),
    EnumVariant { variant_name: CompactString, bindings: Vec<CompactString> },
}

// ---------- Statements ----------

pub type NodeId = usize;

#[derive(Debug, Clone)]
pub struct Param {
    pub name: CompactString,
    pub type_ann: Option<Type>,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: CompactString,
    pub type_ann: Type,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: CompactString,
    pub fields: Vec<Type>,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Box<Expr>>,
    pub body: Box<Expr>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        mutable: bool,
        name: CompactString,
        type_ann: Option<Type>,
        init: Option<Expr>,
    },
    Expr(Expr),
    Return(Option<Expr>),
    Fn {
        name: CompactString,
        params: Vec<Param>,
        return_type: Option<Type>,
        body: Vec<Spanned<Stmt>>,
    },
    Struct {
        name: CompactString,
        fields: Vec<StructField>,
    },
    Enum {
        name: CompactString,
        variants: Vec<EnumVariant>,
    },
    Impl {
        type_name: CompactString,
        methods: Vec<Spanned<Stmt>>,
    },
    Use {
        path: Vec<CompactString>,
    },
    Mod {
        name: CompactString,
        body: Vec<Spanned<Stmt>>,
    },
}

// ---------- Expressions ----------

#[derive(Debug, Clone)]
pub enum Expr {
    // Literals
    Int(i64),
    Float(f64),
    Str(CompactString),
    Bool(bool),
    Unit,

    // References
    Ident(CompactString),

    // Operators
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: UnOp,
        expr: Box<Expr>,
    },

    // Call & Access
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
    },
    MethodCall {
        obj: Box<Expr>,
        method: CompactString,
        args: Vec<Expr>,
    },
    Field {
        obj: Box<Expr>,
        field: CompactString,
    },
    Index {
        obj: Box<Expr>,
        index: Box<Expr>,
    },

    // Blocks & Control Flow
    Block(Vec<Spanned<Stmt>>),
    If {
        cond: Box<Expr>,
        then: Box<Expr>,
        else_: Option<Box<Expr>>,
    },
    While {
        cond: Box<Expr>,
        body: Box<Expr>,
    },
    For {
        var: CompactString,
        iter: Box<Expr>,
        body: Box<Expr>,
    },
    Loop(Box<Expr>),

    // Match
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
    },

    // Return / Break / Continue
    Return(Option<Box<Expr>>),
    Break,
    Continue,

    // Data literals
    StructLit {
        name: CompactString,
        fields: Vec<(CompactString, Expr)>,
    },
    Array(Vec<Expr>),
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
    },

    // Lambda (infrastructure for post-MVP closures)
    Lambda {
        params: Vec<Param>,
        return_type: Option<Type>,
        body: Box<Expr>,
    },
}

// ---------- Program ----------

#[derive(Debug, Clone)]
pub struct Program {
    pub stmts: Vec<Spanned<Stmt>>,
}

impl Program {
    pub fn new() -> Self {
        Self { stmts: Vec::new() }
    }
}

// ---------- Helpers ----------

impl BinOp {
    pub fn from_token(kind: &TokenKind) -> Option<BinOp> {
        match kind {
            TokenKind::Plus => Some(BinOp::Add),
            TokenKind::Minus => Some(BinOp::Sub),
            TokenKind::Star => Some(BinOp::Mul),
            TokenKind::Slash => Some(BinOp::Div),
            TokenKind::Percent => Some(BinOp::Mod),
            TokenKind::EqEq => Some(BinOp::Eq),
            TokenKind::Ne => Some(BinOp::Ne),
            TokenKind::Lt => Some(BinOp::Lt),
            TokenKind::Le => Some(BinOp::Le),
            TokenKind::Gt => Some(BinOp::Gt),
            TokenKind::Ge => Some(BinOp::Ge),
            TokenKind::AndAnd => Some(BinOp::And),
            TokenKind::OrOr => Some(BinOp::Or),
            TokenKind::And => Some(BinOp::BitAnd),
            TokenKind::Or => Some(BinOp::BitOr),
            TokenKind::Caret => Some(BinOp::BitXor),
            TokenKind::Shl => Some(BinOp::Shl),
            TokenKind::Shr => Some(BinOp::Shr),
            TokenKind::Eq => Some(BinOp::Assign),
            _ => None,
        }
    }
}

impl UnOp {
    pub fn from_token(kind: &TokenKind) -> Option<UnOp> {
        match kind {
            TokenKind::Minus => Some(UnOp::Neg),
            TokenKind::Bang => Some(UnOp::Not),
            TokenKind::Tilde => Some(UnOp::BitNot),
            _ => None,
        }
    }
}
