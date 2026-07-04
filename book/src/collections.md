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

Method-call style is also supported:

```rust
let arr = [1, 2, 3];
arr.push(4);             // [1, 2, 3, 4]
arr.pop();               // 4
arr.insert(1, 15);       // [1, 15, 2, 3]
arr.remove(1);           // 15
arr.len();               // 3
arr.contains(2);         // true
arr.is_empty();          // false
arr.clear();
arr.is_empty();          // true
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

Method-call style is also supported:

```rust
let m = map_new();
m.set("hp", 100);
m.set("name", "Hero");

m.get("hp");            // Some(100)
m.has("hp");            // true
m.contains_key("hp");   // true
m.remove("hp");         // Some(100)
m.keys();               // ["name"]
m.values();             // ["Hero"]
m.len();                // 1
m.is_empty();           // false
m.clear();
m.is_empty();           // true
```

### Iterating Maps

```rust
for kv in m {
    let key = kv[0];
    let val = kv[1];
    print(key + ": " + to_str(val));
}
```
