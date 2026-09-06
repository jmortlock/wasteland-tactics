# Wasteland Tactics — Slice 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A playable top-down, real-time-with-pause squad skirmish: 4 soldiers vs 3 enemies on one Tiled map, with line of sight, directional cover, simple enemy AI, and a win/lose overlay.

**Architecture:** One Bevy 0.19 binary crate. Pure, Bevy-free rules (`grid`, `rules`) sit under an ECS `SimulationPlugin` (units, orders, movement, combat, AI, mission end) that runs headless in tests. A separate `PresentationPlugin` (asset loading, Tiled map, sprites, camera, input, UI, audio) is only exercised by running the game.

**Tech Stack:** Rust 1.96 (edition 2024), bevy 0.19.1, bevy_ecs_tiled 0.13.4 (+ bevy_ecs_tilemap 0.19.0), bevy_kira_audio 0.26.0, bevy-inspector-egui 0.37.0 (optional), pathfinding 4.15, rand 0.9 + rand_chacha 0.9, tiled 0.16. Tiled editor from apt. Kenney CC0 art and sound.

**Spec:** `docs/superpowers/specs/2026-09-06-wasteland-tactics-design.md`

## Global Constraints

- Versions pinned exactly: `bevy = "=0.19.1"`, `bevy_ecs_tilemap = "=0.19.0"`, `bevy_ecs_tiled = "=0.13.4"`, `bevy_kira_audio = "=0.26.0"`, `bevy-inspector-egui = "=0.37.0"`. Never run `cargo update` on these.
- Single binary crate, no workspace. Modules under `src/`. Unit tests live in `#[cfg(test)] mod tests` inside each module.
- `grid.rs` and `rules.rs` contain no Bevy systems and no `Query`/`Commands`; only data and functions (they may derive Bevy `Component`/`Resource`).
- Simulation systems never touch `Sprite`, `Camera`, audio or UI. Presentation never mutates `Health`, `Order`, `GridPos` except via the `orders` input system, which only writes `Order` and removes `Path`.
- Grid coordinates: origin is the bottom-left cell, `y` grows upward like Bevy world space. Tiled's rows are top-down; the map loader flips with `grid_y = map_height - 1 - tiled_y`. `TILE_SIZE = 64.0` pixels.
- Commit after every task with the `Co-Authored-By` / `Claude-Session` trailers shown in Task 1. `Closes #<issue for Task N>` means the issue number printed by Task 1 Step 10 for that task's title.
- Every asset file committed must be listed in `CREDITS.md` with author, licence and URL.
- Public repo `jmortlock/wasteland-tactics`; push over the SSH alias `github-personal` (the plain `github.com` host authenticates as the work account). Commit author email for this repo is `john.mortlock@gmail.com`.
- Run the game with `cargo run --features dynamic`. Set `WT_SCREENSHOT=/path/file.png` to have it save a screenshot after 2 s and exit, which is how tasks verify rendering.

---

## File structure

| File | Responsibility |
|---|---|
| `Cargo.toml`, `.cargo/config.toml` | crate, pinned deps, fast-build profile, mold linker |
| `.github/workflows/ci.yml` | fmt, clippy, test on Ubuntu |
| `src/main.rs` | module declarations, CLI seed parsing, `App` assembly |
| `src/grid.rs` | `GridPos`, `Cell`, `CoverSides`, `Grid`, Bresenham `line`, LOS, cover, A* |
| `src/rules.rs` | `GameRng`, `hit_chance`, `roll_hit`, `roll_damage` |
| `src/state.rs` | `AppState`, `toggle_pause`, `MissionOutcome`, `check_mission_end` |
| `src/unit.rs` | unit components, `Faction`, `Order`, `unit_bundle` |
| `src/movement.rs` | `plan_paths`, `follow_paths` |
| `src/combat.rs` | `resolve_attacks`, `apply_death` |
| `src/ai.rs` | `enemy_think` |
| `src/sim.rs` | `SimulationPlugin`, `ShotFired` message, system ordering |
| `src/test_support.rs` | headless `App` builder and `tick` helper (test only) |
| `src/assets.rs` | `GameAssets`, `load_assets`, `check_loaded` |
| `src/map.rs` | `grid_from_tiled`, `spawns_from_tiled`, `spawn_map`, `on_map_created` |
| `src/render.rs` | `attach_sprites`, `tint_dead`, selection/health gizmos, tracers |
| `src/camera.rs` | `spawn_camera`, `pan_camera`, `zoom_camera` |
| `src/orders.rs` | mouse/keyboard → `Order` on selected units |
| `src/ui.rs` | pause and mission-over text overlay |
| `src/audio.rs` | SFX channel, plays sounds on `ShotFired` |
| `src/debug.rs` | `WT_SCREENSHOT` capture-and-exit, optional egui inspector |
| `src/presentation.rs` | `PresentationPlugin` wiring the above |
| `assets/tiles/tileset.png`, `assets/maps/wasteland.tsx`, `assets/maps/mission01.tmx` | map data |
| `assets/sprites/soldier.png`, `assets/sprites/enemy.png` | unit art |
| `assets/audio/shot.ogg`, `assets/audio/hit.ogg` | SFX |
| `CREDITS.md`, `README.md` | attribution, how to run |

---

### Task 1: Crate scaffold, CI, GitHub repo and issue tracking

**Files:**
- Create: `Cargo.toml`, `.cargo/config.toml`, `src/main.rs`, `.gitignore`, `README.md`, `.github/workflows/ci.yml`

**Interfaces:**
- Produces: a compiling crate whose `main` opens a window titled "Wasteland Tactics" with a `Camera2d`. Later tasks add `mod` lines to `src/main.rs`.

- [ ] **Step 1: Fix git identity for this repo and re-author the spec commit**

The global git email is the work address. Set the personal one locally and amend the existing commit (it has not been pushed).

```bash
cd ~/src/wasteland-tactics
git config user.name "John Mortlock"
git config user.email "john.mortlock@gmail.com"
git commit --amend --no-edit --reset-author
git log --format='%an <%ae> %s' -1
```
Expected: `John Mortlock <john.mortlock@gmail.com> Add design spec for Wasteland Tactics first milestone`

- [ ] **Step 2: Write `Cargo.toml`**

```toml
[package]
name = "wasteland-tactics"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
description = "Top-down real-time-with-pause tactical squad shooter"
license = "MIT"

[dependencies]
bevy = "=0.19.1"
bevy_ecs_tilemap = "=0.19.0"
bevy_ecs_tiled = "=0.13.4"
bevy_kira_audio = "=0.26.0"
bevy-inspector-egui = { version = "=0.37.0", optional = true }
pathfinding = "4.15"
rand = "0.9"
rand_chacha = "0.9"
tiled = "0.16"

[features]
default = []
# Fast incremental builds while developing. Never use in CI or release.
dynamic = ["bevy/dynamic_linking"]
# Live entity inspector window (egui).
inspector = ["dep:bevy-inspector-egui"]

# Our code compiles fast, dependencies run fast.
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

- [ ] **Step 3: Write `.cargo/config.toml` (mold linker)**

```toml
# mold + clang: several-second incremental links for Bevy.
# Both are installed on the dev box and in CI (see .github/workflows/ci.yml).
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```

- [ ] **Step 4: Write `src/main.rs`**

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Wasteland Tactics".into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_systems(Startup, spawn_camera)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
```

- [ ] **Step 5: Write `.gitignore` and `README.md`**

`.gitignore`:
```
/target
*.tmx.bak
```

`README.md`:
```markdown
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
```

- [ ] **Step 6: Write `.github/workflows/ci.yml`**

```yaml
name: ci
on:
  push:
    branches: [main]
  pull_request:
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Bevy system deps + mold
        run: |
          sudo apt-get update
          sudo apt-get install -y g++ pkg-config libx11-dev libasound2-dev libudev-dev \
            libxkbcommon-x11-0 libwayland-dev libxkbcommon-dev mold clang
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test
```

- [ ] **Step 7: Build and smoke-run**

```bash
cd ~/src/wasteland-tactics
cargo build --features dynamic 2>&1 | tail -3
timeout 8 cargo run --features dynamic; echo "exit=$?"
```
Expected: build finishes with `Finished`; the run prints Bevy startup logs and is killed by timeout (`exit=124`), with no `panicked` line. (First build takes several minutes.)

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "Scaffold Bevy crate with fast-build profile and CI

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

- [ ] **Step 9: Create the GitHub repo under the personal account and push**

```bash
gh auth switch --user jmortlock
gh repo create jmortlock/wasteland-tactics --public \
  --description "Top-down real-time-with-pause tactical squad shooter in Rust/Bevy"
git remote add origin git@github-personal:jmortlock/wasteland-tactics.git
git push -u origin main
```
Expected: `git push` prints `branch 'main' set up to track 'origin/main'`.

- [ ] **Step 10: Create the milestone and one issue per task (lightweight tracking)**

```bash
gh api repos/jmortlock/wasteland-tactics/milestones -f title="Slice 1: one squad, one map" \
  -f description="First playable: 4 soldiers vs 3 enemies on one map" --jq .number
for t in \
  "Task 2: grid model and coordinates" \
  "Task 3: line of sight and directional cover" \
  "Task 4: A* pathfinding" \
  "Task 5: hit rules and seeded RNG" \
  "Task 6: unit components, app state, movement" \
  "Task 7: combat resolution and death" \
  "Task 8: enemy AI" \
  "Task 9: mission end and pause" \
  "Task 10: art, sound, tileset and mission01 map" \
  "Task 11: asset loading, map spawn, sprites, camera" \
  "Task 12: selection and orders input, gizmos, tracers" \
  "Task 13: UI overlay and audio" \
  "Task 14: inspector feature, README polish"; do
  gh issue create --repo jmortlock/wasteland-tactics --title "$t" \
    --milestone "Slice 1: one squad, one map" \
    --body "See docs/superpowers/plans/2026-09-06-wasteland-tactics-slice1.md"
done
gh auth switch --user jmortlock_lmg
```
Expected: 13 issue URLs printed. Close each issue in its task's final commit message with `Closes #N`.

- [ ] **Step 11: Confirm CI is green**

```bash
gh auth switch --user jmortlock
gh run watch --repo jmortlock/wasteland-tactics --exit-status
gh auth switch --user jmortlock_lmg
```
Expected: `✓ ... ci` and exit 0. If clippy fails on the scaffold, fix and push before continuing.

---

### Task 2: Grid model and coordinates

**Files:**
- Create: `src/grid.rs`
- Modify: `src/main.rs` (add `mod grid;`)

**Interfaces:**
- Produces: `TILE_SIZE: f32`; `GridPos { x, y }` with `new`, `to_world() -> Vec2`, `from_world(Vec2) -> GridPos`, `distance(GridPos) -> i32` (Chebyshev); `CoverSides { n, e, s, w }` with `parse(&str)`, `any()`; `Cell { walkable, blocks_sight, cover }` with consts `FLOOR`, `WALL` and `Cell::cover(CoverSides)`; `Grid` with `new(w, h)`, `from_ascii(&str)`, `width()`, `height()`, `in_bounds(GridPos)`, `get(GridPos) -> Option<&Cell>`, `set(GridPos, Cell)`, `is_walkable(GridPos)`.

- [ ] **Step 1: Write the failing tests**

