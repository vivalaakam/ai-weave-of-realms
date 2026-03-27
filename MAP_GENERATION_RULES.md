# Map Generation Rules

Reference document for all map generation agents and validators.

---

## 1. Coordinate system

- A **chunk** is 32×32 tiles.
- Tile coordinates inside a chunk: `x ∈ [0, 31]`, `y ∈ [0, 31]`.
- Lua index: `i = y * 32 + x + 1` (1-based).
- The assembled **GameMap** is a flat `width × height` tile array (e.g. 96×96 for a 3×3 chunk map).
- The generator receives chunk column `cx` and row `cy` (0-based) so it can derive a unique per-chunk seed if needed.

---

## 2. Tile types

| Name             | Passable | Description                                      |
|------------------|----------|--------------------------------------------------|
| `meadow`         | yes      | Default open terrain                             |
| `forest`         | yes      | Forest cluster                                   |
| `mountain`       | no       | Mountain ridge / impassable highland             |
| `water`          | yes      | Lake / water body                                |
| `river`          | yes      | River channel                                    |
| `road`           | yes      | Constructed road                                 |
| `bridge`         | yes      | Passable crossing placed over a river            |
| `city`           | yes      | City interior tile                               |
| `city_entrance`  | yes      | City entry point                                 |
| `village`        | yes      | Minor settlement / point of interest             |
| `merchant`       | no       | Trade point of interest                          |
| `ruins`          | no       | Adventure point of interest                      |
| `gold`           | no       | Gold mine deposit                                |
| `resource`       | no       | Generic resource deposit                         |

Passability in this table must stay aligned with [`Tiles::is_passable`](crates/rpg-engine/src/map/tile.rs).

---

## 3. Generation pipeline

All registered Lua generators are applied to **every chunk in order**.
Each generator receives `(rng, cx, cy, tiles?)`:
- `rng` — `SeededRng` userdata (deterministic, chunk-unique).
- `cx`, `cy` — chunk column and row (0-based).
- `tiles` — tile table from the previous stage, or `nil` if this is the first stage.

The generator must return a `table[1024]` of tile name strings.

The sections below describe the **default** terrain rules implemented by
[`terrain.lua`](scripts/generators/terrain.lua). Alternative generators may use
different heuristics or stage ordering as long as they:

- return only valid tile identifiers from [`Tiles::as_str()`](crates/rpg-engine/src/map/tile.rs)
- remain deterministic for the same `(map_seed, cx, cy)`
- preserve validator invariants from `scripts/rules/`
- do not rely on global mutable Lua state

### Default stage order (`terrain.lua`)

1. **Base canvas** — fill all tiles with `"meadow"` (skipped when `tiles` arg is provided).
2. **Water** — organic lake blobs with arms.
3. **Rivers** — edge-to-edge meandering rivers.
4. **Forest** — circular forest clusters.
5. **Mountains** — ridge polylines.
6. **Roads** — mostly-straight through-roads.
7. **City** — one 3×3 city block per chunk.
8. **Resources** — 1 gold mine + 4–7 resource deposits.

---

## 4. Water rules

- **1–3 lakes** per chunk, centres kept in `[5, 26]` to avoid touching edges.
- Lake radius: 3–6 tiles.
- Organic blob: probability of painting decreases linearly to 30 % at the edge.
- **Arms**: 2–5 random arms of length `radius+1` to `radius+3`.
- **Protected** (water cannot overwrite): `city`, `city_entrance`, `mountain`, `river`, `road`.

---

## 5. River rules

- **1–2 rivers** per chunk.
- Each river starts on one edge and ends on a different edge.
- Entry/exit points are chosen from `[3, CHUNK_SIZE-4]` to avoid corners.
- Movement: 65 % direct toward exit, 35 % random meander.
- Rivers detour around `mountain` tiles (try perpendicular step).
- Occasional broadening to width-2 (25 % chance per step).
- **Protected** (river cannot overwrite): `city`, `city_entrance`, `mountain`, `water`, `road`.

---

## 6. Forest rules

- **3–7 clusters** per chunk.
- Cluster radius: 3–7 tiles.
- Only overwrites `meadow` tiles (forest never grows on water, mountain, river, road, or city).
- Probability decreases to 20 % at the cluster edge.

---

## 7. Mountain rules

