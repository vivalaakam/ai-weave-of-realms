---
name: rpg-tile-render
description: >
  Implement or fix tile rendering in the ai-rpg-v2 project (crates/rpg-embedded/src/render.rs).
  Use this skill whenever the user asks to: add a new terrain type (mountains, roads, forests,
  rivers, walls), fix visual tile artifacts, implement autotiling / border compositing,
  add sprite overlays, work with the tile atlas (tiles.bin / pack-tiles), or debug
  water/terrain rendering. Trigger even if the user just mentions "тайлы", "отрисовка",
  "terrain sprites", or names a terrain type in the context of how it looks on the map.
---

# RPG Tile Rendering Skill

You are working in the **ai-rpg-v2** project — a Disciples-2-style RPG built with Rust.

Key file: `crates/rpg-embedded/src/render.rs`  
Reference doc: `docs/tile-rendering.md` — **read this first** for the full system overview.

---

## Project constraints

- Crate is `no_std + alloc`. No `std::collections::HashMap` — use `alloc::collections::BTreeMap` or a plain `Vec`.
- No `println!` / `eprintln!`. No `anyhow`. No GDScript.
- Draw target is `embedded-graphics`. All drawing goes through `DrawTarget`.
- Tile atlas is `assets/tiles.bin` (compile-time `include_bytes!`). Regenerate with:
  ```sh
  cargo run -p rpg-tools --bin pack-tiles -- --tile-width 16 --tile-height 16
  ```
  PNGs are packed in **alphabetical order**; tile indices shift when new PNGs are added.

---

## Atlas layout (current)

| File              | Start index | Tile count |
|-------------------|-------------|------------|
| `1_main.png`      | 0           | 1078       |
| `2_water.png`     | 1078        | 5          |
| `3_mountains.png` | 1083        | 5          |

When you add a new PNG, sort alphabetically to find its start index.

---

## Water autotile — the reference implementation

`assets/2_water.png` has 5 tiles (all 16×16):

```
index 0 WATER_FULL         — all 256 pixels set
index 1 WATER_SHORE_LL     — lower-left half of W edge missing
index 2 WATER_SHORE_UL     — upper-left half of W edge missing
index 3 WATER_CORNER_OUTER — diagonal cut at upper-left (outer corner)
index 4 WATER_CORNER_INNER — 3-pixel notch at upper-left (inner corner)
```

Tiles are designed for the **W edge / upper-left corner**. Rotation gives the other 3 directions.

### Neighbor encoding

```
bit 7  NW | bit 0  N  | bit 1  NE
bit 6   W | center    | bit 2   E
bit 5  SW | bit 4  S  | bit 3  SE
```

1 = neighbor is same terrain (water), 0 = different terrain or OOB.

### Composite mask formula

Start with `WATER_FULL`. AND-in fragment pieces:

**Half-edge pieces** (skipped automatically when adjacent outer corner is active):

```
W land, N water  →  &ul        (W upper half)
W land, S water  →  &ll        (W lower half)
N land, W water  →  &ll90      (N left half)
N land, E water  →  &ul90      (N right half)
E land, N water  →  &ll180     (E upper half)   ← ll180, NOT ul180
E land, S water  →  &ul180     (E lower half)   ← ul180, NOT ll180
S land, W water  →  &ul270     (S left half)
S land, E water  →  &ll270     (S right half)
```

> **E-edge asymmetry:** after 180° rotation the lower piece becomes upper. `ll180` = E upper; `ul180` = E lower. Getting
> this backwards causes visual "bumps" on the right side of water tiles.

**Outer corners** (two adjacent land edges):

```
W && N  →  &co      (NW, upper-left)
N && E  →  &co90    (NE, upper-right)
E && S  →  &co180   (SE, lower-right)
S && W  →  &co270   (SW, lower-left)
```

**Inner corners** (diagonal land, both orthogonal neighbors are water):

```
!W && !N && NW land  →  &ci
!N && !E && NE land  →  &ci90
!E && !S && SE land  →  &ci180
!S && !W && SW land  →  &ci270
```

### Rotation function

```rust
fn rotate_cw90(m: &WaterMask) -> WaterMask {
    // (x, y) → (N-1-y, x), N = TILESET_TILE_W = 16
    let n = TILESET_TILE_W;
    let mut out = [0u8; TILESET_MASK_BYTES];
    for y in 0..n {
        for x in 0..n {
            if mask_pixel_arr(m, x, y) {
                let bit = x * n + (n - 1 - y);
                out[bit / 8] |= 1 << (7 - bit % 8);
            }
        }
    }
    out
}
```

Rotation direction reference: left edge → top (CW90), top → right (CW90), right → bottom (CW90).

---

## Adding a new autotile terrain

### Step 1 — design 5 source tiles

