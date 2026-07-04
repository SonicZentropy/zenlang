---
name: zen-skills
description: |
  Comprehensive Zen language coding guidelines. Use when writing, reviewing, or refactoring Zen code. Covers syntax, type system, control flow, functions, modules, memory model, standard library, embedding in Rust, and common patterns. Invoke with /zen-skills.
disable-model-invocation: false
---

# Zen Best Practices

Comprehensive guide for writing high-quality, idiomatic Zen code. Zen is a dynamically-typed, interpreted language embedded in Rust with seamless FFI interop.

## When to Apply

Reference these guidelines when:
- Writing new Zen scripts or modules
- Embedding Zen in a Rust application
- Calling Rust functions from Zen
- Working with Zen's type system or memory model
- Debugging or optimizing Zen code

---

## 1. Syntax Overview

### Variables & Binding

```zen
let x = 42              # immutable by default
let mut y = 10          # mutable
y = 20                  # OK, y is mutable
x = 5                   # ERROR: cannot reassign immutable
```

**Rule `zen-let-mut`:** Use `let` for values that don't change. Use `let mut` only when reassignment is needed. Zen does not have `const` or `static`.

### Data Types

```zen
# Primitives
let int_val = 42            # Int (i64)
let float_val = 3.14        # Float (f64)
let bool_val = true         # Bool
let str_val = "hello"       # String (immutable, refcounted)
let nil_val = nil           # Nil (null/void)

# Collections
let list = [1, 2, 3]        # List (mutable, dynamic array)
let tuple = (1, "two", 3)   # Tuple (fixed size, mixed types)
let map = {"a": 1, "b": 2}  # Map (string keys, mutable)
```

**Rule `zen-str-immutable`:** Strings are immutable in Zen. To modify, convert to list, modify, then join:
```zen
let chars = str.to_list()
chars[0] = 'H'
let new_str = chars.join("")
```

### Operators

```zen
# Arithmetic: + - * / %
# Comparison: == != < > <= >=
# Logical: and or not
# String: ++ (concatenation)
# Member: . (dot access)
# Index: [] (subscript)
```

**Rule `zen-no-ternary`:** Zen has no ternary operator. Use `if` expressions instead:
```zen
let result = if x > 0 { x } else { -x }
```

---

## 2. Type System

### Dynamic Typing with `any`

Zen is dynamically typed. All unannotated parameters default to `any`:

```zen
fn add(a, b) {      # a: any, b: any
    return a + b
}
```

Use `any` explicitly when you want to be clear:

```zen
fn process(data: any) -> any {
    return data
}
```

### Type Annotations (Optional)

Type annotations are hints for documentation and optimization, not enforcement:

```zen
fn greet(name: string) -> string {
    return "Hello, " ++ name
}

fn compute(x: int, y: int) -> float {
    return (x + y) as float
}
```

**Rule `zen-annotate-pub`:** Always annotate public function signatures for clarity:
```zen
# Good
pub fn calculate(x: float, y: float) -> float { ... }

# Bad - unclear interface
pub fn calculate(x, y) { ... }
```

### Type Checking

```zen
let val = 42
print(typeof(val))        # "int"
print(val is int)         # true
print(val is string)      # false
```

**Rule `zen-is-over-typeof`:** Use `is` for type checks, not string comparison on `typeof()`.

---

## 3. Control Flow

### If/Else

```zen
if condition {
    # ...
} else if other {
    # ...
} else {
    # ...
}
```

**Rule `zen-bool-expr`:** Conditions must be `bool`. No truthy/falsy coercion:
```zen
if x { }       # ERROR if x is int
if x != 0 { }  # OK
if list { }     # ERROR
if list.len() > 0 { }  # OK
```

### While Loops

```zen
let i = 0
while i < 10 {
    print(i)
    i = i + 1
}
```

**Rule `zen-no-c-style-for`:** Zen has no C-style `for` loop. Use `while` with a counter or iterate over lists.

### For-In Loops

```zen
# Iterate over list
let items = [1, 2, 3]
for item in items {
    print(item)
}

# Iterate over map
let map = {"a": 1, "b": 2}
for key, value in map {
    print(key ++ ": " ++ str(value))
}

# Iterate over string (yields chars)
for ch in "hello" {
    print(ch)
}
```

### Match Expressions

```zen
match value {
    1 => "one",
    2 => "two",
    _ => "other",
}

# With guards
match x {
    n if n > 0 => "positive",
    n if n < 0 => "negative",
    _ => "zero",
}
```

**Rule `zen-exhaustive-match`:** Always include a `_` wildcard unless you're sure all cases are covered. Zen doesn't enforce exhaustiveness.

---

## 4. Functions

