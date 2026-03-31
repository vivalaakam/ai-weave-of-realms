GODOT        := "godot"
GODOT_PROJECT := "godot"
BIN_DIR      := "godot/bin"

# Sync scripts and tileset into the Godot project directory so res:// can reach them.
_sync-assets:
    mkdir -p godot/bin godot/assets
    rsync -a --delete scripts/ godot/scripts/
    cp tileset/tileset.png godot/assets/tileset.png

# Build GDExtension (debug) and sync assets.
build: _sync-assets
    cargo build -p rpg-godot
    cp target/debug/librpg_godot.dylib {{ BIN_DIR }}/librpg_godot.dylib

# Build GDExtension (release) and sync assets.
build-release: _sync-assets
    cargo build -p rpg-godot --release
    cp target/release/librpg_godot.dylib {{ BIN_DIR }}/librpg_godot.dylib

# Run all workspace tests.
test:
    cargo test --workspace

# Build (debug), then open Godot editor.
editor: build
    {{ GODOT }} --editor --path {{ GODOT_PROJECT }} &

# Build (debug), then run the game.
run: build
    {{ GODOT }} --path {{ GODOT_PROJECT }}

# Build (release), then run the game.
run-release: build-release
    {{ GODOT }} --path {{ GODOT_PROJECT }}

# Generate a map PNG + TMX (default seed).
mapgen:
    cargo run -p rpg-tools --bin mapgen -- --generator scripts/generators/terrain.lua

# Remove build artefacts.
clean:
    cargo clean
    rm -rf {{ BIN_DIR }} godot/scripts godot/assets/tileset.png
