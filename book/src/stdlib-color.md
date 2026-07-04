# Color

Color utilities using 32-bit integers in `0xAARRGGBB` format.

```rust
rgba(r: i64, g: i64, b: i64, a: i64) -> i64    // Create from 0-255 components
hsla(h: f64, s: f64, l: f64, a: i64) -> i64     // Create from HSL + alpha
hex_color(hex: str) -> Option                    // Parse "#RRGGBB" or "#AARRGGBB"
lerp_color(a: i64, b: i64, t: f64) -> i64       // Linear interpolation
color_r(color: i64) -> i64                       // Extract red (0-255)
color_g(color: i64) -> i64                       // Extract green (0-255)
color_b(color: i64) -> i64                       // Extract blue (0-255)
color_a(color: i64) -> i64                       // Extract alpha (0-255)
```

```rust
let red = rgba(255, 0, 0, 255);
assert(color_r(red) == 255);
assert(color_a(red) == 255);

let teal = hex_color("#008080");
assert(is_some(teal));

let pink = hsla(330.0, 1.0, 0.7, 255);
let mid = lerp_color(red, 0x00FF00FF, 0.5);
```
