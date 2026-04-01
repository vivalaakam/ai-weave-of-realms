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
| 1.11 | Проанализировать правила генерации, исправить `MAP_GENERATION_RULES.md` и добавить альтернативный генератор `scripts/generators/codex-variant.lua` | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 1.12 | Расширить `mapgen`: сохранять каждую генерацию в timestamp-директорию, экспортировать PNG + TMX и поддержать `--open` для итогового `.tmx` | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 1.13 | Ввести правила касания чанков по краям: только позиции `index % 3 = 1` для дорог/рек и согласованные полосы для леса/гор/воды; обновить генераторы и `MAP_GENERATION_RULES.md` | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |

## Phase 2 — Tiled Integration

| ID | Task | Assignee | Status | Notes |
|----|------|----------|--------|-------|
| 2.1 | TMX data model structs (Map, Layer, Tileset, Tile) | — | DONE | Встроено в exporter/importer, seed как custom property |
| 2.2 | TMX exporter: `GameMap` → `.tmx` XML | — | DONE | `rpg-tiled::exporter`, `mapgen` использует библиотеку |
| 2.3 | TMX importer: `.tmx` XML → `GameMap` | — | DONE | `rpg-tiled::importer`, quick-xml event parser |
| 2.4 | Isometric tileset descriptor (GID ↔ TileKind mapping) | — | DONE | `Tiles::to_gid` / `from_gid` в rpg-engine |
| 2.5 | Integration test: generate → export TMX → import TMX → compare | — | DONE | round_trip_* тесты в importer.rs |

## Phase 3 — Game Mechanics

| ID | Task | Assignee | Status | Notes |
|----|------|----------|--------|-------|
| 3.1 | `Hero` struct with stats: hp, atk, def, spd, mov | — | DONE | hero.rs, Faction enum, serde |
| 3.2 | Movement: compute reachable tiles given MOV budget | — | DONE | movement.rs, Dijkstra 4-way, road cost=0 |
| 3.3 | Auto-resolve combat: `resolve_combat(attacker, defender, rng)` | — | DONE | combat.rs, initiative by spd, damage formula |
| 3.4 | `ScoreBoard`: track events, compute total score | — | DONE | score.rs, 5 event types |
| 3.5 | `GameState`: map + heroes + turn counter + score | — | DONE | game_state.rs, move_hero, attack_hero |
| 3.6 | Turn manager: advance turn, reset MOV, trigger events | — | DONE | advance_turn в GameState, TurnEvent enum |

## Phase 4 — Godot Bridge

| ID | Task | Assignee | Status | Notes |
|----|------|----------|--------|-------|
| 4.1 | GDExtension scaffold: gdext setup, `.gdextension` file | — | DONE | rpg-godot crate, RpgGodotExtension entry point |
| 4.2 | `MapNode`: render `GameMap` as Godot TileMapLayer | — | DONE | map_node.rs, populate_tilemap, tilemap_populated signal |
| 4.3 | `HeroNode`: render hero sprite, handle tile click → move | — | DONE | hero_node.rs, move_requested/selected signals |
| 4.4 | `GameManager`: owns `GameState`, drives turn loop | — | DONE | game_manager.rs, все сигналы и функции |
| 4.5 | `CombatResolver`: trigger auto-resolve on enemy encounter | — | DONE | встроен в GameManager.move_hero (combat_resolved сигнал) |
| 4.6 | `ScoreUI`: display current score as HUD label | — | DONE | score_ui.rs, format var, on_score_changed |
| 4.7 | Isometric camera + tile hover highlight | — | DONE | camera_controller.gd, tile_highlight.gd, main.tscn |
| 4.8 | Перевести визуализацию Godot-карты на рабочую изометрию и подготовить изометрические тайловые ассеты на основе цветовой привязки `TileKind` | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 4.9 | Восстановить изометрический tileset после `just clean` и исключить его удаление из workflow очистки | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 4.10 | Исправить падение Godot при создании TileSet после обновления изометрического атласа | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 4.11 | Исправить потерю регистрации GDExtension-классов в Godot на macOS | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 4.12 | Убрать зависимость runtime-рендера карты от `.godot/imported/*.ctex` для tileset | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 4.13 | Диагностировать отсутствие видимой карты после восстановления tileset runtime-загрузки | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 4.14 | Добавить keyboard zoom на `-` / `=` и корректное растяжение сцены при fullscreen | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 4.10 | Исправить runtime-настройку `TileSet` в Godot, чтобы `TileMapLayer` использовал изометрический diamond-layout, а не прямоугольную сетку | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 4.15 | Исправить zoom и fullscreen-resize: до `1920×1080` окно должно показывать больше карты без глобального stretch, выше порога — масштабировать содержимое | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |

