use crate::ast::*;
use crate::error::{Error, Result};
use crate::span::{SourceLocation, Span, Spanned};
use crate::token::{Token, TokenKind};

/// Precedence levels for Pratt parsing.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
enum Precedence {
    Lowest,
    Assign,   // =
    Or,       // ||
    And,      // &&
    Compare,  // == != < > <= >=
    Term,     // + -
    Factor,   // * / %
    Unary,    // ! -
    Call,     // . () []
    Primary,
}

impl Precedence {
    fn next(&self) -> Self {
        match self {
            Precedence::Lowest => Precedence::Assign,
            Precedence::Assign => Precedence::Or,
            Precedence::Or => Precedence::And,
            Precedence::And => Precedence::Compare,
            Precedence::Compare => Precedence::Term,
            Precedence::Term => Precedence::Factor,
            Precedence::Factor => Precedence::Unary,
            Precedence::Unary => Precedence::Call,
            Precedence::Call => Precedence::Primary,
            Precedence::Primary => Precedence::Primary,
        }
    }

    fn of(kind: &TokenKind) -> Precedence {
        match kind {
            TokenKind::Eq => Precedence::Assign,
            TokenKind::DotDot | TokenKind::DotDotEq => Precedence::Assign,
            TokenKind::OrOr => Precedence::Or,
            TokenKind::AndAnd => Precedence::And,
            TokenKind::EqEq | TokenKind::Ne | TokenKind::Lt | TokenKind::Le
                | TokenKind::Gt | TokenKind::Ge => Precedence::Compare,
            TokenKind::Plus | TokenKind::Minus => Precedence::Term,
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent
                | TokenKind::Shl | TokenKind::Shr => Precedence::Factor,
            TokenKind::And | TokenKind::Or | TokenKind::Caret => Precedence::Term,
            TokenKind::OpenParen | TokenKind::Dot | TokenKind::OpenBracket => Precedence::Call,
            _ => Precedence::Lowest,
        }
    }
}

