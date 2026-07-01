use std::collections::HashSet;
use std::path::Path;

use crate::ast::*;
use crate::error::{Error, Result};
use crate::span::Spanned;

/// Resolve `mod name;` declarations by loading the corresponding `<name>.zen` files.
///
/// Walks the program AST, finds `Stmt::Mod` nodes with empty bodies (from `mod name;` syntax),
/// and replaces their bodies with the parsed statements from `<name>.zen` located in the
/// same directory as `source_path`. Recursively resolves modules within loaded files.
pub fn resolve_modules(program: &mut Program, source_path: &Path) -> Result<()> {
    let parent_dir = source_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let mut loaded = HashSet::new();
    // Prevent the main file from being loaded as a module of itself
    if let Some(stem) = source_path.file_stem() {
        loaded.insert(stem.to_string_lossy().into_owned());
    }
    resolve_stmts(&mut program.stmts, parent_dir, &mut loaded)
}

fn resolve_stmts(
    stmts: &mut Vec<Spanned<Stmt>>,
    parent_dir: &Path,
    loaded: &mut HashSet<String>,
) -> Result<()> {
    let mut i = 0;
    while i < stmts.len() {
        let is_mod = matches!(&stmts[i].node, Stmt::Mod { .. });
        if is_mod {
            let body_empty = matches!(&stmts[i].node, Stmt::Mod { body, .. } if body.is_empty());
            if body_empty {
                let name = match &stmts[i].node {
                    Stmt::Mod { name, .. } => name.clone(),
                    _ => unreachable!(),
                };
                // File-backed module — load from <name>.zen
                let module_path = parent_dir.join(format!("{}.zen", name));
                let module_name = name.to_string();
                if !loaded.insert(module_name) {
                    // Already loaded — skip (circular / duplicate reference)
                    i += 1;
                    continue;
                }
                let source = std::fs::read_to_string(&module_path)
                    .map_err(|e| Error::Io { source: e })?;
                let tokens = crate::lexer::Lexer::new(&source)
                    .tokenize()
                    .map_err(|e| Error::ModResolution {
                        module: name.to_string(),
                        source: Box::new(e),
                    })?;
                let mut module_program = crate::parser::Parser::new(&source, &tokens)
                    .parse()
                    .map_err(|e| Error::ModResolution {
                        module: name.to_string(),
                        source: Box::new(e),
                    })?;
                // Resolve nested file-backed modules within the loaded file
                let module_dir = module_path.parent().unwrap_or(parent_dir);
                resolve_stmts(&mut module_program.stmts, module_dir, loaded)?;
                // Replace the empty Mod body with the loaded stmts
                if let Stmt::Mod { body, .. } = &mut stmts[i].node {
                    *body = module_program.stmts;
                }
            } else {
                // Inline module — get mutable access to body and recurse
                let body = match &mut stmts[i].node {
                    Stmt::Mod { body, .. } => body,
                    _ => unreachable!(),
                };
                resolve_stmts(body, parent_dir, loaded)?;
            }
        }
        i += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn parse(source: &str) -> Program {
        let tokens = Lexer::new(source).tokenize().unwrap();
        Parser::new(source, &tokens).parse().unwrap()
    }

    #[test]
    fn test_inline_mod_resolved() {
        let mut program = parse(r#"
            mod math {
                fn add(a: i64, b: i64) -> i64 { a + b }
            }
            use math::add;
            add(2, 3)
        "#);
        // Inline modules have non-empty bodies; resolve_modules should pass through
        resolve_modules(&mut program, Path::new("test.zen")).unwrap();
        // Verify the inline mod survived
        assert_eq!(program.stmts.len(), 3);
    }

    #[test]
    fn test_empty_mod_needs_file() {
        // `mod foo;` should fail because foo.zen doesn't exist
        let mut program = parse("mod foo;");
        let result = resolve_modules(&mut program, Path::new("test.zen"));
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::Io { .. } => {} // expected — file not found
            other => panic!("expected Io error, got {other}"),
        }
    }

    #[test]
    fn test_mod_resolution_wraps_lex_errors() {
        // Create a temp dir with a module file with syntax errors
        let dir = std::env::temp_dir().join("zen_mod_test");
        let _ = std::fs::create_dir_all(&dir);
        let mod_file = dir.join("bad_syntax.zen");
        std::fs::write(&mod_file, "fn broken(").unwrap();

        // The source_path should be the directory containing the mod, not the mod itself
        let source_path = dir.join("main.zen");
        std::fs::write(&source_path, "mod bad_syntax;").unwrap();

        let mut program = parse("mod bad_syntax;");
        let result = resolve_modules(&mut program, &source_path);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::ModResolution { module, .. } => {
                assert_eq!(module, "bad_syntax");
            }
            other => panic!("expected ModResolution error, got {other}"),
        }
    }
}
