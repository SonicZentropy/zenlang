# Iterators

The iterator protocol lets you process sequences of values without materializing
intermediate arrays. Use `iter(any)` to obtain an iterator, then chain lazy
adapters and finish with a terminal operation.

## Overview

```
source_iter ──► adapter ──► adapter ──► terminal_op ──► result
```

Each **lazy adapter** creates a lightweight wrapper that delays computation until
a **terminal operation** drives it. No intermediate arrays are allocated.

```rust
// Lazy chain — no computation yet
let it = map(arr, |x| x * 2);
let it = filter(it, |x| x > 10);

// Terminal op drives everything
let result = collect(it);
```

Lazy adapters return `Iter<T>` which cannot be indexed directly — the
type-checker catches this at compile time:

```rust,compile_fail
let r = map(arr, |x| x * 2);
r[0]  // ERROR: cannot index lazy iterator; call collect() to materialize
```

Fix with `collect()`:

```rust
let r = collect(map(arr, |x| x * 2));
r[0]  // OK — r is a materialized array
```

## Pipe Operator

The `|>` operator feeds a value into a function, making chained adapters read
naturally left-to-right:

```rust
arr |> map(_, |x| x * 2) |> filter(_, |x| x > 10) |> collect()
```

The `_` placeholder creates a partial application so `map(_, f)` becomes
`|__p0| map(__p0, f)`.

## Lazy Adapters

All adapters return **`Iter<T>`** and must be consumed by a terminal operation.

| Function | Signature | Description |
|----------|-----------|-------------|
| `iter(x)` | `any → Iter` | Obtain an iterator for any iterable |
| `map(it, f)` | `Iter, (T → U) → Iter<U>` | Transform each element |
| `filter(it, pred)` | `Iter, (T → bool) → Iter<T>` | Keep matching elements |
| `take(it, n)` | `Iter, i64 → Iter` | Yield first `n` elements |
| `skip(it, n)` | `Iter, i64 → Iter` | Skip first `n` elements |
| `chain(a, b)` | `Iter, Iter → Iter` | Concatenate two iterators |
| `zip(a, b)` | `Iter, Iter → Iter<[T, U]>` | Pair elements from two iterators |
| `enumerate(it)` | `Iter → Iter<[i64, T]>` | Pair each element with its index |
| `step_by(it, n)` | `Iter, i64 → Iter` | Yield every nth element |
| `cycle(it)` | `Iter → Iter` | Repeat infinitely |
| `inspect(it, f)` | `Iter, (T → ()) → Iter<T>` | Side-effect for debugging |
| `flatten(it)` | `Iter<Iter<T>> → Iter<T>` | Flatten nested iterators |
| `flat_map(it, f)` | `Iter, (T → Iter<U>) → Iter<U>` | Map then flatten |
| `scan(it, init, f)` | `Iter, U, (U, T → U) → Iter<U>` | Fold-like accumulator stream |

### `iter(x)`

Convert any iterable value into an iterator:

```rust
iter([1, 2, 3])         // array iterator
iter(0..10)             // range iterator
iter("hello")           // string iterator (characters)
iter(some_map)          // map iterator (key-value pairs)
```

The `for` loop calls `iter()` internally, so these are equivalent:

```rust
for x in arr { ... }
// desugars to:
let it = iter(arr);
loop {
    match it.next() {
        Some(v) => { ... }
        None => { break; }
    }
}
```

### `map(it, f)`

Transform each element by applying a closure:

```rust
collect(map([1, 2, 3], |x| x * 2))  // [2, 4, 6]
```

### `filter(it, pred)`

Keep only elements for which the predicate returns `true`:

```rust
collect(filter([1, 2, 3, 4, 5], |x| x % 2 == 0))  // [2, 4]
```

### `take(it, n)`

Yield the first `n` elements, then stop:

```rust
collect(take(0..100, 3))  // [0, 1, 2]
```

### `skip(it, n)`

Skip the first `n` elements, yield the rest:

```rust
collect(skip(0..10, 7))  // [7, 8, 9]
```

### `chain(a, b)`

First yield all elements from `a`, then all from `b`:

```rust
collect(chain([1, 2], [3, 4]))  // [1, 2, 3, 4]
```

### `zip(a, b)`

Pair elements from two iterators into two-element arrays. Stops when either
iterator is exhausted:

```rust
collect(zip(["a", "b", "c"], [1, 2]))
// [["a", 1], ["b", 2]]
```

### `enumerate(it)`

Pair each element with its zero-based index:

```rust
collect(enumerate(["a", "b", "c"]))
// [[0, "a"], [1, "b"], [2, "c"]]
```

### `step_by(it, n)`

Yield every nth element, starting with the first:

```rust
collect(step_by(0..10, 3))  // [0, 3, 6, 9]
```

### `cycle(it)`

Repeat the iterator infinitely. On the first pass elements are cached; after
exhaustion the cache repeats:

