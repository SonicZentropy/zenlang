# Noise

Procedural noise functions for game content generation.

```rust
perlin2d(x: f64, y: f64, seed: i64) -> f64     // 2D value noise in [0, 1]
simplex2d(x: f64, y: f64, seed: i64) -> f64    // 2D simplex-like noise
fbm2d(x: f64, y: f64, octaves: i64, seed: i64) -> f64  // Fractal Brownian Motion
```

```rust
let n = perlin2d(1.5, 2.3, 42);
assert(n >= 0.0 && n <= 1.0);

let terrain = fbm2d(0.5, 0.5, 4, 123);
assert(terrain >= 0.0 && terrain <= 1.0);
```
