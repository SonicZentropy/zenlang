#![allow(deprecated)]

use std::collections::HashMap;

use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    Notification,
};
use lsp_types::request::{Formatting, HoverRequest, Request};
use lsp_types::*;
use tracing::{debug, info, trace, warn};

use crate::ast::{Expr, Pattern, Program, Stmt};
use crate::compiler;
use crate::error::Error;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::resolver;
use crate::span::{Span, Spanned};
use crate::symbol::{SymKind, SymbolTable};
use crate::token::TokenKind;
use crate::typeck::{self, TypeMap};

type DocUri = lsp_types::Uri;

#[allow(dead_code)]
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
    Position {
        line,
        character: col,
    }
}

fn position_to_offset(source: &str, pos: Position) -> Option<usize> {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, c) in source.char_indices() {
        if line == pos.line && col == pos.character {
            return Some(i);
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    // Position at EOF
    if line == pos.line && col == pos.character {
        return Some(source.len());
    }
    None
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
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 0,
                    },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                message: msg.clone(),
                ..Default::default()
            }];
        }
        Error::Io { .. } => return Vec::new(),
        Error::Script { msg } => {
            return vec![Diagnostic {
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 0,
                    },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                message: msg.clone(),
                ..Default::default()
            }];
        }
        Error::ParseMultiple { errors } | Error::CompileMultiple { errors } => {
            return errors
                .iter()
                .flat_map(|e| error_to_diagnostics(source, e))
                .collect();
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
    trace!("compilation: lexing");
    let _tokens = match Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(e) => {
            trace!("compilation: lexing failed");
            return Err(error_to_diagnostics(source, &e));
        }
    };
    trace!("compilation: parsing");
    let parser = Parser::new(source, &_tokens);
    let mut program = match parser.parse() {
        Ok(p) => p,
        Err(e) => {
            trace!("compilation: parsing failed");
            return Err(error_to_diagnostics(source, &e));
        }
    };
    trace!("compilation: resolving");
    let native_names = crate::stdlib::native_names();
    let mut symbols = match resolver::resolve_with_natives(&mut program, &native_names) {
        Ok(s) => s,
        Err(e) => {
            trace!("compilation: resolution failed");
            return Err(error_to_diagnostics(source, &e));
        }
    };
    trace!("compilation: type-checking");
    let types = match typeck::check(&program, &mut symbols) {
        Ok(t) => t,
        Err(e) => {
            trace!("compilation: type-checking failed");
            return Err(error_to_diagnostics(source, &e));
        }
    };
    trace!("compilation: codegen");
    if let Err(e) = compiler::compile(&program, &types, &symbols, &native_names, source) {
        trace!("compilation: codegen failed");
        return Err(error_to_diagnostics(source, &e));
    }
    trace!("compilation: success");
    Ok(DocumentState {
        source: source.to_string(),
        program,
        symbols,
        types,
    })
}

/// Resolve the path for the persistent log file.
///
/// Order of precedence:
/// 1. `ZENLANG_LSP_LOG` env var
/// 2. `./zenlang-lsp.log` (cwd)
/// 3. `{temp_dir}/zenlang-lsp.log`
fn resolve_log_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("ZENLANG_LSP_LOG") {
        return std::path::PathBuf::from(p);
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    let candidate = cwd.join("zenlang-lsp.log");
    if std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&candidate)
        .is_ok()
    {
        return candidate;
    }
    std::env::temp_dir().join("zenlang-lsp.log")
}

/// Initialise tracing for the LSP server.
///
/// Writes to **stderr** (primary — Zed always captures this and surfaces
/// it in its logs / LSP pane) and to a file for persistence (path resolved
/// by [`resolve_log_path`]).
fn init_lsp_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    let log_path = resolve_log_path();

    eprintln!("[zenlang lsp] starting, log: {}", log_path.display());

    // File writer (non‑blocking so the LSP is never stalled by I/O)
    let log_dir = log_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let log_name = log_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "zenlang-lsp.log".into());
    let file_appender = tracing_appender::rolling::never(&log_dir, &log_name);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Two layers: one for stderr, one for the file.
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "zenlang=info".into()),
        );

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "zenlang=debug".into()),
        );

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(file_layer)
        .init();

    eprintln!("[zenlang lsp] ready");
    guard
}

