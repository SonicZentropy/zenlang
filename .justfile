# Zenlang Development Commands
# Usage: just <recipe>

# List available recipes
default:
    @just --list

# ──────────────────────────────────────────────
# Build
# ──────────────────────────────────────────────

# Build the project (debug)
build:
    cargo build

# Build the project (release)
build-release:
    cargo build --release

# Check for compilation errors without building
check:
    cargo check

# Run clippy lints
clippy:
    cargo clippy --lib

# Format code
fmt:
    cargo fmt

# Check formatting without modifying
fmt-check:
    cargo fmt --check

# ──────────────────────────────────────────────
# Test
# ──────────────────────────────────────────────

# Run all tests
test:
    cargo test

# Run tests with output
test-verbose:
    cargo test -- --nocapture

# Run specific test by name pattern
test-filter pattern:
    cargo test {{pattern}}

# ──────────────────────────────────────────────
# Run
# ──────────────────────────────────────────────

# Run a script file
run file:
    cargo run -- run {{file}}

# Run in REPL mode
repl:
    cargo run -- repl

# Type-check a script
check-script file:
    cargo run -- check {{file}}

# Disassemble a script
disasm file:
    cargo run -- disasm {{file}}

# Install zenc binary
install:
    cargo install --path .

# ──────────────────────────────────────────────
# Clean
# ──────────────────────────────────────────────

# Clean build artifacts
clean:
    cargo clean

# ──────────────────────────────────────────────
# MDBook
# ──────────────────────────────────────────────

# Build the mdbook documentation
book-build:
    mdbook build book

# Serve the mdbook locally (http://localhost:3000)
book-serve:
    mdbook serve book

# Watch and rebuild mdbook on changes
book-watch:
    mdbook watch book

# Clean the built book
book-clean:
    rm -rf book/book

# ──────────────────────────────────────────────
# Zed Extension
# ──────────────────────────────────────────────

# Build the Zed extension (native)
zed-build:
    cargo build --manifest-path zed-extension/Cargo.toml

# Build the Zed extension WASM target
zed-wasm:
    cargo build --manifest-path zed-extension/Cargo.toml --target wasm32-wasip1

# Generate the Tree-sitter grammar
zed-grammar:
    cd zed-extension/grammars/zenlang && tree-sitter generate

# Build the Tree-sitter grammar to WASM
zed-grammar-wasm:
    cd zed-extension/grammars/zenlang && tree-sitter build --wasm

# Run Tree-sitter tests for the grammar
zed-grammar-test:
    cd zed-extension/grammars/zenlang && tree-sitter test

# ──────────────────────────────────────────────
# CI / Pre-commit
# ──────────────────────────────────────────────

# Run all checks (format, clippy, tests)
ci: fmt-check clippy test

# ──────────────────────────────────────────────
# Development
# ──────────────────────────────────────────────

# Full rebuild: clean, check, build, test
rebuild: clean check build test

# Run the tour example
tour:
    cargo run -- test examples/tour.zen
