# Bevy Migration Plan: crates/game → bevy-console

## Goal

Rebuild the entire `crates/game` logic set (App, Splash, List, Random Map, TeamSetup, MapView, SaveOverlay, InfoOverlay, TurnOverlay, Session, Input, IO), replacing `embedded-graphics` + manual rendering with Bevy ECS + UI. Preserve all existing game behaviour (screen state machines, input → outcome → state transitions, hero movement, end-turn, save/load).

## Architecture

Use **Bevy states** (`States`) for top-level flow, not a hand-rolled state machine in resources. Each screen is a Bevy state (`OnEnter` spawns UI / meshes, `Update` runs input + business logic, `OnExit` despawns screen entities). Use **Bevy UI** for all menus and overlays and **2D sprites/meshes** for the tile map. Keep the existing `engine` and `mapgen` crates as-is; reuse them inside Bevy systems via `Resource<GameSession>` or event channels. The crate `game` is split into game-logic-only modules (input events, session, map_view, app_host logic) that remain in place, while all embedded-graphics-rendering code lives only in the old frontends. The new `bevy-console` crate depends on `engine` and the game-logic modules directly, not on `game` as a whole (to avoid pulling `embedded-graphics`).

## Tech Stack

- Bevy 0.19 (match repo `~/work/bevy`).
- `bevy::state::States` for screen flow.
- `bevy::ui` (`Node`, `Text`, `TextColor`, `BackgroundColor`, `Button`) for menus and overlays.
- `Sprite` / `Mesh2D` for tile rendering (tiled grid).
- `Camera2d` + `CameraOrder` + `Camera2d::default().layer` for map vs. UI layering.

## Assumptions / Constraints

- `crate engine` and `crate mapgen` are unchanged — only the rendering + input glue is replaced.
- No `embedded-graphics` in `bevy-console`.
- Keep `game` crate intact for existing `sdl2` / `sixel` / `tdeck` frontends. Extract pure logic from `game` where needed (e.g. `input.rs`, `map_view.rs`, `session.rs`, `io.rs`, `app_host.rs`, `random_map.rs`, `team_setup.rs`, `types.rs` are already logic-only and reusable).
- Use Bevy's built-in `Interaction` for buttons, no custom hit-testing.
- Tile atlas (`assets/tiles.bin`) is used as a sprite sheet or texture atlas in Bevy.
- Fullscreen default, `--windowed` flag via CLI (`clap`).

---

## Task Breakdown

### Task 1: Bootstrap `bevy-console` crate

**Objective:** Make `bevy-console` compile with Bevy and a blank window.

**Files:**
- Modify: `bevy-console/Cargo.toml`
- Create: `bevy-console/src/main.rs`

**Steps:**

1. Add dependencies to `bevy-console/Cargo.toml`:
   ```toml
   [dependencies]
   bevy = { path = "../bevy" }
   clap = { workspace = true }
   engine = { path = "../crates/engine" }
   mapgen = { path = "../crates/mapgen" }
   tiled = { path = "../crates/tiled" }
   helpers = { path = "../crates/helpers" }
   tracing = { workspace = true }
   tracing-subscriber = { workspace = true }
   serde_json = { workspace = true }
   ```
2. Write `bevy-console/src/main.rs`:
   ```rust
   use bevy::prelude::*;
   use clap::Parser;

   #[derive(Parser)]
   struct Args { #[arg(long)] windowed: bool }

   fn main() {
       let args = Args::parse();
       let mut app = App::new();
       app.add_plugins(DefaultPlugins.set(WindowPlugin {
           primary_window: Some(Window {
               fullscreen: if args.windowed { None } else { Some(bevy::window::Fullscreen::Borderless(None)) },
               title: "weave of realms".into(),
               ..default()
           }),
           ..default()
       }));
       app.add_systems(Startup, |mut commands: Commands| {
           commands.spawn(Camera2d);
       });
       app.run();
   }
   ```
3. Run `cargo check -p bevy-console`. Verify zero errors.
4. Commit.

---

### Task 2: Define App States & basic state scaffolding

**Objective:** Map every `AppScreen` variant to a Bevy `States` variant and scaffold the state plugin.

**Files:**
- Create: `bevy-console/src/screens/mod.rs`
- Create: `bevy-console/src/screens/splash.rs`
- Modify: `bevy-console/src/main.rs`

