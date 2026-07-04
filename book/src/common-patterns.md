# Common Patterns

## Game Loop

```rust
struct Game {
    player_hp: i64,
    score: i64,
}

impl Game {
    fn new() -> Game { Game { player_hp: 100, score: 0 } }
    fn update(&mut self, dt: f64) {
        // game logic here
    }
    fn is_alive(&self) -> bool { self.player_hp > 0 }
}
```

## Error Handling

```rust
fn load_config(path: str) -> Result<map, str> {
    if !exists(path) {
        return Err("file not found");
    }
    let content = read(path);
    Ok(decode(content))
}

fn main() {
    match load_config("settings.json") {
        Ok(cfg) => print("loaded"),
        Err(e) => print("error: {e}"),
    };
}
```

## State Machine

```rust
enum State { Idle, Running, Paused }

struct Machine { state: State }

impl Machine {
    fn update(&mut self) {
        match self.state {
            Idle => { /* ... */ },
            Running => { /* ... */ },
            Paused => { /* ... */ },
        };
    }
}
```
