
use compact_str::CompactString;

use crate::span::Spanned;
use crate::token::TokenKind;

// ---------- Types ----------

/// Visibility of a declaration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Vis {
    Public,
    Private,
}

impl Vis {
    pub fn is_pub(&self) -> bool {
        matches!(self, Vis::Public)
    }
}

/// A generic type parameter declaration, e.g. `<T>` or `<T: SomeTrait>`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    pub name: CompactString,
    /// Trait bounds (reserved for future use, Phase 1 — Traits).
    pub bounds: Vec<Type>,
}

/// Represents a type annotation in the source language.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// `i64` — 64-bit signed integer.
    I64,
    /// `f32` — 32-bit float (stored as `f64` at runtime).
    F32,
    /// `f64` — 64-bit float.
    F64,
    /// `bool` — boolean.
    Bool,
    /// `str` — string.
    Str,
    /// `()` — unit type.
    Unit,
    /// `any` — dynamically typed, compatible with everything.
    /// Used as the wildcard for type-erased values, unannotated parameters,
    /// and native function signatures that accept any type.
    Any,
    /// A named type reference, e.g. `MyStruct` or `Option`.
    Named(CompactString),
    /// A generic type parameter, e.g. `T` in `fn foo<T>(x: T)`.
    Generic(CompactString),
    /// `[T]` — homogenous array.
    Array(Box<Type>),
    /// Function type (used for first-class function values).
    Fn { params: Vec<Type>, ret: Box<Type> },
    /// `Option<T>` — legacy type (prefer generic `enum Option`).
    Option(Box<Type>),
    /// `Result<T, E>` — legacy type (prefer generic `enum Result`).
    Result(Box<Type>, Box<Type>),
}

// ---------- Operators ----------

/// Binary operator — arithmetic, comparison, logical, bitwise, assignment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
    Assign,
}

/// Unary operator — negation, logical not, bitwise not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnOp {
    Neg, Not, BitNot,
}

// ---------- Patterns (simplified match) ----------

/// A pattern used in `match` arms and `if let` / `while let` desugaring.
#[derive(Debug, Clone)]
pub enum Pattern {
    /// `_` — matches anything, does not bind.
    Wildcard,
    /// `name` — matches anything, binds to `name`.
    Ident(CompactString),
    /// Literal integer pattern.
    Int(i64),
    /// Literal float pattern.
    Float(f64),
    /// Literal string pattern.
    Str(CompactString),
    /// Literal boolean pattern.
    Bool(bool),
    /// Enum variant destructuring: `Some(val)` or `O::Some(val)`.
    EnumVariant { enum_name: Option<CompactString>, variant_name: CompactString, bindings: Vec<CompactString> },
}

// ---------- Statements ----------

/// Opaque identifier for AST nodes, used by the type checker's `TypeMap`.
pub type NodeId = usize;

/// A function parameter with optional type annotation.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: CompactString,
    pub type_ann: Option<Type>,
}

/// A named field in a struct declaration.
#[derive(Debug, Clone)]
pub struct StructField {
    pub name: CompactString,
    pub type_ann: Type,
}

/// A variant in an enum declaration.
#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: CompactString,
    pub fields: Vec<Type>,
}

/// A single arm in a `match` expression.
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    /// Optional guard expression (reserved, not yet implemented).
    pub guard: Option<Box<Expr>>,
    pub body: Box<Expr>,
}

