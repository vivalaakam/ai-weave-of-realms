# Run all workspace tests.
test:
    cargo test --workspace

sdl2-run:
    cargo run --bin weave-of-realms-sdl2

# Generate a map PNG + TMX (default seed).
mapgen:
    cargo run -p rpg-tools --bin mapgen -- --seed "$(DATE)"

# Remove build artefacts.
clean:
    cargo clean
