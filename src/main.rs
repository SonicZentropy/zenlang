use clap::{Parser, Subcommand};

use zenlang::hotreload::HotReloader;
use zenlang::{Error, VM};

#[derive(Parser)]
#[command(name = "zenlang", version, about = "Zenlang scripting language")]
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
}

fn main() -> zenlang::Result<()> {
    zenlang::init_tracing();

    let cli = Cli::parse();

    match &cli.command {
        Some(Command::Run { path }) => run_script(path),
        Some(Command::Repl) => run_repl(),
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
    let parser = zenlang::parser::Parser::new(&tokens);
    let mut program = parser.parse()?;
    let mut symbols = zenlang::resolver::resolve(&mut program)?;
    let types = zenlang::typeck::check(&program, &mut symbols)?;
    let (fns, global_names) = zenlang::compiler::compile(&program, &types, &symbols)?;

    let mut vm = VM::new();
    zenlang::stdlib::register_builtins(&mut vm);
    vm.load_bytecode(fns, global_names);

    let result = vm.run_main()?;
    tracing::info!("result: {:?}", result);

    // Enter hot reload loop
    let mut reloader = HotReloader::new([path.as_std_path().to_path_buf()], vm);

    loop {
        if reloader.tick()? {
            let result = reloader.vm_mut().run_main()?;
            tracing::info!("reload result: {:?}", result);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

fn run_repl() -> zenlang::Result<()> {
    tracing::info!("starting REPL");
    Ok(())
}
