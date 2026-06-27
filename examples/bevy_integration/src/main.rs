//! Bevy integration example: drive game logic with Zenlang scripts.
//!
//! Run from `examples/bevy_integration/`: `cargo run`
//!
//! This shows how to embed Zenlang in a Bevy application:
//! - Register Bevy components/resources as Zenlang foreign types
//! - Call script functions per frame from Bevy systems
//! - Use scripts for game logic (movement, damage, state)

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;

use zenlang::compiler::compile;
use zenlang::interop::{with_foreign, with_foreign_mut, ForeignObject};
use zenlang::lexer::Lexer;
use zenlang::parser::Parser;
use zenlang::resolver::resolve_with_natives;
use zenlang::stdlib::{native_names as stdlib_names, register_builtins};
use zenlang::typeck::check;
use zenlang::vm::VMContext;
use zenlang::{Value, VM};

// ─── Rust type exposed to scripts ───────────────────────────────────────

#[derive(Clone)]
struct ScriptPlayer {
    name: String,
    x: f32,
    y: f32,
    speed: f32,
    health: i32,
}

impl ScriptPlayer {
    fn new(name: &str) -> Self {
        Self { name: name.to_string(), x: 0.0, y: 0.0, speed: 100.0, health: 100 }
    }
}

// ─── Bevy resource for the scripting engine ─────────────────────────────

/// Wraps the Zenlang VM behind interior mutability so Bevy systems can
/// access it via `Res` (shared reference).
struct ScriptingEngine {
    vm: RefCell<VM>,
    source: String,
}

// ─── Setup ───────────────────────────────────────────────────────────────

fn setup_scripting(mut commands: Commands) {
    let mut vm = VM::new();
    register_builtins(&mut vm);

    // Register a native to create player objects from scripts
    vm.register_native("create_player", Rc::new(|_ctx: &mut VMContext, args: &[Value]| {
        let name = args.first().and_then(|v| v.as_str()).unwrap_or_default();
        Ok(Value::Foreign(Rc::new(RefCell::new(
            ForeignObject::new("ScriptPlayer", ScriptPlayer::new(&name)),
        ))))
    }));

    // Register the ScriptPlayer type so scripts can read/write fields
    vm.register_type::<ScriptPlayer>("ScriptPlayer")
        .field("x",
            |obj: &Value| with_foreign::<ScriptPlayer, _, _>(obj, |p| Ok(Value::Float(p.x as f64))),
            |obj: &mut Value, val: Value| {
                let x = val.as_float().unwrap_or(0.0) as f32;
                with_foreign_mut::<ScriptPlayer, _, _>(obj, |p| { p.x = x; Ok(()) })
            },
        )
        .field("y",
            |obj: &Value| with_foreign::<ScriptPlayer, _, _>(obj, |p| Ok(Value::Float(p.y as f64))),
            |obj: &mut Value, val: Value| {
                let y = val.as_float().unwrap_or(0.0) as f32;
                with_foreign_mut::<ScriptPlayer, _, _>(obj, |p| { p.y = y; Ok(()) })
            },
        )
        .field("health",
            |obj: &Value| with_foreign::<ScriptPlayer, _, _>(obj, |p| Ok(Value::Int(p.health as i64))),
            |obj: &mut Value, val: Value| {
                let h = val.as_int().unwrap_or(0) as i32;
                with_foreign_mut::<ScriptPlayer, _, _>(obj, |p| { p.health = h; Ok(()) })
            },
        )
        .field("speed",
            |obj: &Value| with_foreign::<ScriptPlayer, _, _>(obj, |p| Ok(Value::Float(p.speed as f64))),
            |obj: &mut Value, val: Value| {
                let s = val.as_float().unwrap_or(0.0) as f32;
                with_foreign_mut::<ScriptPlayer, _, _>(obj, |p| { p.speed = s; Ok(()) })
            },
        );

    // Compile initial script
    let source = "\
let player = create_player(\"Hero\");
player.speed = 150.0;
print(\"Player created:\", player.name);
player
";

    let mut names = vm.native_names();
    for n in &stdlib_names() {
        if !names.contains(n) {
            names.push(n.clone());
        }
    }

    let tokens = Lexer::new(source).tokenize().unwrap();
    let mut program = Parser::new(&tokens).parse().unwrap();
    let mut symbols = resolve_with_natives(&mut program, &names).unwrap();
    let types = check(&program, &mut symbols).unwrap();
    let (fns, global_names) = compile(&program, &types, &symbols, &names, source).unwrap();

    vm.load_bytecode(fns, global_names);

    commands.insert_resource(ScriptingEngine { vm: RefCell::new(vm), source: source.to_string() });
}

// ─── Per-frame script execution ──────────────────────────────────────────

fn run_scripts(engine: Res<ScriptingEngine>) {
    if let Ok(mut vm) = engine.vm.try_borrow_mut() {
        match vm.run_main() {
            Ok(val) => println!("[script] result: {:?}", val),
            Err(e) => println!("[script] error: {}", e),
        }
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_systems(Startup, setup_scripting)
        .add_systems(Update, run_scripts)
        .run();
}
