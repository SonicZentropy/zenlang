/// Register Rust types with fields and methods accessible from Zenlang scripts.
///
/// Run with: cargo run --example foreign_types

use std::any::TypeId;
use std::rc::Rc;

use zenlang::compiler::compile;
use zenlang::interop::with_foreign;
use zenlang::lexer::Lexer;
use zenlang::parser::Parser;
use zenlang::resolver::resolve_with_natives;
use zenlang::stdlib::{native_names as stdlib_names, register_builtins};
use zenlang::typeck::check;
use zenlang::vm::VMContext;
use zenlang::{Value, VM, ZenForeign};

// A Rust struct we want to expose to scripts.
// The derive macro generates `register_zen_foreign()` with field accessors.
#[derive(Clone, Debug, ZenForeign)]
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

    // Register the Player type with auto-generated field accessors from the macro
    Player::register_zen_foreign(&mut vm);

    // Manually add a method via the (already public) registry.
    // Uses Rc::make_mut since foreign_registry is behind Rc.
    // This is separate because the derive macro currently only handles fields.
    Rc::make_mut(&mut vm.foreign_registry).get_mut(&TypeId::of::<Player>()).unwrap()
        .method("heal_percent", Rc::new(|_ctx: &mut VMContext, args: &[Value]| {
            with_foreign::<Player, _, _>(&args[0], |p| Ok(Value::Float(p.heal_percent())))
        }));

    // Register a native function to create a Player from the script side
    vm.register_native("create_player", Rc::new(|_ctx: &mut VMContext, args: &[Value]| {
        let name = args.first().and_then(|v| v.as_str()).unwrap_or_default();
        let player = Player::new(&name);
        Ok(Value::Foreign(Rc::new(std::cell::RefCell::new(
            zenlang::value::ForeignObject::new("Player", player),
        ))))
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
    let mut program = Parser::new(source, &tokens).parse()?;
    let mut symbols = resolve_with_natives(&mut program, &names)?;
    let types = check(&program, &mut symbols)?;
    let (fns, global_names) = compile(&program, &types, &symbols, &names, source)?;

    vm.load_bytecode(fns, global_names);
    let result = vm.run_main()?;
    println!("final: {:?}", result);

    Ok(())
}
