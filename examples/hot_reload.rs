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

    // Initial compile via VM
    let mut vm = VM::new();
    vm.load_file(&script_path)?;
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