## Phase 5 — Polish & Integration

| ID | Task | Assignee | Status | Notes |
|----|------|----------|--------|-------|
| 5.1 | Map generation on game start from seed input | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 5.2 | Hero placement on generated map start position | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 5.3 | Enemy spawning driven by Lua rules | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 5.4 | Win/loss conditions via score threshold | — | TODO | |
| 5.5 | Save/load `GameState` (serde + JSON or binary) | — | TODO | |
| 5.6 | Добавить циклическое переключение героев по `Tab` и ограничить камеру по краям карты | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 5.7 | Добавить debug-панель камеры: seed, позиция курсора и ручной ввод центральной клетки | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 5.8 | Временно отключить camera clamp для отладки системы координат | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 5.9 | Синхронизировать координаты ввода/камеры с реальным `TileMapLayer` | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |
| 5.10 | Добавить в debug-панель сброс zoom и показ текущего значения | Codex | DONE | `spawn_enemies.lua` + `EnemySpawner`, интеграция в `GameManager` и `MainScene` |

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
| 2026-03-27 | `mapgen` exports both PNG and TMX into a per-run timestamp directory | Keeps generation artefacts grouped and lets TMX reference the shared root tileset |
| 2026-03-27 | Chunk edges use a 3-step connection grid (`pos % 3 == 1`) for roads/rivers and anchored continuous segments for forest/mountain/water | Ensures deterministic chunk-to-chunk connectivity and prevents ragged seam contacts |
| 2026-03-31 | Isometric tileset atlas is generated from `rpg_engine::map::tile::Tiles` instead of being maintained manually | Keeps Godot rendering, TMX export, tile count, and color semantics in sync from one source of truth |

## Phase 6 — Build Version Tracking

| ID | Task | Assignee | Status | Notes |
|----|------|----------|--------|-------|
| 6.1 | Add build script to generate version info from git | Codex | DONE | `crates/rpg-godot/build.rs` generates BUILD_GIT_HASH, BUILD_NUMBER, BUILD_PROFILE |
| 6.2 | Add `build_info` module for runtime version access | Codex | DONE | `crates/rpg-godot/src/build_info.rs` exposes `version_string()` and `build_info()` |
| 6.3 | Display version in UI and console on startup | Codex | DONE | Added `VersionLabel` in main.tscn, `update_version_label()` in MainScene |
| 6.4 | WASD camera controls + arrow keys hero movement | Codex | DONE | Camera uses WASD, active hero moves with arrow keys |
| 6.5 | Highlight selected hero in UI list | Codex | DONE | Yellow modulate on selected hero button |
| 6.6 | Fix borrow conflict on hero movement via arrow keys | Codex | DONE | Collect data before mutation, drop borrow, then mutate |
| 6.7 | Добавить поддержку Sony gamepad: Cross/Circle/R1/L1 + стики для героя и камеры | Codex | DONE | Cross=confirm, Circle=cancel, R1=next hero, L1=end turn dialog, LS=hero movement, RS=camera pan |
| 6.8 | Исправить confirm/cancel в end-turn диалоге на Sony gamepad (работа через polling и авто device id) | Codex | DONE | Убрана зависимость от fixed `device=0`; confirm/cancel читаются через polling + rising-edge по первому подключенному gamepad |
| 6.9 | Переназначить gamepad: D-pad двигает героя, левый стик двигает активный курсор клетки с подсветкой | Codex | DONE | D-pad=hero step movement, LS=grid cursor movement with forced active highlight in `TileHighlight` |