/// Start the LSP language server on stdin/stdout.
///
/// Compatible with Neovim's built-in LSP, VS Code, and any editor that
/// speaks the LSP protocol over stdio. Provides:
/// - Text-sync diagnostics
/// - Completions
/// - Hover type info
/// - Document symbols
/// - Semantic token coloring
/// - Go-to-definition
pub fn run_server() {
    let _guard = init_lsp_tracing();
    info!("LSP server starting");

    let (mut connection, io_threads) = lsp_server::Connection::stdio();

    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        // NOTE: Kind(FULL/INCREMENTAL) and Options({change,openClose,save})
        // have both been tried — Zed still does NOT send didChange/didSave.
        // The root cause appears to be on the client (Zed) side.
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
            ..Default::default()
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
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
            },
        )),
        document_formatting_provider: Some(OneOf::Left(true)),
        ..Default::default()
    };

    let init = serde_json::to_value(&capabilities).unwrap();
    connection.initialize(init).unwrap();
    info!("LSP server initialized with capabilities");

    let mut docs: HashMap<DocUri, DocumentState> = HashMap::new();
    main_loop(&mut connection, &mut docs);
    info!("LSP server shutting down");
    io_threads.join().unwrap();
}

fn main_loop(connection: &mut lsp_server::Connection, docs: &mut HashMap<DocUri, DocumentState>) {
    loop {
        let msg = match connection.receiver.recv() {
            Ok(msg) => msg,
            Err(e) => {
                debug!("main_loop recv error: {e}");
                return;
            }
        };
        match msg {
            lsp_server::Message::Request(req) => {
                debug!("received request: {} (id={:?})", req.method, req.id);
                if connection.handle_shutdown(&req).unwrap_or(false) {
                    info!("received shutdown request");
                    return;
                }
                handle_request(connection, docs, req);
            }
            lsp_server::Message::Notification(notif) => {
                debug!("received notification: {}", notif.method);
                handle_notification(connection, docs, notif);
            }
            lsp_server::Message::Response(resp) => {
                trace!("received response (id={:?})", resp.id);
            }
        }
    }
}