`src/grid.rs`:
```rust
//! Pure grid model: cells, coordinates, line of sight, cover and pathfinding.
//! No systems live here, only data and functions, so everything is unit-testable.

use bevy::prelude::*;

pub const TILE_SIZE: f32 = 64.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_first_line_is_top_row() {
        let g = Grid::from_ascii("#..\n...");
        assert_eq!((g.width(), g.height()), (3, 2));
        assert!(!g.is_walkable(GridPos::new(0, 1)), "wall is on the top row (y=1)");
        assert!(g.is_walkable(GridPos::new(0, 0)));
        assert!(!g.is_walkable(GridPos::new(5, 5)), "out of bounds is not walkable");
    }

    #[test]
    fn ascii_cover_glyphs() {
        let g = Grid::from_ascii("^>v<");
        assert_eq!(g.get(GridPos::new(0, 0)).unwrap().cover, CoverSides { n: true, ..default() });
        assert_eq!(g.get(GridPos::new(1, 0)).unwrap().cover, CoverSides { e: true, ..default() });
        assert_eq!(g.get(GridPos::new(2, 0)).unwrap().cover, CoverSides { s: true, ..default() });
        assert_eq!(g.get(GridPos::new(3, 0)).unwrap().cover, CoverSides { w: true, ..default() });
        assert!(g.is_walkable(GridPos::new(0, 0)), "cover tiles are walkable");
        assert!(!g.get(GridPos::new(0, 0)).unwrap().blocks_sight);
    }

    #[test]
    fn world_round_trip() {
        let p = GridPos::new(3, 7);
        assert_eq!(p.to_world(), Vec2::new(3.5 * TILE_SIZE, 7.5 * TILE_SIZE));
        assert_eq!(GridPos::from_world(p.to_world()), p);
        assert_eq!(GridPos::from_world(Vec2::new(-1.0, 10.0)), GridPos::new(-1, 0));
    }

    #[test]
    fn chebyshev_distance() {
        assert_eq!(GridPos::new(0, 0).distance(GridPos::new(3, -2)), 3);
    }

    #[test]
    fn cover_sides_parse() {
        assert_eq!(CoverSides::parse("n,e"), CoverSides { n: true, e: true, ..default() });
        assert_eq!(CoverSides::parse("sw"), CoverSides { s: true, w: true, ..default() });
        assert!(!CoverSides::parse("xyz").any());
    }
}
```
Add `mod grid;` to the top of `src/main.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test grid::`
Expected: compile error, `GridPos` etc. not found.

- [ ] **Step 3: Implement**

