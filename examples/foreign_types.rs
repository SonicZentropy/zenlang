/// Register Rust types with fields and methods accessible from Zen scripts.
///
/// Run with: cargo run --example foreign_types
use zenlang::vm::VMContext;
use zenlang::{VM, Value, ZenForeign, zen_methods};

// A Rust struct we want to expose to scripts.
// The derive macro generates `register_zen_foreign()` with field accessors,
// and `#[zen_methods]` generates `register_zen_methods()` for method registration.
#[derive(Clone, Debug, ZenForeign)]
struct Player {
    name: String,
    health: i32,
    max_health: i32,
}

#[zen_methods]
impl Player {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            health: 100,
            max_health: 100,
        }
    }

    fn heal_percent(&self) -> f64 {
        self.health as f64 / self.max_health as f64 * 100.0
    }
}

fn main() -> zenlang::Result<()> {
    let mut vm = VM::new();

    // Auto-generated field + method registration
    Player::register_zen_foreign(&mut vm);
    Player::register_zen_methods(&mut vm);

    // Register a native function to create a Player from the script side
    vm.register_native(
        "create_player",
        std::rc::Rc::new(|ctx: &mut VMContext, args: &[Value]| {
            let name = args.first().and_then(|v| v.as_str()).unwrap_or_default();
            let player = Player::new(&name);
            let vm: &mut VM = unsafe { &mut *ctx.raw_vm };
            Ok(vm.wrap_foreign("Player", player))
        }),
    );

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

    vm.load(source)?;
    let result = vm.run_main()?;
    println!("final: {:?}", result);

    Ok(())
}