### Basic Functions

```zen
fn add(a, b) {
    return a + b
}

# Implicit return (last expression)
fn add(a, b) {
    a + b
}
```

### Default Parameters

```zen
fn greet(name, greeting = "Hello") {
    return greeting ++ ", " ++ name ++ "!"
}

greet("World")              # "Hello, World!"
greet("World", "Hi")        # "Hi, World!"
```

### Closures

```zen
let double = fn(x) { x * 2 }
let add = |a, b| { a + b }

# Use with higher-order functions
let nums = [1, 2, 3]
let doubled = nums.map(fn(x) { x * 2 })
let evens = nums.filter(|x| { x % 2 == 0 })
let sum = nums.reduce(0, |acc, x| { acc + x })
```

**Rule `zen-closure-braces`:** Always use braces around closure bodies, even single expressions:
```zen
# Good
nums.map(|x| { x * 2 })

# Bad - syntax error
nums.map(|x| x * 2)
```

### Variadic Functions

```zen
fn sum_all(numbers...) {
    let total = 0
    for n in numbers {
        total = total + n
    }
    return total
}
```

---

## 5. Modules

### Module Structure

```
project/
├── main.zen
├── utils.zen
└── models/
    ├── __init__.zen
    └── user.zen
```

### Importing

```zen
# Import a module
import utils

# Import specific items
import utils.{format_date, parse_config}

# Import with alias
import utils.{format_date as fmt}
```

### Exporting

```zen
# utils.zen
pub fn format_date(d) {
    return str(d.month) ++ "/" ++ str(d.day) ++ "/" ++ str(d.year)
}

# Private by default
fn internal_helper() {
    # ...
}
```

**Rule `zen-pub-intentional`:** Only mark functions `pub` if they're part of the module's external API.

---

## 6. Memory Model

Zen uses reference counting (`Rc<T>`) for memory management. No garbage collector, no borrow checker.

### Ownership Rules

```zen
let a = [1, 2, 3]
let b = a           # b references same list (reference count = 2)
b.append(4)         # Both a and b see the change
print(a)            # [1, 2, 3, 4]
```

### No Move Semantics

Unlike Rust, Zen doesn't have move semantics. Assignment always copies the reference:
```zen
let a = "hello"
let b = a       # Both a and b point to same string
# Neither is invalidated
```

### Cyclic References

**Rule `zen-no-cycles`:** Avoid reference cycles. Zen does not have a tracing GC to collect them:
```zen
# BAD - creates a cycle
let a = []
let b = [a]
a.append(b)     # a -> b -> a

# BETTER - use weak references or restructure
```

---

## 7. Standard Library

### String Operations

```zen
let s = "hello world"
s.len()                # 11
s.to_upper()           # "HELLO WORLD"
s.to_lower()           # "hello world"
s.contains("world")    # true
s.starts_with("hello") # true
s.split(" ")           # ["hello", "world"]
s.replace("hello", "hi") # "hi world"
s.trim()               # "hello world"
s[0..5]                # "hello"
```

### List Operations

```zen
let list = [3, 1, 2]
list.len()             # 3
list.append(4)         # [3, 1, 2, 4]
list.pop()             # 4
list.insert(0, 0)      # [0, 3, 1, 2]
list.remove(1)         # [0, 1, 2]
list.contains(1)       # true
list.sort()            # [0, 1, 2]
list.reverse()         # [2, 1, 0]
list.map(|x| { x * 2 })   # [4, 2, 0]
list.filter(|x| { x > 1 }) # [2, 1]
list.reduce(0, |a, b| { a + b }) # 3
```

### Map Operations

```zen
let map = {"a": 1, "b": 2}
map.len()              # 2
map.keys()             # ["a", "b"]
map.values()           # [1, 2]
map.contains_key("a")  # true
map.get("a")           # 1
map.insert("c", 3)     # {"a": 1, "b": 2, "c": 3}
map.remove("b")        # {"a": 1, "c": 3}
```

### Math & Conversions

```zen
math.abs(-5)           # 5
math.min(1, 2)         # 1
math.max(1, 2)         # 2
math.floor(3.7)        # 3
math.ceil(3.2)         # 4
math.sqrt(16)          # 4.0
math.pi                # 3.14159...

str(42)                # "42"
str(3.14)              # "3.14"
str(true)              # "true"
int("42")              # 42
float("3.14")          # 3.14
```

### Iteration

```zen
# Range
for i in range(10) { }        # 0..9
for i in range(2, 10) { }     # 2..9
for i in range(0, 10, 2) { }  # 0, 2, 4, 6, 8

# Enumerate
for i, val in list.enumerate() { }

# Zip
for a, b in list1.zip(list2) { }
```