**Steps:**

1. Define states in `bevy-console/src/screens/mod.rs`:
   ```rust
   #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
   pub enum AppState {
       #[default]
       Splash,
       MapSelect,
       SaveSelect,
       RandomMap,
       TeamSetup,
       #[default]
       MapView,
   }
   ```
2. Create `SplashPlugin` in `bevy-console/src/screens/splash.rs` with empty `enter_splash` system (spawns a background color rect + title text) and `exit_splash` system that despawns everything with `SplashState` marker component.
3. In `main.rs` replace the simple `Startup` system with `app.init_state::<AppState>()` and register plugins for each state.
4. Run `cargo check -p bevy-console`. Verify zero errors.
5. Commit.

---

### Task 3: Port `SplashScreen` → Bevy UI

**Objective:** Render splash with title, two buttons (`New Game`, `Load Game`) and footer, using Bevy UI `Button` nodes.

**Files:**
- Modify: `bevy-console/src/screens/splash.rs`

**Steps:**

1. Define marker component `#[derive(Component)] struct SplashRoot;`.
2. In `enter_splash` spawn a `Node` with `width: Val::Percent(100)`, `height: Val::Percent(100)`, `justify_content: JustifyContent::Center`, `align_items: AlignItems::Center`, coloured dark.
3. As children spawn two `Button` nodes labelled `New Game` and `Load Game`.
4. Add `update_splash` system running in `Update` during `in_state(AppState::Splash)`:
   - Read `Query<&Interaction, (Changed<Interaction>, With<Button>)>`.
   - On `Interaction::Pressed` push `NextState<AppState>` to `MapSelect` (index 0) or `SaveSelect` (index 1).
   - Read keyboard input (`Res<ButtonInput<KeyCode>>`) for Up/Down + Enter as fallback.
5. `exit_splash` despawn all with `SplashRoot`.
6. Run `cargo check -p bevy-console`.
7. Commit.

---

### Task 4: Bevy `AppHost` Resource (port `app_host.rs`)

**Objective:** Replace `AppHost` trait with a Bevy `Resource` that wraps map/save directories, seed, width, height, and the loader/generator paths.

**Files:**
- Create: `bevy-console/src/app_host.rs`
- Modify: `bevy-console/src/main.rs`

**Steps:**

1. Copy the I/O logic from `crates/game/src/app_host.rs` into a plain Rust `struct AppHost` (no trait), with methods `discover_maps()`, `discover_saves()`, `load_map()`, `load_save()`, `save_game()`, `generate_and_save_map()`, `load_map_only()`, `info_overlay()`.
2. Store it as Bevy resource: `commands.insert_resource(AppHost::new(args: &Args))`.
3. Implement `error_message` on `AppHostError`.
4. Ensure `discover_maps()` inserts the `__random_map` pseudo entry and sorts.
5. Run `cargo check -p bevy-console`.
6. Commit.

---

### Task 5: Port `MapSelect` & `SaveSelect` list screens

**Objective:** Replicate `ListScreen` + `draw_list_screen` in Bevy. Dynamically spawn list rows based on `AppHost::discover_maps()` / `discover_saves()`.

**Files:**
- Create: `bevy-console/src/screens/map_select.rs`
- Create: `bevy-console/src/screens/save_select.rs`

**Steps:**

1. In `enter_map_select`:
   - `Res<AppHost>` → call `discover_maps()`.
   - Spawn root `Node` with column flex layout.
   - For each `ListEntry`, spawn a `Button` child with the label text.
   - Track `selected_idx: usize` in local system state or via a marker component.
2. `update_map_select` system:
   - Read keyboard Up/Down/Enter/Back.
   - On Enter: if `__random_map` → `RandomMap`; else if real map → `load_map_only(entry)` → store `PendingMapData` resource → `TeamSetup`.
   - On Back → `Splash`.
3. `save_select.rs` mirrors this but uses `discover_saves()` and on Enter loads full state into `LoadedGame` resource → `MapView`.
4. Run `cargo check -p bevy-console`.
5. Commit.

---

### Task 6: Port `RandomMapScreen` to Bevy UI

**Objective:** Seed label + two buttons (`Random`, `Play`) and Back.

