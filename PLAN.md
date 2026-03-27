# AI RPG v2 — Project Plan

**Genre:** Strategy RPG (Disciples 2 style)
**Engine:** Godot 4.x (latest)
**Language:** Rust (gdext) + Lua 5.4 (mlua) for scripting
**Platform:** Desktop (Windows / Linux / macOS)
**Mode:** Single-player

---

## Architecture Overview

```
┌─────────────────────────────────────────────────┐
│                  Godot 4 (GDExtension)           │
│  ┌──────────────────────────────────────────┐   │
│  │            rpg-godot (bridge)            │   │
│  │  GodotNode wrappers · Signals · Scenes   │   │
│  └────────────────┬─────────────────────────┘   │
└───────────────────┼─────────────────────────────┘
                    │ calls
    ┌───────────────┼───────────────────────┐
    │               │                       │
    ▼               ▼                       ▼
┌──────────┐  ┌───────────┐         ┌────────────┐
│rpg-engine│  │rpg-mapgen │         │ rpg-tiled  │
│          │  │           │         │            │
│ Hero     │  │ Generator │         │ TMX import │
│ Combat   │  │ Chunks    │         │ TMX export │
│ Score    │  │ Stitcher  │         └────────────┘
│ Map types│  │ Lua loader│
│ PRNG     │  │ Evaluator │
└──────────┘  │ Validator │
              └─────┬─────┘
                    │ loads
              ┌─────▼──────────┐
              │ scripts/       │
              │ generators/*.lua│
              │ rules/*.lua    │
              └────────────────┘
```

### Key Principles
- `rpg-engine` has **zero** Godot dependencies — pure game logic
- `rpg-godot` is the only crate that depends on `gdext`
- All scripting (generation, evaluation, validation) is in **Lua 5.4**
- Determinism guaranteed by **Keccak256-based SeededRng** (`[u8; 32]` seed)
- Errors via `thiserror`, logging via `tracing`

---

## Map Structure

- **Tile:** smallest unit, isometric diamond layout
- **Chunk:** 32×32 tiles, generated independently
- **Map:** grid of chunks (default 3×3 = 96×96 tiles, extensible)
- Each chunk is generated with a seed derived from `map_seed + chunk_coords`
- A **stitcher** smooths boundaries between adjacent chunks
- Map format: TMX (Tiled Map Editor) for storage/exchange

### Chunk Seed Derivation
```
chunk_seed = derive_seed(&map_seed, &[chunk_x as u8, chunk_y as u8])
```

---

## PRNG

Based on Keccak256 hash chain. Implementation lives in `rpg-engine::rng`.

```rust
// Seed from string
let rng = SeededRng::new("my-seed-phrase");

// Derive child RNG for chunk (0, 2)
let chunk_rng = SeededRng::from_bytes(derive_seed(&map_seed, b"chunk_0_2"));
```

---

## Phases

### Phase 0 — Foundation
**Goal:** Cargo workspace, shared types, PRNG, CI scaffold

| # | Task | Crate | Status |
|---|------|-------|--------|
| 0.1 | Init Cargo workspace with all 4 crates | root | TODO |
| 0.2 | Implement `SeededRng` + `keccak256` + `derive_seed` | rpg-engine | TODO |
| 0.3 | Define shared map types: `TileKind`, `Tile`, `Chunk`, `GameMap` | rpg-engine | TODO |
| 0.4 | Define shared error types per crate | all | TODO |
| 0.5 | Set up `tracing` initialization in test harness | all | TODO |
| 0.6 | Init Godot 4 project structure | godot/ | TODO |

---

### Phase 1 — Map Generator
**Goal:** Generate a chunked isometric map via Lua scripts

| # | Task | Crate | Status |
|---|------|-------|--------|
| 1.1 | Lua runtime wrapper (`LuaEngine` struct, load/call script) | rpg-mapgen | TODO |
| 1.2 | Expose `SeededRng` to Lua as userdata | rpg-mapgen | TODO |
| 1.3 | Chunk generator: call Lua script → produce `Chunk` | rpg-mapgen | TODO |
| 1.4 | Chunk stitcher: smooth boundaries between adjacent chunks | rpg-mapgen | TODO |
| 1.5 | Map assembler: generate N×M chunks → assemble `GameMap` | rpg-mapgen | TODO |
| 1.6 | Write `scripts/generators/default.lua` (basic terrain) | scripts/ | TODO |
| 1.7 | Lua evaluator: load `rules/evaluate.lua`, score a map | rpg-mapgen | TODO |
| 1.8 | Lua validator: load `rules/validate.lua`, validate a map | rpg-mapgen | TODO |
| 1.9 | Write `scripts/rules/evaluate.lua` (basic scoring rules) | scripts/ | TODO |
| 1.10 | Write `scripts/rules/validate.lua` (basic validity rules) | scripts/ | TODO |

---

