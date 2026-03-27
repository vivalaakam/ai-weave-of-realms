# Agent Instructions

This file contains mandatory rules for all code agents working on this project.
Every agent MUST read and follow these rules before touching any code.

---

## Language & Runtime

- All game logic and engine code is written in **Rust** (stable toolchain)
- Godot integration uses **gdext** (godot-rust, Godot 4.x)
- Scripting for map generation rules uses **Lua 5.4** via **mlua** crate
- **GDScript is strictly forbidden**
- No `.gd` files should ever be created

---

## Error Handling

- **`anyhow` is forbidden**
- Use **`thiserror`** for all custom error types
- Every module must define its own `Error` enum in `error.rs`
- Every public function that can fail must return `Result<T, crate::error::Error>`

---

## Logging

- **`println!` and `eprintln!` are forbidden** for diagnostic output
- Use **`tracing`** crate for all logging:
  - `tracing::info!` — normal flow events
  - `tracing::warn!` — recoverable anomalies
  - `tracing::error!` — errors
  - `tracing::debug!` — detailed debug info
  - `tracing::trace!` — fine-grained tracing
- Initialize subscriber in main entry points only (`tracing_subscriber`)

---

## Documentation

- **Every public function must have a doc comment** (`///`)
- Doc comments must include:
  - What the function does (one line summary)
  - `# Arguments` section if there are non-obvious parameters
  - `# Returns` section if return value needs explanation
  - `# Errors` section if the function returns `Result`
  - `# Panics` section if the function can panic
- Every public struct and enum must have a doc comment
- Every module (`lib.rs`, `mod.rs`) must have a module-level doc comment (`//!`)

Example:
```rust
/// Generates a single 32×32 chunk using the provided seed and Lua script.
///
/// # Arguments
/// * `seed` - Deterministic seed derived from map seed + chunk coordinates
/// * `script` - Compiled Lua chunk generation function
///
/// # Returns
/// A fully populated `Chunk` with tile data.
///
/// # Errors
/// Returns `Error::LuaExecution` if the Lua script fails.
pub fn generate_chunk(seed: [u8; 32], script: &LuaScript) -> Result<Chunk, Error> {
```

---

## Task Tracking

- **Every task, subtask, and decision must be recorded in `TASKS.md`**
- When starting a task: update status to `[IN PROGRESS]`
- When completing a task: update status to `[DONE]`
- When blocked: update status to `[BLOCKED]` and add a note
- Never start a new task without recording it in `TASKS.md` first
- For significant architectural decisions, add a note in `TASKS.md` under the relevant task

---

## Code Style

- Prefer explicit types over inference in public APIs
- No `unwrap()` or `expect()` in library code (only in tests or main with clear justification)
- Keep modules small and focused — one responsibility per file
- No dead code in committed files (remove or annotate with `#[allow(dead_code)]` + a TODO comment)

---

## Dependency Policy

| Allowed | Forbidden |
|---------|-----------|
| `thiserror` | `anyhow` |
| `tracing`, `tracing-subscriber` | `log`, `env_logger`, `println!` |
| `mlua` (Lua 5.4) | Any other scripting runtime |
| `sha3` (Keccak256) | `rand`, `rand_chacha` (use SeededRng) |
| `gdext` | `godot-rust` (old API) |
| `serde`, `serde_json` | — |
| `quick-xml` | — |

---

## Project Structure

```
ai-rpg-v2/
├── PLAN.md          # Architecture and phase plan
├── TASKS.md         # Task tracking (always up to date)
├── AGENTS.md        # This file
├── Cargo.toml       # Workspace root
├── crates/
│   ├── rpg-engine/  # Pure game logic, zero Godot deps
│   ├── rpg-mapgen/  # Map generator with Lua scripting
│   ├── rpg-tiled/   # TMX import/export
│   └── rpg-godot/   # Godot wrapper (gdext bridge)
├── godot/           # Godot 4 project
└── scripts/         # Lua scripts (generation rules, validators, evaluators)
    ├── generators/
    └── rules/
```

---

## Crate Responsibilities

| Crate | Responsibility | Godot dep |
|-------|---------------|-----------|
| `rpg-engine` | Hero, combat, score, map types, PRNG | No |
| `rpg-mapgen` | Chunk generation, stitching, Lua loader, evaluator, validator | No |
| `rpg-tiled` | TMX parse/serialize, chunk↔TMX conversion | No |
| `rpg-godot` | GodotNode wrappers, signals, scene bridging | Yes |
