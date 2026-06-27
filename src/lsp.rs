#![allow(deprecated)]

use std::collections::HashMap;

use lsp_types::notification::{DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification};
use lsp_types::request::{HoverRequest, Request};
use lsp_types::*;

use crate::ast::{Program, Stmt};
use crate::compiler;
use crate::error::Error;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::resolver;
use crate::span::{Span, Spanned};
use crate::symbol::{SymKind, SymbolTable};
use crate::typeck::{self, TypeMap};
use crate::token::TokenKind;

type DocUri = lsp_types::Uri;

struct DocumentState {
    source: String,
    program: Program,
    symbols: SymbolTable,
    types: TypeMap,
}

fn offset_to_position(source: &str, offset: usize) -> Position {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, c) in source.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Position { line, character: col }
}

fn span_to_range(source: &str, span: Span) -> Range {
    Range {
        start: offset_to_position(source, span.start()),
        end: offset_to_position(source, span.end()),
    }
}

fn error_to_diagnostics(source: &str, err: &Error) -> Vec<Diagnostic> {
    let (location, msg) = match err {
        Error::Parse { location, msg } => (location, msg),
        Error::TypeError { location, msg } => (location, msg),
        Error::Resolve { location, msg } => (location, msg),
        Error::Compile { location, msg } => (location, msg),
        Error::Runtime { msg, .. } => {
            return vec![Diagnostic {
                range: Range { start: Position { line: 0, character: 0 }, end: Position { line: 0, character: 0 } },
                severity: Some(DiagnosticSeverity::ERROR),
                message: msg.clone(),
                ..Default::default()
            }];
        }
        Error::Io { .. } => return Vec::new(),
        Error::Script { msg } => {
            return vec![Diagnostic {
                range: Range { start: Position { line: 0, character: 0 }, end: Position { line: 0, character: 0 } },
                severity: Some(DiagnosticSeverity::ERROR),
                message: msg.clone(),
                ..Default::default()
            }];
        }
        Error::ParseMultiple { errors } | Error::CompileMultiple { errors } => {
            return errors.iter().flat_map(|e| error_to_diagnostics(source, e)).collect();
        }
    };
    let range = span_to_range(source, location.span);
    vec![Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        message: msg.clone(),
        source: Some("zenlang".into()),
        ..Default::default()
    }]
}

fn compile_source(source: &str) -> Result<DocumentState, Vec<Diagnostic>> {
    let _tokens = match Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(e) => return Err(error_to_diagnostics(source, &e)),
    };
    let parser = Parser::new(&_tokens);
    let mut program = match parser.parse() {
        Ok(p) => p,
        Err(e) => return Err(error_to_diagnostics(source, &e)),
    };
    let native_names = crate::stdlib::native_names();
    let mut symbols = match resolver::resolve_with_natives(&mut program, &native_names) {
        Ok(s) => s,
        Err(e) => return Err(error_to_diagnostics(source, &e)),
    };
    let types = match typeck::check(&program, &mut symbols) {
        Ok(t) => t,
        Err(e) => return Err(error_to_diagnostics(source, &e)),
    };
    if let Err(e) = compiler::compile(&program, &types, &symbols, &native_names, source) {
        return Err(error_to_diagnostics(source, &e));
    }
    Ok(DocumentState { source: source.to_string(), program, symbols, types })
}

pub fn run_server() {
    let (mut connection, io_threads) = lsp_server::Connection::stdio();

    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
            ..Default::default()
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        semantic_tokens_provider: Some(
            SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                legend: SemanticTokensLegend {
                    token_types: vec![
                        SemanticTokenType::KEYWORD,
                        SemanticTokenType::STRING,
                        SemanticTokenType::NUMBER,
                        SemanticTokenType::VARIABLE,
                        SemanticTokenType::FUNCTION,
                        SemanticTokenType::TYPE,
                        SemanticTokenType::OPERATOR,
                    ],
                    token_modifiers: vec![],
                },
                full: Some(SemanticTokensFullOptions::Bool(true)),
                ..Default::default()
            }),
        ),
        ..Default::default()
    };

    let init = serde_json::json!({
        "capabilities": capabilities,
    });
    connection.initialize(init).unwrap();

    let mut docs: HashMap<DocUri, DocumentState> = HashMap::new();
    main_loop(&mut connection, &mut docs);
    io_threads.join().unwrap();
}

