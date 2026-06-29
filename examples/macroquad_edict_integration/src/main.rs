use std::cell::RefCell;
use std::rc::Rc;

use macroquad::prelude::*;
use macroquad::rand::gen_range;

use edict::component::Component;
use edict::entity::EntityId;
use edict::world::World;

use zenlang::compiler::compile;
use zenlang::{Value, VM};
use zenlang::error::Result;
use zenlang::interop::{with_foreign, with_foreign_mut};
use zenlang::value::ForeignObject;
use zenlang::lexer::Lexer;
use zenlang::parser::Parser;
use zenlang::resolver::resolve_with_natives;
use zenlang::stdlib::{native_names as stdlib_names, register_builtins};
use zenlang::typeck::check;
use zenlang::vm::VMContext;

// ─── ECS Components ────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Position(f32, f32);

#[derive(Clone, Copy)]
struct Velocity(f32, f32);

#[derive(Clone, Copy)]
struct Renderable {
    size: f32,
    r: u8,
    g: u8,
    b: u8,
}

impl Component for Position {}
impl Component for Velocity {}
impl Component for Renderable {}

// ─── Script-side handle into an ECS entity ─────────────────────

#[derive(Clone)]
struct EntityHandle {
    slot: usize,
}

// ─── Shared ECS state accessible from native fn closures ───────

struct EcsState {
    world: World,
    handles: Vec<EntityId>,
}

// ─── Embedded Zenlang script ───────────────────────────────────

const SCRIPT: &str = r#"
fn update() {
    let i = 0;
    while i < entity_count() {
        let e = get_entity(i);
        e.vx = e.vx + 1.0;
        e.vy = e.vy + 1.0;
        let max_speed = 300.0;
        if abs(e.vx) > max_speed {
            if e.vx > 0.0 { e.vx = max_speed; } else { e.vx = -max_speed; }
        }
        if abs(e.vy) > max_speed {
            if e.vy > 0.0 { e.vy = max_speed; } else { e.vy = -max_speed; }
        }
        i = i + 1;
    }
}

if entity_count() == 0 {
    spawn_entity(100.0, 100.0, 0.0, 0.0, 8.0, 200.0, 200.0, 200.0);
}

if is_key_pressed("space") != 0 {
    spawn_entity(200.0, 200.0, 0.0, 0.0, 10.0, 255.0, 80.0, 80.0);
}

update();
"#;

// ─── Helper: extract slot index from a Value::Foreign ----------

fn get_slot(val: &Value) -> Result<usize> {
    with_foreign::<EntityHandle, usize, _>(val, |h| Ok(h.slot))
}

fn get_slot_mut(val: &mut Value) -> Result<usize> {
    with_foreign_mut::<EntityHandle, usize, _>(val, |h| Ok(h.slot))
}

// ─── Native function registration ──────────────────────────────

fn register_natives(vm: &mut VM, ecs: &Rc<RefCell<EcsState>>) {
    // spawn_entity(x, y, vx, vy, size, r, g, b) -> EntityHandle
    {
        let s = ecs.clone();
        vm.register_native("spawn_entity", Rc::new(move |_: &mut VMContext, args: &[Value]| {
            let x = args.get(0).and_then(|v| v.as_float()).unwrap_or(0.0) as f32;
            let y = args.get(1).and_then(|v| v.as_float()).unwrap_or(0.0) as f32;
            let vx = args.get(2).and_then(|v| v.as_float()).unwrap_or(0.0) as f32;
            let vy = args.get(3).and_then(|v| v.as_float()).unwrap_or(0.0) as f32;
            let size = args.get(4).and_then(|v| v.as_float()).unwrap_or(8.0) as f32;
            let r = args.get(5).and_then(|v| v.as_float()).unwrap_or(255.0) as u8;
            let g = args.get(6).and_then(|v| v.as_float()).unwrap_or(255.0) as u8;
            let b = args.get(7).and_then(|v| v.as_float()).unwrap_or(255.0) as u8;

            let mut guard = s.borrow_mut();
            let entity = guard.world.spawn((
                Position(x, y),
                Velocity(vx, vy),
                Renderable { size, r, g, b },
            ));
            let id = entity.id();
            let slot = guard.handles.len();
            guard.handles.push(id);

            Ok(Value::Foreign(Rc::new(RefCell::new(
                ForeignObject::new("EntityHandle", EntityHandle { slot }),
            ))))
        }));
    }

    // entity_count() -> int
    {
        let s = ecs.clone();
        vm.register_native("entity_count", Rc::new(move |_: &mut VMContext, _: &[Value]| {
            let guard = s.borrow();
            Ok(Value::Int(guard.handles.len() as i64))
        }));
    }

    // get_entity(idx) -> EntityHandle | nil
    {
        let s = ecs.clone();
        vm.register_native("get_entity", Rc::new(move |_: &mut VMContext, args: &[Value]| {
            let idx = args.first().and_then(|v| v.as_int()).unwrap_or(0) as usize;
            let guard = s.borrow();
            if idx < guard.handles.len() {
                Ok(Value::Foreign(Rc::new(RefCell::new(
                    ForeignObject::new("EntityHandle", EntityHandle { slot: idx }),
                ))))
            } else {
                Ok(Value::Nil)
            }
        }));
    }

    // screen_width() -> float
    vm.register_native("screen_width", Rc::new(|_: &mut VMContext, _: &[Value]| {
        Ok(Value::Float(screen_width() as f64))
    }));

    // screen_height() -> float
    vm.register_native("screen_height", Rc::new(|_: &mut VMContext, _: &[Value]| {
        Ok(Value::Float(screen_height() as f64))
    }));

    // is_key_pressed(key) -> bool
    vm.register_native("is_key_pressed", Rc::new(
        |_: &mut VMContext, args: &[Value]| {
            let key = args.first().and_then(|v| v.as_str()).unwrap_or_default();
            let pressed = match key.as_str() {
                "space" => is_key_pressed(KeyCode::Space),
                _ => false,
            };
            Ok(Value::Bool(pressed))
        },
    ));

    // rand_range(min, max) -> float
    vm.register_native("rand_range", Rc::new(
        |_: &mut VMContext, args: &[Value]| {
            let min = args.get(0).and_then(|v| v.as_float()).unwrap_or(0.0);
            let max = args.get(1).and_then(|v| v.as_float()).unwrap_or(1.0);
            Ok(Value::Float(gen_range(min, max)))
        },
    ));

    // delta_time() -> float
    vm.register_native("delta_time", Rc::new(|_: &mut VMContext, _: &[Value]| {
        Ok(Value::Float(get_frame_time() as f64))
    }));
}