---

## 8. Error Handling

Zen uses `nil` for failure, not exceptions:

```zen
# Division by zero returns nil
let result = 10 / 0
print(result)   # nil

# File operations return nil on failure
let content = std.fs.read("nonexistent.txt")
print(content)  # nil
```

**Rule `zen-check-nil`:** Always check for `nil` after operations that can fail:
```zen
let result = risky_operation()
if result == nil {
    print("Operation failed")
    return
}
# Use result
```

---

## 9. Embedding in Rust

### Creating a VM

```rust
use zenlang::vm::VM;

let mut vm = VM::new();
let result = vm.exec("print('Hello from Zen!')")?;
```

### Calling Zen from Rust

```rust
let result = vm.call("function_name", &[arg1, arg2])?;
```

### Exposing Rust to Zen

```rust
use zenlang::value::{Value, ZenForeign};
use zenlang::macros::{zen_methods, ZenForeign};

#[derive(ZenForeign)]
struct Counter {
    count: i64,
}

#[zen_methods]
impl Counter {
    fn new(initial: i64) -> Self {
        Counter { count: initial }
    }

    fn increment(&mut self) {
        self.count += 1;
    }

    fn get(&self) -> i64 {
        self.count
    }
}
```

**Rule `zen-mut-self`:** Use `&mut self` for methods that modify state, `&self` for read-only.

### Registering Types

```rust
vm.register_foreign::<Counter>();
```

---

## 10. Common Anti-Patterns

### Don't: Modify While Iterating

```zen
# BAD
for item in list {
    if item == 2 {
        list.remove(1)    # Undefined behavior
    }
}

# GOOD - filter instead
let result = list.filter(|x| { x != 2 })
```

### Don't: Use `==` for Float Comparison

```zen
# BAD
if x == 3.14 { }

# GOOD - use epsilon comparison
if (x - 3.14).abs() < 0.0001 { }
```

### Don't: Shadow Loop Variables

```zen
# BAD - confusing
let x = 10
for x in list { }
print(x)    # Last value from list, not 10

# GOOD - use different name
let x = 10
for item in list { }
print(x)    # Still 10
```

### Don't: Forget `mut` for Reassignment

```zen
# BAD
let x = 10
x = x + 1     # ERROR

# GOOD
let mut x = 10
x = x + 1     # OK
```

### Don't: Use `is` for Value Equality

```zen
# BAD
if x is 42 { }      # Identity check, not equality

# GOOD
if x == 42 { }      # Value equality
```

---

## 11. Performance Tips

### Prefer `let` Over `let mut` When Possible

Immutable bindings are slightly faster for the VM to optimize.

### Avoid Unnecessary Allocations

```zen
# BAD - creates intermediate string
let result = "hello" ++ " " ++ "world"

# GOOD - single string literal
let result = "hello world"
```

### Use `list` for Homogeneous Data

```zen
# GOOD - same type
let nums = [1, 2, 3]

# AVOID if possible - mixed types
let mixed = [1, "two", 3.0]
```

---

## 12. Testing

### Writing Tests

```zen
# tests/math_test.zen
import assert

fn test_addition() {
    assert.equal(2 + 2, 4)
}

fn test_string_concat() {
    assert.equal("hello" ++ " " ++ "world", "hello world")
}

fn test_list_operations() {
    let list = [1, 2, 3]
    assert.equal(list.len(), 3)
    assert.equal(list[0], 1)
}
```

### Running Tests

```bash
cargo run -- test tests/
```

---

## Quick Reference Table

| Feature | Syntax | Example |
|---------|--------|---------|
| Variable | `let x = val` | `let x = 10` |
| Mutable | `let mut x = val` | `let mut x = 10` |
| Function | `fn name(args) { }` | `fn add(a, b) { a + b }` |
| Closure | `\|args\| { body }` | `\|x\| { x * 2 }` |
| If | `if cond { } else { }` | `if x > 0 { }` |
| While | `while cond { }` | `while i < 10 { }` |
| For | `for x in iter { }` | `for i in range(10) { }` |
| Match | `match val { ... }` | `match x { 1 => "one", _ => "other" }` |
| List | `[val, ...]` | `[1, 2, 3]` |
| Map | `{key: val, ...}` | `{"a": 1}` |
| Tuple | `(val, ...)` | `(1, "two", 3.0)` |
| Import | `import mod` | `import utils` |
| Export | `pub fn name` | `pub fn helper()` |
| Nil | `nil` | `if x == nil { }` |
| Type check | `val is type` | `x is int` |
| String concat | `++` | `"a" ++ "b"` |
| Index | `[n]` | `list[0]` |
| Slice | `[start..end]` | `s[0..5]` |
