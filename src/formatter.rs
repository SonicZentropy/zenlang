use crate::lexer::Lexer;
use crate::span::Spanned;
use crate::token::{Token, TokenKind};

pub fn format_source(source: &str, tab_size: usize) -> Result<String, crate::error::Error> {
    let tokens = Lexer::new(source).tokenize_with_comments()?;
    let mut out = String::with_capacity(source.len());
    let mut f = FmtState { indent: 0, tab_size, bol: true, prev_kind: None };

    let mut i = 0;
    while i < tokens.len() {
        if matches!(&tokens[i].node.kind, TokenKind::Eof) {
            break;
        }
        i = f.emit_token(&tokens, i, &mut out);
    }

    Ok(out.trim_end().to_string() + "\n")
}

struct FmtState {
    indent: usize,
    tab_size: usize,
    bol: bool,
    prev_kind: Option<TokenKind>,
}

impl FmtState {
    /// Emit token at index `i`, return the next index to process.
    fn emit_token(&mut self, tokens: &[Spanned<Token>], i: usize, out: &mut String) -> usize {
        let kind = &tokens[i].node.kind;

        // ---- comment ----
        if matches!(kind, TokenKind::Comment) {
            self.emit_comment(&tokens[i], out);
            return i + 1;
        }

        let next_non_comment = tokens[i + 1..].iter().find(|t| !matches!(t.node.kind, TokenKind::Comment));

        // ---- spacing before ----
        if matches!(kind, TokenKind::CloseBrace) {
            if !matches!(self.prev_kind, Some(TokenKind::OpenBrace)) {
                self.indent = self.indent.saturating_sub(1);
            }
            if !self.bol {
                let indent_str = " ".repeat(self.indent * self.tab_size);
                out.push('\n');
                out.push_str(&indent_str);
            } else {
                let indent_str = " ".repeat(self.indent * self.tab_size);
                out.push_str(&indent_str);
                self.bol = false;
            }
        } else if self.bol {
            let indent_str = " ".repeat(self.indent * self.tab_size);
            out.push_str(&indent_str);
            self.bol = false;
        } else if matches!(kind, TokenKind::Else) {
            // on same line as closing brace
            out.push(' ');
        } else if must_start_line(kind, &self.prev_kind) {
            out.push('\n');
            let indent_str = " ".repeat(self.indent * self.tab_size);
            out.push_str(&indent_str);
        } else if needs_space_before(kind, &self.prev_kind) {
            out.push(' ');
        }

        // ---- emit ----
        out.push_str(&tokens[i].node.lexeme);

        // ---- spacing after / state ----
        let after = match kind {
            TokenKind::OpenBrace => {
                self.indent += 1;
                true  // newline after
            }
            TokenKind::CloseBrace => {
                // newline after unless followed by `else` (which adds its own space)
                let after_is_else = next_non_comment.is_some_and(|s| matches!(&s.node.kind, TokenKind::Else));
                !after_is_else  // true → newline, false → nothing (else handles spacing)
            }
            TokenKind::Semi => true,  // newline
            TokenKind::Comma => {
                out.push(' ');
                false
            }
            TokenKind::Arrow | TokenKind::FatArrow | TokenKind::Eq => {
                out.push(' ');
                false
            }
            k if is_bin_op(k) || is_compare_op(k)
                || matches!(k, TokenKind::And | TokenKind::Or | TokenKind::Caret
                    | TokenKind::Shl | TokenKind::Shr
                    | TokenKind::PlusEq | TokenKind::MinusEq | TokenKind::StarEq
                    | TokenKind::SlashEq | TokenKind::PercentEq
                    | TokenKind::AndEq | TokenKind::OrEq
                    | TokenKind::CaretEq | TokenKind::ShlEq | TokenKind::ShrEq) => {
                out.push(' ');
                false
            }
            _ => false,
        };

        if after {
            out.push('\n');
            if matches!(kind, TokenKind::CloseBrace) && self.indent == 0 {
                out.push('\n'); // blank line between top-level declarations
            }
            self.bol = true;
        }

        self.prev_kind = Some(kind.clone());
        i + 1
    }

    fn emit_comment(&mut self, t: &Spanned<Token>, out: &mut String) {
        let text = t.node.lexeme.as_str();
        let is_line = text.starts_with("//");

        if is_line {
            if !self.bol {
                out.push('\n');
            }
            let indent_str = " ".repeat(self.indent * self.tab_size);
            out.push_str(&indent_str);
            out.push_str(text);
            out.push('\n');
            self.bol = true;
        } else {
            if text.contains('\n') {
                if !self.bol {
                    out.push('\n');
                }
                let indent_str = " ".repeat(self.indent * self.tab_size);
                for line in text.lines() {
                    out.push_str(&indent_str);
                    out.push_str(line.trim());
                    out.push('\n');
                }
                self.bol = true;
            } else {
                if self.bol {
                    let indent_str = " ".repeat(self.indent * self.tab_size);
                    out.push_str(&indent_str);
                } else {
                    out.push(' ');
                }
                out.push_str(text);
                out.push('\n');
                self.bol = true;
            }
        }
    }
}

