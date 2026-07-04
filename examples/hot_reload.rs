/// Hot reload: watch a script file for changes, recompile, and preserve globals.
///
/// Run with: cargo run --example hot_reload
///
/// This creates a temporary script that defines a counter. When you edit the
/// file, the HotReloader detects the change, recompiles, and the counter
/// state is preserved across reloads.
use std::io::Write;

use zenlang::VM;
use zenlang::hotreload::HotReloader;
use zenlang::stdlib::{native_names, register_builtins};

fn main() -> zenlang::Result<()> {
    // Create a temp directory with a script file
    let dir = tempfile::tempdir().unwrap();
    let script_path = dir.path().join("game.zen");

    let initial_source = "\
let counter = 0;

fn main() {
    counter = counter + 1;
    print(\"Counter:\", counter);
    counter
}
";
    let mut file = std::fs::File::create(&script_path).unwrap();
    file.write_all(initial_source.as_bytes()).unwrap();
    drop(file);

    // Initial compile (exactly like the embedding pipeline)
    let source = std::fs::read_to_string(&script_path).unwrap();
    let tokens = zenlang::lexer::Lexer::new(&source).tokenize()?;
    let mut program = zenlang::parser::Parser::new(&source, &tokens).parse()?;
    let names = native_names();
    let mut symbols = zenlang::resolver::resolve_with_natives(&mut program, &names)?;
    let types = zenlang::typeck::check(&program, &mut symbols)?;
    let (fns, global_names) =
        zenlang::compiler::compile(&program, &types, &symbols, &names, &source)?;

    let mut vm = VM::new();
    register_builtins(&mut vm);
    vm.load_bytecode(fns, global_names);
    let result = vm.run_main()?;
    println!("first run: {:?}", result); // Counter: 1

    // Enter hot reload loop with the file path
    let mut reloader = HotReloader::new([script_path.clone()], vm);

    // Simulate a few ticks (in a real app you'd loop with a sleep)
    for tick in 0..3 {
        if reloader.tick()? {
            let result = reloader.vm_mut().run_main()?;
            println!("tick {} result: {:?}", tick, result);
        } else {
            println!("tick {}: no change", tick);
        }
    }

    // Now modify the file to trigger a reload
    let modified_source = "\
let counter = 0;

fn main() {
    counter = counter + 2;
    print(\"Counter (x2):\", counter);
    counter
}
";
    let mut file = std::fs::File::create(&script_path).unwrap();
    file.write_all(modified_source.as_bytes()).unwrap();
    drop(file);

    // Tick again — HotReloader detects the mtime change, recompiles,
    // and the `counter` global is preserved with its previous value.
    // (On the next run, counter will be previous_value + 2)
    if reloader.tick()? {
        let result = reloader.vm_mut().run_main()?;
        println!("after edit: {:?}", result);
    }

    Ok(())
}
