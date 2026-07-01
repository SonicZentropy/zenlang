use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::{Parser, Subcommand};

use zenlang::hotreload::HotReloader;
use zenlang::{Error, VM};

#[derive(Parser)]
#[command(name = "zenc", version, about = "Zenlang scripting language")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Path to a script file to run
    file: Option<camino::Utf8PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Run a script file (with hot reload)
    Run { path: camino::Utf8PathBuf },
    /// Start an interactive REPL
    Repl,
    /// Disassemble a compiled script
    Disasm { path: camino::Utf8PathBuf },
    /// Type-check only (no execution)
    Check { path: camino::Utf8PathBuf },
    /// Start the LSP language server (stdin/stdout)
    Lsp,
    /// Run tests
    Test { paths: Vec<camino::Utf8PathBuf> },
}

fn main() -> zenlang::Result<()> {
    let cli = Cli::parse();

    // LSP sets up its own file-based tracing; all other commands use stderr.
    if !matches!(&cli.command, Some(Command::Lsp)) {
        zenlang::init_tracing();
    }

    match &cli.command {
        Some(Command::Run { path }) => run_script(path),
        Some(Command::Repl) => run_repl(),
        Some(Command::Disasm { path }) => run_disasm(path),
        Some(Command::Check { path }) => run_check(path),
        Some(Command::Test { paths }) => run_tests(paths),
        Some(Command::Lsp) => {
            zenlang::lsp::run_server();
            Ok(())
        }
        None => {
            if let Some(path) = &cli.file {
                run_script(path)
            } else {
                run_repl()
            }
        }
    }
}

fn run_script(path: &camino::Utf8PathBuf) -> zenlang::Result<()> {
    tracing::info!("running script: {}", path);

    let source = std::fs::read_to_string(path.as_std_path())
        .map_err(|e| Error::Io { source: e })?;

    // Initial compile
    let tokens = zenlang::lexer::Lexer::new(&source).tokenize()?;
    let parser = zenlang::parser::Parser::new(&source, &tokens);
    let mut program = parser.parse()?;
    zenlang::mod_resolver::resolve_modules(&mut program, path.as_std_path())?;
    let native_names = zenlang::stdlib::native_names();
    let mut symbols = zenlang::resolver::resolve_with_natives(&mut program, &native_names)?;
    let types = zenlang::typeck::check(&program, &mut symbols)?;
    let (fns, global_names) = zenlang::compiler::compile(&program, &types, &symbols, &native_names, &source)?;

    let mut vm = VM::new();
    zenlang::stdlib::register_builtins(&mut vm);
    vm.load_bytecode(fns, global_names);

    let result = vm.run_main()?;
    tracing::info!("result: {:?}", result);

    // Enter hot reload loop
    let mut reloader = HotReloader::new([path.as_std_path().to_path_buf()], vm);
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).ok();

    while running.load(Ordering::SeqCst) {
        if reloader.tick()? {
            let result = reloader.vm_mut().run_main()?;
            tracing::info!("reload result: {:?}", result);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Ok(())
}

fn run_tests(paths: &[camino::Utf8PathBuf]) -> zenlang::Result<()> {
    let test_files: Vec<camino::Utf8PathBuf> = if paths.is_empty() {
        let test_dir = camino::Utf8Path::new("tests");
        if !test_dir.is_dir() {
            eprintln!("no tests/ directory found and no paths specified");
            std::process::exit(1);
        }
        let mut files: Vec<_> = std::fs::read_dir("tests")
            .map_err(|e| Error::Io { source: e })?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|s| s == "zen").unwrap_or(false))
            .map(|e| camino::Utf8PathBuf::from_path_buf(e.path()).unwrap_or_default())
            .filter(|p| !p.as_str().is_empty())
            .collect();
        files.sort();
        files
    } else {
        paths.to_vec()
    };

    if test_files.is_empty() {
        eprintln!("no test files found");
        std::process::exit(0);
    }

    let total = test_files.len();
    let mut passed = 0usize;
    let mut failed = 0usize;

    for path in &test_files {
        let source = match std::fs::read_to_string(path.as_std_path()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("FAIL {path} — io error: {e}");
                failed += 1;
                continue;
            }
        };

        let result = (|| -> zenlang::Result<()> {
            let tokens = zenlang::lexer::Lexer::new(&source).tokenize()?;
            let parser = zenlang::parser::Parser::new(&source, &tokens);
            let mut program = parser.parse()?;
            zenlang::mod_resolver::resolve_modules(&mut program, path.as_std_path())?;
            let native_names = zenlang::stdlib::native_names();
            let mut symbols = zenlang::resolver::resolve_with_natives(&mut program, &native_names)?;
            let types = zenlang::typeck::check(&program, &mut symbols)?;
            let (fns, global_names) = zenlang::compiler::compile(&program, &types, &symbols, &native_names, &source)?;
            let mut vm = VM::new();
            zenlang::stdlib::register_builtins(&mut vm);
            vm.load_bytecode(fns, global_names);
            vm.run_main()?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                println!("PASS {path}");
                passed += 1;
            }
            Err(e) => {
                eprintln!("FAIL {path} — {e}");
                failed += 1;
            }
        }
    }

    println!("\n{passed}/{total} tests passed");
    if failed > 0 {
        std::process::exit(failed as i32);
    }
    Ok(())
}

