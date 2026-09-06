# Wasteland Tactics — design spec

Date: 2026-09-06
Status: approved in brainstorming, awaiting implementation plan

## Purpose

A hobby project: a top-down, real-time-with-pause tactical squad shooter in
the spirit of Fallout Tactics, written in Rust. The goal is a playable game
quickly, spending hobby time on gameplay rather than engine internals.

Public repo under the personal GitHub account `jmortlock`.

## First milestone ("one squad, one map")

The smallest thing that counts as a game:

- Player controls a squad of 3–4 soldiers on a single hand-made map.
- Real-time simulation with a pause toggle (Space). Orders can be issued while
  paused or unpaused.
- Left click selects a unit; right click on ground orders a move; right click
  on an enemy orders an attack.
- Line of sight, range and directional cover affect who can shoot whom and
  the chance to hit.
- A handful of enemy units with simple AI: idle until a player unit is seen,
  then move to cover and shoot.
- Units die at zero health. Mission ends when one side is eliminated (a plain
  text overlay is sufficient).

Everything else — inventory, multiple maps, objectives, menus, saves,
multiplayer, isometric view, projectile physics — is out of scope for this
milestone.

## Approach and stack

Chosen approach: Bevy with its plugin ecosystem (over macroquad+hecs and
ggez), because the ECS fits a units-and-orders game and the tilemap, Tiled and
audio plumbing already exists.

| Concern | Crate | Version policy |
|---|---|---|
| Engine, 2D sprites, UI, states | `bevy` | 0.19.x, pinned |
| Tilemap rendering | `bevy_ecs_tilemap` | matching Bevy 0.19 |
| Tiled `.tmx` loading | `bevy_ecs_tiled` | latest release that targets Bevy 0.19 — **verify at scaffold time**; if none exists, fall back to loading `.tmx` with the `tiled` crate directly and rendering via `bevy_ecs_tilemap` |
| Audio | `bevy_kira_audio` | matching Bevy 0.19 |
| Debug inspector | `bevy_egui` + `bevy-inspector-egui` | matching Bevy 0.19, dev-only feature |
| Pathfinding | `pathfinding` (A*) | any recent |
| RNG | `rand` + `rand_chacha` | any recent |

All versions are pinned to exact minors. Upgrading Bevy is a deliberate task,
never a side effect of `cargo update`.

## Repo layout and tooling

- Single crate, not a workspace. Modules per subsystem under `src/`.
- `Cargo.toml` dev profile: Bevy `dynamic_linking` feature, `opt-level = 3`
  for dependencies, `opt-level = 1` for the crate itself.
- `.cargo/config.toml` uses `mold` (already installed) as the linker via
  `clang`.
- `assets/` at repo root (Bevy default) with `sprites/`, `tiles/`, `maps/`,
  `audio/`.
- `CREDITS.md` lists every asset pack, its author, licence and URL.
- `.github/workflows/ci.yml`: `cargo fmt --check`, `cargo clippy -D warnings`,
  `cargo test`, on Ubuntu, installing Bevy's Linux system dependencies
  (`libasound2-dev`, `libudev-dev`, etc.). No release builds.
- Tiled installed via `apt install tiled` on the dev machine; not part of the
  repo.

## Game architecture

### States

`AppState` enum: `Loading`, `Playing`, `Paused`. Later additions (`Menu`,
`MissionOver`) are out of scope. Simulation systems (movement, combat, AI)
run only in `Playing`. Input, camera and rendering run in both `Playing` and
`Paused`. Space toggles between the two.

### Modules

