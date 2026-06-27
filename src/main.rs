use clap::{Parser, Subcommand};

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
    /// Run a script file
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

fn run_script(_path: &camino::Utf8PathBuf) -> zenlang::Result<()> {
    tracing::info!("running script: {}", _path);
    // TODO: load, compile, and execute script
    Ok(())
}

fn run_repl() -> zenlang::Result<()> {
    tracing::info!("starting REPL");
    // TODO: implement REPL
    Ok(())
}
