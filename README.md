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
    cargo run --features dynamic,inspector   # live entity inspector (egui)
    WT_SCREENSHOT=out.png cargo run --features dynamic   # screenshot after 2 s, then exit
    cargo run --features dynamic -- --seed 42            # fixed dice stream (frame timing still varies)

Linux builds link with mold and clang (sudo apt install mold clang); see .cargo/config.toml.

Maps are edited with [Tiled](https://www.mapeditor.org/) (`sudo apt install tiled`).
Design spec: `docs/superpowers/specs/`. Art and sound credits: `CREDITS.md`.

Maps: `tools/gen_map.py` regenerates `assets/maps/mission01.tmx` from ASCII; open the result in Tiled to hand-edit.

## Status

Slice 1 (one squad, one map) — see `docs/superpowers/specs/` and the GitHub milestone.
