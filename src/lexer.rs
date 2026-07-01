use compact_str::CompactString;

use crate::error::{Error, Result};
use crate::span::{SourceLocation, Span, Spanned};
use crate::token::{match_keyword, Token, TokenKind};

pub struct Lexer<'a> {
    source: &'a str,
    start: usize,
    current: usize,
    line: usize,
    column: usize,
    errors: Vec<Error>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            start: 0,
            current: 0,
            line: 1,
            column: 1,
            errors: Vec::new(),
        }
    }

    /// Lex the entire source and return all tokens (skipping comments).
    /// Use `tokenize_with_comments()` if you need comment tokens (e.g. for formatting).
    pub fn tokenize(mut self) -> Result<Vec<Spanned<Token>>> {
        let tokens = self.next_token_loop()?;
        // Comments are discarded by default for backward compatibility.
        // Use tokenize_with_comments() if you need them.
        Ok(tokens.into_iter().filter(|t| !matches!(t.node.kind, TokenKind::Comment)).collect())
    }

    /// Lex the entire source and return all tokens **including** comments.
    /// This is used by the formatter.
    pub fn tokenize_with_comments(mut self) -> Result<Vec<Spanned<Token>>> {
        self.next_token_loop()
    }

    fn next_token_loop(&mut self) -> Result<Vec<Spanned<Token>>> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token();
            match token {
                Some(t) => {
                    let is_eof = matches!(t.node.kind, TokenKind::Eof);
                    tokens.push(t);
                    if is_eof {
                        break;
                    }
                }
                None => break,
            }
        }
        if self.errors.is_empty() {
            Ok(tokens)
        } else {
            Err(Error::ParseMultiple { errors: std::mem::take(&mut self.errors) })
        }
    }

    /// Lex one token.
    fn next_token(&mut self) -> Option<Spanned<Token>> {
        self.skip_whitespace();
        self.start = self.current;
        if self.is_at_end() {
            return Some(self.make_token(TokenKind::Eof));
        }

        let c = self.advance();
        Some(match c {
            // Single-character tokens
            '(' => self.make_token(TokenKind::OpenParen),
            ')' => self.make_token(TokenKind::CloseParen),
            '{' => self.make_token(TokenKind::OpenBrace),
            '}' => self.make_token(TokenKind::CloseBrace),
            '[' => self.make_token(TokenKind::OpenBracket),
            ']' => self.make_token(TokenKind::CloseBracket),
            ',' => self.make_token(TokenKind::Comma),
            ';' => self.make_token(TokenKind::Semi),
            ':' => {
                if self.r#match(':') {
                    self.make_token(TokenKind::ColonColon)
                } else {
                    self.make_token(TokenKind::Colon)
                }
            }
            '#' => self.make_token(TokenKind::Hash),
            '@' => self.make_token(TokenKind::At),
            '?' => self.make_token(TokenKind::Question),
            '%' => {
                if self.r#match('=') {
                    self.make_token(TokenKind::PercentEq)
                } else {
                    self.make_token(TokenKind::Percent)
                }
            }

            // Operators that could be one, two, or three chars
            '+' => {
                if self.r#match('=') {
                    self.make_token(TokenKind::PlusEq)
                } else {
                    self.make_token(TokenKind::Plus)
                }
            }
            '-' => {
                if self.r#match('>') {
                    self.make_token(TokenKind::Arrow)
                } else if self.r#match('=') {
                    self.make_token(TokenKind::MinusEq)
                } else {
                    self.make_token(TokenKind::Minus)
                }
            }
            '*' => {
                if self.r#match('=') {
                    self.make_token(TokenKind::StarEq)
                } else {
                    self.make_token(TokenKind::Star)
                }
            }
            '/' => {
                if self.r#match('=') {
                    self.make_token(TokenKind::SlashEq)
                } else if self.r#match('/') {
                    self.skip_to_eol();
                    self.make_token(TokenKind::Comment)
                } else if self.r#match('*') {
                    self.skip_to_block_end();
                    self.make_token(TokenKind::Comment)
                } else {
                    self.make_token(TokenKind::Slash)
                }
            }
            '!' => {
                if self.r#match('=') {
                    self.make_token(TokenKind::Ne)
                } else {
                    self.make_token(TokenKind::Bang)
                }
            }
            '=' => {
                if self.r#match('=') {
                    self.make_token(TokenKind::EqEq)
                } else if self.r#match('>') {
                    self.make_token(TokenKind::FatArrow)
                } else {
                    self.make_token(TokenKind::Eq)
                }
            }
            '<' => {
                if self.r#match('<') {
                    if self.r#match('=') {
                        self.make_token(TokenKind::ShlEq)
                    } else {
                        self.make_token(TokenKind::Shl)
                    }
                } else if self.r#match('=') {
                    self.make_token(TokenKind::Le)
                } else {
                    self.make_token(TokenKind::Lt)
                }
            }
            '>' => {
                if self.r#match('>') {
                    if self.r#match('=') {
                        self.make_token(TokenKind::ShrEq)
                    } else {
                        self.make_token(TokenKind::Shr)
                    }
                } else if self.r#match('=') {
                    self.make_token(TokenKind::Ge)
                } else {
                    self.make_token(TokenKind::Gt)
                }
            }
            '&' => {
                if self.r#match('=') {
                    self.make_token(TokenKind::AndEq)
                } else if self.r#match('&') {
                    self.make_token(TokenKind::AndAnd)
                } else {
                    self.make_token(TokenKind::And)
                }
            }
            '|' => {
                if self.r#match('=') {
                    self.make_token(TokenKind::OrEq)
                } else if self.r#match('|') {
                    self.make_token(TokenKind::OrOr)
                } else {
                    self.make_token(TokenKind::Or)
                }
            }
            '^' => {
                if self.r#match('=') {
                    self.make_token(TokenKind::CaretEq)
                } else {
                    self.make_token(TokenKind::Caret)
                }
            }
            '~' => {
                self.make_token(TokenKind::Tilde)
            }
            '.' => {
                if self.r#match('.') {
                    if self.r#match('=') {
                        self.make_token(TokenKind::DotDotEq)
                    } else {
                        self.make_token(TokenKind::DotDot)
                    }
                } else {
                    self.make_token(TokenKind::Dot)
                }
            }

            // String literal
            '"' => self.string(),

            // Identifiers and keywords
            c if is_ident_start(c) => self.identifier(),

            // Number literals
            c if c.is_ascii_digit() => self.number(),

            // Unexpected character
            _ => self.error_token(&format!("unexpected character '{}'", c)),
        })
    }

    // ---------- character helpers ----------

    fn peek(&self) -> char {
        self.source[self.current..].chars().next().unwrap_or('\0')
    }

    fn peek_next(&self) -> char {
        let mut chars = self.source[self.current..].chars();
        chars.next();
        chars.next().unwrap_or('\0')
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.current..].chars().next().unwrap_or('\0');
        self.current += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        c
    }

    fn r#match(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.peek() != expected {
            return false;
        }
        self.advance();
        true
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn skip_whitespace(&mut self) {
        loop {
            match self.peek() {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '\n' => {
                    self.advance();
                }
                _ => break,
            }
        }
    }

    fn skip_to_eol(&mut self) {
        while self.peek() != '\n' && !self.is_at_end() {
            self.advance();
        }
    }

    fn skip_to_block_end(&mut self) {
        let mut depth = 1;
        while depth > 0 {
            if self.is_at_end() {
                self.errors.push(Error::Parse {
                    location: self.current_location(),
                    msg: "unterminated block comment".into(),
                });
                return;
            }
            if self.peek() == '/' && self.peek_next() == '*' {
                self.advance();
                self.advance();
                depth += 1;
            } else if self.peek() == '*' && self.peek_next() == '/' {
                self.advance();
                self.advance();
                depth -= 1;
            } else {
                self.advance();
            }
        }
    }

    // ---------- token helpers ----------

    fn make_token(&self, kind: TokenKind) -> Spanned<Token> {
        let lexeme = CompactString::from(&self.source[self.start..self.current]);
        Spanned::new(Token::new(kind, lexeme), Span::new(self.start, self.current))
    }

    fn error_token(&mut self, msg: &str) -> Spanned<Token> {
        let location = self.current_location();
        self.errors.push(Error::Parse {
            location,
            msg: msg.into(),
        });
        let lexeme = CompactString::from(&self.source[self.start..self.current]);
        Spanned::new(
            Token::new(TokenKind::Eof, lexeme),
            Span::new(self.start, self.current),
        )
    }

    fn current_location(&self) -> SourceLocation {
        SourceLocation::new(None, Span::new(self.start, self.current), self.line, self.column)
    }

    // ---------- lexing specific token types ----------

    fn identifier(&mut self) -> Spanned<Token> {
        while is_ident_continue(self.peek()) {
            self.advance();
        }
        let ident = &self.source[self.start..self.current];
        let kind = match_keyword(ident).unwrap_or_else(|| {
            TokenKind::Ident(CompactString::from(ident))
        });
        self.make_token(kind)
    }

    fn number(&mut self) -> Spanned<Token> {
        // Check for hex (0x), octal (0o), or binary (0b) prefixes.
        // The initial '0' was already consumed by advance(), so check
        // the source at self.start to confirm it was '0'.
        if self.source.as_bytes().get(self.start) == Some(&b'0') {
            let next = self.peek();
            if next == 'x' || next == 'X' {
                return self.hex_number();
            }
            if next == 'o' || next == 'O' {
                return self.octal_number();
            }
            if next == 'b' || next == 'B' {
                return self.binary_number();
            }
        }

        while self.peek().is_ascii_digit() || self.peek() == '_' {
            self.advance();
        }

        // Check for float: a decimal point followed by at least one digit
        let is_float = if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            self.advance();
            while self.peek().is_ascii_digit() || self.peek() == '_' {
                self.advance();
            }
            true
        } else {
            false
        };

        let raw = &self.source[self.start..self.current];
        let text: String = raw.chars().filter(|&c| c != '_').collect();
        if is_float {
            let val: f64 = text.parse().unwrap_or_else(|_| {
                self.errors.push(Error::Parse {
                    location: self.current_location(),
                    msg: format!("invalid float literal '{}'", raw),
                });
                0.0
            });
            self.make_token(TokenKind::Float(val))
        } else {
            let val: i64 = text.parse().unwrap_or_else(|_| {
                self.errors.push(Error::Parse {
                    location: self.current_location(),
                    msg: format!("invalid integer literal '{}'", raw),
                });
                0
            });
            self.make_token(TokenKind::Int(val))
        }
    }

    fn hex_number(&mut self) -> Spanned<Token> {
        self.advance(); // consume 'x' or 'X' (the '0' was already consumed)
        let start = self.current;
        while self.peek().is_ascii_hexdigit() || self.peek() == '_' {
            self.advance();
        }
        if self.current == start {
            self.errors.push(Error::Parse {
                location: self.current_location(),
                msg: "invalid hex literal: no hexadecimal digits".into(),
            });
        }
        let raw = &self.source[start..self.current];
        let text: String = raw.chars().filter(|&c| c != '_').collect();
        let val = i64::from_str_radix(&text, 16).unwrap_or_else(|_| {
            self.errors.push(Error::Parse {
                location: self.current_location(),
                msg: format!("invalid hex literal '{}'", raw),
            });
            0
        });
        self.make_token(TokenKind::Int(val))
    }

    fn octal_number(&mut self) -> Spanned<Token> {
        self.advance(); // consume 'o' or 'O' (the '0' was already consumed)
        let start = self.current;
        while matches!(self.peek(), '0'..='7' | '_') {
            self.advance();
        }
        if self.current == start {
            self.errors.push(Error::Parse {
                location: self.current_location(),
                msg: "invalid octal literal: no octal digits".into(),
            });
        }
        let raw = &self.source[start..self.current];
        let text: String = raw.chars().filter(|&c| c != '_').collect();
        let val = i64::from_str_radix(&text, 8).unwrap_or_else(|_| {
            self.errors.push(Error::Parse {
                location: self.current_location(),
                msg: format!("invalid octal literal '{}'", raw),
            });
            0
        });
        self.make_token(TokenKind::Int(val))
    }

    fn binary_number(&mut self) -> Spanned<Token> {
        self.advance(); // consume 'b' or 'B' (the '0' was already consumed)
        let start = self.current;
        while matches!(self.peek(), '0' | '1' | '_') {
            self.advance();
        }
        if self.current == start {
            self.errors.push(Error::Parse {
                location: self.current_location(),
                msg: "invalid binary literal: no binary digits".into(),
            });
        }
        let raw = &self.source[start..self.current];
        let text: String = raw.chars().filter(|&c| c != '_').collect();
        let val = i64::from_str_radix(&text, 2).unwrap_or_else(|_| {
            self.errors.push(Error::Parse {
                location: self.current_location(),
                msg: format!("invalid binary literal '{}'", raw),
            });
            0
        });
        self.make_token(TokenKind::Int(val))
    }

    fn string(&mut self) -> Spanned<Token> {
        let mut s = String::new();
        loop {
            if self.is_at_end() {
                self.errors.push(Error::Parse {
                    location: self.current_location(),
                    msg: "unterminated string literal".into(),
                });
                return self.make_token(TokenKind::Str(CompactString::from(&s)));
            }
            let c = self.advance();
            match c {
                '"' => {
                    return self.make_token(TokenKind::Str(CompactString::from(&s)));
                }
                '\\' => {
                    let esc = self.advance();
                    match esc {
                        '"' => s.push('"'),
                        '\\' => s.push('\\'),
                        '{' => s.push('{'),
                        '}' => s.push('}'),
                        'n' => s.push('\n'),
                        'r' => s.push('\r'),
                        't' => s.push('\t'),
                        '0' => s.push('\0'),
                        _ => {
                            self.errors.push(Error::Parse {
                                location: self.current_location(),
                                msg: format!("invalid escape sequence '\\{}'", esc),
                            });
                            s.push(esc);
                        }
                    }
                }
                '\n' => {
                    s.push('\n');
                }
                _ => s.push(c),
            }
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_source() {
        let tokens = Lexer::new("").tokenize().unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].node.kind, TokenKind::Eof);
    }

    #[test]
    fn test_keywords() {
        let tokens = Lexer::new("fn let mut if else while for loop return true false struct enum").tokenize().unwrap();
        let kinds: Vec<&str> = tokens.iter().filter_map(|t| match &t.node.kind {
            TokenKind::Eof => None,
            _ => Some(t.node.lexeme.as_str()),
        }).collect();
        assert_eq!(kinds, vec!["fn", "let", "mut", "if", "else", "while", "for", "loop", "return", "true", "false", "struct", "enum"]);
    }

    #[test]
    fn test_identifiers() {
        let tokens = Lexer::new("foo bar _baz my_var123").tokenize().unwrap();
        let kinds: Vec<&str> = tokens.iter().filter_map(|t| match &t.node.kind {
            TokenKind::Ident(_) => Some(t.node.lexeme.as_str()),
            TokenKind::Eof => None,
            _ => None,
        }).collect();
        assert_eq!(kinds, vec!["foo", "bar", "_baz", "my_var123"]);
    }

    #[test]
    fn test_numbers() {
        let tokens = Lexer::new("42 3.14 0xff 0o77 0b1010").tokenize().unwrap();
        let expected: Vec<TokenKind> = vec![
            TokenKind::Int(42),
            TokenKind::Float(3.14),
            TokenKind::Int(255),
            TokenKind::Int(63),
            TokenKind::Int(10),
        ];
        let kinds: Vec<TokenKind> = tokens.iter().filter_map(|t| match &t.node.kind {
            TokenKind::Eof => None,
            k => Some(k.clone()),
        }).collect();
        assert_eq!(kinds, expected);
    }

    #[test]
    fn test_operators() {
        let tokens = Lexer::new("+ - * / % == != < > <= >= && || ! & | .. ..= -> => :: ; : , . # @ ? ^ ~ << >>").tokenize().unwrap();
        let expected: Vec<TokenKind> = vec![
            TokenKind::Plus, TokenKind::Minus, TokenKind::Star, TokenKind::Slash,
            TokenKind::Percent, TokenKind::EqEq, TokenKind::Ne, TokenKind::Lt,
            TokenKind::Gt, TokenKind::Le, TokenKind::Ge, TokenKind::AndAnd,
            TokenKind::OrOr, TokenKind::Bang, TokenKind::And, TokenKind::Or, TokenKind::DotDot, TokenKind::DotDotEq,
            TokenKind::Arrow, TokenKind::FatArrow, TokenKind::ColonColon,
            TokenKind::Semi, TokenKind::Colon, TokenKind::Comma, TokenKind::Dot,
            TokenKind::Hash, TokenKind::At, TokenKind::Question,
            TokenKind::Caret, TokenKind::Tilde, TokenKind::Shl, TokenKind::Shr,
        ];
        let kinds: Vec<TokenKind> = tokens.iter().filter_map(|t| match &t.node.kind {
            TokenKind::Eof => None,
            k => Some(k.clone()),
        }).collect();
        assert_eq!(kinds, expected);
    }

    #[test]
    fn test_strings() {
        let tokens = Lexer::new(r#""hello" "world\n" "\"quoted\"" "#).tokenize().unwrap();
        let expected: Vec<&str> = vec!["hello", "world\n", "\"quoted\""];
        let strings: Vec<&str> = tokens.iter().filter_map(|t| match &t.node.kind {
            TokenKind::Str(s) => Some(s.as_str()),
            _ => None,
        }).collect();
        assert_eq!(strings, expected);
    }

    #[test]
    fn test_comments() {
        let tokens = Lexer::new("// line comment\n42 /* block /* nested */ */ true").tokenize().unwrap();
        let kinds: Vec<&str> = tokens.iter().filter_map(|t| match &t.node.kind {
            TokenKind::Eof => None,
            _ => Some(t.node.lexeme.as_str()),
        }).collect();
        assert_eq!(kinds, vec!["42", "true"]);
    }

    #[test]
    fn test_grouping() {
        let tokens = Lexer::new("( ) { } [ ]").tokenize().unwrap();
        let expected: Vec<TokenKind> = vec![
            TokenKind::OpenParen, TokenKind::CloseParen,
            TokenKind::OpenBrace, TokenKind::CloseBrace,
            TokenKind::OpenBracket, TokenKind::CloseBracket,
        ];
        let kinds: Vec<TokenKind> = tokens.iter().filter_map(|t| match &t.node.kind {
            TokenKind::Eof => None,
            k => Some(k.clone()),
        }).collect();
        assert_eq!(kinds, expected);
    }

    #[test]
    fn test_ampersand_as_token() {
        let tokens = Lexer::new("& foo").tokenize().unwrap();
        let kinds: Vec<TokenKind> = tokens.iter().filter_map(|t| match &t.node.kind {
            TokenKind::Eof => None,
            k => Some(k.clone()),
        }).collect();
        assert_eq!(kinds, vec![TokenKind::And, TokenKind::Ident("foo".into())]);
    }

    #[test]
    fn test_unterminated_string() {
        let result = Lexer::new(r#""unterminated"#).tokenize();
        assert!(result.is_err());
    }

    #[test]
    fn test_block_comment_unterminated() {
        let result = Lexer::new("/* unterminated").tokenize();
        assert!(result.is_err());
    }
}