fn handle_request(
    connection: &mut lsp_server::Connection,
    docs: &HashMap<DocUri, DocumentState>,
    req: lsp_server::Request,
) {
    let uri_hint: String = match req
        .params
        .get("textDocument")
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
    {
        Some(u) => u.to_string(),
        None => "<unknown>".into(),
    };
    debug!("handling request: {} for {}", req.method, uri_hint);

    let result: Option<serde_json::Value> = match req.method.as_str() {
        HoverRequest::METHOD => {
            let params: HoverParams = serde_json::from_value(req.params).unwrap();
            let uri = params.text_document_position_params.text_document.uri;
            let state = docs.get(&uri);
            let pos = params.text_document_position_params.position;
            let r = state.and_then(|s| hover(s, &pos));
            if r.is_some() {
                debug!("hover: returning result for {uri:?}");
            } else {
                trace!("hover: no result for {uri:?}");
            }
            serde_json::to_value(r).ok()
        }
        "textDocument/completion" => {
            let params: CompletionParams = serde_json::from_value(req.params).unwrap();
            let uri = params.text_document_position.text_document.uri;
            let state = docs.get(&uri);
            let pos = params.text_document_position.position;
            let r = state.and_then(|s| completion(s, &pos));
            debug!(
                "completion: {} items for {uri:?}",
                r.as_ref()
                    .map(|c| match c {
                        CompletionResponse::Array(items) => items.len(),
                        _ => 0,
                    })
                    .unwrap_or(0)
            );
            serde_json::to_value(r).ok()
        }
        "textDocument/definition" => {
            let params: GotoDefinitionParams = serde_json::from_value(req.params).unwrap();
            let r = goto_definition(docs, &params.text_document_position_params);
            trace!("definition: returning {r:?}");
            serde_json::to_value(r).ok()
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
        Formatting::METHOD => {
            let params: DocumentFormattingParams = serde_json::from_value(req.params).unwrap();
            let uri = params.text_document.uri;
            let state = docs.get(&uri);
            let tab_size = params.options.tab_size as usize;
            let r = state.and_then(|s| format_document(s, &uri, tab_size));
            serde_json::to_value(&r).ok()
        }
        method => {
            debug!("unhandled request method: {method}");
            None
        }
    };
    let response = lsp_server::Response {
        id: req.id,
        result,
        error: None,
    };
    connection
        .sender
        .send(lsp_server::Message::Response(response))
        .unwrap();
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
            info!("didOpen: {uri_clone:?}");
            trace!(
                "didOpen source ({} chars):\n{}",
                source.len(),
                &source[..source.len().min(512)]
            );

            match compile_source(&source) {
                Ok(state) => {
                    docs.insert(uri_clone.clone(), state);
                    send_diagnostics(connection, uri_clone, Vec::new());
                }
                Err(diags) => {
                    warn!(
                        "didOpen: compilation produced {} diagnostic(s) for {uri_clone:?}",
                        diags.len()
                    );
                    let tokens = Lexer::new(&source).tokenize().ok().unwrap_or_default();
                    let (program, symbols, types) = if let Ok(parser) = Parser::new(&source, &tokens).parse()
                    {
                        let mut p = parser;
                        let native_names = crate::stdlib::native_names();
                        let mut s = resolver::resolve_with_natives(&mut p, &native_names)
                            .ok()
                            .unwrap_or_else(SymbolTable::new);
                        let t = typeck::check(&p, &mut s).ok().unwrap_or_else(TypeMap::new);
                        (p, s, t)
                    } else {
                        (Program::new(), SymbolTable::new(), TypeMap::new())
                    };
                    docs.insert(
                        uri_clone.clone(),
                        DocumentState {
                            source,
                            program,
                            symbols,
                            types,
                        },
                    );
                    send_diagnostics(connection, uri_clone, diags);
                }
            }
        }
        DidChangeTextDocument::METHOD => {
            let params: DidChangeTextDocumentParams = serde_json::from_value(notif.params).unwrap();
            let uri_clone = params.text_document.uri.clone();
            debug!("didChange: {uri_clone:?}");

            // Apply every content change to the stored source (supports
            // both INCREMENTAL and FULL sync).
            let changed = params.content_changes.iter().any(|c| c.range.is_some());
            let source = if let Some(state) = docs.get_mut(&uri_clone) {
                for change in params.content_changes {
                    if let Some(range) = change.range {
                        // Incremental — replace the range with the new text.
                        if let (Some(start), Some(end)) = (
                            position_to_offset(&state.source, range.start),
                            position_to_offset(&state.source, range.end),
                        ) {
                            let mut buf = String::with_capacity(
                                state.source.len() + change.text.len() - (end - start),
                            );
                            buf.push_str(&state.source[..start]);
                            buf.push_str(&change.text);
                            buf.push_str(&state.source[end..]);
                            state.source = buf;
                        } else {
                            warn!("didChange: invalid range for {uri_clone:?}, skipping");
                        }
                    } else {
                        // Full — replace the whole document.
                        state.source = change.text;
                    }
                }
                if changed {
                    trace!(
                        "source after incremental changes:\n{}",
                        &state.source[..state.source.len().min(512)]
                    );
                }
                state.source.clone()
            } else {
                warn!("didChange: no prior state for {uri_clone:?}, treating as open");
                params
                    .content_changes
                    .into_iter()
                    .last()
                    .map(|c| c.text)
                    .unwrap_or_default()
            };

            match compile_source(&source) {
                Ok(new_state) => {
                    docs.insert(uri_clone.clone(), new_state);
                    send_diagnostics(connection, uri_clone, Vec::new());
                }
                Err(diags) => {
                    debug!(
                        "didChange: compilation produced {} diagnostic(s) for {uri_clone:?}",
                        diags.len()
                    );
                    // If we can parse, salvage partial state for semantic tokens etc.
                    let tokens = Lexer::new(&source).tokenize().ok().unwrap_or_default();
                    let (program, symbols, types) = if let Ok(parser) = Parser::new(&source, &tokens).parse()
                    {
                        let mut p = parser;
                        let native_names = crate::stdlib::native_names();
                        let mut s = resolver::resolve_with_natives(&mut p, &native_names)
                            .ok()
                            .unwrap_or_else(SymbolTable::new);
                        let t = typeck::check(&p, &mut s).ok().unwrap_or_else(TypeMap::new);
                        (p, s, t)
                    } else {
                        (Program::new(), SymbolTable::new(), TypeMap::new())
                    };
                    docs.insert(
                        uri_clone.clone(),
                        DocumentState {
                            source,
                            program,
                            symbols,
                            types,
                        },
                    );
                    send_diagnostics(connection, uri_clone, diags);
                }
            }
        }
        DidSaveTextDocument::METHOD => {
            let params: DidSaveTextDocumentParams = serde_json::from_value(notif.params).unwrap();
            let uri = &params.text_document.uri;
            debug!("didSave: {uri:?}");
            if let Some(state) = docs.get(uri) {
                let source = state.source.clone();
                match compile_source(&source) {
                    Ok(new_state) => {
                        docs.insert(uri.clone(), new_state);
                        send_diagnostics(connection, uri.clone(), Vec::new());
                    }
                    Err(diags) => {
                        debug!(
                            "didSave: compilation produced {} diagnostic(s) for {uri:?}",
                            diags.len()
                        );
                        let tokens = Lexer::new(&source).tokenize().ok().unwrap_or_default();
                        let (program, symbols, types) =
                            if let Ok(parser) = Parser::new(&source, &tokens).parse() {
                                let mut p = parser;
                                let native_names = crate::stdlib::native_names();
                                let mut s = resolver::resolve_with_natives(&mut p, &native_names)
                                    .ok()
                                    .unwrap_or_else(SymbolTable::new);
                                let t = typeck::check(&p, &mut s).ok().unwrap_or_else(TypeMap::new);
                                (p, s, t)
                            } else {
                                (Program::new(), SymbolTable::new(), TypeMap::new())
                            };
                        docs.insert(
                            uri.clone(),
                            DocumentState {
                                source,
                                program,
                                symbols,
                                types,
                            },
                        );
                        send_diagnostics(connection, uri.clone(), diags);
                    }
                }
            }
        }
        DidCloseTextDocument::METHOD => {
            let params: DidCloseTextDocumentParams = serde_json::from_value(notif.params).unwrap();
            docs.remove(&params.text_document.uri);
            info!("didClose: {:?}", params.text_document.uri);
        }
        method => {
            debug!("unhandled notification: {method}");
        }
    }
}