/// Tokens that should always start on a new line (unless at BOL).
fn must_start_line(kind: &TokenKind, _prev: &Option<TokenKind>) -> bool {
    match kind {
        // Top-level declarations
        TokenKind::Fn | TokenKind::Struct | TokenKind::Enum | TokenKind::Impl | TokenKind::Trait => true,
        // Statement keywords that start a line
        TokenKind::If | TokenKind::While | TokenKind::For | TokenKind::Loop => true,
        // Let binding
        TokenKind::Let => true,
        // Return keyword
        TokenKind::Return => true,
        // Break/continue
        TokenKind::Break | TokenKind::Continue => true,
        // Close brace already handled separately above
        _ => false,
    }
}

fn needs_space_before(kind: &TokenKind, prev: &Option<TokenKind>) -> bool {
    match kind {
        // Open brace always preceded by space
        TokenKind::OpenBrace => true,
        // Binary / comparison operators
        k if is_bin_op(k) || is_compare_op(k) => true,
        TokenKind::And | TokenKind::Or | TokenKind::Caret | TokenKind::Shl | TokenKind::Shr => true,
        // Assignment
        TokenKind::Eq => true,
        // Compound assignment
        TokenKind::PlusEq | TokenKind::MinusEq | TokenKind::StarEq
        | TokenKind::SlashEq | TokenKind::PercentEq
        | TokenKind::AndEq | TokenKind::OrEq
        | TokenKind::CaretEq | TokenKind::ShlEq | TokenKind::ShrEq => true,
        // Arrows
        TokenKind::Arrow | TokenKind::FatArrow => true,
        // Colon (type annotation)
        TokenKind::Colon => true,
        // After keywords that need a space before their argument
        _ => match prev {
            Some(p) if is_keyword_that_needs_space(p) => true,
            _ => false,
        },
    }
}

fn is_keyword_that_needs_space(k: &TokenKind) -> bool {
    matches!(k,
        TokenKind::Fn | TokenKind::Let | TokenKind::Mut |
        TokenKind::If | TokenKind::While | TokenKind::For | TokenKind::Loop |
        TokenKind::Return | TokenKind::Match | TokenKind::Struct |
        TokenKind::Enum | TokenKind::Impl | TokenKind::Trait | TokenKind::Pub | TokenKind::Use |
        TokenKind::Const | TokenKind::Type | TokenKind::In
    )
}

fn is_bin_op(k: &TokenKind) -> bool {
    matches!(k,
        TokenKind::Plus | TokenKind::Minus | TokenKind::Star |
        TokenKind::Slash | TokenKind::Percent |
        TokenKind::Shl | TokenKind::Shr | TokenKind::Caret
    )
}

fn is_compare_op(k: &TokenKind) -> bool {
    matches!(k,
        TokenKind::EqEq | TokenKind::Ne | TokenKind::Lt |
        TokenKind::Gt | TokenKind::Le | TokenKind::Ge |
        TokenKind::AndAnd | TokenKind::OrOr
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_formatting() {
        let src = "fn main() -> int {\n    // it works\n    42\n}\n\nfn testai()->void{\nlet x=42;\nlet y=x+1;\n/* block */\ny\n}\n";
        let result = format_source(src, 4).unwrap();
        println!("=== FORMATTED ===");
        println!("{result}");
        println!("=== END ===");

        assert!(result.contains("fn main() -> int {"));
        assert!(result.contains("    // it works"));
        assert!(result.contains("    42"));
        assert!(result.contains("fn testai() -> void {"));
        assert!(result.contains("    let x = 42;"));
        assert!(result.contains("    let y = x + 1;"));
        assert!(result.contains("    /* block */"));
        assert!(result.contains("    y"));
    }

    #[test]
    fn test_blank_lines_between_fns() {
        let src = "fn main() -> int {\n    //\"test\"\n    42\n}\n\nfn testai() -> void {\n    return ;\n}\n\n\nfn an_error() -> int {\n    \"test\"\n}\n";
        let result = format_source(src, 4).unwrap();
        println!("=== FORMATTED ===");
        println!("{result}");
        println!("=== END ===");
        for (i, l) in result.lines().enumerate() {
            println!("{i:>3}: |{l}|");
        }
        // Exactly 1 blank line between each function
        assert_eq!(result.lines().nth(3), Some("}"));
        assert_eq!(result.lines().nth(4), Some(""));
        assert_eq!(result.lines().nth(5), Some("fn testai() -> void {"));
    }

    #[test]
    fn test_if_else() {
        let src = "fn main() -> int {\nif true{42}else{0}\n}\n";
        let result = format_source(src, 4).unwrap();
        println!("=== IF ELSE ===");
        println!("{result}");
        println!("=== END ===");

        assert!(result.contains("if true {"));
        assert!(result.contains("} else {"));
        assert!(result.contains("    42"));
        assert!(result.contains("    0"));
    }
}
