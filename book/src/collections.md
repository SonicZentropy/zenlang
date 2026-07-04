# Arrays and Maps

## Arrays

Arrays are heterogeneous, dynamically-sized sequences.

```rust
let arr = [1, 2, 3];
push(arr, 4);         // [1, 2, 3, 4]
let last = pop(arr);  // 4, arr is now [1, 2, 3]
insert(arr, 1, 15);   // [1, 15, 2, 3]
remove(arr, 1);       // 15, arr is now [1, 2, 3]
len(arr);             // 3
arr[0];               // 1
```

## Maps (Dictionaries)

Maps are key-value dictionaries. Keys can be any value type.

```rust
let m = map_new();
map_set(m, "hp", 100);
map_set(m, "name", "Hero");

map_get(m, "hp");       // Some(100)
map_has(m, "hp");       // true
map_remove(m, "hp");    // Some(100)
map_keys(m);            // ["name"]
map_values(m);          // ["Hero"]
map_len(m);             // 1
map_clear(m);
len(m);                 // 0
```

### Iterating Maps

```rust
for kv in m {
    let key = kv[0];
    let val = kv[1];
    print(key + ": " + to_str(val));
}
```