fn send_diagnostics(connection: &mut lsp_server::Connection, uri: DocUri, diags: Vec<Diagnostic>) {
    debug!(
        "send_diagnostics: {} diagnostic(s) for {uri:?}",
        diags.len()
    );
    for d in &diags {
        trace!(
            "  diag: line {}:{} - {}",
            d.range.start.line, d.range.start.character, d.message,
        );
    }
    let params = PublishDiagnosticsParams {
        uri,
        diagnostics: diags,
        version: None,
    };
    let notif = lsp_server::Notification::new(
        "textDocument/publishDiagnostics".to_string(),
        serde_json::to_value(params).unwrap(),
    );
    connection
        .sender
        .send(lsp_server::Message::Notification(notif))
        .unwrap();
}

fn hover(state: &DocumentState, pos: &Position) -> Option<Hover> {
    let offset = position_to_offset(&state.source, *pos)?;
    let source = state.source.as_bytes();
    if offset >= source.len() {
        return None;
    }
    // Extract the identifier at the cursor
    if !source[offset].is_ascii_alphanumeric() && source[offset] != b'_' {
        return None;
    }
    let start = (0..offset)
        .rev()
        .take_while(|&i| source[i].is_ascii_alphanumeric() || source[i] == b'_')
        .last()
        .unwrap_or(offset);
    let end = offset
        + 1
        + (offset + 1..source.len())
            .take_while(|&i| source[i].is_ascii_alphanumeric() || source[i] == b'_')
            .count();
    let name = &state.source[start..end];

    let kind = state
        .symbols
        .symbols
        .iter()
        .rev()
        .find(|(n, _)| n == name)
        .map(|(_, k)| k)?;
    // Find the definition's top-level statement for its comment
    let def_span = state
        .program
        .stmts
        .iter()
        .find(|s| match &s.node {
            Stmt::Fn { name: n, .. }
            | Stmt::Struct { name: n, .. }
            | Stmt::Enum { name: n, .. } => n == name,
            Stmt::Impl { type_name, .. } => type_name == name,
            Stmt::Trait { name: n, .. } => n == name,
            _ => false,
        })
        .map(|s| s.span);

    let mut text = String::new();
    if let Some(span) = def_span {
        if let Some(c) = comment_before(&state.source, span.start()) {
            text.push_str(&c);
            text.push_str("\n\n");
        }
    }
    text.push_str(&format!("`{}`: {}", name, symkind_display(kind)));
    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(text)),
        range: None,
    })
}

fn comment_before(source: &str, offset: usize) -> Option<String> {
    let prefix = &source[..offset];
    let mut comments = Vec::new();
    for line in prefix.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            comments.push(trimmed);
        } else if trimmed.is_empty() {
            continue;
        } else {
            break;
        }
    }
    if comments.is_empty() {
        return None;
    }
    comments.reverse();
    Some(comments.join("\n"))
}