fn main_loop(connection: &mut lsp_server::Connection, docs: &mut HashMap<DocUri, DocumentState>) {
    loop {
        let msg = match connection.receiver.recv() {
            Ok(msg) => msg,
            Err(_) => return,
        };
        match msg {
            lsp_server::Message::Request(req) => {
                if connection.handle_shutdown(&req).unwrap_or(false) {
                    return;
                }
                handle_request(connection, docs, req);
            }
            lsp_server::Message::Notification(notif) => {
                handle_notification(connection, docs, notif);
            }
            lsp_server::Message::Response(_resp) => {}
        }
    }
}

fn handle_request(
    connection: &mut lsp_server::Connection,
    docs: &HashMap<DocUri, DocumentState>,
    req: lsp_server::Request,
) {
    let result: Option<serde_json::Value> = match req.method.as_str() {
        HoverRequest::METHOD => {
            let params: HoverParams = serde_json::from_value(req.params).unwrap();
            let uri = params.text_document_position_params.text_document.uri;
            let state = docs.get(&uri);
            let pos = params.text_document_position_params.position;
            serde_json::to_value(state.and_then(|s| hover(s, &pos))).ok()
        }
        "textDocument/completion" => {
            let params: CompletionParams = serde_json::from_value(req.params).unwrap();
            let uri = params.text_document_position.text_document.uri;
            let state = docs.get(&uri);
            let pos = params.text_document_position.position;
            serde_json::to_value(state.and_then(|s| completion(s, &pos))).ok()
        }
        "textDocument/definition" => {
            let params: GotoDefinitionParams = serde_json::from_value(req.params).unwrap();
            serde_json::to_value(goto_definition(docs, &params.text_document_position_params)).ok()
        }
        "textDocument/documentSymbol" => {
            let params: DocumentSymbolParams = serde_json::from_value(req.params).unwrap();
            let uri = params.text_document.uri;
            let state = docs.get(&uri);
            serde_json::to_value(state.and_then(document_symbols)).ok()
        }
        "textDocument/semanticTokens/full" => {
            let params: SemanticTokensParams = serde_json::from_value(req.params).unwrap();
            let uri = params.text_document.uri;
            let state = docs.get(&uri);
            serde_json::to_value(state.and_then(semantic_tokens)).ok()
        }
        _ => None,
    };
    let response = lsp_server::Response {
        id: req.id,
        result,
        error: None,
    };
    connection.sender.send(lsp_server::Message::Response(response)).unwrap();
}