fn run_disasm(path: &camino::Utf8PathBuf) -> zenlang::Result<()> {
    let source = std::fs::read_to_string(path.as_std_path())
        .map_err(|e| Error::Io { source: e })?;

    let tokens = zenlang::lexer::Lexer::new(&source).tokenize()?;
    let parser = zenlang::parser::Parser::new(&source, &tokens);
    let mut program = parser.parse()?;
    zenlang::mod_resolver::resolve_modules(&mut program, path.as_std_path())?;
    let native_names = zenlang::stdlib::native_names();
    let mut symbols = zenlang::resolver::resolve_with_natives(&mut program, &native_names)?;
    let types = zenlang::typeck::check(&program, &mut symbols)?;
    let (fns, _global_names) = zenlang::compiler::compile(&program, &types, &symbols, &native_names, &source)?;

    for func in &fns {
        func.disassemble();
    }
    Ok(())
}

fn run_check(path: &camino::Utf8PathBuf) -> zenlang::Result<()> {
    let source = std::fs::read_to_string(path.as_std_path())
        .map_err(|e| Error::Io { source: e })?;

    let tokens = zenlang::lexer::Lexer::new(&source).tokenize()?;
    let parser = zenlang::parser::Parser::new(&source, &tokens);
    let mut program = parser.parse()?;
    zenlang::mod_resolver::resolve_modules(&mut program, path.as_std_path())?;
    let native_names = zenlang::stdlib::native_names();
    let mut symbols = zenlang::resolver::resolve_with_natives(&mut program, &native_names)?;
    let _types = zenlang::typeck::check(&program, &mut symbols)?;

    println!("type check passed");
    Ok(())
}

/// Check if brackets are balanced in the input.
fn is_balanced(s: &str) -> bool {
    let mut depth = 0i32;
    let mut parens = 0i32;
    let mut brack = 0i32;
    for c in s.chars() {
        match c {
            '{' | '}' => depth += if c == '{' { 1 } else { -1 },
            '(' | ')' => parens += if c == '(' { 1 } else { -1 },
            '[' | ']' => brack += if c == '[' { 1 } else { -1 },
            _ => {}
        }
        if depth < 0 || parens < 0 || brack < 0 {
            return false; // unbalanced close
        }
    }
    depth == 0 && parens == 0 && brack == 0
}

fn run_repl() -> zenlang::Result<()> {
    let mut vm = VM::new();
    zenlang::stdlib::register_builtins(&mut vm);

    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut source_buf = String::new();

    loop {
        if source_buf.is_empty() {
            print!("> ");
        } else {
            print!("... ");
        }
        let _ = std::io::Write::flush(&mut std::io::stdout());

        let mut line = String::new();
        if reader.read_line(&mut line).map_err(|e| Error::Io { source: e })? == 0 {
            break;
        }
        source_buf.push_str(&line);

        if !is_balanced(&source_buf) {
            continue;
        }

        let trimmed = source_buf.trim();
        if trimmed.is_empty() {
            source_buf.clear();
            continue;
        }

        // Parse and compile
        let source = std::mem::take(&mut source_buf);
        let tokens = match zenlang::lexer::Lexer::new(&source).tokenize() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("parse error: {e}");
                continue;
            }
        };
    let parser = zenlang::parser::Parser::new(&source, &tokens);
        let mut program = match parser.parse() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("parse error: {e}");
                continue;
            }
        };
        // REPL input has no backing file, so pass "." as path — module loading won't apply
        let _ = zenlang::mod_resolver::resolve_modules(&mut program, std::path::Path::new("."));
        let native_names = zenlang::stdlib::native_names();
        let mut symbols = match zenlang::resolver::resolve_with_natives(&mut program, &native_names) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: {e}");
                continue;
            }
        };
        let types = match zenlang::typeck::check(&program, &mut symbols) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("type error: {e}");
                continue;
            }
        };
        let (fns, global_names) = match zenlang::compiler::compile(&program, &types, &symbols, &native_names, &source) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("compile error: {e}");
                continue;
            }
        };

        // Reload VM bytecode preserving globals
        if let Err(e) = vm.reload_functions(fns, global_names) {
            eprintln!("error: {e}");
            continue;
        }

        match vm.run_main() {
            Ok(val) => println!("=> {val:?}"),
            Err(e) => eprintln!("error: {e}"),
        }
    }

    println!();
    Ok(())
}