fn completion(state: &DocumentState, _pos: &Position) -> Option<CompletionResponse> {
    let mut items = Vec::new();
    for kw in &[
        "fn", "let", "mut", "if", "else", "while", "for", "loop", "break", "continue", "return",
        "match", "in", "struct", "enum", "impl", "trait", "true", "false",
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
            SymKind::EnumConstructor { .. } => CompletionItemKind::ENUM,
            SymKind::TypeParam(_) => CompletionItemKind::TYPE_PARAMETER,
            SymKind::Trait(_) => CompletionItemKind::INTERFACE,
            SymKind::Module(_) => CompletionItemKind::MODULE,
        };
        items.push(CompletionItem {
            label: name.clone(),
            kind: Some(item_kind),
            detail: Some(symkind_display(kind)),
            ..Default::default()
        });
    }
    if items.is_empty() {
        None
    } else {
        Some(CompletionResponse::Array(items))
    }
}

fn goto_definition(
    docs: &HashMap<DocUri, DocumentState>,
    params: &TextDocumentPositionParams,
) -> Option<GotoDefinitionResponse> {
    let uri = &params.text_document.uri;
    let state = docs.get(uri)?;
    let offset = position_to_offset(&state.source, params.position)?;
    let source = state.source.as_bytes();
    if offset >= source.len()
        || !(source[offset].is_ascii_alphanumeric() || source[offset] == b'_')
    {
        return None;
    }
    let start = (0..offset)
        .rev()
        .take_while(|&i| source[i].is_ascii_alphanumeric() || source[i] == b'_')
        .last()
        .unwrap_or(offset);
    let end = offset
        + 1
        + (offset + 1..source.len())
            .take_while(|&i| source[i].is_ascii_alphanumeric() || source[i] == b'_')
            .count();
    let name = &state.source[start..end];

    // Verify the symbol exists in the symbol table
    state.symbols.lookup(name)?;

    // Walk the AST to find the definition span
    let def_span = find_definition_in_stmts(&state.program.stmts, &state.source, name)?;
    let range = span_to_range(&state.source, def_span);
    Some(GotoDefinitionResponse::Scalar(Location {
        uri: uri.clone(),
        range,
    }))
}

/// Recursively search statements for a definition of `name`.
fn find_definition_in_stmts(
    stmts: &[Spanned<Stmt>],
    source: &str,
    name: &str,
) -> Option<Span> {
    for stmt in stmts {
        if let Some(span) = find_definition_in_stmt(stmt, source, name) {
            return Some(span);
        }
    }
    None
}

fn find_definition_in_stmt(
    stmt: &Spanned<Stmt>,
    source: &str,
    name: &str,
) -> Option<Span> {
    match &stmt.node {
        Stmt::Fn { name: fn_name, params, body, .. } => {
            if fn_name.as_str() == name {
                return Some(name_span_in_source(source, stmt.span, name)?);
            }
            // Check params
            for param in params {
                if param.name.as_str() == name {
                    return Some(name_span_in_source(source, stmt.span, name)?);
                }
            }
            // Check body statements
            find_definition_in_stmts(body, source, name)
        }
        Stmt::Let { name: let_name, init, .. } => {
            if let_name.as_str() == name {
                return Some(name_span_in_source(source, stmt.span, name)?);
            }
            if let Some(init_expr) = init {
                find_definition_in_expr(init_expr, source, name)
            } else {
                None
            }
        }
        Stmt::Struct { name: struct_name, .. }
        | Stmt::Enum { name: struct_name, .. } => {
            if struct_name.as_str() == name {
                return Some(name_span_in_source(source, stmt.span, name)?);
            }
            None
        }
        Stmt::Impl { methods, .. } => {
            for method in methods {
                if let Some(span) = find_definition_in_stmt(method, source, name) {
                    return Some(span);
                }
            }
            None
        }
        Stmt::Trait { name: trait_name, methods, .. } => {
            if trait_name.as_str() == name {
                return Some(name_span_in_source(source, stmt.span, name)?);
            }
            for method in methods {
                if let Some(span) = find_definition_in_stmt(method, source, name) {
                    return Some(span);
                }
            }
            None
        }
        Stmt::Expr(expr) => find_definition_in_expr(expr, source, name),
        Stmt::Return(Some(expr)) => find_definition_in_expr(expr, source, name),
        Stmt::Return(None) => None,
        Stmt::Use { .. } => None,
        Stmt::Mod { body, .. } => find_definition_in_stmts(body, source, name),
    }
}