fn handle_notification(
    connection: &mut lsp_server::Connection,
    docs: &mut HashMap<DocUri, DocumentState>,
    notif: lsp_server::Notification,
) {
    match notif.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(notif.params).unwrap();
            let uri_clone = params.text_document.uri.clone();
            let source = params.text_document.text;
            match compile_source(&source) {
                Ok(state) => {
                    docs.insert(uri_clone.clone(), state);
                    send_diagnostics(connection, uri_clone, Vec::new());
                }
                Err(diags) => {
                    let tokens = Lexer::new(&source).tokenize().ok().unwrap_or_default();
                    let (program, symbols, types) = if let Ok(parser) = Parser::new(&tokens).parse() {
                        let mut p = parser;
                        let native_names = crate::stdlib::native_names();
                        let mut s = resolver::resolve_with_natives(&mut p, &native_names).ok().unwrap_or_else(SymbolTable::new);
                        let t = typeck::check(&p, &mut s).ok().unwrap_or_else(TypeMap::new);
                        (p, s, t)
                    } else {
                        (Program::new(), SymbolTable::new(), TypeMap::new())
                    };
                    docs.insert(uri_clone.clone(), DocumentState { source, program, symbols, types });
                    send_diagnostics(connection, uri_clone, diags);
                }
            }
        }
        DidChangeTextDocument::METHOD => {
            let params: DidChangeTextDocumentParams = serde_json::from_value(notif.params).unwrap();
            let uri_clone = params.text_document.uri.clone();
            if let Some(change) = params.content_changes.into_iter().last() {
                let source = change.text;
                match compile_source(&source) {
                    Ok(state) => {
                        docs.insert(uri_clone.clone(), state);
                        send_diagnostics(connection, uri_clone, Vec::new());
                    }
                    Err(diags) => {
                        let tokens = Lexer::new(&source).tokenize().ok().unwrap_or_default();
                        let (program, symbols, types) = if let Ok(parser) = Parser::new(&tokens).parse() {
                            let mut p = parser;
                            let native_names = crate::stdlib::native_names();
                            let mut s = resolver::resolve_with_natives(&mut p, &native_names).ok().unwrap_or_else(SymbolTable::new);
                            let t = typeck::check(&p, &mut s).ok().unwrap_or_else(TypeMap::new);
                            (p, s, t)
                        } else {
                            (Program::new(), SymbolTable::new(), TypeMap::new())
                        };
                        docs.insert(uri_clone.clone(), DocumentState { source, program, symbols, types });
                        send_diagnostics(connection, uri_clone, diags);
                    }
                }
            }
        }
        DidCloseTextDocument::METHOD => {
            let params: DidCloseTextDocumentParams = serde_json::from_value(notif.params).unwrap();
            docs.remove(&params.text_document.uri);
        }
        _ => {}
    }
}

fn send_diagnostics(connection: &mut lsp_server::Connection, uri: DocUri, diags: Vec<Diagnostic>) {
    let params = PublishDiagnosticsParams {
        uri,
        diagnostics: diags,
        version: None,
    };
    let notif = lsp_server::Notification::new(
        "textDocument/publishDiagnostics".to_string(),
        serde_json::to_value(params).unwrap(),
    );
    connection.sender.send(lsp_server::Message::Notification(notif)).unwrap();
}

fn hover(state: &DocumentState, _pos: &Position) -> Option<Hover> {
    state.symbols.symbols.last().map(|(name, kind)| Hover {
        contents: HoverContents::Scalar(MarkedString::String(format!("{}: {}", name, symkind_display(kind)))),
        range: None,
    })
}

fn completion(state: &DocumentState, _pos: &Position) -> Option<CompletionResponse> {
    let mut items = Vec::new();
    for kw in &[
        "fn", "let", "mut", "if", "else", "while", "for", "loop",
        "break", "continue", "return", "match", "in", "struct", "enum",
        "impl", "true", "false",
    ] {
        items.push(CompletionItem {
            label: kw.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            ..Default::default()
        });
    }
    for (name, kind) in &state.symbols.symbols {
        let item_kind = match kind {
            SymKind::Variable(_) => CompletionItemKind::VARIABLE,
            SymKind::Function(_) => CompletionItemKind::FUNCTION,
            SymKind::Struct(_) => CompletionItemKind::STRUCT,
            SymKind::Enum(_) => CompletionItemKind::ENUM,
        };
        items.push(CompletionItem {
            label: name.clone(),
            kind: Some(item_kind),
            detail: Some(symkind_display(kind)),
            ..Default::default()
        });
    }
    if items.is_empty() { None } else { Some(CompletionResponse::Array(items)) }
}

fn goto_definition(
    _docs: &HashMap<DocUri, DocumentState>,
    _params: &TextDocumentPositionParams,
) -> Option<GotoDefinitionResponse> {
    None
}

