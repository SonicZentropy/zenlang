# Operators

## Arithmetic

| Operator | Description |
|----------|-------------|
| `+` | Addition |
| `-` | Subtraction / Negation |
| `*` | Multiplication |
| `/` | Division |
| `%` | Modulo |

## Comparison

| Operator | Description |
|----------|-------------|
| `==` | Equal |
| `!=` | Not equal |
| `<` | Less than |
| `>` | Greater than |
| `<=` | Less than or equal |
| `>=` | Greater than or equal |

## Logical

| Operator | Description |
|----------|-------------|
| `&&` | Logical AND (short-circuit) |
| `\|\|` | Logical OR (short-circuit) |
| `!` | Logical NOT |

## Bitwise

| Operator | Description |
|----------|-------------|
| `&` | Bitwise AND |
| `\|` | Bitwise OR |
| `^` | Bitwise XOR |
| `~` | Bitwise NOT |
| `<<` | Left shift |
| `>>` | Right shift |

## Compound Assignment

```
+=  -=  *=  /=  %=  &=  |=  ^=  <<=  >>=
```

## Other

| Operator | Description |
|----------|-------------|
| `.` | Field access / method call |
| `..` | Exclusive range |
| `..=` | Inclusive range |
| `\|>` | Pipe (forward application) |
| `->` | Return type annotation |
| `=>` | Match arm separator |
| `?` | Try (unwrap or propagate error) |
| `_` | Wildcard pattern / partial application placeholder |

## Pipe Operator

The pipe operator `|>` forwards the left-hand value as the first argument to the right-hand call.

```rust
fn double(x) { x * 2 }
5 |> double         // double(5) → 10

fn add(a, b) { a + b }
10 |> add(5)        // add(10, 5) → 15
```

Chaining makes data flow read naturally left-to-right:

```rust
[1, 2, 3]
    |> map(_, |x| x * 2)
    |> filter(_, |x| x > 2)
    |> collect()
// → [4, 6]
```

Pipe has lower precedence than `+`/`-`, so `x |> f + g` parses as `(f + g)(x)`.