**Files:**
- Create: `bevy-console/src/screens/random_map.rs`
- Modify: `bevy-console/src/screens/map_select.rs` (transition into RandomMap)

**Steps:**

1. Define `RandomMapState` resource holding `(seed: Option<String>)`.
2. `enter_random_map` spawns a title, a status text ("Press Random to generate seed"), and three buttons: `Random`, `Play`, `Back`.
3. `update_random_map` system:
   - `Random` pressed → generate seed (use `sha3` deterministic seed helper), update status text entity via `Query<&mut Text, With<StatusText>>`.
   - `Play` pressed → call `generate_and_save_map(seed)` → `load_map_only(...)` → store `PendingMapData` → `NextState::TeamSetup`.
   - `Back` → `NextState::MapSelect`.
4. Run `cargo check -p bevy-console`.
5. Commit.

---

### Task 7: Port `TeamSetupScreen` to Bevy UI

**Objective:** Replicate team setup: count selector, per-team name/colour/controller rows, and Play/Back buttons.

**Files:**
- Create: `bevy-console/src/screens/team_setup.rs`

**Steps:**

1. Copy `generate_team_name` and HSL→RGB helpers from `crates/game/src/team_setup.rs` into a pure module (no Bevy dep).
2. Define `TeamSetupState` resource:
   ```rust
   pub struct TeamSetupState {
       pub teams: Vec<TeamRow>,
       pub count: usize,        // 1..8
       pub selected_row: usize, // currently highlighted row
       pub confirm_focus: bool, // true = Play button highlighted, false = row editing
   }
   ```
3. `enter_team_setup` spawns a root node with rows laid out in a `Node` flex column. Each row is a horizontal flex `Node` containing:
   - colour block (small rectangle `Node` with `BackgroundColor`)
   - name text
   - Human / CPU toggle (radio style with left/right arrows or two buttons)
   - Team count selector at top, Play/Back buttons at bottom.
4. `update_team_setup`:
   - Handle Up/Down to navigate rows / Play / Back.
   - Handle Left/Right to adjust `count` (1...8) or toggle Human/CPU.
   - On `Enter`:
     - If Play: construct `Vec<TeamConfig>` → `build_state_with_teams(map, &team_cfgs)` using `PendingMapData` resource → spawn `GameSession` resource → `NextState::MapView`.
     - If Back: clear `PendingMapData` → `NextState::MapSelect`.
5. Run `cargo check -p bevy-console`.
6. Commit.

---

### Task 8: Port `MapViewApp` / `GameSession` to Bevy Resource + Systems

**Objective:** Convert the imperative `MapViewApp::handle_input()` into an ECS system. Store the session as a resource and update it with Bevy input events.

**Files:**
- Create: `bevy-console/src/game_session.rs` (wrap `GameSession` as resource)
- Create: `bevy-console/src/screens/map_view.rs`
- Create: `bevy-console/src/map_renderer.rs`

**Steps:**

1. Wrap `GameSession` as `#[derive(Resource)] pub struct SessionRes(pub GameSession);`.
2. Add `MapViewState` resource:
   ```rust
   pub struct MapViewState {
       pub view_x: usize,
       pub view_y: usize,
       pub cursor_x: usize,
       pub cursor_y: usize,
       pub status: Option<String>,
       pub show_info: bool,
       pub show_save: bool,
       pub show_end_turn: bool,
   }
   ```
3. `enter_map_view`:
   - Spawn tile sprites based on `SessionRes.0.state().map`. Iterate `GameMap` tiles; for each tile spawn a `Sprite` with colour from a tile-colour lookup. Group into a `MapRoot` marker for easy cleanup.
   - Spawn a HUD entity (camera overlay) using `Camera` with `CameraOrder(1)` for UI on top.
4. `update_map_view` system:
   - Read `Res<ButtonInput<KeyCode>>`, map arrow keys / WASD / Tab / Enter / i / p to the same logic as `MapViewApp::handle_input`.
   - Update `SessionRes` via `SessionRes.0.move_selected_hero()`, `SessionRes.0.cycle_selected_hero()`.
   - On `RequestEndTurn` spawn `EndTurnOverlay` entities.
   - On `BackRequested` → `NextState::MapSelect`.
   - On `GameOver` → `NextState::Splash` with message.
