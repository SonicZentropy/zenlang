# Iterators

The standard `std::iter` module provides iterator adapters:

```rust
use std::iter::*;

map(arr, fn)          // Transform each element
filter(arr, fn)       // Keep elements matching predicate
reduce(arr, fn, init) // Fold left
zip(a, b)             // Pair-wise combine two arrays
```

## Examples

```rust
let doubled = map([1, 2, 3], |x| x * 2);
// [2, 4, 6]

let evens = filter([1, 2, 3, 4], |x| x % 2 == 0);
// [2, 4]

let sum = reduce([1, 2, 3], |a, b| a + b, 0);
// 6

let pairs = zip(["a", "b"], [1, 2]);
// [["a", 1], ["b", 2]]
```