pub struct Parser<'a> {
    tokens: &'a [Spanned<Token>],
    current: usize,
    errors: Vec<Error>,
    source: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str, tokens: &'a [Spanned<Token>]) -> Self {
        Self { tokens, current: 0, errors: Vec::new(), source }
    }

    fn byte_offset_to_line_col(&self, offset: usize) -> (usize, usize) {
        let source = self.source;
        let mut line = 1usize;
        let mut col = 1usize;
        for (_, c) in source[..offset.min(source.len())].chars().enumerate() {
            if c == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    pub fn parse(mut self) -> Result<Program> {
        let mut stmts = Vec::new();
        while !self.is_at_end() {
            match self.declaration() {
                Ok(stmt) => stmts.push(stmt),
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }
        if self.errors.is_empty() {
            Ok(Program { stmts })
        } else {
            Err(Error::ParseMultiple { errors: std::mem::take(&mut self.errors) })
        }
    }

    // ---------- Declarations (top-level items) ----------

    fn declaration(&mut self) -> Result<Spanned<Stmt>> {
        if self.r#match(TokenKind::Fn) {
            self.function_decl()
        } else if self.r#match(TokenKind::Struct) {
            self.struct_decl()
        } else if self.r#match(TokenKind::Enum) {
            self.enum_decl()
        } else if self.r#match(TokenKind::Impl) {
            self.impl_decl()
        } else if self.r#match(TokenKind::Use) {
            self.use_decl()
        } else if self.r#match(TokenKind::Mod) {
            self.mod_decl()
        } else if self.r#match(TokenKind::Pub) {
            // TODO: visibility tracking not yet implemented - parse the declaration but ignore `pub`
            self.declaration()
        } else {
            self.statement()
        }
    }

    // ---------- Use and Mod declarations ----------

    fn use_decl(&mut self) -> Result<Spanned<Stmt>> {
        let start = self.prev_span();
        let mut path = vec![self.expect_ident()?.into()];
        while self.r#match(TokenKind::ColonColon) {
            path.push(self.expect_ident()?.into());
        }
        self.expect(TokenKind::Semi)?;
        let span = start.merge(&self.prev_span());
        Ok(Spanned::new(Stmt::Use { path }, span))
    }

    fn mod_decl(&mut self) -> Result<Spanned<Stmt>> {
        let start = self.prev_span();
        let name = self.expect_ident()?.into();
        let body = self.block_stmts()?;
        let span = start.merge(&self.prev_span());
        Ok(Spanned::new(Stmt::Mod { name, body }, span))
    }

    // ---------- Statements (inside blocks) ----------

    fn statement(&mut self) -> Result<Spanned<Stmt>> {
        if self.r#match(TokenKind::Pub) {
            self.statement()
        } else if self.r#match(TokenKind::Fn) {
            self.function_decl()
        } else if self.r#match(TokenKind::Struct) {
            self.struct_decl()
        } else if self.r#match(TokenKind::Enum) {
            self.enum_decl()
        } else if self.r#match(TokenKind::Impl) {
            self.impl_decl()
        } else if self.r#match(TokenKind::Use) {
            self.use_decl()
        } else if self.r#match(TokenKind::Mod) {
            self.mod_decl()
        } else if self.r#match(TokenKind::Let) {
            self.let_stmt()
        } else if self.check(&TokenKind::If) {
            let start = self.peek().span.start();
            let expr = self.if_stmt()?;
            let end = self.prev_span().end();
            self.r#match(TokenKind::Semi);
            Ok(Spanned::new(Stmt::Expr(expr), Span::new(start, end)))
        } else if self.check(&TokenKind::While) {
            let start = self.peek().span.start();
            let expr = self.while_stmt()?;
            let end = self.prev_span().end();
            self.r#match(TokenKind::Semi);
            Ok(Spanned::new(Stmt::Expr(expr), Span::new(start, end)))
        } else if self.check(&TokenKind::For) {
            let start = self.peek().span.start();
            let expr = self.for_stmt()?;
            let end = self.prev_span().end();
            self.r#match(TokenKind::Semi);
            Ok(Spanned::new(Stmt::Expr(expr), Span::new(start, end)))
        } else if self.check(&TokenKind::Loop) {
            let start = self.peek().span.start();
            let expr = self.loop_stmt()?;
            let end = self.prev_span().end();
            self.r#match(TokenKind::Semi);
            Ok(Spanned::new(Stmt::Expr(expr), Span::new(start, end)))
        } else if self.r#match(TokenKind::Return) {
            self.return_stmt()
        } else if self.r#match(TokenKind::Break) {
            let span = self.prev_span();
            self.r#match(TokenKind::Semi);
            Ok(Spanned::new(Stmt::Expr(Expr::Break), span))
        } else if self.r#match(TokenKind::Continue) {
            let span = self.prev_span();
            self.r#match(TokenKind::Semi);
            Ok(Spanned::new(Stmt::Expr(Expr::Continue), span))
        } else if self.check(&TokenKind::OpenBrace) {
            let start = self.peek().span.start();
            let expr = self.block()?;
            let end = self.prev_span().end();
            self.r#match(TokenKind::Semi);
            Ok(Spanned::new(Stmt::Expr(expr), Span::new(start, end)))
        } else {
            self.expr_stmt()
        }
    }

    fn let_stmt(&mut self) -> Result<Spanned<Stmt>> {
        let start = self.prev_span();
        let mutable = self.r#match(TokenKind::Mut);

        let name = self.expect_ident()?;
        let type_ann = if self.r#match(TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let init = if self.r#match(TokenKind::Eq) {
            Some(self.expression(Precedence::Lowest)?)
        } else {
            None
        };

        self.expect(TokenKind::Semi)?;
        let span = start.merge(&self.prev_span());
        Ok(Spanned::new(Stmt::Let { mutable, name: name.into(), type_ann, init }, span))
    }

    fn expr_stmt(&mut self) -> Result<Spanned<Stmt>> {
        let start = self.peek().span.start();
        let expr = self.expression(Precedence::Lowest)?;
        let end = self.prev_span().end();
        self.r#match(TokenKind::Semi);
        let span = Span::new(start, end);
        Ok(Spanned::new(Stmt::Expr(expr), span))
    }

    fn return_stmt(&mut self) -> Result<Spanned<Stmt>> {
        let start = self.prev_span();
        let value = if self.check(&TokenKind::Semi) || self.check(&TokenKind::CloseBrace) {
            None
        } else {
            Some(self.expression(Precedence::Lowest)?)
        };
        self.r#match(TokenKind::Semi);
        let span = start.merge(&self.prev_span());
        Ok(Spanned::new(Stmt::Return(value), span))
    }

    // ---------- Function declarations ----------

    fn function_decl(&mut self) -> Result<Spanned<Stmt>> {
        let start = self.prev_span();
        let name = self.expect_ident()?;

        self.expect(TokenKind::OpenParen)?;
        let params = self.param_list()?;
        self.expect(TokenKind::CloseParen)?;

        let return_type = if self.r#match(TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.block_stmts()?;
        let span = start.merge(&self.prev_span());
        Ok(Spanned::new(Stmt::Fn { name: name.into(), params, return_type, body }, span))
    }

    fn param_list(&mut self) -> Result<Vec<Param>> {
        let mut params = Vec::new();
        if self.check(&TokenKind::CloseParen) {
            return Ok(params);
        }
        loop {
            // Allow optional leading & and &mut (references not semantically meaningful)
            self.r#match(TokenKind::And);
            self.r#match(TokenKind::Mut);
            // Allow `self` keyword as a parameter name
            let name = if self.r#match(TokenKind::Self_) {
                "self".to_string()
            } else {
                self.expect_ident()?
            };
            let type_ann = if self.r#match(TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            params.push(Param { name: name.into(), type_ann });
            if !self.r#match(TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    // ---------- Struct declarations ----------

    fn struct_decl(&mut self) -> Result<Spanned<Stmt>> {
        let start = self.prev_span();
        let name = self.expect_ident()?;

        self.expect(TokenKind::OpenBrace)?;
        let mut fields = Vec::new();
        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            let fname = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ftype = self.parse_type()?;
            fields.push(StructField { name: fname.into(), type_ann: ftype });
            self.r#match(TokenKind::Comma); // optional trailing comma
        }
        self.expect(TokenKind::CloseBrace)?;
        let span = start.merge(&self.prev_span());
        Ok(Spanned::new(Stmt::Struct { name: name.into(), fields }, span))
    }

    // ---------- Enum declarations ----------

    fn enum_decl(&mut self) -> Result<Spanned<Stmt>> {
        let start = self.prev_span();
        let name = self.expect_ident()?;

        self.expect(TokenKind::OpenBrace)?;
        let mut variants = Vec::new();
        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            let vname = self.expect_ident()?;
            let fields = if self.r#match(TokenKind::OpenParen) {
                let mut f = Vec::new();
                while !self.check(&TokenKind::CloseParen) && !self.is_at_end() {
                    f.push(self.parse_type()?);
                    self.r#match(TokenKind::Comma);
                }
                self.expect(TokenKind::CloseParen)?;
                f
            } else {
                Vec::new()
            };
            variants.push(EnumVariant { name: vname.into(), fields });
            self.r#match(TokenKind::Comma);
        }
        self.expect(TokenKind::CloseBrace)?;
        let span = start.merge(&self.prev_span());
        Ok(Spanned::new(Stmt::Enum { name: name.into(), variants }, span))
    }

    // ---------- Impl blocks ----------

    fn impl_decl(&mut self) -> Result<Spanned<Stmt>> {
        let start = self.prev_span();
        let type_name = self.expect_ident()?;

        self.expect(TokenKind::OpenBrace)?;
        let mut methods = Vec::new();
        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            if self.r#match(TokenKind::Fn) {
                let method = self.function_decl()?;
                methods.push(method);
            } else {
                return Err(self.error("expected method declaration inside impl block"));
            }
        }
        self.expect(TokenKind::CloseBrace)?;
        let span = start.merge(&self.prev_span());
        Ok(Spanned::new(Stmt::Impl { type_name: type_name.into(), methods }, span))
    }

    // ---------- Expressions (Pratt parser) ----------

    fn expression(&mut self, precedence: Precedence) -> Result<Expr> {
        let mut expr = self.prefix()?;

        loop {
            let tok = self.peek().node.kind.clone();
            let op_prec = Precedence::of(&tok);
            if op_prec < precedence {
                break;
            }
            if matches!(tok, TokenKind::Eq | TokenKind::OrOr | TokenKind::AndAnd
                | TokenKind::EqEq | TokenKind::Ne | TokenKind::Lt | TokenKind::Le
                | TokenKind::Gt | TokenKind::Ge | TokenKind::Plus | TokenKind::Minus
                | TokenKind::Star | TokenKind::Slash | TokenKind::Percent
                | TokenKind::And | TokenKind::Or | TokenKind::Caret
                | TokenKind::Shl | TokenKind::Shr)
            {
                self.advance();
                let op = BinOp::from_token(&tok).unwrap();
                let rhs = self.expression(Precedence::of(&tok).next());
                expr = Expr::Binary { op, lhs: Box::new(expr), rhs: Box::new(rhs?) };
            } else if matches!(tok, TokenKind::PlusEq | TokenKind::MinusEq
                | TokenKind::StarEq | TokenKind::SlashEq | TokenKind::PercentEq
                | TokenKind::AndEq | TokenKind::OrEq
                | TokenKind::CaretEq | TokenKind::ShlEq | TokenKind::ShrEq)
            {
                self.advance();
                let compound_op = compound_to_binop(&tok);
                let rhs = self.expression(Precedence::of(&tok).next());
                // Desugar x += y  →  x = x + y
                expr = Expr::Binary {
                    op: BinOp::Assign,
                    lhs: Box::new(expr.clone()),
                    rhs: Box::new(Expr::Binary {
                        op: compound_op,
                        lhs: Box::new(expr),
                        rhs: Box::new(rhs?),
                    }),
                };
            } else if matches!(tok, TokenKind::OpenParen) {
                self.advance();
                let args = self.arg_list()?;
                self.expect(TokenKind::CloseParen)?;
                expr = Expr::Call { func: Box::new(expr), args };
            } else if matches!(tok, TokenKind::DotDot | TokenKind::DotDotEq) {
                self.advance();
                let inclusive = matches!(tok, TokenKind::DotDotEq);
                let end = self.expression(Precedence::of(&tok).next());
                expr = Expr::Range { start: Box::new(expr), end: Box::new(end?), inclusive };
            } else if matches!(tok, TokenKind::Dot) {
                self.advance();
                let field = self.expect_ident()?;
                if self.r#match(TokenKind::OpenParen) {
                    let args = self.arg_list()?;
                    self.expect(TokenKind::CloseParen)?;
                    expr = Expr::MethodCall { obj: Box::new(expr), method: field.into(), args };
                } else {
                    expr = Expr::Field { obj: Box::new(expr), field: field.into() };
                }
            } else if matches!(tok, TokenKind::OpenBracket) {
                self.advance();
                let index = self.expression(Precedence::Lowest)?;
                self.expect(TokenKind::CloseBracket)?;
                expr = Expr::Index { obj: Box::new(expr), index: Box::new(index) };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn prefix(&mut self) -> Result<Expr> {
         let tok = self.peek().node.kind.clone();
        match tok {
            // Literals
            TokenKind::Int(n) => { self.advance(); Ok(Expr::Int(n)) }
            TokenKind::Float(n) => { self.advance(); Ok(Expr::Float(n)) }
            TokenKind::Str(s) => { self.advance(); Ok(Expr::Str(s)) }
            TokenKind::Bool(b) => { self.advance(); Ok(Expr::Bool(b)) }
            TokenKind::Underscore => { self.advance(); Ok(Expr::Unit) }

            // Unary operators
            TokenKind::Minus | TokenKind::Bang | TokenKind::Tilde => {
                self.advance();
                let op = UnOp::from_token(&tok).unwrap();
                let expr = self.expression(Precedence::Unary)?;
                Ok(Expr::Unary { op, expr: Box::new(expr) })
            }

            // Grouping / Block / Unit
            TokenKind::OpenParen => {
                self.advance();
                if self.r#match(TokenKind::CloseParen) {
                    Ok(Expr::Unit)
                } else {
                    let expr = self.expression(Precedence::Lowest)?;
                    self.expect(TokenKind::CloseParen)?;
                    Ok(expr)
                }
            }
            TokenKind::OpenBrace => {
                let block = self.block()?;
                Ok(block)
            }

            // Control flow (these consume their own keyword token)
            TokenKind::If => self.if_stmt(),
            TokenKind::While => self.while_stmt(),
            TokenKind::For => self.for_stmt(),
            TokenKind::Loop => self.loop_stmt(),
            TokenKind::Match => self.match_expr(),

            TokenKind::Ident(_) => {
                let name = self.expect_ident()?;
                if self.is_struct_lit_start() {
                    self.advance();
                    let mut fields = Vec::new();
                    while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
                        let fname = self.expect_ident()?;
                        self.expect(TokenKind::Colon)?;
                        let val = self.expression(Precedence::Lowest)?;
                        fields.push((fname.into(), val));
                        self.r#match(TokenKind::Comma);
                    }
                    self.expect(TokenKind::CloseBrace)?;
                    Ok(Expr::StructLit { name: name.into(), fields })
                } else {
                    Ok(Expr::Ident(name.into()))
                }
            }

            // Array literal
            TokenKind::OpenBracket => {
                self.advance();
                let mut elems = Vec::new();
                while !self.check(&TokenKind::CloseBracket) && !self.is_at_end() {
                    elems.push(self.expression(Precedence::Lowest)?);
                    self.r#match(TokenKind::Comma);
                }
                self.expect(TokenKind::CloseBracket)?;
                Ok(Expr::Array(elems))
            }

            // Range literal starting with ..
            TokenKind::DotDot | TokenKind::DotDotEq => {
                self.advance();
                let inclusive = matches!(self.prev().node.kind, TokenKind::DotDotEq);
                // ..expr or ..=expr
                let start = Box::new(Expr::Int(0)); // default start
                let end = Box::new(self.expression(Precedence::Lowest)?);
                Ok(Expr::Range { start, end, inclusive })
            }

            // Lambda: || expr (no params)
            TokenKind::OrOr => {
                self.advance();
                let body = self.expression(Precedence::Lowest)?;
                Ok(Expr::Lambda { params: vec![], return_type: None, body: Box::new(body) })
            }

            // Lambda: |params| expr
            TokenKind::Or => {
                self.advance();
                let params = self.lambda_param_list()?;
                self.expect(TokenKind::Or)?;
                let body = self.expression(Precedence::Lowest)?;
                Ok(Expr::Lambda { params, return_type: None, body: Box::new(body) })
            }

            _ => Err(self.error(&format!("unexpected token '{}'", self.peek().node.lexeme)))
        }
    }

    fn lambda_param_list(&mut self) -> Result<Vec<Param>> {
        let mut params = Vec::new();
        loop {
            if self.check(&TokenKind::Or) || self.check(&TokenKind::CloseParen) {
                break;
            }
            self.r#match(TokenKind::And); // allow &mut
            self.r#match(TokenKind::Mut);
            let name = self.expect_ident()?;
            let type_ann = if self.r#match(TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            params.push(Param { name: name.into(), type_ann });
            if !self.r#match(TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    /// Parse a primary expression (without struct literal interpretation for ident {).  
    fn primary_expr(&mut self) -> Result<Expr> {
        let tok = self.peek().node.kind.clone();
        match tok {
            TokenKind::Int(n) => { self.advance(); Ok(Expr::Int(n)) }
            TokenKind::Float(n) => { self.advance(); Ok(Expr::Float(n)) }
            TokenKind::Str(s) => { self.advance(); Ok(Expr::Str(s)) }
            TokenKind::Bool(b) => { self.advance(); Ok(Expr::Bool(b)) }
            TokenKind::Underscore => { self.advance(); Ok(Expr::Unit) }
            TokenKind::Ident(_) => {
                let name = self.expect_ident()?;
                Ok(Expr::Ident(name.into()))
            }
            TokenKind::OpenParen => {
                self.advance();
                let expr = self.expression(Precedence::Lowest)?;
                self.expect(TokenKind::CloseParen)?;
                Ok(expr)
            }
            _ => self.prefix(), // fall back to full prefix parsing
        }
    }

    // ---------- Control flow expressions ----------

    fn if_stmt(&mut self) -> Result<Expr> {
        self.advance(); // consume 'if'
        let cond = self.expression(Precedence::Lowest)?;
        let then = self.block()?;
        let else_ = if self.r#match(TokenKind::Else) {
            if self.r#match(TokenKind::If) {
                Some(Box::new(self.if_stmt()?))
            } else {
                Some(Box::new(self.block()?))
            }
        } else {
            None
        };
        Ok(Expr::If { cond: Box::new(cond), then: Box::new(then), else_ })
    }

    fn while_stmt(&mut self) -> Result<Expr> {
        self.advance(); // consume 'while'
        let cond = self.expression(Precedence::Lowest)?;
        let body = self.block()?;
        Ok(Expr::While { cond: Box::new(cond), body: Box::new(body) })
    }

    fn for_stmt(&mut self) -> Result<Expr> {
        self.advance(); // consume 'for'
        let var = self.expect_ident()?;
        self.expect(TokenKind::In)?; // "for var in iter"
        let iter = self.expression(Precedence::Lowest)?;
        let body = self.block()?;
        Ok(Expr::For { var: var.into(), iter: Box::new(iter), body: Box::new(body) })
    }

    fn loop_stmt(&mut self) -> Result<Expr> {
        self.advance(); // consume 'loop'
        let body = self.block()?;
        Ok(Expr::Loop(Box::new(body)))
    }

    fn block(&mut self) -> Result<Expr> {
        let stmts = self.block_stmts()?;
        Ok(Expr::Block(stmts))
    }

    fn block_stmts(&mut self) -> Result<Vec<Spanned<Stmt>>> {
        self.expect(TokenKind::OpenBrace)?;
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            match self.statement() {
                Ok(stmt) => stmts.push(stmt),
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize_block();
                }
            }
        }
        self.expect(TokenKind::CloseBrace)?;
        Ok(stmts)
    }

    // ---------- Match expression ----------

    /// Parse the target expression of a `match`, stopping at `{`.
    /// This avoids the ambiguity where `Ident { ... }` would be parsed as a struct literal.
    fn match_target(&mut self) -> Result<Expr> {
        if self.check(&TokenKind::OpenParen) {
            self.advance();
            let expr = self.expression(Precedence::Lowest)?;
            self.expect(TokenKind::CloseParen)?;
            Ok(expr)
        } else if matches!(self.peek().node.kind, TokenKind::Ident(_)) {
            let name = self.expect_ident()?;
            let mut expr = Expr::Ident(name.into());
            // Allow continued field access: match x.y { ... }
            while self.r#match(TokenKind::Dot) {
                let field = self.expect_ident()?;
                expr = Expr::Field { obj: Box::new(expr), field: field.into() };
            }
            Ok(expr)
        } else {
            // Fall back to a primary expression (literals, parens, etc.)
            // but avoid struct-literal parsing.
            self.primary_expr()
        }
    }

    fn match_expr(&mut self) -> Result<Expr> {
        self.advance(); // consume 'match'
        // Parse the match target without allowing struct literals.
        // This avoids ambiguity where `match x { ... }` would parse `x { ... }` as a struct lit.
        let expr = self.match_target()?;
        self.expect(TokenKind::OpenBrace)?;
        let mut arms = Vec::new();
        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            let pattern = self.parse_pattern()?;
            let guard = if self.r#match(TokenKind::If) {
                Some(Box::new(self.expression(Precedence::Lowest)?))
            } else {
                None
            };
            self.expect(TokenKind::FatArrow)?;
            let body = self.expression(Precedence::Lowest)?;
            self.r#match(TokenKind::Comma);
            arms.push(MatchArm { pattern, guard, body: Box::new(body) });
        }
        self.expect(TokenKind::CloseBrace)?;
        Ok(Expr::Match { expr: Box::new(expr), arms })
    }

    fn parse_pattern(&mut self) -> Result<Pattern> {
        let tok = self.peek().node.kind.clone();
        match tok {
            TokenKind::Underscore => { self.advance(); Ok(Pattern::Wildcard) }
            TokenKind::Int(n) => { self.advance(); Ok(Pattern::Int(n)) }
            TokenKind::Float(n) => { self.advance(); Ok(Pattern::Float(n)) }
            TokenKind::Str(s) => { self.advance(); Ok(Pattern::Str(s)) }
            TokenKind::Bool(b) => { self.advance(); Ok(Pattern::Bool(b)) }
            TokenKind::Ident(_) => {
                let name = self.expect_ident()?;
                Ok(Pattern::Ident(name.into()))
            }
            _ => Err(self.error("expected pattern"))
        }
    }

    // ---------- Type parsing ----------

    fn parse_type(&mut self) -> Result<Type> {
        let tok = self.peek().node.kind.clone();
        match tok {
            TokenKind::Ident(s) if s == "i32" || s == "i64" => { self.advance(); Ok(Type::I64) }
            TokenKind::Ident(s) if s == "f32" => { self.advance(); Ok(Type::F32) }
            TokenKind::Ident(s) if s == "f64" => { self.advance(); Ok(Type::F64) }
            TokenKind::Ident(s) if s == "bool" => { self.advance(); Ok(Type::Bool) }
            TokenKind::Ident(s) if s == "str" => { self.advance(); Ok(Type::Str) }
            TokenKind::Ident(s) if s == "void" => { self.advance(); Ok(Type::Unit) }
            TokenKind::Ident(_) => {
                let name = self.expect_ident()?;
                // Check for array type: [Type] or [Type; N]
                if self.r#match(TokenKind::OpenBracket) {
                    if self.r#match(TokenKind::CloseBracket) {
                        return Ok(Type::Array(Box::new(Type::Named(name.into()))));
                    }
                    // For now, just handle [Type]
                    let inner = self.parse_type()?;
                    self.expect(TokenKind::CloseBracket)?;
                    Ok(Type::Array(Box::new(inner)))
                } else {
                    Ok(Type::Named(name.into()))
                }
            }
            TokenKind::OpenParen => {
                self.advance();
                let params = self.parse_types_list()?;
                self.expect(TokenKind::CloseParen)?;
                if self.r#match(TokenKind::Arrow) {
                    let ret = self.parse_type()?;
                    Ok(Type::Fn { params, ret: Box::new(ret) })
                } else if params.len() == 1 {
                    Ok(params.into_iter().next().unwrap())
                } else {
                    Ok(Type::Unit) // () is unit
                }
            }
            _ => Err(self.error("expected type"))
        }
    }

    fn parse_types_list(&mut self) -> Result<Vec<Type>> {
        let mut types = Vec::new();
        if self.check(&TokenKind::CloseParen) {
            return Ok(types);
        }
        loop {
            types.push(self.parse_type()?);
            if !self.r#match(TokenKind::Comma) {
                break;
            }
        }
        Ok(types)
    }

    // ---------- Helper methods ----------

    fn arg_list(&mut self) -> Result<Vec<Expr>> {
        let mut args = Vec::new();
        if self.check(&TokenKind::CloseParen) {
            return Ok(args);
        }
        loop {
            args.push(self.expression(Precedence::Lowest)?);
            if !self.r#match(TokenKind::Comma) {
                break;
            }
        }
        Ok(args)
    }

    fn expect_ident(&mut self) -> Result<String> {
        let tok = self.peek();
        if let TokenKind::Ident(s) = &tok.node.kind {
            let name = s.to_string();
            self.advance();
            Ok(name)
        } else {
            Err(self.error(&format!("expected identifier, found '{}'", tok.node.lexeme)))
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<()> {
        if self.r#match(kind.clone()) {
            Ok(())
        } else {
            let tok = self.peek();
            Err(self.error(&format!("expected '{}', found '{}'", kind, tok.node.lexeme)))
        }
    }

    fn is_struct_lit_start(&self) -> bool {
        if !self.check(&TokenKind::OpenBrace) {
            return false;
        }
        // Look ahead past `{` to see if it's a struct literal (ident: or })
        let i = self.current;
        if i + 1 >= self.tokens.len() {
            return false;
        }
        let after_brace = &self.tokens[i + 1].node.kind;
        // Empty struct: Foo {}
        if matches!(after_brace, TokenKind::CloseBrace) {
            return true;
        }
        // Struct with fields: Foo { field: value, ... }
        if matches!(after_brace, TokenKind::Ident(_)) {
            if i + 2 < self.tokens.len() {
                return matches!(&self.tokens[i + 2].node.kind, TokenKind::Colon);
            }
        }
        false
    }

    fn check(&self, kind: &TokenKind) -> bool {
        !self.is_at_end() && &self.peek().node.kind == kind
    }

    fn advance(&mut self) -> &Spanned<Token> {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.prev()
    }

    fn r#match(&mut self, kind: TokenKind) -> bool {
        if self.check(&kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> &Spanned<Token> {
        &self.tokens[self.current]
    }

    fn prev(&self) -> &Spanned<Token> {
        &self.tokens[self.current - 1]
    }

    fn prev_span(&self) -> Span {
        self.tokens[self.current - 1].span
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek().node.kind, TokenKind::Eof)
    }

    fn error(&self, msg: &str) -> Error {
        let span = self.peek().span;
        let (line, col) = self.byte_offset_to_line_col(span.start());
        Error::Parse {
            location: SourceLocation::new(None, span, line, col),
            msg: msg.into(),
        }
    }

    /// Synchronise to the next statement boundary after a parse error.
    fn synchronize(&mut self) {
        self.advance();
        while !self.is_at_end() {
            if matches!(self.prev().node.kind, TokenKind::Semi) {
                return;
            }
            match self.peek().node.kind {
                TokenKind::Fn | TokenKind::Let | TokenKind::Struct
                | TokenKind::Enum | TokenKind::Impl
                | TokenKind::Pub | TokenKind::Use | TokenKind::Mod => return,
                _ => {}
            }
            self.advance();
        }
    }

    fn synchronize_block(&mut self) {
        self.advance();
        while !self.is_at_end() && !matches!(self.peek().node.kind, TokenKind::CloseBrace) {
            if matches!(self.prev().node.kind, TokenKind::Semi) {
                return;
            }
            self.advance();
        }
    }

}