5. `map_renderer.rs`: hold a lookup table for `Tiles → Color`. Each tile is a `Sprite` positioned by `Transform`. Water/mountain autotile neighbours computed once per chunk via `terrain_neighbor_bits` and applied by swapping sprite texture / uv.
6. Run `cargo check -p bevy-console`.
7. Commit.

---

### Task 9: Port overlays (Info, Save, End Turn)

**Objective:** Convert `InfoOverlay`, `SaveOverlay`, and `EndTurnOverlay` into Bevy UI panels spawned over the map.

**Files:**
- Create: `bevy-console/src/overlays/info.rs`
- Create: `bevy-console/src/overlays/save.rs`
- Create: `bevy-console/src/overlays/end_turn.rs`
- Modify: `bevy-console/src/screens/map_view.rs`

**Steps:**

1. Each overlay is spawned by a system when a boolean resource `show_info`, `show_save`, or `show_end_turn` is true. Remove with `OnExit` or manual despawn when the flag is cleared.
2. **InfoOverlay:** panel `Node` with title + body lines + footer. Close on Enter/Back/Escape.
3. **SaveOverlay:** three sub-screens (menu, save-name entry, load-list). Represented by an enum in a resource:
   ```rust
   enum SaveOverlayState { Menu, SaveName(String), LoadList(Vec<ListEntry>) }
   ```
   Use buttons for menu items, a `TextInput` (or editable text field) for save-name. For now use simple buttons (`Save`, `Load`, `Cancel`) + a text field that accumulates key presses.
4. **EndTurnOverlay:** simple centred panel with text "End turn?" + two buttons `Yes` / `No`. On Yes call `SessionRes.0.end_turn()` and close.
5. Run `cargo check -p bevy-console`.
6. Commit.

---

### Task 10: Port tile atlas rendering (autotile + sprites)

**Objective:** Replace the `embedded-graphics` tile atlas with Bevy `SpriteSheet` or individual `TextureAtlas` entries.

**Files:**
- Create: `bevy-console/src/atlas.rs`
- Modify: `bevy-console/src/map_renderer.rs`

**Steps:**

1. At startup (`asset_server.load` or bytes embedded via `include_bytes!`): load PNG assets (`1_main.png`, `2_water.png`, `3_mountains.png`, etc.) as `Handle<Image>`.
2. Build a `TextureAtlasLayout` (Bevy helper for sprite sheets) mapping each tile index to a UV rect.
3. In `map_renderer.rs`, when creating each tile sprite:
   - Base terrain → look up sprite index from `GRASS_INDICES`, `FOREST_INDICES`.
   - Water/mountain autotile → compute `terrain_neighbor_bits` → look up water/mountain specific composite index → use that sprite index.
4. If using `Sprite` per tile, batch into a single mesh or enable `Sprite` batching by default in Bevy.
5. Cache map sprites: on each viewport move only reposition the camera / transform, don't rebuild sprites.
6. Run `cargo check -p bevy-console`.
7. Commit.

---

### Task 11: Connect launch args `--windowed` and `--seed`

**Objective:** Parse CLI exactly as `sdl2-console` does.

**Files:**
- Modify: `bevy-console/src/main.rs`

**Steps:**

1. Use `clap::Parser` to define CLI struct:
   - `windowed: bool`
   - `seed: Option<String>`
   - `width: u32` (default 96)
   - `height: u32` (default 96)
   - `map: Option<String>`
2. Store parsed args in a Bevy resource `LaunchConfig`.
3. If `start_map` is set, skip `Splash` and go straight to `MapSelect` / `MapView` per existing logic.
4. Run `cargo check -p bevy-console`.
5. Commit.

---

### Task 12: Extract pure-logic modules from `game` (shared with Bevy)

**Objective:** Ensure `bevy-console` can depend on the pure state/input code without pulling `embedded-graphics`.

**Files:**
- Modify: `crates/game/Cargo.toml`
- Modify: `crates/game/src/lib.rs`

**Steps:**

