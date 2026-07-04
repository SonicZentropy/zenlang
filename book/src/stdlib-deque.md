# Deque

A double-ended queue backed by an array. Supports O(1) push and pop at both ends.

```rust
deque_new() -> Deque                      // Create an empty deque
deque_push_front(deque, value)            // Add to front
deque_push_back(deque, value)             // Add to back
deque_pop_front(deque) -> Any             // Remove and return front
deque_pop_back(deque) -> Any              // Remove and return back
deque_peek_front(deque) -> Any            // View front without removing
deque_peek_back(deque) -> Any             // View back without removing
deque_len(deque) -> i64                   // Number of elements
deque_is_empty(deque) -> bool             // Check if empty
deque_to_array(deque) -> Array            // Convert to array
```

```rust
let d = deque_new();
deque_push_back(d, 10);
deque_push_back(d, 20);
deque_push_front(d, 5);

assert(deque_len(d) == 3);
assert(deque_pop_front(d) == 5);
assert(deque_pop_back(d) == 20);
assert(deque_is_empty(d) == false);
```
