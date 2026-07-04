# v0.4.0 Implementation Plan

## Done

### Method Chaining on Iterators ✅
- `Type::Iter(_)` arm in method-call type-checker (typeck.rs) — allows `.map()`, `.filter()`, `.collect()` etc. on lazy iterator values
- `add_iter_methods()` helper registers 26 methods on all 17 iterator types: `.map()`, `.filter()`, `.take()`, `.skip()`, `.chain()`, `.zip()`, `.enumerate()`, `.step_by()`, `.cycle()`, `.inspect()`, `.flatten()`, `.flat_map()`, `.scan()`, `.collect()`, `.fold()`, `.count()`, `.all()`, `.any()`, `.find()`, `.position()`, `.sum()`, `.product()`, `.join()`, `.partition()`, `.min()`, `.max()`
- `iter(arr).map(f).filter(g).collect()` now works end-to-end
- New test: `tests/iterator_chaining.zen` with 20+ chaining scenarios
- Updated `examples/tour.zen` section 36

## Remaining (for future work)

2. Collection Literals
Set literals: {1, 2, 3}
Tuple syntax: (1, "hello") (currently tuples are [1, "hello"] arrays)
Map literals: {"key": value} (currently map_set(map_new(), "key", value))

3. zenc test improvements
Auto-discover tests (find all tests/*.zen)
--watch mode to re-run on file changes
--bench to run benchmarks (you already have benches/)

4. Standard Library Additions
std/datetime: now(), parse(), format()
Collection types: Set, Deque, SortedMap