- **1–3 ridges** per chunk, each built from 3–5 connected segments.
- Ridge half-width: 1–3 tiles.
- Step length between ridge points: 8–16 tiles, random angle.
- Only overwrites `meadow` and `forest`.
- **Protected** (mountains cannot overwrite): `city`, `city_entrance`, `water`, `river`.

---

## 8. Road rules

- **0–2 roads** per chunk (30 % chance of 0, 45 % chance of 1, 25 % chance of 2).
- Each road starts and ends on a different edge; entry points are confined to the centre third of each edge.
- Movement: 85 % direct toward exit, 15 % meander.
- Roads navigate around blockers by trying perpendicular alternatives.
- **Blocked** (road cannot overwrite or pass through): `city`, `city_entrance`, `water`, `mountain`, `river`.

---

## 9. City rules

- **Current generators and validators assume at most one city per chunk.**
- The city block is 3 tiles wide × 3 tiles tall.
- Required layout (`bx`, `by` = top-left corner):
  - Rows `by`, `by+1` — all 3 columns: `city` (6 tiles).
  - Row `by+2` — `city_entrance` at `bx`, then `city` at `bx+1` and `bx+2`.
- The top-left corner is restricted to `bx ∈ [1, 28]`, `by ∈ [1, 26]` (1-tile margin on all sides).
- **Placement check**: the 3×3 block plus a 1-tile margin (5×5 scan) must be completely free of `water`, `river`, and `mountain`. Up to 20 candidate positions are tried; if none pass, the city is skipped for this chunk.
- After placement, the 5 tiles immediately adjacent to `city_entrance` (but outside the 3×3 block) are forced to `meadow` or `road` so the entrance is always accessible.
- City is placed **before** resources so the resource stage respects the exclusion zone.

Alternative generators should keep this layout unless
`scripts/rules/03_city_rules.lua` is updated in the same change.

### Entrance adjacency requirement

The following tiles must not be `water`, `river`, or `mountain`:
- `(bx-1, by+2)` — left of entrance
- `(bx+3, by+2)` — right of entrance
- `(bx, by+3)`, `(bx+1, by+3)`, `(bx+2, by+3)` — below entrance

---

## 10. Resource rules

- **1 gold mine** + **4–7 generic resource deposits** per chunk.
- A candidate tile is valid when **all 9 tiles in the 3×3 area** centred on it are `meadow`. This guarantees resources are surrounded by accessible land.
- Resources must not be placed within **2 tiles** of any settlement tile (`city`, `city_entrance`, `village`).
- Resources must be at least **4 tiles apart** from each other (Euclidean distance).
- Up to 120 placement attempts per resource; silently skipped if no valid position is found.
- Resources can **never** appear on `water`, `river`, `mountain`, `road`, `forest`, or any city tile.

## 10.1 Optional POI / bridge rules

- `bridge` is intended to replace a `river` tile when a road or route crosses it.
- `village`, `merchant`, and `ruins` are optional point-of-interest tiles; they
  should only be placed on passable land and should not overwrite `water`,
  `river`, `mountain`, `city`, or `city_entrance`.
- Settlements (`city`, `city_entrance`, `village`, `merchant`) should keep at
  least a small accessible apron of passable tiles around them.

---

## 11. Map-level validation rules (`scripts/rules/`)

Loaded in sorted filename order; each file must return `function(map) → bool, string?`.

| File                       | Rule                                              |
|----------------------------|---------------------------------------------------|
| `01_passable_terrain.lua`  | At least 50 % of tiles must be passable           |
| `02_impassable_limit.lua`  | At most 40 % of tiles may be impassable           |
| `03_city_rules.lua`        | Every `city_entrance` must be adjacent to ≥1 passable non-city tile |

---

## 12. Priority / override table

Higher rows override lower rows (a tile type earlier in the list wins when two stages conflict).

| Priority | Tile types                        |
|----------|-----------------------------------|
| 1 (top)  | `city`, `city_entrance`           |
| 2        | `mountain`                        |
| 3        | `water`, `river`                  |
| 4        | `bridge`, `road`                  |
| 5        | `village`, `merchant`, `ruins`    |
| 6        | `gold`, `resource`                |
| 7        | `forest`                          |
| 8 (base) | `meadow`                          |

---

## 13. RNG contract

- Each chunk receives a **unique, deterministic seed** derived from the map seed + chunk coordinates via `derive_seed(map_seed, context)`.
- The same map seed + chunk position always produces an identical chunk.
- Generators must not rely on global mutable state; all randomness must go through the provided `rng` userdata.