Insert above the `#[cfg(test)]` block in `src/grid.rs`:
```rust
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug, Reflect)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

impl GridPos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// World-space centre of this cell. Origin is the bottom-left cell; y grows upward.
    pub fn to_world(self) -> Vec2 {
        Vec2::new((self.x as f32 + 0.5) * TILE_SIZE, (self.y as f32 + 0.5) * TILE_SIZE)
    }

    pub fn from_world(p: Vec2) -> Self {
        Self::new((p.x / TILE_SIZE).floor() as i32, (p.y / TILE_SIZE).floor() as i32)
    }

    /// Chebyshev (king-move) distance. Weapon range and sight range use this.
    pub fn distance(self, other: GridPos) -> i32 {
        (self.x - other.x).abs().max((self.y - other.y).abs())
    }
}

/// Which sides of a cell give cover to a unit standing in it.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct CoverSides {
    pub n: bool,
    pub e: bool,
    pub s: bool,
    pub w: bool,
}

impl CoverSides {
    /// Parses a Tiled property such as `"n"`, `"ne"` or `"n,e"`. Unknown characters are ignored.
    pub fn parse(s: &str) -> Self {
        let mut c = Self::default();
        for ch in s.chars() {
            match ch {
                'n' => c.n = true,
                'e' => c.e = true,
                's' => c.s = true,
                'w' => c.w = true,
                _ => {}
            }
        }
        c
    }

    pub fn any(self) -> bool {
        self.n || self.e || self.s || self.w
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub walkable: bool,
    pub blocks_sight: bool,
    pub cover: CoverSides,
}

impl Cell {
    const NO_COVER: CoverSides = CoverSides { n: false, e: false, s: false, w: false };
    pub const FLOOR: Cell = Cell { walkable: true, blocks_sight: false, cover: Self::NO_COVER };
    pub const WALL: Cell = Cell { walkable: false, blocks_sight: true, cover: Self::NO_COVER };

    /// A walkable, see-through tile (sandbags, low wall) that shelters the given sides.
    pub fn cover(sides: CoverSides) -> Cell {
        Cell { walkable: true, blocks_sight: false, cover: sides }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct Grid {
    width: i32,
    height: i32,
    cells: Vec<Cell>,
}

impl Grid {
    /// An all-floor grid.
    pub fn new(width: i32, height: i32) -> Self {
        Self { width, height, cells: vec![Cell::FLOOR; (width * height) as usize] }
    }

    /// Builds a grid from ASCII art. The FIRST line is the TOP row (highest y).
    /// `#` wall, `.` floor, `^` `>` `v` `<` cover tile sheltering its north/east/south/west side.
    pub fn from_ascii(art: &str) -> Self {
        let rows: Vec<&str> = art.lines().filter(|l| !l.trim().is_empty()).map(str::trim).collect();
        let height = rows.len() as i32;
        let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
        let mut grid = Self::new(width, height);
        for (row_idx, row) in rows.iter().enumerate() {
            let y = height - 1 - row_idx as i32;
            for (x, ch) in row.chars().enumerate() {
                let cell = match ch {
                    '#' => Cell::WALL,
                    '^' => Cell::cover(CoverSides { n: true, ..default() }),
                    '>' => Cell::cover(CoverSides { e: true, ..default() }),
                    'v' => Cell::cover(CoverSides { s: true, ..default() }),
                    '<' => Cell::cover(CoverSides { w: true, ..default() }),
                    _ => Cell::FLOOR,
                };
                grid.set(GridPos::new(x as i32, y), cell);
            }
        }
        grid
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn in_bounds(&self, p: GridPos) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height
    }

    pub fn get(&self, p: GridPos) -> Option<&Cell> {
        self.in_bounds(p).then(|| &self.cells[(p.y * self.width + p.x) as usize])
    }

    pub fn set(&mut self, p: GridPos, cell: Cell) {
        if self.in_bounds(p) {
            self.cells[(p.y * self.width + p.x) as usize] = cell;
        }
    }

    pub fn is_walkable(&self, p: GridPos) -> bool {
        self.get(p).is_some_and(|c| c.walkable)
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test grid::`
Expected: 5 passed. (Warnings about unused items are fine for now; clippy `dead_code` is silenced at the crate level in Task 14 only if any remain.)

- [ ] **Step 5: Commit**

```bash
git add src/grid.rs src/main.rs
git commit -m "Add pure grid model with ASCII builder and world conversion

Closes #<issue for Task 2>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 3: Line of sight and directional cover

**Files:**
- Modify: `src/grid.rs`

**Interfaces:**
- Produces: `grid::line(a, b) -> Vec<GridPos>` (Bresenham, inclusive of both ends); `Grid::has_line_of_sight(a, b) -> bool`; `Grid::cover_against(target, shooter) -> bool`.

- [ ] **Step 1: Write the failing tests**

Add inside `mod tests` in `src/grid.rs`:
```rust
    #[test]
    fn bresenham_line_is_inclusive_and_connected() {
        let l = line(GridPos::new(0, 0), GridPos::new(4, 2));
        assert_eq!(l.first(), Some(&GridPos::new(0, 0)));
        assert_eq!(l.last(), Some(&GridPos::new(4, 2)));
        assert_eq!(l.len(), 5, "one cell per x step on a shallow line");
        for w in l.windows(2) {
            assert_eq!(w[0].distance(w[1]), 1, "consecutive cells are king-adjacent");
        }
        assert_eq!(line(GridPos::new(2, 2), GridPos::new(2, 2)), vec![GridPos::new(2, 2)]);
    }

    #[test]
    fn walls_block_sight_but_cover_does_not() {
        let g = Grid::from_ascii(".#.\n...\n.^.");
        // top row y=2: (0,2) . (1,2) # (2,2) .
        assert!(!g.has_line_of_sight(GridPos::new(0, 2), GridPos::new(2, 2)), "wall between");
        assert!(g.has_line_of_sight(GridPos::new(0, 0), GridPos::new(2, 0)), "cover between");
        assert!(g.has_line_of_sight(GridPos::new(0, 1), GridPos::new(2, 1)));
        assert!(g.has_line_of_sight(GridPos::new(0, 2), GridPos::new(1, 2)), "endpoints never block");
    }

    #[test]
    fn cover_is_directional() {
        let g = Grid::from_ascii("...\n.^.\n...");
        let target = GridPos::new(1, 1); // sheltered on its north side
        assert!(g.cover_against(target, GridPos::new(1, 2)), "shot from north");
        assert!(!g.cover_against(target, GridPos::new(1, 0)), "shot from south");
        assert!(!g.cover_against(target, GridPos::new(0, 1)), "shot from west");
        assert!(g.cover_against(target, GridPos::new(0, 2)), "diagonal from north-west crosses the north side");
        assert!(!g.cover_against(GridPos::new(0, 0), GridPos::new(2, 2)), "plain floor gives no cover");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test grid::`
Expected: compile error, `line` / `has_line_of_sight` / `cover_against` not found.

- [ ] **Step 3: Implement**

Add to `src/grid.rs` (outside `impl Grid`):
```rust
/// Bresenham line from `a` to `b`, inclusive of both ends, one cell per step.
pub fn line(a: GridPos, b: GridPos) -> Vec<GridPos> {
    let (mut x, mut y) = (a.x, a.y);
    let dx = (b.x - a.x).abs();
    let dy = -(b.y - a.y).abs();
    let sx = if a.x < b.x { 1 } else { -1 };
    let sy = if a.y < b.y { 1 } else { -1 };
    let mut err = dx + dy;
    let mut out = vec![a];
    while (x, y) != (b.x, b.y) {
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        out.push(GridPos::new(x, y));
    }
    out
}
```
And inside `impl Grid`:
```rust
    /// True when no sight-blocking cell lies strictly between `a` and `b`.
    pub fn has_line_of_sight(&self, a: GridPos, b: GridPos) -> bool {
        let cells = line(a, b);
        cells[1..cells.len().saturating_sub(1).max(1)]
            .iter()
            .all(|p| !self.get(*p).is_some_and(|c| c.blocks_sight))
    }

    /// True when the last step of the shooter→target line enters the target's cell
    /// through a side that cell shelters. Diagonal entries count either side.
    pub fn cover_against(&self, target: GridPos, shooter: GridPos) -> bool {
        let Some(cell) = self.get(target) else { return false };
        if !cell.cover.any() {
            return false;
        }
        let cells = line(shooter, target);
        if cells.len() < 2 {
            return false;
        }
        let prev = cells[cells.len() - 2];
        let dx = target.x - prev.x; // >0 : shot travelling east, enters through the west side
        let dy = target.y - prev.y; // >0 : shot travelling north, enters through the south side
        (dy < 0 && cell.cover.n) || (dy > 0 && cell.cover.s) || (dx < 0 && cell.cover.e) || (dx > 0 && cell.cover.w)
    }
```
Note on `has_line_of_sight`: when `a == b` the slice is `cells[1..1]`, empty, so it returns true.

- [ ] **Step 4: Run tests**

Run: `cargo test grid::`
Expected: 8 passed.

- [ ] **Step 5: Commit**

```bash
git add src/grid.rs
git commit -m "Add Bresenham line of sight and directional cover

Closes #<issue for Task 3>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 4: A* pathfinding

**Files:**
- Modify: `src/grid.rs`

**Interfaces:**
- Produces: `Grid::neighbours(p) -> Vec<(GridPos, u32)>` (8-way, no corner cutting, cost 10 straight / 14 diagonal); `Grid::find_path(from, to) -> Option<Vec<GridPos>>` (excludes `from`, ends at `to`; `None` if `to` is unwalkable or unreachable; `Some(vec![])` when `from == to`).

- [ ] **Step 1: Write the failing tests**

Add inside `mod tests`:
```rust
    #[test]
    fn path_straight_corridor() {
        let g = Grid::from_ascii("#####\n#...#\n#####");
        let p = g.find_path(GridPos::new(1, 1), GridPos::new(3, 1)).unwrap();
        assert_eq!(p, vec![GridPos::new(2, 1), GridPos::new(3, 1)]);
        assert_eq!(g.find_path(GridPos::new(1, 1), GridPos::new(1, 1)), Some(vec![]));
    }

    #[test]
    fn path_goes_around_walls() {
        let g = Grid::from_ascii("...\n.#.\n...");
        let p = g.find_path(GridPos::new(0, 0), GridPos::new(2, 2)).unwrap();
        assert_eq!(p.last(), Some(&GridPos::new(2, 2)));
        assert!(p.iter().all(|c| g.is_walkable(*c)));
        assert_eq!(p.len(), 4, "no corner cutting past the centre wall: e.g. (1,0) (2,0) (2,1) (2,2)");
    }

    #[test]
    fn no_corner_cutting() {
        // Moving (0,0)->(1,1) diagonally would squeeze between two walls; must go the long way or fail.
        let g = Grid::from_ascii("#.\n.#");
        assert_eq!(g.find_path(GridPos::new(0, 0), GridPos::new(1, 1)), None);
    }

    #[test]
    fn unreachable_or_unwalkable_goal_is_none() {
        let g = Grid::from_ascii(".#.\n.#.\n.#.");
        assert_eq!(g.find_path(GridPos::new(0, 0), GridPos::new(2, 0)), None);
        assert_eq!(g.find_path(GridPos::new(0, 0), GridPos::new(1, 0)), None, "goal is a wall");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test grid::path`
Expected: compile error, `find_path` not found.

- [ ] **Step 3: Implement**

Add to `impl Grid`:
```rust
    /// Walkable king-move neighbours with costs (10 orthogonal, 14 diagonal).
    /// Diagonals are only allowed when both adjacent orthogonal cells are walkable.
    pub fn neighbours(&self, p: GridPos) -> Vec<(GridPos, u32)> {
        let mut out = Vec::with_capacity(8);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let n = GridPos::new(p.x + dx, p.y + dy);
                if !self.is_walkable(n) {
                    continue;
                }
                let diagonal = dx != 0 && dy != 0;
                if diagonal
                    && !(self.is_walkable(GridPos::new(p.x + dx, p.y))
                        && self.is_walkable(GridPos::new(p.x, p.y + dy)))
                {
                    continue;
                }
                out.push((n, if diagonal { 14 } else { 10 }));
            }
        }
        out
    }

    /// A* path from `from` to `to`, excluding `from`, ending at `to`.
    pub fn find_path(&self, from: GridPos, to: GridPos) -> Option<Vec<GridPos>> {
        if !self.is_walkable(to) {
            return None;
        }
        let (mut path, _cost) = pathfinding::directed::astar::astar(
            &from,
            |p| self.neighbours(*p),
            |p| p.distance(to) as u32 * 10,
            |p| *p == to,
        )?;
        path.remove(0);
        Some(path)
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test grid::`
Expected: 12 passed.

- [ ] **Step 5: Commit**

```bash
git add src/grid.rs
git commit -m "Add 8-way A* pathfinding without corner cutting

Closes #<issue for Task 4>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 5: Hit rules and seeded RNG

**Files:**
- Create: `src/rules.rs`
- Modify: `src/main.rs` (add `mod rules;`)

**Interfaces:**
- Produces: `GameRng(pub ChaCha8Rng)` resource with `GameRng::seeded(u64)`; `hit_chance(distance: i32, range: i32, in_cover: bool) -> f32`; `roll_hit(&mut impl Rng, chance: f32) -> bool`; `roll_damage(&mut impl Rng, min: i32, max: i32) -> i32`.

- [ ] **Step 1: Write the failing tests**

`src/rules.rs`:
```rust
//! Combat arithmetic. Pure functions plus the single seeded RNG resource.

use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_chance_falls_with_distance_and_is_zero_out_of_range() {
        assert_eq!(hit_chance(0, 8, false), 0.9);
        assert!(hit_chance(4, 8, false) < hit_chance(1, 8, false));
        assert_eq!(hit_chance(9, 8, false), 0.0);
        assert!(hit_chance(8, 8, false) >= 0.1, "never below the floor while in range");
    }

    #[test]
    fn cover_halves_hit_chance() {
        assert_eq!(hit_chance(2, 8, true), hit_chance(2, 8, false) * 0.5);
    }

    #[test]
    fn seeded_rng_is_deterministic() {
        let mut a = GameRng::seeded(42);
        let mut b = GameRng::seeded(42);
        let ra: Vec<bool> = (0..20).map(|_| roll_hit(&mut a.0, 0.5)).collect();
        let rb: Vec<bool> = (0..20).map(|_| roll_hit(&mut b.0, 0.5)).collect();
        assert_eq!(ra, rb);
        assert!(ra.iter().any(|h| *h) && ra.iter().any(|h| !*h), "not degenerate");
    }

    #[test]
    fn damage_is_within_bounds() {
        let mut rng = GameRng::seeded(1);
        for _ in 0..200 {
            let d = roll_damage(&mut rng.0, 10, 25);
            assert!((10..=25).contains(&d));
        }
        assert!(!roll_hit(&mut rng.0, 0.0));
        assert!(roll_hit(&mut rng.0, 1.0));
    }
}
```
Add `mod rules;` to `src/main.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test rules::`
Expected: compile error.

- [ ] **Step 3: Implement**

Insert above the tests in `src/rules.rs`:
```rust
/// The one RNG the simulation uses, so a battle replays identically for a given seed.
#[derive(Resource)]
pub struct GameRng(pub ChaCha8Rng);

impl GameRng {
    pub fn seeded(seed: u64) -> Self {
        Self(ChaCha8Rng::seed_from_u64(seed))
    }
}

/// Probability (0..=1) of a shot landing. 0.9 point blank, −0.04 per cell, floor 0.1, halved by cover.
pub fn hit_chance(distance: i32, range: i32, in_cover: bool) -> f32 {
    if distance < 0 || distance > range {
        return 0.0;
    }
    let base = (0.9 - 0.04 * distance as f32).clamp(0.1, 0.95);
    if in_cover { base * 0.5 } else { base }
}

pub fn roll_hit(rng: &mut impl Rng, chance: f32) -> bool {
    chance > 0.0 && rng.random::<f32>() < chance
}

pub fn roll_damage(rng: &mut impl Rng, min: i32, max: i32) -> i32 {
    rng.random_range(min..=max)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test rules::`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add src/rules.rs src/main.rs
git commit -m "Add hit chance rules and seeded game RNG

Closes #<issue for Task 5>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 6: Unit components, app state, simulation plugin and movement

**Files:**
- Create: `src/unit.rs`, `src/state.rs`, `src/sim.rs`, `src/movement.rs`, `src/test_support.rs`
- Modify: `src/main.rs` (add mods)

**Interfaces:**
- Produces:
  - `unit::Faction { Player, Enemy }` (Component), `Health { current, max }` with `is_dead()`, `Weapon { range, damage_min, damage_max, cooldown_secs }`, `AttackCooldown(pub f32)` seconds remaining, `Order { None, MoveTo(GridPos), Attack(Entity) }` (Component, Default = None), `Speed(pub f32)` px/s, `SightRange(pub i32)`, `Path(pub VecDeque<GridPos>)`, `Dead`, `Selected` marker components; `unit_bundle(faction, pos) -> impl Bundle`.
  - `state::AppState { Loading, Playing, Paused }` (States, default Loading); `state::toggle_pause` system.
  - `sim::SimulationPlugin`; `sim::ShotFired { from: Vec2, to: Vec2, hit: bool }` (Message).
  - `movement::plan_paths`, `movement::follow_paths` systems.
  - `test_support::headless_app(grid, seed) -> App`, `test_support::tick(&mut App, secs)`, `test_support::spawn_unit(&mut App, faction, pos) -> Entity`.

- [ ] **Step 1: Write `src/unit.rs`**

```rust
//! Components shared by soldiers and enemies, and the bundle that spawns one.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::grid::GridPos;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Reflect)]
pub enum Faction {
    Player,
    Enemy,
}

#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct Health {
    pub current: i32,
    pub max: i32,
}

impl Health {
    pub fn is_dead(&self) -> bool {
        self.current <= 0
    }
}

#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct Weapon {
    /// Maximum Chebyshev distance in cells.
    pub range: i32,
    pub damage_min: i32,
    pub damage_max: i32,
    pub cooldown_secs: f32,
}

/// Seconds until this unit may fire again. Zero means ready.
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
pub struct AttackCooldown(pub f32);

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default, Reflect)]
pub enum Order {
    #[default]
    None,
    MoveTo(GridPos),
    Attack(Entity),
}

/// Movement speed in world pixels per second.
#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct Speed(pub f32);

/// How far (cells) this unit notices enemies. Used by AI only.
#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct SightRange(pub i32);

/// Remaining cells to walk through, front first. Present only while moving.
#[derive(Component, Debug, Default)]
pub struct Path(pub VecDeque<GridPos>);

#[derive(Component, Debug, Default)]
pub struct Dead;

#[derive(Component, Debug, Default)]
pub struct Selected;

pub fn unit_bundle(faction: Faction, pos: GridPos) -> impl Bundle {
    let (name, weapon) = match faction {
        Faction::Player => (
            "Soldier",
            Weapon { range: 8, damage_min: 20, damage_max: 35, cooldown_secs: 0.8 },
        ),
        Faction::Enemy => (
            "Raider",
            Weapon { range: 7, damage_min: 10, damage_max: 25, cooldown_secs: 1.0 },
        ),
    };
    (
        Name::new(name),
        faction,
        pos,
        Health { current: 100, max: 100 },
        weapon,
        AttackCooldown(0.0),
        Order::None,
        Speed(160.0),
        SightRange(9),
        Transform::from_translation(pos.to_world().extend(10.0)),
    )
}
```

- [ ] **Step 2: Write `src/state.rs`**

```rust
//! Top-level app state and the pause toggle. Mission end is added in a later task.

use bevy::prelude::*;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    Loading,
    Playing,
    Paused,
}

pub fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
) {
    if !keys.just_pressed(KeyCode::Space) {
        return;
    }
    match state.get() {
        AppState::Playing => next.set(AppState::Paused),
        AppState::Paused => next.set(AppState::Playing),
        AppState::Loading => {}
    }
}
```

- [ ] **Step 3: Write `src/sim.rs`**

```rust
//! The headless-capable simulation: everything that changes game state.
//! Presentation (sprites, camera, input, UI, audio) lives in `presentation.rs`.

use bevy::prelude::*;

use crate::movement;
use crate::rules::GameRng;
use crate::state::{self, AppState};

/// Emitted once per shot so presentation can draw a tracer and play a sound.
#[derive(Message, Clone, Copy, Debug)]
pub struct ShotFired {
    pub from: Vec2,
    pub to: Vec2,
    pub hit: bool,
}

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ShotFired>();
        if !app.world().contains_resource::<GameRng>() {
            app.insert_resource(GameRng::seeded(0));
        }
        app.add_systems(
            Update,
            (movement::plan_paths, movement::follow_paths)
                .chain()
                .run_if(in_state(AppState::Playing)),
        );
        app.add_systems(
            Update,
            state::toggle_pause.run_if(in_state(AppState::Playing).or(in_state(AppState::Paused))),
        );
    }
}
```

- [ ] **Step 4: Write `src/test_support.rs`**

```rust
//! Helpers for headless ECS tests: a minimal App with the simulation, a fixed grid and a seed.

#![cfg(test)]

use std::time::Duration;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use crate::grid::{Grid, GridPos};
use crate::rules::GameRng;
use crate::sim::SimulationPlugin;
use crate::state::AppState;
use crate::unit::{unit_bundle, Faction};

pub fn headless_app(grid: Grid, seed: u64) -> App {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.insert_resource(Time::<()>::default());
    app.insert_resource(grid);
    app.insert_resource(GameRng::seeded(seed));
    app.insert_state(AppState::Playing);
    app.add_plugins(SimulationPlugin);
    app
}

/// Advance the fake clock by `secs` and run one frame.
pub fn tick(app: &mut App, secs: f32) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f32(secs));
    app.update();
}

pub fn spawn_unit(app: &mut App, faction: Faction, pos: GridPos) -> Entity {
    app.world_mut().spawn(unit_bundle(faction, pos)).id()
}

pub fn set_order(app: &mut App, unit: Entity, order: crate::unit::Order) {
    let mut e = app.world_mut().entity_mut(unit);
    *e.get_mut::<crate::unit::Order>().unwrap() = order;
    e.remove::<crate::unit::Path>();
}

pub fn grid_pos(app: &App, unit: Entity) -> GridPos {
    *app.world().entity(unit).get::<GridPos>().unwrap()
}

pub fn order(app: &App, unit: Entity) -> crate::unit::Order {
    *app.world().entity(unit).get::<crate::unit::Order>().unwrap()
}
```

- [ ] **Step 5: Write the failing movement tests**

`src/movement.rs`:
```rust
//! Turns `Order::MoveTo` into a `Path`, then walks units along it.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::grid::{Grid, GridPos};
use crate::unit::{Dead, Order, Path, Speed};

#[cfg(test)]
mod tests {
    use crate::grid::{Grid, GridPos};
    use crate::test_support::*;
    use crate::unit::{Faction, Order, Path};

    #[test]
    fn unit_walks_to_goal_then_clears_order() {
        let mut app = headless_app(Grid::new(10, 3), 1);
        let u = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 1));
        set_order(&mut app, u, Order::MoveTo(GridPos::new(3, 1)));
        // 160 px/s over 3 cells of 64 px = 1.2 s. Give it 1.5 s in 0.1 s ticks.
        for _ in 0..15 {
            tick(&mut app, 0.1);
        }
        assert_eq!(grid_pos(&app, u), GridPos::new(3, 1));
        assert_eq!(order(&app, u), Order::None);
        assert!(app.world().entity(u).get::<Path>().is_none());
        let t = app.world().entity(u).get::<bevy::prelude::Transform>().unwrap();
        assert_eq!(t.translation.truncate(), GridPos::new(3, 1).to_world());
    }

    #[test]
    fn unreachable_goal_clears_order_immediately() {
        let mut app = headless_app(Grid::from_ascii(".#."), 1);
        let u = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        set_order(&mut app, u, Order::MoveTo(GridPos::new(2, 0)));
        tick(&mut app, 0.1);
        assert_eq!(order(&app, u), Order::None);
        assert_eq!(grid_pos(&app, u), GridPos::new(0, 0));
    }

    #[test]
    fn occupied_cell_stops_the_mover() {
        let mut app = headless_app(Grid::new(5, 1), 1);
        let mover = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        let _blocker = spawn_unit(&mut app, Faction::Enemy, GridPos::new(1, 0));
        set_order(&mut app, mover, Order::MoveTo(GridPos::new(2, 0)));
        for _ in 0..10 {
            tick(&mut app, 0.1);
        }
        assert_eq!(grid_pos(&app, mover), GridPos::new(0, 0));
        assert_eq!(order(&app, mover), Order::None);
    }
}
```
Add to `src/main.rs`: `mod movement; mod sim; mod state; mod test_support; mod unit;` (one per line, alphabetical with the others).

- [ ] **Step 6: Run tests to verify they fail**

Run: `cargo test movement::`
Expected: compile error, `plan_paths` / `follow_paths` not found (referenced from `sim.rs`).

- [ ] **Step 7: Implement the movement systems**

Insert above the tests in `src/movement.rs`:
```rust
/// Gives every unit with a fresh `MoveTo` order a `Path`, or cancels the order if there is none.
pub fn plan_paths(
    mut commands: Commands,
    grid: Res<Grid>,
    mut units: Query<(Entity, &GridPos, &mut Order), (Without<Path>, Without<Dead>)>,
) {
    for (entity, pos, mut order) in &mut units {
        let Order::MoveTo(goal) = *order else { continue };
        match grid.find_path(*pos, goal) {
            Some(path) if !path.is_empty() => {
                commands.entity(entity).insert(Path(path.into()));
            }
            _ => *order = Order::None,
        }
    }
}

/// Moves each unit toward the next cell of its `Path`. Arriving updates `GridPos`.
/// A cell occupied by another living unit aborts the path (and a `MoveTo` order).
pub fn follow_paths(
    mut commands: Commands,
    time: Res<Time>,
    mut set: ParamSet<(
        Query<(Entity, &GridPos), Without<Dead>>,
        Query<(Entity, &mut GridPos, &mut Transform, &mut Path, &mut Order, &Speed), Without<Dead>>,
    )>,
) {
    let occupied: HashMap<GridPos, Entity> = set.p0().iter().map(|(e, p)| (*p, e)).collect();
    let dt = time.delta_secs();

    for (entity, mut pos, mut transform, mut path, mut order, speed) in &mut set.p1() {
        let Some(&next) = path.0.front() else {
            commands.entity(entity).remove::<Path>();
            if matches!(*order, Order::MoveTo(_)) {
                *order = Order::None;
            }
            continue;
        };
        if occupied.get(&next).is_some_and(|other| *other != entity) {
            commands.entity(entity).remove::<Path>();
            if matches!(*order, Order::MoveTo(_)) {
                *order = Order::None;
            }
            continue;
        }
        let target = next.to_world();
        let current = transform.translation.truncate();
        let delta = target - current;
        let step = speed.0 * dt;
        if delta.length() > 0.001 {
            transform.rotation = Quat::from_rotation_z(delta.to_angle());
        }
        if delta.length() <= step {
            transform.translation.x = target.x;
            transform.translation.y = target.y;
            *pos = next;
            path.0.pop_front();
        } else {
            let movement = delta.normalize() * step;
            transform.translation.x += movement.x;
            transform.translation.y += movement.y;
        }
    }
}
```

- [ ] **Step 8: Run tests**

Run: `cargo test`
Expected: all pass (movement 3, grid 12, rules 4).

- [ ] **Step 9: Commit**

```bash
git add src/
git commit -m "Add unit components, app state, simulation plugin and movement

Closes #<issue for Task 6>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 7: Combat resolution and death

**Files:**
- Create: `src/combat.rs`
- Modify: `src/sim.rs` (system chain), `src/main.rs` (`mod combat;`)

**Interfaces:**
- Consumes: `Grid::has_line_of_sight`, `Grid::cover_against`, `Grid::find_path`, `rules::*`, `unit::*`, `sim::ShotFired`.
- Produces: `combat::resolve_attacks`, `combat::apply_death` systems. Dead units get the `Dead` marker, `Order::None`, and lose their `Path`.

- [ ] **Step 1: Write the failing tests**

`src/combat.rs`:
```rust
//! Executes `Order::Attack`: closes distance if needed, then fires on cooldown.
//! Shots are instant-hit; `ShotFired` lets presentation draw and sound them.

use bevy::prelude::*;

use crate::grid::{Grid, GridPos};
use crate::rules::{self, GameRng};
use crate::sim::ShotFired;
use crate::unit::{AttackCooldown, Dead, Health, Order, Path, Weapon};

#[cfg(test)]
mod tests {
    use crate::grid::{Grid, GridPos};
    use crate::test_support::*;
    use crate::unit::{Dead, Faction, Health, Order, Path};

    fn health(app: &bevy::prelude::App, e: bevy::prelude::Entity) -> i32 {
        app.world().entity(e).get::<Health>().unwrap().current
    }

    fn set_health(app: &mut bevy::prelude::App, e: bevy::prelude::Entity, hp: i32) {
        app.world_mut().entity_mut(e).get_mut::<Health>().unwrap().current = hp;
    }

    #[test]
    fn adjacent_attacker_damages_target_on_cooldown() {
        let mut app = headless_app(Grid::new(5, 1), 7);
        let a = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        let t = spawn_unit(&mut app, Faction::Enemy, GridPos::new(1, 0));
        set_health(&mut app, a, 100_000); // enemy AI returns fire; keep the attacker alive
        set_health(&mut app, t, 100_000);
        set_order(&mut app, a, Order::Attack(t));
        tick(&mut app, 0.1); // first shot is immediate (cooldown starts at 0)
        let after_first = health(&app, t);
        tick(&mut app, 0.1); // cooldown 0.8 s not elapsed: no second shot
        assert_eq!(health(&app, t), after_first);
        for _ in 0..20 {
            tick(&mut app, 0.5);
        }
        assert!(health(&app, t) < 100_000 - 5 * 20, "many shots landed over 10 s");
        assert_eq!(order(&app, a), Order::Attack(t), "keeps attacking a living target");
    }

    #[test]
    fn attacker_moves_until_it_has_line_of_sight() {
        // Wall column at x=2 with a gap at y=0; target at (4,2) is hidden from (0,2).
        let g = Grid::from_ascii("..#..\n..#..\n.....");
        let mut app = headless_app(g, 3);
        let a = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 2));
        let t = spawn_unit(&mut app, Faction::Enemy, GridPos::new(4, 2));
        set_health(&mut app, a, 100_000);
        set_health(&mut app, t, 100_000);
        set_order(&mut app, a, Order::Attack(t));
        tick(&mut app, 0.1);
        assert!(app.world().entity(a).get::<Path>().is_some(), "no LOS: starts walking");
        for _ in 0..40 {
            tick(&mut app, 0.1);
        }
        assert!(health(&app, t) < 100_000, "eventually gets a shot off");
        assert!(app.world().entity(a).get::<Path>().is_none(), "stops walking once in range with LOS");
    }

    #[test]
    fn cover_reduces_damage_taken() {
        fn damage_after(seed: u64, target_cell_art: &str) -> i32 {
            // Shooter at (2,0) fires north at a target at (2,2); row art decides the target's cell.
            let art = format!("..{}..\n.....\n.....", target_cell_art);
            let mut app = headless_app(Grid::from_ascii(&art), seed);
            let a = spawn_unit(&mut app, Faction::Player, GridPos::new(2, 0));
            let t = spawn_unit(&mut app, Faction::Enemy, GridPos::new(2, 2));
            set_health(&mut app, a, 1_000_000);
            set_health(&mut app, t, 1_000_000);
            {
                let mut e = app.world_mut().entity_mut(a);
                e.get_mut::<crate::unit::Weapon>().unwrap().cooldown_secs = 0.01;
            }
            set_order(&mut app, a, Order::Attack(t));
            for _ in 0..400 {
                tick(&mut app, 0.02);
            }
            1_000_000 - health(&app, t)
        }
        let open = damage_after(11, ".");
        let covered = damage_after(11, "v"); // sheltered on its south side, the shooter's side
        let wrong_side = damage_after(11, "^");
        assert!(covered < open * 7 / 10, "cover: {covered} vs open: {open}");
        assert!((wrong_side - open).abs() < open / 5, "cover facing away does nothing: {wrong_side} vs {open}");
    }

    #[test]
    fn killing_the_target_marks_it_dead_and_clears_the_order() {
        let mut app = headless_app(Grid::new(3, 1), 5);
        let a = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        let t = spawn_unit(&mut app, Faction::Enemy, GridPos::new(1, 0));
        set_health(&mut app, a, 100_000);
        set_health(&mut app, t, 1);
        set_order(&mut app, a, Order::Attack(t));
        for _ in 0..30 {
            tick(&mut app, 0.5);
        }
        assert!(app.world().entity(t).get::<Dead>().is_some());
        assert_eq!(order(&app, a), Order::None);
        assert_eq!(order(&app, t), Order::None);
    }
}
```
Add `mod combat;` to `src/main.rs`.

- [ ] **Step 2: Wire the systems into `src/sim.rs`**

Replace the movement `add_systems` call with:
```rust
        app.add_systems(
            Update,
            (
                movement::plan_paths,
                movement::follow_paths,
                combat::resolve_attacks,
                combat::apply_death,
            )
                .chain()
                .run_if(in_state(AppState::Playing)),
        );
