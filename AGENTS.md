# Repository Guidelines

## Project Structure & Module Organization
The workspace is defined in `Cargo.toml` and centers on shared Rust crates under `crates/`. Core logic lives in `crates/engine`, with generation and interchange split into `crates/mapgen` (Lua-backed generation scripts in `scripts/`) and `crates/tiled` (TMX import/export). Developer tooling sits in `crates/rpg-tools`, while `crates/helpers` supplies cross-cutting utilities consumed by the app crates. The interactive frontend is `bevy-console/`, which wires Bevy to the shared crates for running the game and map tools from a desktop host.

## Build, Test, and Development Commands
Use the `justfile` targets as the canonical shortcuts:
- `just test` → `cargo test --workspace`
- `just sdl2-run` → run the SDL2 console binary
- `just mapgen` → run the `mapgen` CLI with a timestamp seed
- `just clean` → `cargo clean`
Run a single test with `cargo test -p <crate> <test_name>`.

## Coding Style & Naming Conventions
Formatting is enforced by `rustfmt.toml` (edition 2021, 4-space indentation, max width 100). Clippy is configured in `clippy.toml` with elevated thresholds for argument count and cognitive complexity. For error handling, use `thiserror` (`anyhow` is not allowed), define per-module `error.rs` enums, and return `Result<T, crate::error::Error>` from fallible public APIs. Logging should use `tracing` (no `println!`/`eprintln!`). Public items require doc comments with clear `# Arguments`/`# Returns`/`# Errors`/`# Panics` sections when applicable, and avoid `unwrap()` or dead code in library modules.

## Testing Guidelines
Workspace tests run with `just test` or `cargo test --workspace`. No coverage tooling or extra test harnesses are configured.

## Commit & Pull Request Guidelines
Recent history favors short, imperative commit messages; optional prefixes like `fix:`, `feat:`, or `docs:` appear, and some commits reference issues via `(#N)`. No PR template is present in the repo.

## Agent Instructions
All tasks and decisions must be tracked in `TASKS.md` with status updates (`IN PROGRESS`, `DONE`, `BLOCKED`). When working on specialized areas, consult the relevant guide under `.agents/skills/` before making changes.
