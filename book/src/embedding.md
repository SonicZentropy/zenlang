# Embedding the VM

Add Zenlang as a dependency and create a VM:

```rust
use zenlang::vm::Vm;

fn main() {
    let mut vm = Vm::new();
    vm.eval("print(42)").unwrap();
}
```

## With Configuration

```rust
let config = VmConfig {
    instruction_limit: 100_000,
    module_search_paths: vec!["scripts".into()],
};
let mut vm = Vm::with_config(config);
```

## Running Files

```rust
let result = vm.eval_file("scripts/main.zen")?;
```

## Evaluating Expressions

```rust
let result = vm.eval("1 + 2 * 3").unwrap();
assert_eq!(result.as_i64(), Some(7));
```

## Error Handling

```rust
match vm.eval("x") {
    Ok(val) => println!("Got: {:?}", val),
    Err(Error::Runtime(msg)) => eprintln!("Runtime error: {}", msg),
    Err(Error::Compile(errors)) => {
        for e in errors {
            eprintln!("Compile error: {}", e);
        }
    }
    Err(Error::Panic(msg)) => eprintln!("Script panicked: {}", msg),
}
```
