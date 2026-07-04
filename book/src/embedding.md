# Embedding the VM

Add Zen as a dependency:

```toml
[dependencies]
zenlang = "0.1.0"
```

## Basic Usage

```rust
use zenlang::VM;

let mut vm = VM::new();
let result = vm.exec("print(42); 1 + 2")?;
println!("{:?}", result); // Int(3)
```

`VM::new()` creates a VM with all standard builtin functions pre-registered.

## One-Shot Execution

For quick scripts that don't need persistent state or registered natives:

```rust
use zenlang::run;

let result = run("1 + 2 * 3")?;
assert_eq!(result.as_i64(), Some(7));
```

The free function `zenlang::run(source)` creates a temporary VM, compiles and
executes the source, and returns the result value.

## Two-Step (Load + Run)

When you need to register natives before execution, or run `main()` separately:

```rust
let mut vm = VM::new();
vm.register_native("my_fn", Rc::new(|_, args| { /* ... */ }));
vm.load(source)?;
let result = vm.run_main()?;
```

`load()` compiles and loads bytecode into the VM's scope. `run_main()` calls
the script's `fn main()` entry point.

## With Configuration

Use `CompileConfig` to customise compilation:

```rust
use zenlang::CompileConfig;

let config = CompileConfig {
    type_check: true,          // enable type checking (default)
    with_prelude: true,        // inject prelude (default)
    module_path: Some("scripts".into()), // base path for `import`
    source_name: "game.zen".into(),      // name in error messages
};

vm.exec_with(source, &config)?;
// or: vm.load_with(source, &config)?; vm.run_main()?;
```

`CompileConfig` implements `Default` so you can use `..Default::default()` to
fill in the remaining fields.

## Running Files

```rust
vm.load_file("scripts/main.zen")?;
let result = vm.run_main()?;
```

`load_file()` automatically uses the file's parent directory as the
`module_path` for module resolution.

## Error Handling

```rust
match vm.exec("x") {
    Ok(val) => println!("Got: {:?}", val),
    Err(zenlang::Error::Runtime(msg)) => eprintln!("Runtime error: {msg}"),
    Err(zenlang::Error::Compile(errors)) => {
        for e in errors {
            eprintln!("Compile error: {e}");
        }
    }
    Err(zenlang::Error::Panic(msg)) => eprintln!("Script panicked: {msg}"),
    Err(e) => eprintln!("Other error: {e}"),
}
```

Compile errors include `SourceLocation` with file, span, line, and column info.
Runtime errors include a stack trace.

## Disassembly

```rust
vm.disassemble();
```

Prints bytecode opcodes, constants, and source-line mappings for every loaded
function to stdout.
