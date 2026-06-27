/// Register Rust types with fields and methods accessible from Zenlang scripts.
///
/// Run with: cargo run --example foreign_types

use std::rc::Rc;

use zenlang::compiler::compile;
use zenlang::interop::{with_foreign, with_foreign_mut};
use zenlang::lexer::Lexer;
use zenlang::parser::Parser;
use zenlang::resolver::resolve_with_natives;
use zenlang::stdlib::{native_names as stdlib_names, register_builtins};
use zenlang::typeck::check;
use zenlang::vm::VMContext;
use zenlang::{Value, VM};

// A Rust struct we want to expose to scripts.
#[derive(Clone, Debug)]
struct Player {
    name: String,
    health: i32,
    max_health: i32,
}

impl Player {
    fn new(name: &str) -> Self {
        Self { name: name.to_string(), health: 100, max_health: 100 }
    }

    fn heal_percent(&self) -> f64 {
        self.health as f64 / self.max_health as f64 * 100.0
    }
}

fn main() -> zenlang::Result<()> {
    let mut vm = VM::new();
    register_builtins(&mut vm);

    // Register a native function to create a Player from the script side
    vm.register_native("create_player", Rc::new(|_ctx: &mut VMContext, args: &[Value]| {
        let name = args.first().and_then(|v| v.as_str()).unwrap_or_default();
        let player = Player::new(&name);
        Ok(Value::Foreign(Rc::new(std::cell::RefCell::new(
            zenlang::value::ForeignObject::new("Player", player),
        ))))
    }));

    // Register the Player type with fields and methods
    vm.register_type::<Player>("Player")
        .field("name",
            |obj: &Value| -> zenlang::Result<Value> {
                with_foreign::<Player, _, _>(obj, |p| Ok(Value::Str(p.name.clone().into())))
            },
            |obj: &mut Value, val: Value| -> zenlang::Result<()> {
                let name = val.as_str().unwrap_or_default();
                with_foreign_mut::<Player, _, _>(obj, |p| { p.name = name; Ok(()) })
            },
        )
        .field("health",
            |obj: &Value| -> zenlang::Result<Value> {
                with_foreign::<Player, _, _>(obj, |p| Ok(Value::Int(p.health as i64)))
            },
            |obj: &mut Value, val: Value| -> zenlang::Result<()> {
                let h = val.as_int().unwrap_or(0) as i32;
                with_foreign_mut::<Player, _, _>(obj, |p| { p.health = h; Ok(()) })
            },
        )
        .method("heal_percent", Rc::new(|_ctx: &mut VMContext, args: &[Value]| {
            with_foreign::<Player, _, _>(&args[0], |p| Ok(Value::Float(p.heal_percent())))
        }));

    let mut names = vm.native_names();
    for n in &stdlib_names() {
        if !names.contains(n) {
            names.push(n.clone());
        }
    }

    let source = "\
let p = create_player(\"Aria\");
print(\"Name:\", p.name);
print(\"Health:\", p.health);
let pct = p.heal_percent();
print(\"HP%:\", pct);
p.health = 50;
print(\"After damage:\", p.health);
let pct2 = p.heal_percent();
print(\"HP% now:\", pct2);
pct2
";

    let tokens = Lexer::new(source).tokenize()?;
    let mut program = Parser::new(&tokens).parse()?;
    let mut symbols = resolve_with_natives(&mut program, &names)?;
    let types = check(&program, &mut symbols)?;
    let (fns, global_names) = compile(&program, &types, &symbols, &names, source)?;

    vm.load_bytecode(fns, global_names);
    let result = vm.run_main()?;
    println!("final: {:?}", result);

    Ok(())
}
