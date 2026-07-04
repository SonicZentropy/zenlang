use std::collections::HashMap;
use std::io::BufRead;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use clap::{Parser, Subcommand};

use zenlang::hotreload::HotReloader;
use zenlang::{Error, VM};

#[derive(Parser)]
#[command(name = "zenc", version, about = "Zen scripting language")]
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
    /// Create a new Zen project
    New { name: String },
    /// Build (type-check) a Zen project
    Build { path: Option<camino::Utf8PathBuf> },
    /// Start the LSP language server (stdin/stdout)
    Lsp,
    /// Start the DAP debug adapter server (stdin/stdout)
    Dap { path: camino::Utf8PathBuf },
    /// Run tests (or benchmarks with --bench)
    Test {
        paths: Vec<camino::Utf8PathBuf>,
        /// Re-run tests when files change
        #[arg(long)]
        watch: bool,
        /// Run benchmarks from benches/ instead of tests/
        #[arg(long)]
        bench: bool,
    },
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
        Some(Command::Test { paths, watch, bench }) => run_tests(paths, *watch, *bench),
        Some(Command::Lsp) => {
            zenlang::lsp::run_server();
            Ok(())
        }
        Some(Command::Dap { path }) => {
            let source = std::fs::read_to_string(path.as_std_path())
                .map_err(|e| zenlang::Error::Io { source: e })?;
            zenlang::dap::run_dap(&source, Some(path.as_std_path()))
        }
        Some(Command::New { name }) => cmd_new(name),
        Some(Command::Build { path }) => cmd_build(path.as_ref()),
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

    let mut vm = VM::new();
    vm.load_file(path.as_std_path())?;

    let result = vm.run_main()?;
    tracing::info!("result: {:?}", result);

    // Enter hot reload loop
    let mut reloader = HotReloader::new([path.as_std_path().to_path_buf()], vm);
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .ok();

    while running.load(Ordering::SeqCst) {
        if reloader.tick()? {
            let result = reloader.vm_mut().run_main()?;
            tracing::info!("reload result: {:?}", result);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Ok(())
}

fn run_tests(
    paths: &[camino::Utf8PathBuf],
    watch: bool,
    bench: bool,
) -> zenlang::Result<()> {
    let dir = if bench { "benches" } else { "tests" };
    let label = if bench { "benchmark" } else { "test" };

    let test_files = discover_files(paths, dir)?;

    if test_files.is_empty() {
        eprintln!("no {label} files found in {dir}/");
        return Ok(());
    }

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .ok();

    loop {
        let (passed, failed) = run_test_suite(&test_files, label)?;

        let total = test_files.len();
        println!("\n{passed}/{total} {label}s passed");
        if failed > 0 && !watch {
            std::process::exit(failed as i32);
        }

        if !watch {
            break;
        }

        // Watch mode: poll for file changes
        println!("\n  watching for changes... (Ctrl+C to stop)");
        if !wait_for_changes(&test_files, &running) {
            break;
        }
        println!();
    }
    Ok(())
}

/// Discover .zen files. If `paths` is non-empty, use those; otherwise scan `dir/`.
fn discover_files(paths: &[camino::Utf8PathBuf], dir: &str) -> zenlang::Result<Vec<camino::Utf8PathBuf>> {
    if !paths.is_empty() {
        return Ok(paths.to_vec());
    }
    let test_dir = camino::Utf8Path::new(dir);
    if !test_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| Error::Io { source: e })?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|s| s == "zen").unwrap_or(false))
        .map(|e| camino::Utf8PathBuf::from_path_buf(e.path()).unwrap_or_default())
        .filter(|p| !p.as_str().is_empty())
        .collect();
    files.sort();
    Ok(files)
}

/// Run a single test/bench file, returning Ok(()) on success.
fn run_single_test(path: &camino::Utf8PathBuf) -> zenlang::Result<()> {
    let source = std::fs::read_to_string(path.as_std_path())
        .map_err(|e| Error::Io { source: e })?;
    let mut vm = VM::new();
    vm.load_with(&source, &zenlang::vm::CompileConfig {
        module_path: path.parent().map(|p| p.as_std_path().to_path_buf()),
        ..Default::default()
    })?;
    vm.run_main()?;
    Ok(())
}

/// Run the full test suite, returning (passed, failed) counts.
fn run_test_suite(
    test_files: &[camino::Utf8PathBuf],
    _label: &str,
) -> zenlang::Result<(usize, usize)> {
    let _total = test_files.len();
    let mut passed = 0usize;
    let mut failed = 0usize;

    for path in test_files {
        let result = run_single_test(path);

        let status = if result.is_ok() { "PASS" } else { "FAIL" };
        match &result {
            Ok(()) => {
                println!("{status} {path}");
                passed += 1;
            }
            Err(e) => {
                eprintln!("{status} {path} — {e}");
                failed += 1;
            }
        }
    }

    Ok((passed, failed))
}