/// A top-level or nested statement in the AST.
#[derive(Debug, Clone)]
pub enum Stmt {
    /// `let [mut] name [: type] = expr;`
    Let {
        mutable: bool,
        name: CompactString,
        type_ann: Option<Type>,
        init: Option<Expr>,
    },
    /// `[pub] const name [: type] = expr;`
    Const {
        vis: Vis,
        name: CompactString,
        type_ann: Option<Type>,
        init: Expr,
    },
    /// `[pub] type Name<T> = AliasType;`
    Type {
        vis: Vis,
        name: CompactString,
        type_params: Vec<TypeParam>,
        alias: Type,
    },
    /// A standalone expression statement.
    Expr(Expr),
    /// `return [expr];`
    Return(Option<Expr>),
    /// `[pub] fn name<T, U>(params) [: ret_type] { body }`
    Fn {
        vis: Vis,
        name: CompactString,
        type_params: Vec<TypeParam>,
        params: Vec<Param>,
        return_type: Option<Type>,
        body: Vec<Spanned<Stmt>>,
    },
    /// `[pub] struct Name<T> { field: type, ... }`
    Struct {
        vis: Vis,
        name: CompactString,
        type_params: Vec<TypeParam>,
        fields: Vec<StructField>,
    },
    /// `[pub] enum Name<T> { Variant(fields...), ... }`
    Enum {
        vis: Vis,
        name: CompactString,
        type_params: Vec<TypeParam>,
        variants: Vec<EnumVariant>,
    },
    /// `impl<T> TypeName { methods }` or `impl<T> TraitName for TypeName { methods }`
    Impl {
        type_name: CompactString,
        type_params: Vec<TypeParam>,
        trait_name: Option<CompactString>,
        methods: Vec<Spanned<Stmt>>,
    },
    /// `[pub] trait Name<T> { fn method(...) -> Type; ... }`
    /// Method bodies are empty (`vec![]`) — trait methods are signatures only.
    Trait {
        vis: Vis,
        name: CompactString,
        type_params: Vec<TypeParam>,
        methods: Vec<Spanned<Stmt>>,
    },
    /// `[pub] use path::to::item;`
    Use {
        vis: Vis,
        path: Vec<CompactString>,
    },
    /// `[pub] mod name { ... }`
    Mod {
        vis: Vis,
        name: CompactString,
        body: Vec<Spanned<Stmt>>,
    },
}

// ---------- Expressions ----------

/// An expression node in the AST.
#[derive(Debug, Clone)]
pub enum Expr {
    // Literals
    /// Integer literal, e.g. `42`.
    Int(i64),
    /// Float literal, e.g. `3.14`.
    Float(f64),
    /// String literal, e.g. `"hello"`.
    Str(CompactString),
    /// Boolean literal: `true` or `false`.
    Bool(bool),
    /// Unit literal: `()`.
    Unit,

    // References
    /// Identifier reference, e.g. `x` or `my_function`.
    Ident(CompactString),

    // Operators
    /// Binary operation: `a + b`, `x == y`, etc.
    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    /// Unary operation: `-x`, `!flag`, `~bits`.
    Unary { op: UnOp, expr: Box<Expr> },

    // Call & Access
    /// Function/constructor call: `foo(a, b)`.
    Call { func: Box<Expr>, args: Vec<Expr> },
    /// Method call: `obj.method(a, b)`.
    MethodCall { obj: Box<Expr>, method: CompactString, args: Vec<Expr> },
    /// Field access: `obj.field`.
    Field { obj: Box<Expr>, field: CompactString },
    /// Index access: `arr[i]`.
    Index { obj: Box<Expr>, index: Box<Expr> },

    // Blocks & Control Flow
    /// A block expression: `{ stmts; expr }`.
    Block(Vec<Spanned<Stmt>>),
    /// If expression: `if cond { then } else { else_ }`.
    If { cond: Box<Expr>, then: Box<Expr>, else_: Option<Box<Expr>> },
    /// While loop: `while cond { body }`.
    While { cond: Box<Expr>, body: Box<Expr> },
    /// For loop: `for var in iter { body }`.
    For { var: CompactString, iter: Box<Expr>, body: Box<Expr> },
    /// Infinite loop: `loop { body }`.
    Loop(Box<Expr>),

    // Match
    /// Match expression: `match expr { pattern => body, ... }`.
    Match { expr: Box<Expr>, arms: Vec<MatchArm> },

    // Return / Break / Continue / Yield
    Return(Option<Box<Expr>>),
    Break,
    Continue,
    /// Yield expression: `yield value` — produces a value from a generator.
    Yield(Box<Expr>),

    // Data literals
    /// Struct literal: `Point { x: 1, y: 2 }` or `Point { x, y }` or `Point { ..base }`.
    StructLit { name: CompactString, fields: Vec<(CompactString, Expr)>, spread: Option<Box<Expr>> },
    /// Array literal: `[1, 2, 3]`.
    Array(Vec<Expr>),
    /// Range literal: `0..5` or `0..=5`.
    Range { start: Box<Expr>, end: Box<Expr>, inclusive: bool },

    /// Lambda/closure: `\|x, y\| x + y`.
    Lambda { params: Vec<Param>, return_type: Option<Type>, body: Box<Expr> },
    /// Qualified enum variant constructor: `O::Some(x)` or `Option::None`.
    EnumCtor { enum_name: CompactString, variant_name: CompactString, args: Vec<Expr> },
}

// ---------- Program ----------

/// A complete Zenlang program — a sequence of top-level statements.
#[derive(Debug, Clone)]
pub struct Program {
    pub stmts: Vec<Spanned<Stmt>>,
}

impl Program {
    /// Create an empty program.
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