// ─── Foreign type field registration ───────────────────────────

fn register_entity_type(vm: &mut VM, ecs: &Rc<RefCell<EcsState>>) {
    // Position fields: x, y
    vm.register_type::<EntityHandle>("EntityHandle")
        .field(
            "x",
            read_entity(ecs, |w, id| {
                w.get::<&Position>(id)
                    .map(|p| Value::Float(p.0 as f64))
                    .unwrap_or(Value::Nil)
            }),
            write_entity(ecs, |w, id, v| {
                if let Ok(p) = w.get::<&mut Position>(id) {
                    p.0 = v as f32;
                }
            }),
        )
        .field(
            "y",
            read_entity(ecs, |w, id| {
                w.get::<&Position>(id)
                    .map(|p| Value::Float(p.1 as f64))
                    .unwrap_or(Value::Nil)
            }),
            write_entity(ecs, |w, id, v| {
                if let Ok(p) = w.get::<&mut Position>(id) {
                    p.1 = v as f32;
                }
            }),
        )
        // Velocity fields: vx, vy
        .field(
            "vx",
            read_entity(ecs, |w, id| {
                w.get::<&Velocity>(id)
                    .map(|v| Value::Float(v.0 as f64))
                    .unwrap_or(Value::Nil)
            }),
            write_entity(ecs, |w, id, v| {
                if let Ok(vel) = w.get::<&mut Velocity>(id) {
                    vel.0 = v as f32;
                }
            }),
        )
        .field(
            "vy",
            read_entity(ecs, |w, id| {
                w.get::<&Velocity>(id)
                    .map(|v| Value::Float(v.1 as f64))
                    .unwrap_or(Value::Nil)
            }),
            write_entity(ecs, |w, id, v| {
                if let Ok(vel) = w.get::<&mut Velocity>(id) {
                    vel.1 = v as f32;
                }
            }),
        )
        // Renderable fields: size, r, g, b
        .field(
            "size",
            read_entity(ecs, |w, id| {
                w.get::<&Renderable>(id)
                    .map(|r| Value::Float(r.size as f64))
                    .unwrap_or(Value::Nil)
            }),
            write_entity(ecs, |w, id, v| {
                if let Ok(r) = w.get::<&mut Renderable>(id) {
                    r.size = v as f32;
                }
            }),
        )
        .field(
            "r",
            read_entity(ecs, |w, id| {
                w.get::<&Renderable>(id)
                    .map(|r| Value::Float(r.r as f64))
                    .unwrap_or(Value::Nil)
            }),
            write_entity(ecs, |w, id, v| {
                if let Ok(r) = w.get::<&mut Renderable>(id) {
                    r.r = v as u8;
                }
            }),
        )
        .field(
            "g",
            read_entity(ecs, |w, id| {
                w.get::<&Renderable>(id)
                    .map(|r| Value::Float(r.g as f64))
                    .unwrap_or(Value::Nil)
            }),
            write_entity(ecs, |w, id, v| {
                if let Ok(r) = w.get::<&mut Renderable>(id) {
                    r.g = v as u8;
                }
            }),
        )
        .field(
            "b",
            read_entity(ecs, |w, id| {
                w.get::<&Renderable>(id)
                    .map(|r| Value::Float(r.b as f64))
                    .unwrap_or(Value::Nil)
            }),
            write_entity(ecs, |w, id, v| {
                if let Ok(r) = w.get::<&mut Renderable>(id) {
                    r.b = v as u8;
                }
            }),
        );
}