/// Poll test files for changes, sleeping ~500ms between checks.
/// Returns `false` when the running flag is cleared (Ctrl+C).
fn wait_for_changes(
    files: &[camino::Utf8PathBuf],
    running: &AtomicBool,
) -> bool {
    let mut mtimes: HashMap<camino::Utf8PathBuf, SystemTime> = HashMap::new();
    for f in files {
        if let Ok(meta) = std::fs::metadata(f.as_std_path()) {
            if let Ok(mtime) = meta.modified() {
                mtimes.insert(f.clone(), mtime);
            }
        }
    }

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(500));
        for f in files {
            if let Ok(meta) = std::fs::metadata(f.as_std_path()) {
                if let Ok(mtime) = meta.modified() {
                    if mtimes.get(f) != Some(&mtime) {
                        return true; // change detected
                    }
                }
            }
        }
    }
    false // Ctrl+C
}

fn run_disasm(path: &camino::Utf8PathBuf) -> zenlang::Result<()> {
    let mut vm = VM::new();
    vm.load_file(path.as_std_path())?;
    vm.disassemble();
    Ok(())
}

fn run_check(path: &camino::Utf8PathBuf) -> zenlang::Result<()> {
    let mut vm = VM::new();
    vm.load_with(
        &std::fs::read_to_string(path.as_std_path()).map_err(|e| Error::Io { source: e })?,
        &zenlang::vm::CompileConfig {
            module_path: path.parent().map(|p| p.as_std_path().to_path_buf()),
            type_check: true,
            ..Default::default()
        },
    )?;
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
        if reader
            .read_line(&mut line)
            .map_err(|e| Error::Io { source: e })?
            == 0
        {
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

        let source = std::mem::take(&mut source_buf);

        if let Err(e) = vm.reload(
            &source,
            &zenlang::vm::CompileConfig::default(),
        ) {
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

fn cmd_new(name: &str) -> zenlang::Result<()> {
    let dir = std::path::Path::new(name);
    if dir.exists() {
        eprintln!("error: directory '{}' already exists", name);
        std::process::exit(1);
    }

    let src_dir = dir.join("src");
    std::fs::create_dir_all(&src_dir).map_err(|e| Error::Io { source: e })?;

    let main_zen = r#"// Zen script
// Entry point — the result of the top-level expression is the script's return value.
let greeting = "Hello from Zen!";
print(greeting);
42
"#;
    std::fs::write(src_dir.join("main.zen"), main_zen).map_err(|e| Error::Io { source: e })?;

    let config = format!(
        r#"{{
    "name": "{}",
    "entry": "src/main.zen"
}}
"#,
        name
    );
    std::fs::write(dir.join("zenc.json"), config).map_err(|e| Error::Io { source: e })?;

    println!("Created project '{}'", name);
    println!("  cd {} && zenc run src/main.zen", name);
    Ok(())
}

fn cmd_build(path: Option<&camino::Utf8PathBuf>) -> zenlang::Result<()> {
    let project_dir = path
        .map(|p| p.as_std_path().to_path_buf())
        .unwrap_or_else(|| {
            std::env::current_dir()
                .map_err(|e| Error::Io { source: e })
                .unwrap()
        });
    let config_path = project_dir.join("zenc.json");
    let config_str = std::fs::read_to_string(&config_path).map_err(|_| Error::Runtime {
        msg: format!("no zenc.json found in {}", project_dir.display()),
        stack_trace: Vec::new(),
    })?;
    let config: serde_json::Value =
        serde_json::from_str(&config_str).map_err(|e| Error::Runtime {
            msg: format!("invalid zenc.json: {}", e),
            stack_trace: Vec::new(),
        })?;
    let entry = config["entry"].as_str().ok_or_else(|| Error::Runtime {
        msg: "zenc.json missing 'entry' field".into(),
        stack_trace: Vec::new(),
    })?;

    let entry_path = project_dir.join(entry);
    let source = std::fs::read_to_string(&entry_path).map_err(|e| Error::Io { source: e })?;
    let path = Some(entry_path.as_path());

    // Run full compilation pipeline
    let mut vm = VM::new();
    vm.load_with(&source, &zenlang::vm::CompileConfig {
        module_path: path.map(|p| p.to_path_buf()),
        ..Default::default()
    })?;

    println!("Build succeeded");
    Ok(())
}