```
and add `use crate::combat;` to the imports.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test combat::`
Expected: compile error, `resolve_attacks` / `apply_death` not found.

- [ ] **Step 4: Implement**

Insert above the tests in `src/combat.rs`:
```rust
pub fn resolve_attacks(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    mut rng: ResMut<GameRng>,
    mut shots: MessageWriter<ShotFired>,
    mut attackers: Query<
        (Entity, &GridPos, &Weapon, &mut AttackCooldown, &mut Order, &mut Transform, Option<&Path>),
        Without<Dead>,
    >,
    mut targets: Query<(&GridPos, &mut Health), Without<Dead>>,
) {
    let dt = time.delta_secs();
    for (entity, pos, weapon, mut cooldown, mut order, mut transform, path) in &mut attackers {
        cooldown.0 = (cooldown.0 - dt).max(0.0);
        let Order::Attack(target) = *order else { continue };
        let Ok((target_pos, mut health)) = targets.get_mut(target) else {
            // Target dead or gone.
            *order = Order::None;
            commands.entity(entity).remove::<Path>();
            continue;
        };
        let target_pos = *target_pos;
        let distance = pos.distance(target_pos);
        let can_fire = distance <= weapon.range && grid.has_line_of_sight(*pos, target_pos);
        if !can_fire {
            if path.is_none() {
                match grid.find_path(*pos, target_pos) {
                    Some(p) if !p.is_empty() => {
                        commands.entity(entity).insert(Path(p.into()));
                    }
                    _ => *order = Order::None,
                }
            }
            continue;
        }
        if path.is_some() {
            commands.entity(entity).remove::<Path>();
        }
        if cooldown.0 > 0.0 {
            continue;
        }
        cooldown.0 = weapon.cooldown_secs;
        let aim = target_pos.to_world() - pos.to_world();
        transform.rotation = Quat::from_rotation_z(aim.to_angle());
        let in_cover = grid.cover_against(target_pos, *pos);
        let chance = rules::hit_chance(distance, weapon.range, in_cover);
        let hit = rules::roll_hit(&mut rng.0, chance);
        if hit {
            health.current -= rules::roll_damage(&mut rng.0, weapon.damage_min, weapon.damage_max);
        }
        shots.write(ShotFired {
            from: pos.to_world(),
            to: target_pos.to_world(),
            hit,
        });
    }
}

pub fn apply_death(
    mut commands: Commands,
    mut units: Query<(Entity, &Health, &mut Order), Without<Dead>>,
) {
    for (entity, health, mut order) in &mut units {
        if health.is_dead() {
            *order = Order::None;
            commands.entity(entity).insert(Dead).remove::<Path>();
        }
    }
}
```
Note: `attackers` and `targets` both read `GridPos` immutably and touch disjoint mutable components, so Bevy accepts them side by side. A unit attacking itself is impossible because `targets.get_mut(target)` would alias `attackers` for the same entity only if `target == entity`, which the input layer never issues; the AI never targets its own faction.

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: all pass, including the 4 combat tests. If `cover_reduces_damage_taken` is flaky for the chosen seed, the shot count is ~400 so the halving is statistically robust; check `cover_against` orientation before touching thresholds.

