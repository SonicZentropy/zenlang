/// Bidirectional interop: Rust registers natives that scripts call,
/// and scripts define functions/globals that Rust reads after execution.
///
/// Run with: cargo run --example cross_call

use std::rc::Rc;

use zenlang::compiler::compile;
use zenlang::lexer::Lexer;
use zenlang::parser::Parser;
use zenlang::resolver::resolve_with_natives;
use zenlang::stdlib::{native_names as stdlib_names, register_builtins};
use zenlang::typeck::check;
use zenlang::vm::VMContext;
use zenlang::{Value, VM};

fn main() -> zenlang::Result<()> {
    let mut vm = VM::new();
    register_builtins(&mut vm);

    // --- Rust-provided function #1: compute stats ---
    vm.register_native("compute_stats", Rc::new(|_ctx: &mut VMContext, args: &[Value]| {
        let base = args.first().and_then(|v| v.as_int()).unwrap_or(0);
        let level = args.get(1).and_then(|v| v.as_int()).unwrap_or(1);
        let hp = base * 10 + level * 5;
        let atk = base + level * 2;
        // Return a struct-like value — scripts destructure the result
        // We use an array as a simple way to return multiple values
        Ok(Value::Array(Rc::new(std::cell::RefCell::new(vec![
            Value::Int(hp),
            Value::Int(atk),
        ]))))
    }));

    // --- Rust-provided function #2: damage formula ---
    vm.register_native("damage_formula", Rc::new(|_ctx: &mut VMContext, args: &[Value]| {
        let atk = args.first().and_then(|v| v.as_int()).unwrap_or(0);
        let def = args.get(1).and_then(|v| v.as_int()).unwrap_or(0);
        let multiplier = args.get(2).and_then(|v| v.as_float()).unwrap_or(1.0);
        let raw = (atk as f64) - (def as f64 * 0.5);
        let dmg = (raw.max(1.0) * multiplier) as i64;
        Ok(Value::Int(dmg))
    }));

    let mut names = vm.native_names();
    for n in &stdlib_names() {
        if !names.contains(n) {
            names.push(n.clone());
        }
    }

    // Script calls Rust natives and returns a summary
    let source = "\
let stats = compute_stats(12, 3);
let hp = stats[0];
let atk = stats[1];
let dmg = damage_formula(atk, 8, 1.5);
print(\"HP:\", hp, \"ATK:\", atk, \"DMG:\", dmg);
dmg
";

    let tokens = Lexer::new(source).tokenize()?;
    let mut program = Parser::new(&tokens).parse()?;
    let mut symbols = resolve_with_natives(&mut program, &names)?;
    let types = check(&program, &mut symbols)?;
    let (fns, global_names) = compile(&program, &types, &symbols, &names, source)?;

    vm.load_bytecode(fns, global_names);
    let result = vm.run_main()?;
    println!("damage dealt: {:?}", result);
    // (12 + 3*2) = 18 atk; (18 - 8*0.5) = 14 raw; 14 * 1.5 = 21

    Ok(())
}
