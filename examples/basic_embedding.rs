/// Basic embedding: compile and run a Zen script from Rust.
///
/// Run with: cargo run --example basic_embedding
use zenlang::{VM, Value};

fn main() -> zenlang::Result<()> {
    let source = "\
fn greet(name) {
    print(\"Hello, \" + name + \"!\");
}

let msg = greet(\"world\");
let answer = 6 * 7;
answer
";

    let mut vm = VM::new();
    let result = vm.exec(source)?;
    println!("script returned: {:?}", result);
    assert_eq!(result, Value::Int(42));

    Ok(())
}