- [ ] **Step 6: Commit**

```bash
git add src/
git commit -m "Add instant-hit combat with cover, cooldowns and death

Closes #<issue for Task 7>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 8: Enemy AI

**Files:**
- Create: `src/ai.rs`
- Modify: `src/sim.rs`, `src/main.rs`

**Interfaces:**
- Produces: `ai::enemy_think` system. Enemies with `Order::None` that see a player unit either attack it (if already in cover against it, or no cover nearby) or move to the nearest cover cell within 3 cells that has LOS to it.

- [ ] **Step 1: Write the failing tests**

`src/ai.rs`:
```rust
//! Enemy behaviour: idle → see a player → take nearby cover → shoot.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::grid::{Grid, GridPos};
use crate::unit::{Dead, Faction, Order, Path, SightRange};

#[cfg(test)]
mod tests {
    use crate::grid::{Grid, GridPos};
    use crate::test_support::*;
    use crate::unit::{Faction, Order};

    #[test]
    fn enemy_in_the_open_moves_to_cover_facing_the_player() {
        // Player at (0,1) west of the enemy at (5,1). Cover '<' at (4,1) shelters its west side.
        let g = Grid::from_ascii(".......\n....<..\n.......");
        let mut app = headless_app(g, 1);
        let _p = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 1));
        let e = spawn_unit(&mut app, Faction::Enemy, GridPos::new(5, 1));
        tick(&mut app, 0.1);
        assert_eq!(order(&app, e), Order::MoveTo(GridPos::new(4, 1)));
    }

    #[test]
    fn enemy_with_no_cover_nearby_attacks_directly() {
        let mut app = headless_app(Grid::new(8, 1), 1);
        let p = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        let e = spawn_unit(&mut app, Faction::Enemy, GridPos::new(5, 0));
        tick(&mut app, 0.1);
        assert_eq!(order(&app, e), Order::Attack(p));
    }

    #[test]
    fn enemy_already_in_cover_attacks() {
        let g = Grid::from_ascii("....<..");
        let mut app = headless_app(g, 1);
        let p = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        let e = spawn_unit(&mut app, Faction::Enemy, GridPos::new(4, 0));
        tick(&mut app, 0.1);
        assert_eq!(order(&app, e), Order::Attack(p));
    }

    #[test]
    fn enemy_ignores_unseen_or_distant_players() {
        let g = Grid::from_ascii("..#............"); // 15 cells wide
        let mut app = headless_app(g, 1);
        let _behind_wall = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        let e = spawn_unit(&mut app, Faction::Enemy, GridPos::new(4, 0));
        let _too_far = spawn_unit(&mut app, Faction::Player, GridPos::new(14, 0)); // 10 cells > sight 9
        tick(&mut app, 0.1);
        assert_eq!(order(&app, e), Order::None);
    }

    #[test]
    fn players_are_not_driven_by_ai() {
        let mut app = headless_app(Grid::new(5, 1), 1);
        let p = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        let _e = spawn_unit(&mut app, Faction::Enemy, GridPos::new(3, 0));
        tick(&mut app, 0.1);
        assert_eq!(order(&app, p), Order::None);
    }
}
```
Add `mod ai;` to `src/main.rs`. In `src/sim.rs` add `use crate::ai;` and put `ai::enemy_think,` as the FIRST entry of the chained tuple (before `movement::plan_paths`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test ai::`
Expected: compile error, `enemy_think` not found.

- [ ] **Step 3: Implement**

Insert above the tests in `src/ai.rs`:
```rust
const COVER_SEARCH_RADIUS: i32 = 3;

pub fn enemy_think(
    mut commands: Commands,
    grid: Res<Grid>,
    everyone: Query<(Entity, &GridPos, &Faction), Without<Dead>>,
    mut enemies: Query<(Entity, &GridPos, &SightRange, &Faction, &mut Order), Without<Dead>>,
) {
    let players: Vec<(Entity, GridPos)> = everyone
        .iter()
        .filter(|(_, _, f)| **f == Faction::Player)
        .map(|(e, p, _)| (e, *p))
        .collect();
    let occupied: HashSet<GridPos> = everyone.iter().map(|(_, p, _)| *p).collect();

    for (entity, pos, sight, faction, mut order) in &mut enemies {
        if *faction != Faction::Enemy || *order != Order::None {
            continue;
        }
        let Some((target, target_pos)) = players
            .iter()
            .filter(|(_, p)| pos.distance(*p) <= sight.0 && grid.has_line_of_sight(*pos, *p))
            .min_by_key(|(_, p)| pos.distance(*p))
            .copied()
        else {
            continue;
        };

        if grid.cover_against(*pos, target_pos) {
            *order = Order::Attack(target);
            continue;
        }

        let mut best: Option<(i32, GridPos)> = None;
        for dx in -COVER_SEARCH_RADIUS..=COVER_SEARCH_RADIUS {
            for dy in -COVER_SEARCH_RADIUS..=COVER_SEARCH_RADIUS {
                let c = GridPos::new(pos.x + dx, pos.y + dy);
                if !grid.is_walkable(c)
                    || occupied.contains(&c)
                    || !grid.cover_against(c, target_pos)
                    || !grid.has_line_of_sight(c, target_pos)
                {
                    continue;
                }
                let d = pos.distance(c);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, c));
                }
            }
        }

        *order = match best {
            Some((_, cover)) => Order::MoveTo(cover),
            None => Order::Attack(target),
        };
        commands.entity(entity).remove::<Path>();
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: all pass (5 new AI tests). The combat test `attacker_moves_until_it_has_line_of_sight` still passes because its enemy has no LOS to the player at the start and the player reaches firing position first.

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "Add enemy AI: spot player, take cover, return fire

Closes #<issue for Task 8>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 9: Mission end and pause

**Files:**
- Modify: `src/state.rs`, `src/sim.rs`

**Interfaces:**
- Produces: `state::MissionOutcome { Victory, Defeat }` resource (absent until the mission ends); `state::check_mission_end` system that inserts it and switches to `AppState::Paused`.

- [ ] **Step 1: Write the failing tests**

Add to `src/state.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Grid, GridPos};
    use crate::test_support::*;
    use crate::unit::{Faction, Health};

    fn kill(app: &mut App, e: Entity) {
        app.world_mut().entity_mut(e).get_mut::<Health>().unwrap().current = 0;
    }

    #[test]
    fn all_enemies_dead_is_victory_and_pauses() {
        let mut app = headless_app(Grid::new(4, 1), 1);
        let _p = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        let e = spawn_unit(&mut app, Faction::Enemy, GridPos::new(3, 0));
        tick(&mut app, 0.1);
        assert!(app.world().get_resource::<MissionOutcome>().is_none());
        kill(&mut app, e);
        tick(&mut app, 0.1);
        tick(&mut app, 0.1); // state transition applies next frame
        assert_eq!(*app.world().resource::<MissionOutcome>(), MissionOutcome::Victory);
        assert_eq!(*app.world().resource::<State<AppState>>().get(), AppState::Paused);
    }

    #[test]
    fn all_players_dead_is_defeat() {
        let mut app = headless_app(Grid::new(4, 1), 1);
        let p = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        let _e = spawn_unit(&mut app, Faction::Enemy, GridPos::new(3, 0));
        kill(&mut app, p);
        tick(&mut app, 0.1);
        assert_eq!(*app.world().resource::<MissionOutcome>(), MissionOutcome::Defeat);
    }

    #[test]
    fn a_map_with_only_one_faction_never_ends() {
        let mut app = headless_app(Grid::new(4, 1), 1);
        let _p = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        tick(&mut app, 0.1);
        assert!(app.world().get_resource::<MissionOutcome>().is_none());
    }
}
```

- [ ] **Step 2: Wire into `src/sim.rs`**

Append `state::check_mission_end,` as the LAST entry of the chained tuple (after `combat::apply_death`).

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test state::`
Expected: compile error, `MissionOutcome` / `check_mission_end` not found.

- [ ] **Step 4: Implement**

Add to `src/state.rs` (above the tests):
```rust
use crate::unit::{Dead, Faction};

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MissionOutcome {
    Victory,
    Defeat,
}

/// Ends the mission when a faction that had units has none left alive.
pub fn check_mission_end(
    mut commands: Commands,
    units: Query<(&Faction, Option<&Dead>)>,
    outcome: Option<Res<MissionOutcome>>,
    mut next: ResMut<NextState<AppState>>,
) {
    if outcome.is_some() {
        return;
    }
    let wiped_out = |faction: Faction| {
        let mut any = false;
        let mut alive = false;
        for (f, dead) in &units {
            if *f == faction {
                any = true;
                alive |= dead.is_none();
            }
        }
        any && !alive
    };
    let result = if wiped_out(Faction::Player) {
        MissionOutcome::Defeat
    } else if wiped_out(Faction::Enemy) {
        MissionOutcome::Victory
    } else {
        return;
    };
    commands.insert_resource(result);
    next.set(AppState::Paused);
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: all pass. Total so far: 12 grid + 4 rules + 3 movement + 4 combat + 5 ai + 3 state = 31.

- [ ] **Step 6: Commit**

```bash
git add src/
git commit -m "End the mission when a side is wiped out

Closes #<issue for Task 9>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 10: Art, sound, tileset and the mission map

**Files:**
- Create: `assets/sprites/soldier.png`, `assets/sprites/enemy.png`, `assets/tiles/tileset.png`, `assets/maps/wasteland.tsx`, `assets/maps/mission01.tmx`, `assets/audio/shot.ogg`, `assets/audio/hit.ogg`, `CREDITS.md`, `src/map.rs`
- Modify: `src/main.rs` (`mod map;`)

**Interfaces:**
- Consumes: `Grid`, `GridPos`, `CoverSides`, `Faction`.
- Produces: `map::grid_from_tiled(&tiled::Map) -> Grid`, `map::spawns_from_tiled(&tiled::Map) -> Vec<(Faction, GridPos)>`. Tile properties read: `walkable` (bool), `blocks_sight` (bool), `cover` (string of `n e s w`). Object property read: `faction` = `player` | `enemy`. Tileset ids: 0 ground, 1 wall, 2 sandbag sheltering n+s, 3 sandbag sheltering e+w.

- [ ] **Step 1: Download the Kenney packs (human step if the executor cannot fetch)**