fn find_definition_in_expr(expr: &Expr, source: &str, name: &str) -> Option<Span> {
    match expr {
        Expr::Block(stmts) => find_definition_in_stmts(stmts, source, name),
        Expr::Lambda { params: _, body, .. } => {
            find_definition_in_expr(body, source, name)
        }
        Expr::For { iter, body, .. } => {
            // for-var definitions don't have individual spans, skip
            find_definition_in_expr(iter, source, name)
                .or_else(|| find_definition_in_expr(body, source, name))
        }
        Expr::Match { expr: match_expr, arms } => {
            // Check match expr
            if let Some(span) = find_definition_in_expr(match_expr, source, name) {
                return Some(span);
            }
            for arm in arms {
                if let Pattern::Ident(pat_name) = &arm.pattern {
                    if pat_name.as_str() == name {
                        // pattern identifiers don't have spans; skip
                    }
                }
                if let Some(guard) = &arm.guard {
                    if let Some(span) = find_definition_in_expr(guard, source, name) {
                        return Some(span);
                    }
                }
                if let Some(span) = find_definition_in_expr(&arm.body, source, name) {
                    return Some(span);
                }
            }
            None
        }
        Expr::If { cond, then, else_ } => {
            find_definition_in_expr(cond, source, name)
                .or_else(|| find_definition_in_expr(then, source, name))
                .or_else(|| {
                    else_.as_ref()
                        .and_then(|e| find_definition_in_expr(e, source, name))
                })
        }
        Expr::While { cond, body } => find_definition_in_expr(cond, source, name)
            .or_else(|| find_definition_in_expr(body, source, name)),
        Expr::Loop(body) => find_definition_in_expr(body, source, name),
        Expr::Binary { lhs, rhs, .. } => find_definition_in_expr(lhs, source, name)
            .or_else(|| find_definition_in_expr(rhs, source, name)),
        Expr::Unary { expr: inner, .. } => find_definition_in_expr(inner, source, name),
        Expr::Call { func, args } => {
            let mut result = find_definition_in_expr(func, source, name);
            for arg in args {
                if result.is_some() {
                    break;
                }
                result = find_definition_in_expr(arg, source, name);
            }
            result
        }
        Expr::MethodCall { obj, args, .. } => {
            let mut result = find_definition_in_expr(obj, source, name);
            for arg in args {
                if result.is_some() {
                    break;
                }
                result = find_definition_in_expr(arg, source, name);
            }
            result
        }
        Expr::Field { obj, .. } => find_definition_in_expr(obj, source, name),
        Expr::Index { obj, index } => find_definition_in_expr(obj, source, name)
            .or_else(|| find_definition_in_expr(index, source, name)),
        Expr::StructLit { fields, spread, .. } => {
            for (_, val) in fields {
                if let Some(span) = find_definition_in_expr(val, source, name) {
                    return Some(span);
                }
            }
            if let Some(spread_expr) = spread {
                if let Some(span) = find_definition_in_expr(spread_expr, source, name) {
                    return Some(span);
                }
            }
            None
        }
        Expr::Array(elems) => {
            for elem in elems {
                if let Some(span) = find_definition_in_expr(elem, source, name) {
                    return Some(span);
                }
            }
            None
        }
        Expr::Range { start, end, .. } => find_definition_in_expr(start, source, name)
            .or_else(|| find_definition_in_expr(end, source, name)),
        Expr::Return(Some(inner)) => find_definition_in_expr(inner, source, name),
        // Literals and control-flow keywords have no sub-definitions
        Expr::Int(_) | Expr::Float(_) | Expr::Str(_) | Expr::Bool(_)
        | Expr::Unit | Expr::Ident(_) | Expr::Break | Expr::Continue
        | Expr::Return(None) => None,
    }
}

/// Find the byte span of `name` considered as an identifier in `source`
/// within the approximate range `hint_span` (used as fallback bounds).
fn name_span_in_source(source: &str, hint_span: Span, name: &str) -> Option<Span> {
    let search_start = hint_span.0.saturating_sub(16);
    let search_end = (hint_span.1 + 16).min(source.len());
    let search_slice = &source[search_start..search_end];
    // Find the name as a whole word
    let mut i = 0;
    while i < search_slice.len() {
        if let Some(pos) = search_slice[i..].find(name) {
            let abs_pos = search_start + i + pos;
            // Check word boundaries
            let before = abs_pos.saturating_sub(1);
            let after = abs_pos + name.len();
            let prev_ok = before < search_start
                || !source.as_bytes()[before].is_ascii_alphanumeric();
            let next_ok = after >= search_end
                || after >= source.len()
                || !source.as_bytes()[after].is_ascii_alphanumeric();
            if prev_ok && next_ok {
                return Some(Span(abs_pos, abs_pos + name.len()));
            }
            i += pos + 1;
        } else {
            break;
        }
    }
    // Fallback: just use the hint span
    Some(Span(hint_span.0, hint_span.0 + name.len().min(hint_span.1 - hint_span.0)))
}

