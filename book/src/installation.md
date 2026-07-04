# Installation

## Building from Source

Zen is a Rust project. You'll need the Rust toolchain installed (https://rustup.rs).

```bash
git clone https://github.com/SonicZentropy/zenlang
cd zenlang
cargo build --release
```

The `zenc` binary will be at `target/release/zenc` (or `target/release/zenc.exe` on Windows).

## Using as a Library

Add the following to your `Cargo.toml`:

```toml
[dependencies]
zenlang = { git = "https://github.com/SonicZentropy/zenlang" }
```

## Verifying the Installation

```bash
zenc --help
```

You should see the available subcommands: `run`, `repl`, `check`, `disasm`, `lsp`, `new`, `build`, `dap`, `test`.
