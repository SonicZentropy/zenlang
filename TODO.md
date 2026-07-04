# v0.4.0 Implementation Plan

- datetime: now(), format(date, fmt)
- Set — unordered unique collection (add, remove, contains, iter)
- Deque — double-ended queue (push_front, push_back, pop_front, pop_back)
- SortedMap — ordered key-value (insert, get, remove, iter, range)
Additional collection types useful for games:
- Queue/Stack — could be covered by Deque naming

1. Base64 — encode/decode for data serialization (no deps)
2. UUID v4 — unique IDs for entities/saves (no deps, just rand+clock)
3. Noise — perlin2d, simplex2d for procedural generation
4. Hashing — md5 / sha256 for asset caching, checksums
5. Color — rgba(r,g,b,a) / hsla(h,s,l,a) create colors as ints, lerp_color, hex parse
6. Extra math — deg_to_rad, rad_to_deg, clamp, smoothstep, lerp, remap