fn document_symbols(state: &DocumentState) -> Option<Vec<DocumentSymbol>> {
    let mut symbols = Vec::new();
    for stmt in &state.program.stmts {
        if let Some(sym) = stmt_to_document_symbol(stmt, &state.source) {
            symbols.push(sym);
        }
    }
    if symbols.is_empty() {
        None
    } else {
        Some(symbols)
    }
}

fn stmt_to_document_symbol(stmt: &Spanned<Stmt>, source: &str) -> Option<DocumentSymbol> {
    let range = span_to_range(source, stmt.span);
    match &stmt.node {
        Stmt::Fn { name, params, .. } => {
            let detail = Some(format!(
                "fn({})",
                params
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            Some(DocumentSymbol {
                name: name.to_string(),
                detail,
                kind: SymbolKind::FUNCTION,
                range,
                selection_range: range,
                children: None,
                deprecated: None,
                tags: None,
            })
        }
        Stmt::Struct { name, fields, .. } => {
            let children = fields
                .iter()
                .map(|f| DocumentSymbol {
                    name: f.name.to_string(),
                    detail: Some(format!("{}", type_display(&f.type_ann))),
                    kind: SymbolKind::FIELD,
                    range,
                    selection_range: range,
                    children: None,
                    deprecated: None,
                    tags: None,
                })
                .collect();
            Some(DocumentSymbol {
                name: name.to_string(),
                detail: None,
                kind: SymbolKind::STRUCT,
                range,
                selection_range: range,
                children: Some(children),
                deprecated: None,
                tags: None,
            })
        }
        Stmt::Enum { name, variants, .. } => {
            let children = variants
                .iter()
                .map(|v| DocumentSymbol {
                    name: v.name.to_string(),
                    detail: None,
                    kind: SymbolKind::ENUM_MEMBER,
                    range,
                    selection_range: range,
                    children: None,
                    deprecated: None,
                    tags: None,
                })
                .collect();
            Some(DocumentSymbol {
                name: name.to_string(),
                detail: None,
                kind: SymbolKind::ENUM,
                range,
                selection_range: range,
                children: Some(children),
                deprecated: None,
                tags: None,
            })
        }
        Stmt::Trait { name, methods, .. } => {
            let children = methods
                .iter()
                .filter_map(|m| {
                    if let Stmt::Fn { name: fn_name, params, .. } = &m.node {
                        let detail = Some(format!(
                            "fn({})",
                            params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ")
                        ));
                        Some(DocumentSymbol {
                            name: fn_name.to_string(),
                            detail,
                            kind: SymbolKind::METHOD,
                            range,
                            selection_range: range,
                            children: None,
                            deprecated: None,
                            tags: None,
                        })
                    } else {
                        None
                    }
                })
                .collect();
            Some(DocumentSymbol {
                name: name.to_string(),
                detail: None,
                kind: SymbolKind::INTERFACE,
                range,
                selection_range: range,
                children: Some(children),
                deprecated: None,
                tags: None,
            })
        }
        Stmt::Let { name, .. } => Some(DocumentSymbol {
            name: name.to_string(),
            detail: None,
            kind: SymbolKind::VARIABLE,
            range,
            selection_range: range,
            children: None,
            deprecated: None,
            tags: None,
        }),
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
            TokenKind::Fn
            | TokenKind::Let
            | TokenKind::Mut
            | TokenKind::If
            | TokenKind::Else
            | TokenKind::While
            | TokenKind::For
            | TokenKind::Loop
            | TokenKind::Break
            | TokenKind::Continue
            | TokenKind::Return
            | TokenKind::Match
            | TokenKind::In
            | TokenKind::Struct
            | TokenKind::Enum
            | TokenKind::Impl
            | TokenKind::Trait
            | TokenKind::Self_
            | TokenKind::Pub
            | TokenKind::Use
            | TokenKind::Mod
            | TokenKind::Const
            | TokenKind::Type
            | TokenKind::Underscore => SemanticTokenType::KEYWORD,
            TokenKind::Str(_) => SemanticTokenType::STRING,
            TokenKind::Int(_) | TokenKind::Float(_) => SemanticTokenType::NUMBER,
            TokenKind::Bool(_) => SemanticTokenType::KEYWORD,
            TokenKind::Ident(_) => {
                let ident_name = tok.lexeme.to_string();
                if let Some(entry) = state.symbols.lookup(&ident_name) {
                    match &entry.kind {
                        SymKind::Function(_) => SemanticTokenType::FUNCTION,
                        SymKind::Variable(_) => SemanticTokenType::VARIABLE,
                        SymKind::Struct(_) | SymKind::Enum(_) | SymKind::EnumConstructor { .. } | SymKind::TypeParam(_) | SymKind::Module(_) | SymKind::Trait(_) => SemanticTokenType::TYPE,
                    }
                } else {
                    SemanticTokenType::VARIABLE
                }
            }
            TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::Eq
            | TokenKind::EqEq
            | TokenKind::Ne
            | TokenKind::Lt
            | TokenKind::Gt
            | TokenKind::Le
            | TokenKind::Ge
            | TokenKind::And
            | TokenKind::AndAnd
            | TokenKind::Or
            | TokenKind::OrOr
            | TokenKind::Bang
            | TokenKind::Dot
            | TokenKind::DotDot
            | TokenKind::DotDotEq
            | TokenKind::Comma
            | TokenKind::Semi
            | TokenKind::Colon
            | TokenKind::ColonColon
            | TokenKind::Arrow
            | TokenKind::FatArrow => SemanticTokenType::OPERATOR,
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
        prev_col = pos.character;
    }

    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data: tokens,
    }))
}