fn document_symbols(state: &DocumentState) -> Option<Vec<DocumentSymbol>> {
    let mut symbols = Vec::new();
    for stmt in &state.program.stmts {
        if let Some(sym) = stmt_to_document_symbol(stmt, &state.source) {
            symbols.push(sym);
        }
    }
    if symbols.is_empty() { None } else { Some(symbols) }
}

fn stmt_to_document_symbol(stmt: &Spanned<Stmt>, source: &str) -> Option<DocumentSymbol> {
    let range = span_to_range(source, stmt.span);
    match &stmt.node {
        Stmt::Fn { name, params, .. } => {
            let detail = Some(format!("fn({})", params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ")));
            Some(DocumentSymbol {
                name: name.clone(),
                detail,
                kind: SymbolKind::FUNCTION,
                range,
                selection_range: range,
                children: None,
                deprecated: None,
                tags: None,
            })
        }
        Stmt::Struct { name, fields } => {
            let children = fields.iter().map(|f| DocumentSymbol {
                name: f.name.clone(),
                detail: Some(format!("{}", type_display(&f.type_ann))),
                kind: SymbolKind::FIELD,
                range,
                selection_range: range,
                children: None,
                deprecated: None,
                tags: None,
            }).collect();
            Some(DocumentSymbol {
                name: name.clone(),
                detail: None,
                kind: SymbolKind::STRUCT,
                range,
                selection_range: range,
                children: Some(children),
                deprecated: None,
                tags: None,
            })
        }
        Stmt::Enum { name, variants } => {
            let children = variants.iter().map(|v| DocumentSymbol {
                name: v.name.clone(),
                detail: None,
                kind: SymbolKind::ENUM_MEMBER,
                range,
                selection_range: range,
                children: None,
                deprecated: None,
                tags: None,
            }).collect();
            Some(DocumentSymbol {
                name: name.clone(),
                detail: None,
                kind: SymbolKind::ENUM,
                range,
                selection_range: range,
                children: Some(children),
                deprecated: None,
                tags: None,
            })
        }
        Stmt::Let { name, .. } => {
            Some(DocumentSymbol {
                name: name.clone(),
                detail: None,
                kind: SymbolKind::VARIABLE,
                range,
                selection_range: range,
                children: None,
                deprecated: None,
                tags: None,
            })
        }
        _ => None,
    }
}

fn semantic_tokens(state: &DocumentState) -> Option<SemanticTokensResult> {
    let mut tokens: Vec<SemanticToken> = Vec::new();
    let mut prev_line = 0u32;
    let mut prev_col = 0u32;

    let lex_tokens = Lexer::new(&state.source).tokenize().ok()?;
    for spanned in &lex_tokens {
        let pos = offset_to_position(&state.source, spanned.span.start());
        let len = (spanned.span.end() - spanned.span.start()) as u32;
        if len == 0 {
            continue;
        }

        let tok = &spanned.node;
        let token_type = match &tok.kind {
            TokenKind::Fn | TokenKind::Let | TokenKind::Mut
            | TokenKind::If | TokenKind::Else | TokenKind::While
            | TokenKind::For | TokenKind::Loop | TokenKind::Break
            | TokenKind::Continue | TokenKind::Return | TokenKind::Match
            | TokenKind::In | TokenKind::Struct | TokenKind::Enum
            | TokenKind::Impl | TokenKind::Self_ | TokenKind::Pub
            | TokenKind::Use | TokenKind::Mod | TokenKind::Const
            | TokenKind::Type | TokenKind::Underscore => SemanticTokenType::KEYWORD,
            TokenKind::Str(_) => SemanticTokenType::STRING,
            TokenKind::Int(_) | TokenKind::Float(_) => SemanticTokenType::NUMBER,
            TokenKind::Bool(_) => SemanticTokenType::KEYWORD,
            TokenKind::Ident(_) => {
                let ident_name = tok.lexeme.to_string();
                if let Some(entry) = state.symbols.lookup(&ident_name) {
                    match &entry.kind {
                        SymKind::Function(_) => SemanticTokenType::FUNCTION,
                        SymKind::Variable(_) => SemanticTokenType::VARIABLE,
                        SymKind::Struct(_) | SymKind::Enum(_) => SemanticTokenType::TYPE,
                    }
                } else {
                    SemanticTokenType::VARIABLE
                }
            }
            TokenKind::Plus | TokenKind::Minus | TokenKind::Star
            | TokenKind::Slash | TokenKind::Percent | TokenKind::Eq
            | TokenKind::EqEq | TokenKind::Ne | TokenKind::Lt
            | TokenKind::Gt | TokenKind::Le | TokenKind::Ge
            | TokenKind::And | TokenKind::AndAnd | TokenKind::Or
            | TokenKind::OrOr | TokenKind::Bang | TokenKind::Dot
            | TokenKind::DotDot | TokenKind::DotDotEq | TokenKind::Comma
            | TokenKind::Semi | TokenKind::Colon | TokenKind::ColonColon
            | TokenKind::Arrow | TokenKind::FatArrow => SemanticTokenType::OPERATOR,
            _ => continue,
        };

        let token_type_idx = token_type_index(token_type);

        let delta_line = if prev_line == 0 && prev_col == 0 {
            pos.line
        } else if pos.line == prev_line {
            0
        } else {
            pos.line - prev_line
        };
        let delta_col = if pos.line == prev_line {
            pos.character - prev_col
        } else {
            pos.character
        };

        tokens.push(SemanticToken {
            delta_line,
            delta_start: delta_col,
            length: len,
            token_type: token_type_idx,
            token_modifiers_bitset: 0,
        });
        prev_line = pos.line;
        prev_col = pos.character + len;
    }

    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data: tokens,
    }))
}