// ─── Helpers to build field accessors ─────────────────────────

fn read_entity<F>(ecs: &Rc<RefCell<EcsState>>, f: F) -> impl Fn(&Value) -> Result<Value>
where
    F: Fn(&mut World, EntityId) -> Value + 'static,
{
    let s = ecs.clone();
    move |obj: &Value| -> Result<Value> {
        let slot = get_slot(obj)?;
        let mut guard = s.borrow_mut();
        if let Some(id) = guard.handles.get(slot).copied() {
            return Ok(f(&mut guard.world, id));
        }
        Ok(Value::Nil)
    }
}

fn write_entity<F>(ecs: &Rc<RefCell<EcsState>>, f: F) -> impl Fn(&mut Value, Value) -> Result<()>
where
    F: Fn(&mut World, EntityId, f64) + 'static,
{
    let s = ecs.clone();
    move |obj: &mut Value, val: Value| -> Result<()> {
        let v = val.as_float().unwrap_or(0.0);
        let slot = get_slot_mut(obj)?;
        let mut guard = s.borrow_mut();
        if let Some(id) = guard.handles.get(slot).copied() {
            f(&mut guard.world, id, v);
        }
        Ok(())
    }
}

// ─── Initialisation ────────────────────────────────────────────

struct Game {
    vm: RefCell<VM>,
    ecs: Rc<RefCell<EcsState>>,
}

fn setup() -> Game {
    let ecs = Rc::new(RefCell::new(EcsState {
        world: World::new(),
        handles: Vec::new(),
    }));

    let mut vm = VM::new();
    register_builtins(&mut vm);
    register_natives(&mut vm, &ecs);
    register_entity_type(&mut vm, &ecs);

    let mut names = vm.native_names();
    for n in stdlib_names() {
        if !names.contains(&n) {
            names.push(n);
        }
    }

    let tokens = Lexer::new(SCRIPT).tokenize().unwrap();
    let mut program = Parser::new(&tokens).parse().unwrap();
    let mut symbols = resolve_with_natives(&mut program, &names).unwrap();
    let types = check(&program, &mut symbols).unwrap();
    let (fns, global_names) = compile(&program, &types, &symbols, &names, SCRIPT).unwrap();

    vm.load_bytecode(fns, global_names);
    let _ = vm.run_main();

    Game { vm: RefCell::new(vm), ecs }
}

// ─── Entry point ───────────────────────────────────────────────

#[macroquad::main("Zenlang + Macroquad + Edict")]
async fn main() {
    let game = setup();

    loop {
        clear_background(Color::from_rgba(15, 15, 30, 255));

        // 1. Run the Zenlang script (AI / gameplay logic)
        if let Ok(mut vm) = game.vm.try_borrow_mut() {
            let _ = vm.run_main();
        }

        let dt = get_frame_time();

        // 2. Physics: apply velocity, bounce off walls
        {
            let ecs = game.ecs.borrow_mut();
            for (pos, vel) in ecs.world.view::<(&mut Position, &Velocity)>() {
                pos.0 += vel.0 * dt;
                pos.1 += vel.1 * dt;
            }
            for (pos, vel) in ecs.world.view::<(&mut Position, &mut Velocity)>() {
                let sw = screen_width();
                let sh = screen_height();
                if pos.0 > sw {
                    pos.0 = sw;
                    vel.0 = -vel.0;
                }
                if pos.0 < 0.0 {
                    pos.0 = 0.0;
                    vel.0 = -vel.0;
                }
                if pos.1 > sh {
                    pos.1 = sh;
                    vel.1 = -vel.1;
                }
                if pos.1 < 0.0 {
                    pos.1 = 0.0;
                    vel.1 = -vel.1;
                }
            }
        }

        // 3. Render all entities
        {
            let ecs = game.ecs.borrow();
            for (pos, rend) in ecs.world.view::<(&Position, &Renderable)>() {
                let c = Color::from_rgba(rend.r, rend.g, rend.b, 255);
                draw_circle(pos.0, pos.1, rend.size, c);
            }
        }

        // 4. UI overlay
        {
            let ecs = game.ecs.borrow();
            let info = format!("Entities: {}  |  SPACE to spawn", ecs.handles.len());
            draw_text(&info, 12.0, 24.0, 20.0, WHITE);
            draw_text(
                "Script-driven bouncing particles",
                12.0,
                48.0,
                14.0,
                Color::from_rgba(180, 180, 200, 255),
            );
        }

        next_frame().await;
    }
}