fn compound_to_binop(tok: &TokenKind) -> BinOp {
    match tok {
        TokenKind::PlusEq => BinOp::Add,
        TokenKind::MinusEq => BinOp::Sub,
        TokenKind::StarEq => BinOp::Mul,
        TokenKind::SlashEq => BinOp::Div,
        TokenKind::PercentEq => BinOp::Mod,
        TokenKind::AndEq => BinOp::And,
        TokenKind::OrEq => BinOp::Or,
        TokenKind::CaretEq => BinOp::BitXor,
        TokenKind::ShlEq => BinOp::Shl,
        TokenKind::ShrEq => BinOp::Shr,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(source: &str) -> Result<Program> {
        let tokens = Lexer::new(source).tokenize()?;
        Parser::new(source, &tokens).parse()
    }

    #[test]
    fn test_empty_program() {
        let prog = parse("").unwrap();
        assert!(prog.stmts.is_empty());
    }

    #[test]
    fn test_let_stmt() {
        let prog = parse("let x = 42;").unwrap();
        assert_eq!(prog.stmts.len(), 1);
        match &prog.stmts[0].node {
            Stmt::Let { mutable, name, type_ann: _, init: _ } => {
                assert!(!mutable);
                assert_eq!(name, "x");
            }
            _ => panic!("expected let stmt"),
        }
    }

    #[test]
    fn test_let_mut() {
        let prog = parse("let mut x = 10;").unwrap();
        match &prog.stmts[0].node {
            Stmt::Let { mutable, name, .. } => {
                assert!(*mutable);
                assert_eq!(name, "x");
            }
            _ => panic!("expected let stmt"),
        }
    }

    #[test]
    fn test_fn_decl() {
        let prog = parse("fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();
        assert_eq!(prog.stmts.len(), 1);
        match &prog.stmts[0].node {
            Stmt::Fn { name, params, return_type, body } => {
                assert_eq!(name, "add");
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].name, "a");
                assert!(return_type.is_some());
                assert_eq!(body.len(), 1);
            }
            _ => panic!("expected fn stmt"),
        }
    }

    #[test]
    fn test_if_expr() {
        let prog = parse("let x = if true { 1 } else { 2 };").unwrap();
        match &prog.stmts[0].node {
            Stmt::Let { init: Some(Expr::If { cond, then, else_ }), .. } => {
                assert!(matches!(cond.as_ref(), Expr::Bool(true)));
                assert!(matches!(then.as_ref(), Expr::Block(_)));
                assert!(else_.is_some());
            }
            _ => panic!("expected let with if init"),
        }
    }

    #[test]
    fn test_if_field_cond() {
        parse("fn f() { if e.vx > m { 1 } }").unwrap();
    }

    #[test]
    fn test_if_after_let() {
        parse("fn f() { let x = 1; if x > m { 2 } }").unwrap();
    }

    #[test]
    fn test_if_abs_condition() {
        parse("fn f() { if abs(e.vx) > m { 1 } }").unwrap();
    }

    #[test]
    fn test_field_access_in_rhs() {
        // field access should work inside binary expression RHS
        let prog = parse("let x = a + e.vx;").unwrap();
        match &prog.stmts[0].node {
            Stmt::Let { init: Some(Expr::Binary { op: BinOp::Add, lhs, rhs }), .. } => {
                assert!(matches!(lhs.as_ref(), Expr::Ident(_)));
                assert!(matches!(rhs.as_ref(), Expr::Field { .. }));
            }
            _ => panic!("expected binary add with field access"),
        }
    }

    #[test]
    fn test_call_in_rhs() {
        // function call should work inside binary expression RHS
        let prog = parse("let x = a + foo(1);").unwrap();
        match &prog.stmts[0].node {
            Stmt::Let { init: Some(Expr::Binary { op: BinOp::Add, lhs, rhs }), .. } => {
                assert!(matches!(lhs.as_ref(), Expr::Ident(_)));
                assert!(matches!(rhs.as_ref(), Expr::Call { .. }));
            }
            _ => panic!("expected binary add with function call"),
        }
    }

    #[test]
    fn test_full_script_parse() {
        parse(r#"
fn update() {
    let i = 0;
    while i < entity_count() {
        let e = get_entity(i);
        e.vx = e.vx + 1.0;
        e.vy = e.vy + 1.0;
        let max_speed = 300.0;
        if abs(e.vx) > max_speed {
            if e.vx > 0.0 { e.vx = max_speed; } else { e.vx = -max_speed; }
        }
        if abs(e.vy) > max_speed {
            if e.vy > 0.0 { e.vy = max_speed; } else { e.vy = -max_speed; }
        }
        i = i + 1;
    }
}

if entity_count() == 0 {
    spawn_entity(100.0, 100.0, 0.0, 0.0, 8.0, 200.0, 200.0, 200.0);
}

if is_key_pressed("space") {
    spawn_entity(200.0, 200.0, 0.0, 0.0, 10.0, 255.0, 80.0, 80.0);
}

update();
"#).unwrap();
    }

    #[test]
    fn test_while_loop() {
        let prog = parse("while x < 10 { x = x + 1; }").unwrap();
        assert_eq!(prog.stmts.len(), 1);
        match &prog.stmts[0].node {
            Stmt::Expr(Expr::While { cond, body }) => {
                assert!(matches!(cond.as_ref(), Expr::Binary { op: BinOp::Lt, .. }));
                assert!(matches!(body.as_ref(), Expr::Block(_)));
            }
            _ => panic!("expected while expr"),
        }
    }

    #[test]
    fn test_for_loop() {
        let prog = parse("for i in 0..10 { print(i); }").unwrap();
        assert_eq!(prog.stmts.len(), 1);
        match &prog.stmts[0].node {
            Stmt::Expr(Expr::For { var, iter, body }) => {
                assert_eq!(var, "i");
                assert!(matches!(iter.as_ref(), Expr::Range { .. }));
                assert!(matches!(body.as_ref(), Expr::Block(_)));
            }
            _ => panic!("expected for expr"),
        }
    }

    #[test]
    fn test_struct_decl() {
        let prog = parse("struct Vec2 { x: f64, y: f64 }").unwrap();
        match &prog.stmts[0].node {
            Stmt::Struct { name, fields } => {
                assert_eq!(name, "Vec2");
                assert_eq!(fields.len(), 2);
            }
            _ => panic!("expected struct decl"),
        }
    }

    #[test]
    fn test_enum_decl() {
        let prog = parse("enum Option { Some(i32), None }").unwrap();
        match &prog.stmts[0].node {
            Stmt::Enum { name, variants } => {
                assert_eq!(name, "Option");
                assert_eq!(variants.len(), 2);
            }
            _ => panic!("expected enum decl"),
        }
    }

    #[test]
    fn test_match_expr() {
        let prog = parse("match x { 1 => true, _ => false }").unwrap();
        match &prog.stmts[0].node {
            Stmt::Expr(Expr::Match { expr, arms }) => {
                assert!(matches!(expr.as_ref(), Expr::Ident(_)));
                assert_eq!(arms.len(), 2);
            }
            _ => panic!("expected match expr"),
        }
    }

    #[test]
    fn test_block_expr() {
        let prog = parse("let x = { let y = 1; y + 2 };").unwrap();
        match &prog.stmts[0].node {
            Stmt::Let { init: Some(Expr::Block(stmts)), .. } => {
                assert_eq!(stmts.len(), 2);
            }
            _ => panic!("expected let with block init"),
        }
    }

    #[test]
    fn test_impl_block() {
        let prog = parse("impl Vec2 { fn length(&self) -> f64 { 0.0 } }").unwrap();
        assert_eq!(prog.stmts.len(), 1);
        match &prog.stmts[0].node {
            Stmt::Impl { type_name, methods } => {
                assert_eq!(type_name, "Vec2");
                assert_eq!(methods.len(), 1);
            }
            _ => panic!("expected impl block"),
        }
    }

    #[test]
    fn test_method_call() {
        let prog = parse("obj.method(1, 2);").unwrap();
        match &prog.stmts[0].node {
            Stmt::Expr(Expr::MethodCall { obj, method, args }) => {
                assert!(matches!(obj.as_ref(), Expr::Ident(_)));
                assert_eq!(method, "method");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("expected method call"),
        }
    }

    #[test]
    fn test_field_access() {
        let prog = parse("obj.field;").unwrap();
        match &prog.stmts[0].node {
            Stmt::Expr(Expr::Field { obj, field }) => {
                assert!(matches!(obj.as_ref(), Expr::Ident(_)));
                assert_eq!(field, "field");
            }
            _ => panic!("expected field access"),
        }
    }

    #[test]
    fn test_array_literal() {
        let prog = parse("let arr = [1, 2, 3];").unwrap();
        match &prog.stmts[0].node {
            Stmt::Let { init: Some(Expr::Array(elems)), .. } => {
                assert_eq!(elems.len(), 3);
            }
            _ => panic!("expected array literal"),
        }
    }

    #[test]
    fn test_struct_literal() {
        let prog = parse("let v = Vec2 { x: 1.0, y: 2.0 };").unwrap();
        match &prog.stmts[0].node {
            Stmt::Let { init: Some(Expr::StructLit { name, fields }), .. } => {
                assert_eq!(name, "Vec2");
                assert_eq!(fields.len(), 2);
            }
            _ => panic!("expected struct literal"),
        }
    }

    #[test]
    fn test_call_expr() {
        let prog = parse("foo(1, 2, 3);").unwrap();
        match &prog.stmts[0].node {
            Stmt::Expr(Expr::Call { func, args }) => {
                assert!(matches!(func.as_ref(), Expr::Ident(_)));
                assert_eq!(args.len(), 3);
            }
            _ => panic!("expected call"),
        }
    }
}