fn format_document(state: &DocumentState, _uri: &DocUri, tab_size: usize) -> Option<Vec<TextEdit>> {
    let formatted = crate::formatter::format_source(&state.source, tab_size).ok()?;
    if formatted == state.source {
        return Some(Vec::new());
    }
    let range = Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: offset_to_position(&state.source, state.source.len()),
    };
    Some(vec![TextEdit {
        range,
        new_text: formatted,
    }])
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
            let params: Vec<String> = sig
                .params
                .iter()
                .map(|(n, t)| format!("{}: {}", n, type_display(t)))
                .collect();
            format!(
                "fn({}) -> {}",
                params.join(", "),
                sig.return_type
                    .as_ref()
                    .map(|t| type_display(t))
                    .unwrap_or_else(|| "?".to_string())
            )
        }
        SymKind::Struct(def) => format!(
            "struct {{ {} }}",
            def.fields
                .iter()
                .map(|f| format!("{}: {}", f.name, type_display(&f.type_ann)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        SymKind::Enum(def) => format!(
            "enum {{ {} }}",
            def.variants
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        SymKind::Module(_) => "module".to_string(),
        SymKind::Trait(def) => format!("trait {{ {} }}", def.method_sigs.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ")),
        SymKind::EnumConstructor { enum_name, variant_name, tag: _, fields } => {
            let fields_str = if fields.is_empty() {
                String::new()
            } else {
                format!(
                    "({})",
                    fields.iter().map(|t| type_display(t)).collect::<Vec<_>>().join(", ")
                )
            };
            format!("{}{} (enum {})", variant_name, fields_str, enum_name)
        }
        SymKind::TypeParam(name) => format!("type param '{}'", name),
    }
}

fn type_display(ty: &crate::ast::Type) -> String {
    match ty {
        crate::ast::Type::I64 => "int".into(),
        crate::ast::Type::F32 => "f32".into(),
        crate::ast::Type::F64 => "float".into(),
        crate::ast::Type::Bool => "bool".into(),
        crate::ast::Type::Str => "str".into(),
        crate::ast::Type::Unit => "()".into(),
        crate::ast::Type::Named(n) => n.to_string(),
        crate::ast::Type::Generic(n) => n.to_string(),
        crate::ast::Type::Array(inner) => format!("[{}]", type_display(inner)),
        crate::ast::Type::Fn { params, ret } => {
            let ps: Vec<String> = params.iter().map(type_display).collect();
            format!("fn({}) -> {}", ps.join(", "), type_display(ret))
        }
        crate::ast::Type::Option(inner) => format!("Option<{}>", type_display(inner)),
        crate::ast::Type::Result(ok, err) => format!("Result<{}, {}>", type_display(ok), type_display(err)),
    }
}