1. In `crates/game/Cargo.toml`, split `embedded-graphics` into an optional feature (e.g. `render`) so it can be excluded.
2. In `crates/game/src/lib.rs`, gate `pub mod render;`, `pub mod prelude;`, and `pub mod app;` behind `#[cfg(feature = "render")]` or a similar feature, while keeping `pub mod input; pub mod session; pub mod map_view; pub mod io; pub mod app_host; pub mod random_map; pub mod team_setup; pub mod types; pub mod splash; pub mod list; pub mod save_overlay; pub mod info_overlay; pub mod turn_overlay;` available unconditionally.
   *Alternative:* since `bevy-console` may not need the `game` crate at all (it can use `engine` + its own Bevy systems), we can skip this if we don't add `game` as a dependency. This plan **does not** add `game` as a dep to avoid `embedded-graphics`.
3. Update `sdl2-console` and `sixel-console` Cargo.toml if a feature flag changes.
4. Run `cargo check --workspace`.
5. Commit.

---

### Task 13: Wire up keyboard + gamepad input in Bevy (port `input.rs`)

**Objective:** Bevy `Res<ButtonInput<KeyCode>>` and `Gamepad` events provide raw input. Map them to `InputEvent` or directly to state actions.

**Files:**
- Create: `bevy-console/src/input.rs`

**Steps:**

1. Create a system `read_keyboard_input(keys: Res<ButtonInput<KeyCode>>, mut ev: EventWriter<AppInputEvent>)` that mirrors existing `sdl2-console/src/input.rs` mappings:
   - Arrow keys → Cursor / movement
   - WASD → Pan
   - HJKL → Cursor
   - Tab → NextHero
   - Enter → Enter (also gamepad A)
   - Escape / Backspace → Back
   - Space → NextTurn
   - `i` → Info overlay
   - `p` → Save overlay
2. Gamepad system using `bevy::input::gamepad::Gamepad` events: left stick pan, right stick cursor (with deadzone). Use constants from SDL2 controller mapping.
3. Emit a custom `AppInputEvent` event (clone of `game::input::InputEvent`) each frame. Consume the event in the active state's update system.
4. Run `cargo check -p bevy-console`.
5. Commit.

---

### Task 14: Build + smoke test for clippy + test compliance

**Objective:** Guarantee `bevy-console` is lint-clean and doesn't break the workspace.

**Files:**
- Modify: `bevy-console/Cargo.toml`
- Modify: `bevy-console/src/*.rs`

**Steps:**

1. Run `cargo check --workspace` — fix any new error from feature-gate changes.
2. Run `cargo clippy --workspace` — zero warnings from `bevy-console`.
3. Run `cargo test --workspace` — existing tests still pass.
4. Run `cargo run -p bevy-console` and verify a window appears with the splash screen.
5. Commit.

---

## Summary of Files to Create/Modify

### New files in `bevy-console/src/`
```
main.rs
app_host.rs
input.rs
atlas.rs
map_renderer.rs
game_session.rs
screens/mod.rs
screens/splash.rs
screens/map_select.rs
screens/save_select.rs
screens/random_map.rs
screens/team_setup.rs
screens/map_view.rs
overlays/mod.rs
overlays/info.rs
overlays/save.rs
overlays/end_turn.rs
```

### Modified files outside `bevy-console/`
```
Cargo.toml (workspace root — add bevy-console to members if not already present)
crates/game/Cargo.toml (optional feature split for embedded-graphics)
crates/game/src/lib.rs (feature-gate render modules)
sdl2-console/Cargo.toml (possibly add feature = "render" for game dep if needed)
sixel-console/Cargo.toml (same)
```

## Verification Steps

- [ ] `cargo check --workspace` passes with zero errors.
- [ ] `cargo clippy --workspace` passes with zero warnings from `bevy-console`.
- [ ] `cargo run -p bevy-console` shows the splash screen in fullscreen.
- [ ] Pressing `New Game` → shows map list.
- [ ] Selecting a map → `TeamSetup` screen.
- [ ] Adjusting team count / names / colours / controller works.
- [ ] Pressing `Play` → `MapView` with terrain and heroes rendered.
- [ ] WASD pans camera, arrows move hero, Tab cycles heroes, Space shows end-turn overlay.
- [ ] `i` opens info overlay, Escape closes it.
- [ ] `p` opens save overlay, save/load operations persist to disk.
- [ ] Pressing Back from map view returns to map select.
- [ ] `--windowed` starts in windowed mode.
- [ ] `--seed <x>` and `--map <name>` direct-boot works.