| Module | Responsibility | Depends on |
|---|---|---|
| `grid` | Pure data: `Grid` of cells with `walkable` and directional `cover` flags; grid↔world conversion; Bresenham line walk; LOS; cover lookup. No Bevy types. | nothing |
| `rules` | Pure functions: hit chance from range and cover, damage roll. No Bevy types. | `grid`, `rand` |
| `map` | Loads the Tiled map, builds the `Grid` resource from tile properties, spawns units from the object layer. | `grid`, `bevy_ecs_tiled` |
| `unit` | Components shared by soldiers and enemies: `GridPos`, `Faction`, `Health`, `Weapon`, `Order`, `Facing`, `Speed`. Spawning helpers. | `grid` |
| `orders` | Player input: selection, issuing `Order::MoveTo` / `Order::Attack` to selected units. | `unit`, `camera` |
| `movement` | Executes `MoveTo` orders along A* paths; interpolates world position for rendering. | `unit`, `grid`, `pathfinding` |
| `combat` | Executes `Attack` orders: LOS check, range check, hit roll via `rules`, damage, death. Spawns a short-lived tracer sprite and plays a sound. | `unit`, `grid`, `rules` |
| `ai` | Enemy behaviour: idle → (sees player unit) → move to nearest cover with LOS → attack. Reissues orders via the same `Order` component the player uses. | `unit`, `grid`, `rules` |
| `camera` | 2D camera: WASD / edge-scroll pan, wheel zoom. | nothing |
| `ui` | Bevy UI: health bars over units, pause indicator, end-of-mission text. | `unit` |
| `audio` | Loads SFX/music into channels; a `PlaySfx` event other modules send. | `bevy_kira_audio` |
| `assets` | `Loading` state: one resource holding every asset handle; transitions to `Playing` when all are loaded. | everything |

### Positions

Units hold a `GridPos` (logic) and a Bevy `Transform` (rendering). All
tactical rules — LOS, range, cover, pathing — operate on the grid so they are
testable without a renderer. `movement` interpolates `Transform` toward the
target cell each frame.

### Combat model

Instant-hit. An attack resolves in one tick: LOS via Bresenham across the
grid, range check against the weapon, hit chance from `rules`, damage
applied. No physics engine, no projectile entities. A tracer sprite from
shooter to target is spawned for a fraction of a second purely as feedback.

Cover is directional: a cell edge flagged as cover reduces hit chance for
shots whose LOS line crosses that edge into the target's cell.

### Randomness

A single `GameRng` resource seeded at startup (from a CLI flag or the clock).
Tests seed it explicitly so battles replay deterministically.

## Asset pipeline

- **Art:** Kenney CC0 packs — "Top-down Shooter" for characters/weapons,
  "Topdown Tanks Redux" or "Roguelike" packs for tiles, walls and props.
  Spritesheet + atlas loaded with Bevy's `TextureAtlasLayout`.
- **Tiles:** one tileset PNG referenced from Tiled as an external `.tsx`.
  Custom tile properties `walkable: bool` and `cover: string` (e.g. `n`, `e`,
  `ne`) are read by `map` to build the `Grid`.
- **Map:** `assets/maps/mission01.tmx` with tile layers `ground` and `walls`,
  plus an object layer `spawns` whose objects carry a `faction` property
  (`player` | `enemy`).
- **Audio:** Kenney "Impact Sounds" / "Sci-Fi Sounds" OGGs for gunfire and
  hits. Two `bevy_kira_audio` channels, `sfx` and `music`, with independent
  volume.
- **Loading:** all handles loaded in `AppState::Loading`; no lazy loading.
- **Distribution:** packs downloaded once by hand from kenney.nl and
  committed (a few MB). No git LFS. Every pack recorded in `CREDITS.md`.

## Testing

- `grid` and `rules` are pure and get ordinary unit tests: LOS through/around
  walls, cover edge detection, hit-chance tables, grid↔world round trips,
  path existence.
- Headless ECS tests using Bevy's `MinimalPlugins`: build a small `Grid` in
  code, spawn two units, run the schedule N ticks with a seeded `GameRng`,
  assert outcomes (e.g. "unit in cover takes fewer hits than unit in the
  open over 1000 seeded shots").
- No rendering or audio tests; those are verified by playing.
- CI runs the full test suite on every push and PR.

## Non-goals (explicit)

Isometric projection, projectile physics, procedural maps, inventory/loot,
save games, menus beyond a text overlay, multiplayer, WASM/web builds,
custom art. Any of these is a separate spec if it ever comes up.
