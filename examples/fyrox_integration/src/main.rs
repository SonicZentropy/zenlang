//! Fyrox integration example: embed Zenlang scripting into a Fyrox game.
//!
//! Run from `examples/fyrox_integration/`: `cargo run`
//!
//! Shows how to register Fyrox components as Zenlang foreign types and use
//! scripts to define entity behaviour (movement, AI, game logic).

use std::cell::RefCell;
use std::rc::Rc;

use fyrox::{
    core::{algebra::Vector3, pool::Handle},
    engine::{Engine, EngineInitParams, SerializationContext},
    event_loop::EventLoop,
    plugin::{Plugin, PluginConstructor, PluginContext},
    scene::{node::Node, Scene},
};

use zenlang::compiler::compile;
use zenlang::interop::{with_foreign, ForeignObject};
use zenlang::lexer::Lexer;
use zenlang::parser::Parser;
use zenlang::resolver::resolve_with_natives;
use zenlang::stdlib::{native_names as stdlib_names, register_builtins};
use zenlang::typeck::check;
use zenlang::vm::VMContext;
use zenlang::{Value, VM};

// ─── Rust type exposed to scripts ───────────────────────────────────────

#[derive(Clone)]
struct ScriptedEntity {
    name: String,
    x: f32,
    y: f32,
    z: f32,
    speed: f32,
    health: i32,
}

impl ScriptedEntity {
    fn new(name: &str) -> Self {
        Self { name: name.to_string(), x: 0.0, y: 0.0, z: 0.0, speed: 50.0, health: 100 }
    }
}

// ─── Plugin ──────────────────────────────────────────────────────────────

struct ScriptingPlugin;

impl PluginConstructor for ScriptingPlugin {
    fn create_plugin(
        &self,
        _scene_path: Option<&str>,
        _context: PluginContext,
    ) -> Box<dyn Plugin> {
        let mut vm = VM::new();
        register_builtins(&mut vm);

        // Register a native to spawn entities from scripts
        vm.register_native("spawn_entity", Rc::new(|_ctx: &mut VMContext, args: &[Value]| {
            let name = args.first().and_then(|v| v.as_str()).unwrap_or_default();
            Ok(Value::Foreign(Rc::new(RefCell::new(
                ForeignObject::new("ScriptedEntity", ScriptedEntity::new(&name)),
            ))))
        }));

        // Register the ScriptedEntity type
        vm.register_type::<ScriptedEntity>("ScriptedEntity")
            .field("x",
                |obj: &Value| with_foreign::<ScriptedEntity, _, _>(obj, |e| Ok(Value::Float(e.x as f64))),
                |obj: &mut Value, val: Value| {
                    let x = val.as_float().unwrap_or(0.0) as f32;
                    with_foreign_mut::<ScriptedEntity, _, _>(obj, |e| { e.x = x; Ok(()) })
                },
            )
            .field("y",
                |obj: &Value| with_foreign::<ScriptedEntity, _, _>(obj, |e| Ok(Value::Float(e.y as f64))),
                |obj: &mut Value, val: Value| {
                    let y = val.as_float().unwrap_or(0.0) as f32;
                    with_foreign_mut::<ScriptedEntity, _, _>(obj, |e| { e.y = y; Ok(()) })
                },
            )
            .field("health",
                |obj: &Value| with_foreign::<ScriptedEntity, _, _>(obj, |e| Ok(Value::Int(e.health as i64))),
                |obj: &mut Value, val: Value| {
                    let h = val.as_int().unwrap_or(0) as i32;
                    with_foreign_mut::<ScriptedEntity, _, _>(obj, |e| { e.health = h; Ok(()) })
                },
            );

        let source = "\
let goblin = spawn_entity(\"Goblin\");
goblin.x = 10.0;
goblin.y = 5.0;
print(\"Spawned:\", goblin.name, \"at\", goblin.x, goblin.y);
goblin
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
        vm.run_main().ok();

        Box::new(ScriptingPluginInstance { vm })
    }
}

struct ScriptingPluginInstance {
    vm: VM,
}

impl Plugin for ScriptingPluginInstance {
    fn update(&mut self, _engine: &mut Engine, _context: &mut PluginContext, _control: &mut ()) {
        // Each frame: run script logic
        // In a real game you would re-run per-frame scripts here,
        // update entity positions from script-managed data, etc.
        self.vm.run_main().ok();
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let engine = Engine::new(EngineInitParams::default(), SerializationContext::new()).unwrap();

    let mut plugin_paths = engine.embedded_plugins();
    plugin_paths.push(Box::new(ScriptingPlugin));

    engine.add_plugin_constructors(plugin_paths);

    // In a real project you'd load a scene and run the event loop.
    // For this example we just show the setup.
    println!("Fyrox + Zenlang example loaded.");
}