---

**Latest Change (2026-04-01)**

- Added `build.rs` to generate compile-time version info from git
- Created `build_info` module with `GIT_HASH`, `BUILD_NUMBER`, `BUILD_PROFILE`, `GIT_TIMESTAMP`
- Added version display in UI (bottom of right panel) and console logging on startup
- Changed camera controls from arrow keys to WASD
- Added arrow keys movement for active hero (with borrow conflict fix)
- Added highlight for selected hero in UI list (yellow modulate)

## Phase 8 — City Interaction & Hero Hiring

| ID | Task | Assignee | Status | Notes |
|----|------|----------|--------|-------|
| 8.1 | Показывать диалог найма героя при клике/нажатии X на городе без героя игрока | Copilot | DONE | Мышь + gamepad X; `is_city_tile()` и `get_next_hero_id()` в GameManager; `_create_hire_hero_dialog` в MainScene |
| 8.2 | Три команды Red/Blue/Enemy, разные города, цветные маркеры, владение городами | Copilot | DONE | `Team::color()`, `Team::red/blue/enemies()`, `GameState::city_owners`, `find_city_entrance_spawns()`, HeroNode modulate по team_name, найм только в своих городах |

---

**Latest Change (2026-04-01)**

- Added `is_city_tile(x, y)` and `get_next_hero_id()` to `GameManager`
- Added `hire_hero_dialog` and `hire_target_tile` fields to `MainScene`
- Mouse click on City/CityEntrance tile (no player hero present) → shows hire dialog
- Gamepad X button with cursor on City/CityEntrance tile (no player hero present) → shows hire dialog
- Confirming dialog spawns a new player-controlled hero at the city tile
- Gamepad Cross/Circle confirms/cancels the hire dialog (same pattern as end-turn dialog)
- Input blocked for all dialogs via `is_any_dialog_visible()`

| ID | Task | Assignee | Status | Notes |
|----|------|----------|--------|-------|
| 7.1 | Replace coordinate-based `move_hero` with direction-based single-step movement | — | DONE | Added `Direction` enum (N/E/S/W) in `map/game_map.rs`; `move_hero` now takes `Direction`, checks passability, occupancy, budget; errors: `ImpassableTile`, `NoMovementPoints`, `OutOfBounds` |

---

**Latest Change (2026-04-01)**

- Added `Direction` enum (North/East/South/West) with `apply()` method to `rpg-engine::map::game_map`
- Added `ImpassableTile` and `NoMovementPoints` error variants to `rpg-engine::error`
- `GameState::move_hero` now accepts `Direction` instead of `MapCoord`; performs single-step adjacency move with passability and occupancy checks; no longer uses Dijkstra pathfinding
- Updated Godot bridge `GameManager::move_hero(hero_id, direction: i64)` (0=N, 1=E, 2=S, 3=W)
- Updated `HeroNode::move_requested` signal and `request_move` to pass direction int
- Updated keyboard handler (arrow keys → direction) and mouse-click handler (adjacent tile → direction)
- Added Sony gamepad controls in Godot bridge: Cross confirm, Circle cancel, R1 next hero, L1 end-turn dialog, left stick hero movement (deadzone + repeat), right stick camera pan
- Fixed Sony confirm/cancel reliability in end-turn dialog: use connected-gamepad auto-detect and per-frame button polling with edge detection
- Remapped controls: D-pad now moves selected hero; left stick now moves active grid cursor; `TileHighlight` supports forced active tile with stronger color to indicate gamepad focus