Download these CC0 packs from kenney.nl and unzip them into the scratchpad directory:
- https://kenney.nl/assets/top-down-shooter (characters + tiles)
- https://kenney.nl/assets/impact-sounds (hit sounds)
- https://kenney.nl/assets/sci-fi-sounds (laser/shot sounds)

```bash
S=/tmp/claude-1000/-home-mortlock-src/302ad263-77ce-466c-8f05-4a78fe90fc83/scratchpad/kenney
mkdir -p "$S" && cd "$S"
# If direct download works (Kenney serves zips from kenney.nl/media/pages/assets/...), use curl -L; otherwise John downloads in a browser and drops the zips here.
ls *.zip
for z in *.zip; do unzip -oq "$z" -d "${z%.zip}"; done
find . -maxdepth 3 -type d | head -40
```
Expected: directories containing `PNG/` (characters in subfolders such as `Soldier 1/`, `Robot 1/`, and a `Tiles/` folder of 64×64 PNGs) and audio `.ogg` files.

- [ ] **Step 2: Pick and copy the sprites and sounds**

Look at the character folders (use the Read tool on a few PNGs to see them). Pick one figure with a gun for the player and a clearly different one for enemies. Kenney's files are named like `soldier1_gun.png`, `robot1_gun.png`, `hitman1_gun.png`.
```bash
cd ~/src/wasteland-tactics
mkdir -p assets/sprites assets/tiles assets/maps assets/audio
cp "$S"/top-down-shooter*/PNG/Soldier\ 1/soldier1_gun.png assets/sprites/soldier.png
cp "$S"/top-down-shooter*/PNG/Robot\ 1/robot1_gun.png assets/sprites/enemy.png
# Any short punchy sounds will do; substitute names that exist in the packs.
cp "$S"/sci-fi-sounds*/Audio/laserSmall_000.ogg assets/audio/shot.ogg
cp "$S"/impact-sounds*/Audio/impactMetal_medium_000.ogg assets/audio/hit.ogg
identify assets/sprites/*.png assets/tiles/*.png 2>/dev/null; file assets/audio/*.ogg
```
Expected: two PNGs roughly 50×45 px, two OGG files. If a path differs, `find "$S" -name '*gun.png' | head` and pick equivalents.

- [ ] **Step 3: Build the 4-tile tileset**

Choose three tiles from the pack's `Tiles/` folder by viewing them: a plain ground tile, a solid wall/brick tile, and a sandbag or crate tile. Then:
```bash
cd ~/src/wasteland-tactics
GROUND="$S/top-down-shooter*/PNG/Tiles/tile_01.png"   # replace with the chosen files
WALL="$S/top-down-shooter*/PNG/Tiles/tile_08.png"
SANDBAG="$S/top-down-shooter*/PNG/Tiles/tile_22.png"
convert $GROUND $WALL $SANDBAG \( $SANDBAG -rotate 90 \) -resize 64x64\! +append assets/tiles/tileset.png
identify assets/tiles/tileset.png
```
Expected: `assets/tiles/tileset.png PNG 256x64 ...`. Read the PNG to confirm the order is ground, wall, sandbag, rotated sandbag.

- [ ] **Step 4: Write the tileset `assets/maps/wasteland.tsx`**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<tileset version="1.10" tiledversion="1.10.2" name="wasteland" tilewidth="64" tileheight="64" tilecount="4" columns="4">
 <image source="../tiles/tileset.png" width="256" height="64"/>
 <tile id="0">
  <properties>
   <property name="walkable" type="bool" value="true"/>
  </properties>
 </tile>
 <tile id="1">
  <properties>
   <property name="walkable" type="bool" value="false"/>
   <property name="blocks_sight" type="bool" value="true"/>
  </properties>
 </tile>
 <tile id="2">
  <properties>
   <property name="walkable" type="bool" value="true"/>
   <property name="cover" value="ns"/>
  </properties>
 </tile>
 <tile id="3">
  <properties>
   <property name="walkable" type="bool" value="true"/>
   <property name="cover" value="ew"/>
  </properties>
 </tile>
</tileset>
```

- [ ] **Step 5: Generate `assets/maps/mission01.tmx` from ASCII**

Save this script as `tools/gen_map.py` (committed, so the map can be regenerated) and run it. `#` wall, `=` sandbag sheltering north/south, `|` sandbag sheltering east/west, `P` player spawn, `E` enemy spawn, `.` floor. First line is the top row.
```python
#!/usr/bin/env python3
"""Generates assets/maps/mission01.tmx from the ASCII layout below. Edit in Tiled afterwards if you like."""
ART = """\
####################
#..................#
#......#........E..#
#......#....=...E..#
#......#.......E...#
#..........###.....#
#..|...............#
#..|.......#.......#
#..........#...=...#
#....###...#.......#
#..................#
#.........|........#
#..P.P.............#
#..P.P.............#
####################"""
TILE = 64
GID = {".": 1, "#": 2, "=": 3, "|": 4}  # firstgid 1 + tile id
rows = ART.splitlines()
h, w = len(rows), len(rows[0])
ground = ",".join("1" for _ in range(w * h))
walls = ",".join(str(GID.get(c, 0)) if c in "#=|" else "0" for r in rows for c in r)
objects, oid = [], 1
for y, row in enumerate(rows):
    for x, c in enumerate(row):
        if c in "PE":
            faction = "player" if c == "P" else "enemy"
            objects.append(
                f'  <object id="{oid}" name="{faction}{oid}" x="{x * TILE + TILE // 2}" y="{y * TILE + TILE // 2}">\n'
                f'   <point/>\n   <properties>\n    <property name="faction" value="{faction}"/>\n   </properties>\n  </object>'
            )
            oid += 1
tmx = f"""<?xml version="1.0" encoding="UTF-8"?>
<map version="1.10" tiledversion="1.10.2" orientation="orthogonal" renderorder="right-down" width="{w}" height="{h}" tilewidth="{TILE}" tileheight="{TILE}" infinite="0" nextlayerid="4" nextobjectid="{oid}">
 <tileset firstgid="1" source="wasteland.tsx"/>
 <layer id="1" name="ground" width="{w}" height="{h}">
  <data encoding="csv">
{ground}
</data>
 </layer>
 <layer id="2" name="walls" width="{w}" height="{h}">
  <data encoding="csv">
{walls}
</data>
 </layer>
 <objectgroup id="3" name="spawns">
{chr(10).join(objects)}
 </objectgroup>
</map>
"""
open("assets/maps/mission01.tmx", "w").write(tmx)
print(f"wrote {w}x{h} map with {oid - 1} spawns")
```
```bash
cd ~/src/wasteland-tactics && python3 tools/gen_map.py
```
Expected: `wrote 20x15 map with 7 spawns`.

- [ ] **Step 6: Write `CREDITS.md`**

```markdown
# Asset credits

All art and audio in `assets/` is CC0 1.0 Universal (public domain) from
[Kenney](https://kenney.nl), used with thanks.

| Files | Pack | Licence |
|---|---|---|
| `assets/sprites/soldier.png`, `assets/sprites/enemy.png`, `assets/tiles/tileset.png` | [Top-down Shooter](https://kenney.nl/assets/top-down-shooter) | CC0 1.0 |
| `assets/audio/hit.ogg` | [Impact Sounds](https://kenney.nl/assets/impact-sounds) | CC0 1.0 |
| `assets/audio/shot.ogg` | [Sci-Fi Sounds](https://kenney.nl/assets/sci-fi-sounds) | CC0 1.0 |

`assets/tiles/tileset.png` is a montage of four tiles from the Top-down Shooter
pack (one rotated), built by `convert ... +append`.
`assets/maps/mission01.tmx` is generated by `tools/gen_map.py`.
```

- [ ] **Step 7: Write the failing map-loading tests**

`src/map.rs`:
```rust
//! Turns a Tiled map into the `Grid` resource and unit spawn list.
//! The pure functions here are tested by loading `assets/maps/mission01.tmx` from disk.

use bevy::prelude::*;
use tiled::PropertyValue;

use crate::grid::{CoverSides, Grid, GridPos};
use crate::unit::Faction;

#[cfg(test)]
mod tests {
    use super::*;

    fn load() -> tiled::Map {
        tiled::Loader::new().load_tmx_map("assets/maps/mission01.tmx").expect("mission01.tmx loads")
    }

    #[test]
    fn grid_matches_the_generated_layout() {
        let grid = grid_from_tiled(&load());
        assert_eq!((grid.width(), grid.height()), (20, 15));
        assert!(!grid.is_walkable(GridPos::new(0, 0)), "bottom-left corner is wall");
        assert!(!grid.is_walkable(GridPos::new(19, 14)), "top-right corner is wall");
        assert!(grid.is_walkable(GridPos::new(1, 1)));
        assert!(grid.get(GridPos::new(0, 0)).unwrap().blocks_sight);
        // '=' on ASCII row 3 (from top), column 12 -> y = 14 - 3 = 11
        let sandbag = grid.get(GridPos::new(12, 11)).unwrap();
        assert!(sandbag.walkable && !sandbag.blocks_sight);
        assert_eq!(sandbag.cover, CoverSides { n: true, s: true, ..default() });
        // '|' on row 6, column 3 -> y = 8
        assert_eq!(grid.get(GridPos::new(3, 8)).unwrap().cover, CoverSides { e: true, w: true, ..default() });
    }

    #[test]
    fn spawns_come_from_the_object_layer() {
        let spawns = spawns_from_tiled(&load());
        let players = spawns.iter().filter(|(f, _)| *f == Faction::Player).count();
        let enemies = spawns.iter().filter(|(f, _)| *f == Faction::Enemy).count();
        assert_eq!((players, enemies), (4, 3));
        // 'P' on ASCII row 12, column 3 -> (3, 2)
        assert!(spawns.contains(&(Faction::Player, GridPos::new(3, 2))));
        // 'E' on row 2, column 16 -> (16, 12)
        assert!(spawns.contains(&(Faction::Enemy, GridPos::new(16, 12))));
    }
}
```
Add `mod map;` to `src/main.rs`.

- [ ] **Step 8: Run tests to verify they fail**

Run: `cargo test map::`
Expected: compile error, `grid_from_tiled` / `spawns_from_tiled` not found.

- [ ] **Step 9: Implement the pure loaders**

Insert above the tests in `src/map.rs`:
```rust
/// Builds the walkability/sight/cover grid from every tile layer's tile properties.
/// Later layers override earlier ones for the same cell (walls layer sits above ground).
pub fn grid_from_tiled(map: &tiled::Map) -> Grid {
    let (w, h) = (map.width as i32, map.height as i32);
    let mut grid = Grid::new(w, h);
    for layer in map.layers() {
        let Some(tiles) = layer.as_tile_layer() else { continue };
        for tiled_y in 0..h {
            for x in 0..w {
                let Some(layer_tile) = tiles.get_tile(x, tiled_y) else { continue };
                let Some(tile) = layer_tile.get_tile() else { continue };
                let pos = GridPos::new(x, h - 1 - tiled_y);
                let mut cell = *grid.get(pos).expect("in bounds");
                if let Some(PropertyValue::BoolValue(b)) = tile.properties.get("walkable") {
                    cell.walkable = *b;
                }
                if let Some(PropertyValue::BoolValue(b)) = tile.properties.get("blocks_sight") {
                    cell.blocks_sight = *b;
                }
                if let Some(PropertyValue::StringValue(s)) = tile.properties.get("cover") {
                    cell.cover = CoverSides::parse(s);
                }
                grid.set(pos, cell);
            }
        }
    }
    grid
}

/// Reads point objects with a `faction` property from every object layer.
pub fn spawns_from_tiled(map: &tiled::Map) -> Vec<(Faction, GridPos)> {
    let h = map.height as i32;
    let (tw, th) = (map.tile_width as f32, map.tile_height as f32);
    let mut out = Vec::new();
    for layer in map.layers() {
        let Some(objects) = layer.as_object_layer() else { continue };
        for obj in objects.objects() {
            let faction = match obj.properties.get("faction") {
                Some(PropertyValue::StringValue(s)) if s == "player" => Faction::Player,
                Some(PropertyValue::StringValue(s)) if s == "enemy" => Faction::Enemy,
                _ => continue,
            };
            let pos = GridPos::new((obj.x / tw).floor() as i32, h - 1 - (obj.y / th).floor() as i32);
            out.push((faction, pos));
        }
    }
    out
}
```