### Phase 2 — Tiled Integration
**Goal:** Export generated map to TMX, import TMX back to `GameMap`

| # | Task | Crate | Status |
|---|------|-------|--------|
| 2.1 | TMX data model (structs mirroring TMX XML schema) | rpg-tiled | TODO |
| 2.2 | TMX exporter: `GameMap` → TMX XML file | rpg-tiled | TODO |
| 2.3 | TMX importer: TMX XML file → `GameMap` | rpg-tiled | TODO |
| 2.4 | Isometric tileset descriptor (tile GIDs, terrain mapping) | rpg-tiled | TODO |
| 2.5 | Integration test: generate → export → import → compare | rpg-tiled | TODO |

---

### Phase 3 — Game Mechanics
**Goal:** Hero, movement, auto-resolve combat, score system

| # | Task | Crate | Status |
|---|------|-------|--------|
| 3.1 | `Hero` struct with base stats (HP, ATK, DEF, SPD, MOV) | rpg-engine | TODO |
| 3.2 | Movement system: reachable tiles given MOV points | rpg-engine | TODO |
| 3.3 | Auto-resolve combat: stats × RNG → outcome | rpg-engine | TODO |
| 3.4 | Score system: `ScoreBoard`, events that award points | rpg-engine | TODO |
| 3.5 | Game state: `GameState` (map, heroes, turn, score) | rpg-engine | TODO |
| 3.6 | Turn manager: advance turn, reset MOV, trigger events | rpg-engine | TODO |

---

### Phase 4 — Godot Bridge
**Goal:** Connect engine logic to Godot scenes

| # | Task | Crate | Status |
|---|------|-------|--------|
| 4.1 | GDExtension scaffold (gdext, `.gdextension` file) | rpg-godot | TODO |
| 4.2 | `MapNode`: render `GameMap` as isometric TileMap | rpg-godot | TODO |
| 4.3 | `HeroNode`: render hero, handle input, emit move signal | rpg-godot | TODO |
| 4.4 | `GameManager`: owns `GameState`, drives turn loop | rpg-godot | TODO |
| 4.5 | `CombatResolver`: trigger auto-resolve, show result | rpg-godot | TODO |
| 4.6 | `ScoreUI`: display current score | rpg-godot | TODO |
| 4.7 | Basic isometric camera + tile highlight | godot/ | TODO |

---

### Phase 5 — Polish & Integration
**Goal:** Playable vertical slice

| # | Task | Crate | Status |
|---|------|-------|--------|
| 5.1 | Map generation on game start (seed from UI input) | all | TODO |
| 5.2 | Hero placement on generated map | all | TODO |
| 5.3 | Enemy spawning (from Lua rules) | all | TODO |
| 5.4 | Win/loss conditions via score | rpg-engine | TODO |
| 5.5 | Save/load game state (serialize `GameState`) | rpg-engine | TODO |

---

## Dependency Manifest

```toml
# Shared across crates
thiserror    = "2"
tracing      = "0.1"

# rpg-engine
sha3         = "0.10"

# rpg-mapgen
mlua         = { version = "0.10", features = ["lua54", "vendored"] }

# rpg-tiled
quick-xml    = "0.37"
serde        = { version = "1", features = ["derive"] }

# rpg-godot
godot        = { git = "https://github.com/godot-rust/gdext", branch = "master" }

# dev / testing
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

---

## Lua Script Contract

### Generator script (`scripts/generators/*.lua`)
```lua
-- Called once per chunk
-- @param rng  SeededRng userdata (methods: next_f64, random_range_u32, random_bool)
-- @param x    chunk X index
-- @param y    chunk Y index
-- @returns    table[32*32] of tile kind strings
function generate_chunk(rng, x, y)
  local tiles = {}
  for i = 1, 32*32 do
    tiles[i] = "grass"
  end
  return tiles
end
```

### Evaluator script (`scripts/rules/evaluate.lua`)
```lua
-- @param map  table with map metadata and chunk data
-- @returns    number score (higher = better)
function evaluate(map)
  return 0.0
end
```

### Validator script (`scripts/rules/validate.lua`)
```lua
-- @param map  table with map metadata and chunk data
-- @returns    bool, string|nil  (valid, error_message)
function validate(map)
  return true, nil
end
```

---

## Tile Kinds (initial set)

| ID | Name | Passable | Notes |
|----|------|----------|-------|
| 0 | `grass` | yes | default terrain |
| 1 | `water` | no | blocks movement |
| 2 | `forest` | yes | +1 MOV cost |
| 3 | `mountain` | no | blocks movement |
| 4 | `road` | yes | -1 MOV cost |
| 5 | `ruins` | yes | point of interest |

---

## Hero Stats (initial set)

| Stat | Description |
|------|-------------|
| `hp` | Hit points |
| `atk` | Attack power |
| `def` | Defense |
| `spd` | Speed (combat priority) |
| `mov` | Movement points per turn |