Create `assets/N_<name>.png` (replace N to control atlas position alphabetically).  
All 5 tiles are 16×16. Design only the **W / upper-left** orientation:

| Tile           | Content                                                        |
|----------------|----------------------------------------------------------------|
| 0 FULL         | All pixels set (or the full base pattern for this terrain)     |
| 1 SHORE_LL     | Lower half of W edge missing (terrain fades toward lower-left) |
| 2 SHORE_UL     | Upper half of W edge missing (terrain fades toward upper-left) |
| 3 CORNER_OUTER | Diagonal cut at upper-left — where two land edges meet         |
| 4 CORNER_INNER | 3-pixel notch at upper-left — diagonal land neighbor only      |

Run `scripts/gen_water_tiles.py` as reference for how the water tiles were built.

### Step 2 — regenerate atlas and add constants

```sh
cargo run -p rpg-tools --bin pack-tiles -- --tile-width 16 --tile-height 16
```

In `render.rs`:

```rust
const MY_BASE: usize = <start_index>;          // determined by alphabetical PNG order
const MY_FULL: usize         = MY_BASE;
const MY_SHORE_LL: usize     = MY_BASE + 1;
const MY_SHORE_UL: usize     = MY_BASE + 2;
const MY_CORNER_OUTER: usize = MY_BASE + 3;
const MY_CORNER_INNER: usize = MY_BASE + 4;

type MyMask = [u8; TILESET_MASK_BYTES];
```

### Step 3 — add cache field to RenderCache

```rust
pub struct RenderCache {
    map_view: Option<MapViewCache>,
    water_masks: Vec<Option<WaterMask>>,
    my_masks: Vec<Option<MyMask>>,   // ← add
}
```

Add a `cached_my_mask(my_masks: &mut Vec<Option<MyMask>>, bits: u8) -> MyMask` function
(identical to `cached_water_mask` but using the new constants).

### Step 4 — implement compute function

Copy `compute_water_composite` verbatim. Change only:

- The 5 source tile constants (`MY_FULL`, etc.)
- The neighbor predicate if needed (e.g., river matches river OR lake)

### Step 5 — wire into draw_cell

```rust
// In draw_cell, add after forest/meadow branches:
if matches!(tile.kind, Tiles::MyTerrain) {
    let bits = my_neighbor_bits(map, coord);   // same structure as water_neighbor_bits
    let mask = cached_my_mask(&mut water_masks_ref, bits);  // pass &mut RenderCache.my_masks
    draw_grass_sprite(display, top_left, &mask, (theme.tile_sprite_color)(tile.kind));
}
```

Pass `&mut render_cache.my_masks` through `draw_cell` the same way `water_masks` is threaded.
Use **separate** mutable slice references to avoid borrow conflicts with `render_cache.map_view`.

---

## Simple sprite overlay (no compositing)

For terrain that doesn't need edge transitions (decorative forests, ruins, etc.):

```rust
const FOREST_INDICES: [usize; 6] = [49, 50, 51, 52, 53, 54];

if matches!(tile.kind, Tiles::Forest) {
    let idx = (coord.x as usize * 31 + coord.y as usize * 17) % FOREST_INDICES.len();
    draw_grass_sprite(
        display, top_left,
        tile_atlas_mask(FOREST_INDICES[idx]),
        (theme.tile_sprite_color)(tile.kind),
    );
}
```

The `x * 31 + y * 17` hash gives stable pseudo-random variation without a PRNG.

---

## Debugging checklist

- **Bumps on E edge**: `ll180` and `ul180` are swapped. `ll180` = E upper, `ul180` = E lower.
- **Wrong corner shape**: verify `CORNER_OUTER` tile was drawn for the NW (W+N land) case; other corners are
  90°/180°/270° rotations.
- **Tile index off by one**: check alphabetical sort order of PNGs; rerun pack-tiles and print first few bytes of
  tiles.bin to verify count.
- **Cache stale after map scroll**: `reset_cache` clears `map_view` but NOT `water_masks` — terrain mask cache is
  intentionally persistent (atlas is compile-time). If atlas changes at runtime (impossible in current design), clear
  the Vec.
- **Borrow conflict in draw loop**: `cache` borrows `render_cache.map_view`; pass `&mut render_cache.my_masks` as a
  separate parameter to avoid double-borrow.

---

## File locations

| Path                                     | Purpose                                |
|------------------------------------------|----------------------------------------|
| `crates/rpg-embedded/src/render.rs`      | All rendering logic                    |
| `crates/rpg-tools/src/bin/pack_tiles.rs` | Atlas packer                           |
| `scripts/gen_water_tiles.py`             | Water tile generator (reference)       |
| `assets/2_water.png`                     | Water source tiles (5 tiles, 80×16 px) |
| `assets/tiles.bin`                       | Packed atlas (binary, committed)       |
| `docs/tile-rendering.md`                 | Full system documentation              |