fn token_type_index(ty: SemanticTokenType) -> u32 {
    match ty {
        t if t == SemanticTokenType::KEYWORD => 0,
        t if t == SemanticTokenType::STRING => 1,
        t if t == SemanticTokenType::NUMBER => 2,
        t if t == SemanticTokenType::VARIABLE => 3,
        t if t == SemanticTokenType::FUNCTION => 4,
        t if t == SemanticTokenType::TYPE => 5,
        t if t == SemanticTokenType::OPERATOR => 6,
        _ => 0,
    }
}

fn symkind_display(kind: &SymKind) -> String {
    match kind {
        SymKind::Variable(ty) => type_display(ty),
        SymKind::Function(sig) => {
            let params: Vec<String> = sig.params.iter().map(|(n, t)| format!("{}: {}", n, type_display(t))).collect();
            format!("fn({}) -> {}", params.join(", "), sig.return_type.as_ref().map(|t| type_display(t)).unwrap_or_else(|| "?".to_string()))
        }
        SymKind::Struct(def) => format!("struct {{ {} }}", def.fields.iter().map(|f| format!("{}: {}", f.name, type_display(&f.type_ann))).collect::<Vec<_>>().join(", ")),
        SymKind::Enum(def) => format!("enum {{ {} }}", def.variants.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")),
    }
}

fn type_display(ty: &crate::ast::Type) -> String {
    match ty {
        crate::ast::Type::I32 => "int".into(),
        crate::ast::Type::F64 => "float".into(),
        crate::ast::Type::Bool => "bool".into(),
        crate::ast::Type::Str => "str".into(),
        crate::ast::Type::Unit => "()".into(),
        crate::ast::Type::Named(n) => n.clone(),
        crate::ast::Type::Array(inner) => format!("[{}]", type_display(inner)),
        crate::ast::Type::Fn { params, ret } => {
            let ps: Vec<String> = params.iter().map(type_display).collect();
            format!("fn({}) -> {}", ps.join(", "), type_display(ret))
        }
    }
}