- [ ] **Step 10: Run tests**

Run: `cargo test map::`
Expected: 2 passed. If `load_tmx_map` fails on the tileset image, check the `<image source>` path in the `.tsx` is `../tiles/tileset.png` relative to `assets/maps/`.

- [ ] **Step 11: Open the map in Tiled once to prove it round-trips (optional but recommended)**

```bash
sudo apt install -y tiled   # if not installed
tiled ~/src/wasteland-tactics/assets/maps/mission01.tmx &
```
Expected: the map renders with the four tiles and seven point objects. Close without saving (or save; Tiled rewrites the XML equivalently).

- [ ] **Step 12: Commit**

```bash
git add assets tools CREDITS.md src/map.rs src/main.rs
git commit -m "Add Kenney art and sound, 4-tile tileset, mission01 map and loaders

Closes #<issue for Task 10>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 11: Asset loading, map spawn, sprites, camera — first render

**Files:**
- Create: `src/assets.rs`, `src/camera.rs`, `src/render.rs`, `src/debug.rs`, `src/presentation.rs`
- Modify: `src/map.rs` (add the two systems), `src/main.rs` (full app assembly)

**Interfaces:**
- Produces: `assets::GameAssets { soldier, enemy, map, shot_sfx, hit_sfx }`; `assets::load_assets`, `assets::check_loaded`; `map::spawn_map`, `map::on_map_created`; `camera::spawn_camera`, `camera::pan_camera`, `camera::zoom_camera`; `render::attach_sprites`, `render::tint_dead`; `debug::screenshot_and_exit`; `presentation::PresentationPlugin`.

- [ ] **Step 1: Write `src/assets.rs`**

```rust
//! Every handle the game needs, loaded up front during `AppState::Loading`.

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledMapAsset;
use bevy_kira_audio::AudioSource;

use crate::grid::Grid;
use crate::state::AppState;

#[derive(Resource)]
pub struct GameAssets {
    pub soldier: Handle<Image>,
    pub enemy: Handle<Image>,
    pub map: Handle<TiledMapAsset>,
    pub shot_sfx: Handle<AudioSource>,
    pub hit_sfx: Handle<AudioSource>,
}

pub fn load_assets(mut commands: Commands, server: Res<AssetServer>) {
    commands.insert_resource(GameAssets {
        soldier: server.load("sprites/soldier.png"),
        enemy: server.load("sprites/enemy.png"),
        map: server.load("maps/mission01.tmx"),
        shot_sfx: server.load("audio/shot.ogg"),
        hit_sfx: server.load("audio/hit.ogg"),
    });
}

/// Moves to `Playing` once the images are in and the map has produced a `Grid`.
pub fn check_loaded(
    server: Res<AssetServer>,
    assets: Res<GameAssets>,
    grid: Option<Res<Grid>>,
    mut next: ResMut<NextState<AppState>>,
) {
    let images_ready = server.is_loaded_with_dependencies(&assets.soldier)
        && server.is_loaded_with_dependencies(&assets.enemy);
    if images_ready && grid.is_some() {
        info!("assets loaded, starting mission");
        next.set(AppState::Playing);
    }
}
```

- [ ] **Step 2: Add the map systems to `src/map.rs`**

Add these imports at the top:
```rust
use bevy_ecs_tiled::prelude::*;

use crate::assets::GameAssets;
use crate::unit::unit_bundle;
```
and these systems above the tests:
```rust
/// Spawns the map entity anchored so grid cell (0,0) is the bottom-left tile at world (0,0).
/// Units are spawned at z=10 and the dead at z=5, so map layers must stay below that.
pub fn spawn_map(mut commands: Commands, assets: Res<GameAssets>) {
    commands.spawn((
        Name::new("map"),
        TiledMap(assets.map.clone()),
        TilemapAnchor::BottomLeft,
        // Layers 1 z-unit apart (default is 100, which would bury units at z=10 under the walls layer).
        TiledMapLayerZOffset(1.0),
    ));
}

/// When the map finishes loading: build the `Grid` and spawn the units from its object layer.
pub fn on_map_created(
    mut commands: Commands,
    mut events: MessageReader<TiledEvent<MapCreated>>,
    maps: Res<Assets<TiledMapAsset>>,
) {
    for event in events.read() {
        let Some(map) = event.get_map(&maps) else { continue };
        let grid = grid_from_tiled(map);
        let spawns = spawns_from_tiled(map);
        info!("map ready: {}x{} grid, {} spawns", grid.width(), grid.height(), spawns.len());
        commands.insert_resource(grid);
        for (faction, pos) in spawns {
            commands.spawn(unit_bundle(faction, pos));
        }
    }
}
```

- [ ] **Step 3: Write `src/camera.rs`**

```rust
//! 2D camera with keyboard pan and wheel zoom.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

const PAN_SPEED: f32 = 600.0;

pub fn spawn_camera(mut commands: Commands) {
    // Centre of the 20x15 map (1280 x 960 px).
    commands.spawn((Camera2d, Transform::from_xyz(640.0, 480.0, 0.0)));
}

pub fn pan_camera(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut camera: Single<&mut Transform, With<Camera2d>>,
) {
    let mut dir = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dir.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dir.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        dir.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        dir.x += 1.0;
    }
    if dir != Vec2::ZERO {
        let step = dir.normalize() * PAN_SPEED * time.delta_secs();
        camera.translation.x += step.x;
        camera.translation.y += step.y;
    }
}

pub fn zoom_camera(
    mut wheel: MessageReader<MouseWheel>,
    mut projection: Single<&mut Projection, With<Camera2d>>,
) {
    for event in wheel.read() {
        if let Projection::Orthographic(ortho) = &mut **projection {
            let factor = if event.y > 0.0 { 0.9 } else { 1.1 };
            ortho.scale = (ortho.scale * factor).clamp(0.4, 3.0);
        }
    }
}
```

- [ ] **Step 4: Write `src/render.rs`**

```rust
//! Visual representation of units: sprites, death tint. Gizmo overlays are added in Task 12.

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::unit::{Dead, Faction};

pub const UNIT_SPRITE_SIZE: f32 = 48.0;

pub fn attach_sprites(
    mut commands: Commands,
    assets: Res<GameAssets>,
    units: Query<(Entity, &Faction), Added<Faction>>,
) {
    for (entity, faction) in &units {
        let image = match faction {
            Faction::Player => assets.soldier.clone(),
            Faction::Enemy => assets.enemy.clone(),
        };
        commands.entity(entity).insert(Sprite {
            image,
            custom_size: Some(Vec2::splat(UNIT_SPRITE_SIZE)),
            ..default()
        });
    }
}

pub fn tint_dead(mut units: Query<(&mut Sprite, &mut Transform), Added<Dead>>) {
    for (mut sprite, mut transform) in &mut units {
        sprite.color = Color::srgb(0.35, 0.35, 0.35);
        transform.translation.z = 5.0; // under the living
    }
}
```

- [ ] **Step 5: Write `src/debug.rs`**

```rust
//! Developer conveniences: `WT_SCREENSHOT=<png path>` saves a screenshot after 2 s and exits.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

pub fn screenshot_and_exit(
    mut commands: Commands,
    time: Res<Time>,
    mut requested: Local<bool>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(path) = std::env::var("WT_SCREENSHOT") else { return };
    if !*requested && time.elapsed_secs() > 2.0 {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *requested = true;
    } else if *requested && time.elapsed_secs() > 3.0 {
        exit.write(AppExit::Success);
    }
}
```

- [ ] **Step 6: Write `src/presentation.rs`**

```rust
//! Everything with a window: loading, map, sprites, camera, input, UI, audio, debug.

use bevy::prelude::*;

use crate::state::AppState;
use crate::{assets, camera, debug, map, render};

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::Loading),
            (assets::load_assets, map::spawn_map, camera::spawn_camera).chain(),
        )
        .add_systems(
            Update,
            (map::on_map_created, assets::check_loaded)
                .chain()
                .run_if(in_state(AppState::Loading)),
        )
        .add_systems(
            Update,
            (
                render::attach_sprites,
                render::tint_dead,
                camera::pan_camera,
                camera::zoom_camera,
                debug::screenshot_and_exit,
            ),
        );
    }
}
```

- [ ] **Step 7: Rewrite `src/main.rs`**

```rust
mod ai;
mod assets;
mod camera;
mod combat;
mod debug;
mod grid;
mod map;
mod movement;
mod presentation;
mod render;
mod rules;
mod sim;
mod state;
mod test_support;
mod unit;

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledPlugin;

use crate::presentation::PresentationPlugin;
use crate::rules::GameRng;
use crate::sim::SimulationPlugin;
use crate::state::AppState;

fn main() {
    let seed = seed_from_args().unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    });
    println!("seed {seed}");

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Wasteland Tactics".into(),
                        ..default()
                    }),
                    ..default()
                })
                .disable::<bevy::audio::AudioPlugin>(),
        )
        .add_plugins((TiledPlugin::default(), bevy_kira_audio::AudioPlugin))
        .init_state::<AppState>()
        .insert_resource(GameRng::seeded(seed))
        .add_plugins((SimulationPlugin, PresentationPlugin))
        .run();
}

/// `--seed N` makes a battle reproducible.
fn seed_from_args() -> Option<u64> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == "--seed" {
            return args.next()?.parse().ok();
        }
    }
    None
}
```
- [ ] **Step 8: Build, test, and screenshot**

```bash
cd ~/src/wasteland-tactics
cargo test 2>&1 | tail -3
WT_SCREENSHOT=/tmp/claude-1000/-home-mortlock-src/302ad263-77ce-466c-8f05-4a78fe90fc83/scratchpad/shot11.png \
  timeout 60 cargo run --features dynamic -- --seed 1 2>&1 | grep -E "map ready|assets loaded|panicked|ERROR" 
```
Expected: tests all pass; log shows `map ready: 20x15 grid, 7 spawns` then `assets loaded, starting mission`; the process exits by itself. Read the screenshot PNG: the tiled map fills the view with a walled border, and 7 unit sprites stand at the spawn points (4 bottom-left, 3 top-right). Enemies may already be walking toward cover or shooting.

If the map is offset from the units (units floating off the tiles), the anchor is wrong: confirm `TilemapAnchor::BottomLeft` is on the map entity and that the map entity's `Transform` is the default.

- [ ] **Step 9: Commit**

```bash
git add src/
git commit -m "Render the map, units and camera; add screenshot debug hook

Closes #<issue for Task 11>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 12: Selection and orders input, gizmo overlays, tracers

**Files:**
- Create: `src/orders.rs`
- Modify: `src/render.rs`, `src/presentation.rs`, `src/main.rs` (`mod orders;`)

**Interfaces:**
- Consumes: `Camera::viewport_to_world_2d`, `Selected`, `Order`, `Path`, `ShotFired`.
- Produces: `orders::player_input` system (left click select / 1–4 keys select nth soldier, right click move or attack); `render::draw_overlays` (selection rings, health bars, current move target); `render::spawn_tracers` + `render::draw_tracers` (`Tracer` component with a 0.12 s lifetime).

- [ ] **Step 1: Write `src/orders.rs`**

