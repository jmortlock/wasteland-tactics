# Wasteland Tactics

A hobby top-down, real-time-with-pause tactical squad shooter in Rust + Bevy,
in the spirit of Fallout Tactics.

## Run

    cargo run --features dynamic

`--features dynamic` links Bevy dynamically for fast rebuilds. Omit it for a
standalone binary.

Squad turn-based: each soldier has action points (1 per cell moved, 3 per shot).
Left click or keys 1–4 select a soldier; hovering shows reachable cells, the path
and its AP cost, or the hit chance on an enemy. Right click moves or shoots.
Enter ends your turn; enemies then act. Unspent AP fires reaction shots at enemies
that move into view during their turn. R restarts, WASD pans, mouse wheel zooms.

## Develop

    cargo test
    cargo clippy --all-targets -- -D warnings
    cargo fmt
    cargo run --features dynamic,inspector   # live entity inspector (egui)
    WT_SCREENSHOT=out.png cargo run --features dynamic   # screenshot after 2 s, then exit
    cargo run --features dynamic -- --seed 42            # same seed + same clicks = same battle

Linux builds link with mold and clang (sudo apt install mold clang); see .cargo/config.toml.

Maps are edited with [Tiled](https://www.mapeditor.org/) (`sudo apt install tiled`).
Design spec: `docs/superpowers/specs/`. Art and sound credits: `CREDITS.md`.

Maps: `tools/gen_map.py` regenerates `assets/maps/mission01.tmx` from ASCII; open the result in Tiled to hand-edit.

## Status

Slice 2 (squad turn-based core loop) — see `docs/superpowers/specs/2026-09-06-slice2-turn-based-design.md`.
