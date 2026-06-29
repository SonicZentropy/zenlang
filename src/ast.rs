
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
    Named(String),
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
    Assign,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}

// ---------- Patterns (simplified match) ----------

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard,
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

// ---------- Statements ----------

pub type NodeId = usize;

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub type_ann: Option<Type>,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub type_ann: Type,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
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
        name: String,
        type_ann: Option<Type>,
        init: Option<Expr>,
    },
    Expr(Expr),
    Return(Option<Expr>),
    Fn {
        name: String,
        params: Vec<Param>,
        return_type: Option<Type>,
        body: Vec<Spanned<Stmt>>,
    },
    Struct {
        name: String,
        fields: Vec<StructField>,
    },
    Enum {
        name: String,
        variants: Vec<EnumVariant>,
    },
    Impl {
        type_name: String,
        methods: Vec<Spanned<Stmt>>,
    },
}

// ---------- Expressions ----------

#[derive(Debug, Clone)]
pub enum Expr {
    // Literals
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Unit,

    // References
    Ident(String),

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
        method: String,
        args: Vec<Expr>,
    },
    Field {
        obj: Box<Expr>,
        field: String,
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
        var: String,
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
        name: String,
        fields: Vec<(String, Expr)>,
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
            _ => None,
        }
    }
}