```rust
//! Player input: selection and issuing orders to selected soldiers.
//! Runs while Playing or Paused so orders can be queued during a pause.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::grid::{Grid, GridPos};
use crate::render::UNIT_SPRITE_SIZE;
use crate::unit::{Dead, Faction, Order, Path, Selected};

const PICK_RADIUS: f32 = UNIT_SPRITE_SIZE * 0.6;

fn cursor_world(window: &Window, camera: &Camera, camera_tf: &GlobalTransform) -> Option<Vec2> {
    let cursor = window.cursor_position()?;
    camera.viewport_to_world_2d(camera_tf, cursor).ok()
}

pub fn player_input(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    grid: Res<Grid>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    units: Query<(Entity, &Faction, &Transform, Option<&Selected>), Without<Dead>>,
    mut orders: Query<&mut Order>,
) {
    // Number keys select the nth living soldier (in spawn order).
    let hotkeys = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];
    for (i, key) in hotkeys.iter().enumerate() {
        if keys.just_pressed(*key) {
            let soldiers: Vec<Entity> =
                units.iter().filter(|(_, f, _, _)| **f == Faction::Player).map(|(e, ..)| e).collect();
            if let Some(&chosen) = soldiers.get(i) {
                for (e, _, _, selected) in &units {
                    if selected.is_some() {
                        commands.entity(e).remove::<Selected>();
                    }
                }
                commands.entity(chosen).insert(Selected);
            }
        }
    }

    let left = mouse.just_pressed(MouseButton::Left);
    let right = mouse.just_pressed(MouseButton::Right);
    if !left && !right {
        return;
    }
    let (cam, cam_tf) = *camera;
    let Some(world) = cursor_world(&window, cam, cam_tf) else { return };

    let under_cursor = units
        .iter()
        .filter(|(_, _, tf, _)| tf.translation.truncate().distance(world) <= PICK_RADIUS)
        .min_by(|a, b| {
            let da = a.2.translation.truncate().distance(world);
            let db = b.2.translation.truncate().distance(world);
            da.total_cmp(&db)
        });

    if left {
        let additive = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        if !additive {
            for (e, _, _, selected) in &units {
                if selected.is_some() {
                    commands.entity(e).remove::<Selected>();
                }
            }
        }
        if let Some((e, Faction::Player, _, _)) = under_cursor {
            commands.entity(e).insert(Selected);
        }
        return;
    }

    // Right click: attack an enemy under the cursor, otherwise move to the cell.
    let new_order = match under_cursor {
        Some((target, Faction::Enemy, _, _)) => Order::Attack(target),
        _ => {
            let cell = GridPos::from_world(world);
            if !grid.is_walkable(cell) {
                return;
            }
            Order::MoveTo(cell)
        }
    };
    for (e, faction, _, selected) in &units {
        if *faction == Faction::Player && selected.is_some() {
            if let Ok(mut order) = orders.get_mut(e) {
                *order = new_order;
            }
            commands.entity(e).remove::<Path>();
        }
    }
}
```
Add `mod orders;` to `src/main.rs`.

- [ ] **Step 2: Add overlays and tracers to `src/render.rs`**

Add imports:
```rust
use bevy::color::palettes::css::{LIME, ORANGE, RED, WHITE, YELLOW};

use crate::sim::ShotFired;
use crate::unit::{Health, Order, Selected};
```
and these items:
```rust
#[derive(Component)]
pub struct Tracer {
    pub from: Vec2,
    pub to: Vec2,
    pub hit: bool,
    pub ttl: f32,
}

/// Selection rings, health bars and move-target markers, drawn with gizmos every frame.
pub fn draw_overlays(
    mut gizmos: Gizmos,
    units: Query<(&Transform, &Health, &Faction, &Order, Option<&Selected>, Option<&Dead>)>,
) {
    for (transform, health, faction, order, selected, dead) in &units {
        if dead.is_some() {
            continue;
        }
        let p = transform.translation.truncate();
        if selected.is_some() {
            gizmos.circle_2d(Isometry2d::from_translation(p), UNIT_SPRITE_SIZE * 0.6, WHITE);
            if let Order::MoveTo(goal) = order {
                gizmos.circle_2d(Isometry2d::from_translation(goal.to_world()), 8.0, LIME);
            }
        }
        // Health bar just above the sprite.
        let width = UNIT_SPRITE_SIZE;
        let left = p + Vec2::new(-width / 2.0, UNIT_SPRITE_SIZE * 0.65);
        let frac = (health.current.max(0) as f32 / health.max as f32).clamp(0.0, 1.0);
        let colour = match faction {
            Faction::Player => LIME,
            Faction::Enemy => RED,
        };
        gizmos.line_2d(left, left + Vec2::X * width, Color::srgb(0.2, 0.2, 0.2));
        gizmos.line_2d(left, left + Vec2::X * width * frac, colour);
    }
}

pub fn spawn_tracers(mut commands: Commands, mut shots: MessageReader<ShotFired>) {
    for shot in shots.read() {
        commands.spawn(Tracer { from: shot.from, to: shot.to, hit: shot.hit, ttl: 0.12 });
    }
}

pub fn draw_tracers(
    mut commands: Commands,
    time: Res<Time>,
    mut gizmos: Gizmos,
    mut tracers: Query<(Entity, &mut Tracer)>,
) {
    for (entity, mut tracer) in &mut tracers {
        tracer.ttl -= time.delta_secs();
        if tracer.ttl <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let colour = if tracer.hit { ORANGE } else { YELLOW };
        gizmos.line_2d(tracer.from, tracer.to, colour);
    }
}
```

- [ ] **Step 3: Wire into `src/presentation.rs`**

Add `orders` to the `use crate::{...}` list and extend the always-on system tuple:
```rust
        .add_systems(
            Update,
            (
                render::attach_sprites,
                render::tint_dead,
                render::draw_overlays,
                render::spawn_tracers,
                render::draw_tracers,
                camera::pan_camera,
                camera::zoom_camera,
                debug::screenshot_and_exit,
            ),
        )
        .add_systems(
            Update,
            orders::player_input.run_if(in_state(AppState::Playing).or(in_state(AppState::Paused))),
        );
```

- [ ] **Step 4: Build, test, screenshot, and play**

```bash
cd ~/src/wasteland-tactics
cargo test 2>&1 | tail -2
cargo clippy --all-targets -- -D warnings 2>&1 | tail -3
WT_SCREENSHOT=/tmp/claude-1000/-home-mortlock-src/302ad263-77ce-466c-8f05-4a78fe90fc83/scratchpad/shot12.png \
  timeout 60 cargo run --features dynamic -- --seed 1 2>&1 | grep -E "panicked|ERROR"
```
Expected: tests and clippy clean, no panics; the screenshot shows health bars over every unit (green for soldiers, red for enemies). Then run without `WT_SCREENSHOT` and play for a minute: click a soldier (white ring), right-click ground (green dot appears, soldier walks), right-click an enemy (tracers flash, its bar shrinks). Space pauses movement but clicks still change orders.

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "Add selection and orders input, health bars, tracers

Closes #<issue for Task 12>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 13: UI overlay and audio

**Files:**
- Create: `src/ui.rs`, `src/audio.rs`
- Modify: `src/presentation.rs`, `src/main.rs` (`mod audio; mod ui;`)

**Interfaces:**
- Consumes: `AppState`, `MissionOutcome`, `ShotFired`, `GameAssets::{shot_sfx, hit_sfx}`.
- Produces: `ui::spawn_overlay`, `ui::update_overlay`; `audio::Sfx` channel marker, `audio::play_shot_sounds`.

- [ ] **Step 1: Write `src/ui.rs`**

```rust
//! Text overlay: PAUSED / VICTORY / DEFEAT plus a one-line controls hint.

use bevy::prelude::*;

use crate::state::{AppState, MissionOutcome};

#[derive(Component)]
pub struct StatusText;

pub fn spawn_overlay(mut commands: Commands) {
    commands.spawn((
        Name::new("status text"),
        StatusText,
        Text::new(""),
        TextFont { font_size: 40.0, ..default() },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));
    commands.spawn((
        Name::new("controls hint"),
        Text::new("LMB select  RMB move/attack  1-4 squad  Space pause  WASD pan  wheel zoom"),
        TextFont { font_size: 16.0, ..default() },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(12.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));
}

pub fn update_overlay(
    state: Res<State<AppState>>,
    outcome: Option<Res<MissionOutcome>>,
    mut text: Single<&mut Text, With<StatusText>>,
) {
    let label = match (outcome.as_deref(), state.get()) {
        (Some(MissionOutcome::Victory), _) => "VICTORY",
        (Some(MissionOutcome::Defeat), _) => "DEFEAT",
        (None, AppState::Paused) => "PAUSED",
        (None, AppState::Loading) => "Loading...",
        (None, AppState::Playing) => "",
    };
    if text.0 != label {
        text.0 = label.to_string();
    }
}
```

- [ ] **Step 2: Write `src/audio.rs`**

```rust
//! Plays a sound for every shot. Uses a dedicated SFX channel so music can be added later.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;

use crate::assets::GameAssets;
use crate::sim::ShotFired;

/// Marker for the SFX channel resource `AudioChannel<Sfx>`.
#[derive(Resource)]
pub struct Sfx;

pub fn play_shot_sounds(
    mut shots: MessageReader<ShotFired>,
    assets: Res<GameAssets>,
    sfx: Res<AudioChannel<Sfx>>,
) {
    for shot in shots.read() {
        sfx.play(assets.shot_sfx.clone());
        if shot.hit {
            sfx.play(assets.hit_sfx.clone());
        }
    }
}
```

- [ ] **Step 3: Wire into `src/presentation.rs`**

Add `use bevy_kira_audio::prelude::AudioApp;` and `audio`, `ui` to the crate imports. In `build`:
```rust
        app.add_audio_channel::<audio::Sfx>();
```
Add `ui::spawn_overlay` to the `OnEnter(AppState::Loading)` chain (after `camera::spawn_camera`), and add `ui::update_overlay` and `audio::play_shot_sounds` to the always-on system tuple.

- [ ] **Step 4: Build and play**

```bash
cd ~/src/wasteland-tactics
cargo test 2>&1 | tail -2
cargo clippy --all-targets -- -D warnings 2>&1 | tail -3
cargo run --features dynamic -- --seed 1
```
Expected: the hint line sits at the bottom; Space shows PAUSED top-left; shots are audible; when all enemies die the text reads VICTORY and the sim freezes. If there is no sound, confirm the OGG files play with `paplay assets/audio/shot.ogg` and that Bevy's own audio plugin is disabled in `main.rs` (two audio backends fight for the device).

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "Add status overlay and shot sound effects

Closes #<issue for Task 13>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 14: Inspector feature, README polish, final CI check

**Files:**
- Modify: `src/debug.rs`, `src/presentation.rs`, `README.md`

- [ ] **Step 1: Add the optional inspector to `src/debug.rs`**

```rust
#[cfg(feature = "inspector")]
pub fn add_inspector(app: &mut App) {
    use bevy_inspector_egui::bevy_egui::EguiPlugin;
    use bevy_inspector_egui::quick::WorldInspectorPlugin;
    app.add_plugins((EguiPlugin::default(), WorldInspectorPlugin::new()));
}

#[cfg(not(feature = "inspector"))]
pub fn add_inspector(_app: &mut App) {}
```
In `presentation.rs` `build`, call `debug::add_inspector(app);` first.

- [ ] **Step 2: Verify both feature combinations build and clippy is clean**

```bash
cd ~/src/wasteland-tactics
cargo clippy --all-targets -- -D warnings 2>&1 | tail -2
cargo clippy --all-targets --features inspector -- -D warnings 2>&1 | tail -2
cargo fmt --all
cargo test 2>&1 | tail -2
timeout 20 cargo run --features dynamic,inspector -- --seed 1 2>&1 | grep -E "panicked|ERROR"; echo done
```
Expected: no warnings, all tests pass, the inspector window lists entities when run with the feature (John can verify visually).

- [ ] **Step 3: Update `README.md`**

Append under "Develop":
```markdown
    cargo run --features dynamic,inspector   # live entity inspector (egui)
    WT_SCREENSHOT=out.png cargo run --features dynamic   # screenshot after 2 s, then exit
    cargo run --features dynamic -- --seed 42            # reproducible battle

Maps: `tools/gen_map.py` regenerates `assets/maps/mission01.tmx` from ASCII; open the result in Tiled to hand-edit.

## Status

Slice 1 (one squad, one map) — see `docs/superpowers/specs/` and the GitHub milestone.
```

- [ ] **Step 4: Commit, push, confirm CI**

```bash
git add -A
git commit -m "Add optional egui inspector and document dev workflow

Closes #<issue for Task 14>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
git push
gh auth switch --user jmortlock
gh run watch --repo jmortlock/wasteland-tactics --exit-status
gh auth switch --user jmortlock_lmg
```
Expected: CI green. All 13 issues closed by the commit trailers; the milestone shows 100%.

---

## Not in this plan (later slices)

Player units returning fire automatically, unit facing art variants, music, menus, multiple maps, saving, inventory. Each is a new brainstorm.
