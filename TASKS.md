# Task Tracker

All tasks must be recorded here before work begins.
Update status as work progresses.

**Statuses:** `TODO` | `IN PROGRESS` | `DONE` | `BLOCKED`

---

## Phase 0 — Foundation

| ID | Task | Assignee | Status | Notes |
|----|------|----------|--------|-------|
| 0.1 | Init Cargo workspace with crates: rpg-engine, rpg-mapgen, rpg-tiled, rpg-godot | — | DONE | |
| 0.2 | Implement `SeededRng`, `keccak256`, `derive_seed` in rpg-engine::rng | — | DONE | 21 тест, детерминированность подтверждена |
| 0.3 | Define map types: `TileKind`, `Tile`, `Chunk`, `GameMap` in rpg-engine::map | — | DONE | Vec<Tile> вместо Box<[Tile;1024]> из-за serde |
| 0.4 | Define `Error` enums via thiserror in all crates | — | DONE | |
| 0.5 | Set up tracing-subscriber in test harness (once_cell or similar) | — | DONE | OnceLock в test_utils.rs |
| 0.6 | Init Godot 4 project in `godot/` directory | — | DONE | project.godot + .gdextension + Lua scripts |

## Phase 1 — Map Generator

| ID | Task | Assignee | Status | Notes |
|----|------|----------|--------|-------|
| 1.1 | `LuaEngine` struct: init mlua runtime, load/call script | — | DONE | Встроен в каждый компонент |
| 1.2 | Expose `SeededRng` to Lua as mlua UserData | — | DONE | LuaRng(RefCell<SeededRng>) |
| 1.3 | Chunk generator: invoke Lua `generate_chunk` → `Chunk` | — | DONE | ChunkGenerator |
| 1.4 | Chunk stitcher: blend/smooth chunk boundaries | — | DONE | majority-vote по 4 соседям |
| 1.5 | Map assembler: generate N×M chunks → `GameMap` | — | DONE | MapAssembler + generate_best_of |
| 1.6 | `scripts/generators/default.lua` — basic terrain generation | — | DONE | |
| 1.7 | Lua evaluator: invoke `evaluate(map)` → f64 score | — | DONE | MapEvaluator |
| 1.8 | Lua validator: invoke `validate(map)` → (bool, msg) | — | DONE | MapValidator |
| 1.9 | `scripts/rules/evaluate.lua` — basic map scoring rules | — | DONE | |
| 1.10 | `scripts/rules/validate.lua` — basic map validity rules | — | DONE | |
| 1.11 | Проанализировать правила генерации, исправить `MAP_GENERATION_RULES.md` и добавить альтернативный генератор `scripts/generators/codex-variant.lua` | Codex | DONE | `codex-variant.lua` добавлен; правила синхронизированы с `tile.rs`, мостами/POI и инвариантом города из `03_city_rules.lua` |

## Phase 2 — Tiled Integration

| ID | Task | Assignee | Status | Notes |
|----|------|----------|--------|-------|
| 2.1 | TMX data model structs (Map, Layer, Tileset, Tile) | — | TODO | |
| 2.2 | TMX exporter: `GameMap` → `.tmx` XML | — | TODO | Isometric staggered format |
| 2.3 | TMX importer: `.tmx` XML → `GameMap` | — | TODO | |
| 2.4 | Isometric tileset descriptor (GID ↔ TileKind mapping) | — | TODO | |
| 2.5 | Integration test: generate → export TMX → import TMX → compare | — | TODO | |

## Phase 3 — Game Mechanics

| ID | Task | Assignee | Status | Notes |
|----|------|----------|--------|-------|
| 3.1 | `Hero` struct with stats: hp, atk, def, spd, mov | — | TODO | |
| 3.2 | Movement: compute reachable tiles given MOV budget | — | TODO | BFS/Dijkstra on tile graph |
| 3.3 | Auto-resolve combat: `resolve_combat(attacker, defender, rng)` | — | TODO | |
| 3.4 | `ScoreBoard`: track events, compute total score | — | TODO | |
| 3.5 | `GameState`: map + heroes + turn counter + score | — | TODO | |
| 3.6 | Turn manager: advance turn, reset MOV, trigger events | — | TODO | |

## Phase 4 — Godot Bridge

| ID | Task | Assignee | Status | Notes |
|----|------|----------|--------|-------|
| 4.1 | GDExtension scaffold: gdext setup, `.gdextension` file | — | TODO | |
| 4.2 | `MapNode`: render `GameMap` as Godot TileMapLayer | — | TODO | |
| 4.3 | `HeroNode`: render hero sprite, handle tile click → move | — | TODO | |
| 4.4 | `GameManager`: owns `GameState`, drives turn loop | — | TODO | |
| 4.5 | `CombatResolver`: trigger auto-resolve on enemy encounter | — | TODO | |
| 4.6 | `ScoreUI`: display current score as HUD label | — | TODO | |
| 4.7 | Isometric camera + tile hover highlight | — | TODO | |

## Phase 5 — Polish & Integration

| ID | Task | Assignee | Status | Notes |
|----|------|----------|--------|-------|
| 5.1 | Map generation on game start from seed input | — | TODO | |
| 5.2 | Hero placement on generated map start position | — | TODO | |
| 5.3 | Enemy spawning driven by Lua rules | — | TODO | |
| 5.4 | Win/loss conditions via score threshold | — | TODO | |
| 5.5 | Save/load `GameState` (serde + JSON or binary) | — | TODO | |

---

## Decisions Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-03-27 | Use Keccak256-based `SeededRng` ([u8;32] seed) | Deterministic, user-provided implementation |
| 2026-03-27 | Use Lua 5.4 via mlua for generation scripts | Simpler than TypeScript, well-supported in Rust |
| 2026-03-27 | Chunk size fixed at 32×32 tiles | Balances granularity and generation performance |
| 2026-03-27 | Default map is 3×3 chunks (96×96 tiles) | Extensible — assembler accepts any N×M |
| 2026-03-27 | Isometric staggered diamond layout | Matches Disciples 2 visual style |
| 2026-03-27 | `anyhow` forbidden, use `thiserror` per crate | Consistent structured error handling |
| 2026-03-27 | `tracing` for all logging | Structured, filterable, async-compatible |
| 2026-03-27 | `MAP_GENERATION_RULES.md` must match runtime truth from `Tiles` and Lua validators | Avoid drift between documentation, generator scripts, and validation invariants |
