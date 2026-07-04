/// Basic embedding: compile and run a Zenlang script from Rust.
///
/// Run with: cargo run --example basic_embedding
use zenlang::compiler::compile;
use zenlang::lexer::Lexer;
use zenlang::parser::Parser;
use zenlang::resolver::resolve_with_natives;
use zenlang::stdlib::{native_names, register_builtins};
use zenlang::typeck::check;
use zenlang::{VM, Value};

fn main() -> zenlang::Result<()> {
    // 1. Source code
    let source = "\
fn greet(name) {
    print(\"Hello, \" + name + \"!\");
}

let msg = greet(\"world\");
let answer = 6 * 7;
answer
";

    // 2. Full pipeline: lex → parse → resolve → type-check → compile
    let tokens = Lexer::new(source).tokenize()?;
    let mut program = Parser::new(source, &tokens).parse()?;
    let names = native_names();
    let mut symbols = resolve_with_natives(&mut program, &names)?;
    let types = check(&program, &mut symbols)?;
    let (fns, global_names) = compile(&program, &types, &symbols, &names, source)?;

    // 3. Create VM, register stdlib, load bytecode
    let mut vm = VM::new();
    register_builtins(&mut vm);
    vm.load_bytecode(fns, global_names);

    // 4. Run the main function
    let result = vm.run_main()?;
    println!("script returned: {:?}", result);
    assert_eq!(result, Value::Int(42));

    Ok(())
}
