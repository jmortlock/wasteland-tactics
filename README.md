# Wasteland Tactics

A hobby top-down, real-time-with-pause tactical squad shooter in Rust + Bevy,
in the spirit of Fallout Tactics.

## Run

    cargo run --features dynamic

`--features dynamic` links Bevy dynamically for fast rebuilds. Omit it for a
standalone binary.

Controls: left click selects a soldier (or keys 1–4), right click moves,
right click on an enemy attacks, Space pauses, WASD pans, mouse wheel zooms.

## Develop

    cargo test
    cargo clippy --all-targets -- -D warnings
    cargo fmt

Maps are edited with [Tiled](https://www.mapeditor.org/) (`sudo apt install tiled`).
Design spec: `docs/superpowers/specs/`. Art and sound credits: `CREDITS.md`.