```rust
collect(take(cycle([1, 2, 3]), 7))  // [1, 2, 3, 1, 2, 3, 1]
```

### `inspect(it, f)`

Call `f` on each element for debugging or logging, then pass it through:

```rust
let r = collect(inspect([1, 2, 3], |x| print("got:" + to_str(x))));
// prints "got:1", "got:2", "got:3"
// r == [1, 2, 3]
```

### `flatten(it)`

Flatten one level of nested iterators:

```rust
collect(flatten([[1, 2], [3, 4, 5]]))  // [1, 2, 3, 4, 5]
```

### `flat_map(it, f)`

Map each element to an iterator, then flatten the results:

```rust
collect(flat_map([1, 2, 3], |x| [x, x * 10]))
// [1, 10, 2, 20, 3, 30]
```

### `scan(it, init, f)`

Like `fold` but yields each accumulator value as an iterator:

```rust
collect(scan([1, 2, 3, 4], 0, |acc, x| acc + x))
// [1, 3, 6, 10]
```

## Terminal Operations

Terminal operations eagerly consume the iterator and return a single value.

| Function | Signature | Description |
|----------|-----------|-------------|
| `collect(it)` | `Iter → [T]` | Materialize into an array |
| `fold(it, init, f)` | `Iter, U, (U, T → U) → U` | Reduce to single value |
| `count(it)` | `Iter → i64` | Count elements |
| `sum(it)` | `Iter → i64` | Sum of integers |
| `product(it)` | `Iter → i64` | Product of integers |
| `all(it, pred)` | `Iter, (T → bool) → bool` | True if all match |
| `any(it, pred)` | `Iter, (T → bool) → bool` | True if any match |
| `find(it, pred)` | `Iter, (T → bool) → Option<T>` | First matching element |
| `position(it, pred)` | `Iter, (T → bool) → Option<i64>` | Index of first match |
| `min(it)` | `Iter → Option<i64>` | Minimum (integers) |
| `max(it)` | `Iter → Option<i64>` | Maximum (integers) |
| `join(it, sep)` | `Iter, str → str` | Concatenate string representations |
| `partition(it, pred)` | `Iter, (T → bool) → [[T], [T]]` | Split by predicate |

### `collect(it)`

Consume the iterator and collect all elements into an array:

```rust
let arr = collect(0..5);  // [0, 1, 2, 3, 4]
```

This is the primary way to materialize a lazy chain:

```rust
let result = arr |> map(_, |x| x * 2) |> filter(_, |x| x > 5) |> collect();
```

### `fold(it, init, f)`

Accumulate elements left-to-right:

```rust
fold([1, 2, 3], 0, |acc, x| acc + x)  // 6
```

### `count(it)`

Count the number of elements:

```rust
count(0..100)  // 100
```

### `sum(it)` / `product(it)`

Sum or product of integer elements (panics on non-integer types):

```rust
sum([1, 2, 3, 4, 5])      // 15
product([1, 2, 3, 4, 5])  // 120
```

### `all(it, pred)` / `any(it, pred)`

Test whether all or any elements satisfy a predicate. Short-circuits:

```rust
all([1, 2, 3], |x| x > 0)   // true
any([1, 2, 3], |x| x > 5)   // false
```

### `find(it, pred)` / `position(it, pred)`

Find the first matching element or its index:

```rust
find([1, 2, 3], |x| x > 1)         // Some(2)
position([1, 2, 3], |x| x > 1)     // Some(1)
```

### `min(it)` / `max(it)`

Minimum or maximum of integer elements:

```rust
min([3, 1, 4, 1, 5, 9])   // Some(1)
max([3, 1, 4, 1, 5, 9])   // Some(9)
```

### `join(it, sep)`

Convert each element to its string representation and concatenate with a separator:

```rust
join([1, 2, 3], ", ")  // "1, 2, 3"
```

### `partition(it, pred)`

Split elements into two arrays: those matching the predicate (first) and those
not matching (second):

```rust
partition([1, 2, 3, 4], |x| x % 2 == 0)
// [[2, 4], [1, 3]]
```

## Chaining Examples

Combine adapters with the pipe operator for readable data pipelines:

```rust
// Sum of squares of even numbers
let sum = arr
    |> filter(_, |x| x % 2 == 0)
    |> map(_, |x| x * x)
    |> fold(_, 0, |acc, x| acc + x);

// First 3 non-empty string lengths
let result = strings
    |> filter(_, |s| len(s) > 0)
    |> map(_, |s| len(s))
    |> take(_, 3)
    |> collect();
```

## Performance

Lazy adapters allocate one `ForeignObject` per adapter in the chain —
approximately 40-80 bytes each. Per-element overhead is roughly
200 nanoseconds per adapter layer (release mode). A single `map` adapter adds
about 40% overhead versus a raw `for` loop, but avoids allocating intermediate
arrays that the old eager prelude required (which was 1.5-2.7x slower than the
lazy approach on a 100K-element benchmark).
