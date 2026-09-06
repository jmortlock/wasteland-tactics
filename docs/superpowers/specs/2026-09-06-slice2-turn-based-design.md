# Wasteland Tactics — Slice 2: squad turn-based core loop

Date: 2026-09-06
Status: approved in brainstorming, awaiting implementation plan
Supersedes: the "real-time with pause" loop of the slice 1 spec
(`2026-09-06-wasteland-tactics-design.md`). Everything else in that spec
(purpose, stack, repo layout, assets, grid/cover/LOS rules) still stands.

## Purpose

Slice 1 shipped a real-time-with-pause skirmish. Playing it made clear the
game is meant to be **squad turn-based** (XCOM / Fallout Tactics "squad turn"
mode): the whole squad acts with action points, the player ends the turn, all
enemies act, repeat. Slice 2 replaces the core loop with a pure, deterministic
turn-based battle engine and turns Bevy into its animator, then adds the
planning previews a turn-based game needs (reach, path cost, hit chance) and
reaction fire.

## Player-visible result

- Each soldier has action points (AP). Moving costs 1 AP per cell, shooting
  costs the weapon's AP cost. Any order of actions, any unit, until AP runs out
  or the player ends the turn.
- Selecting a soldier tints the cells it can reach this turn. Hovering a cell
  draws the path and its AP cost; right click moves. Hovering a visible enemy
  shows the hit chance and whether it is in cover; right click shoots.
- Enter ends the turn. Enemies then act one action at a time, animated at the
  same pace as the player's units.
- **Reaction fire:** AP a unit did not spend becomes shots during the
  opponent's turn: whenever an enemy unit steps into a cell that a unit with
  enough AP can see, that unit fires at it. Reaction shots are drawn and
  sounded distinctly from ordinary shots.
- The overlay shows `YOUR TURN <n>` / `ENEMY TURN` / `VICTORY` / `DEFEAT`.
  `R` restarts the battle with the same map and seed. Space is unused.
- Same map, same seven units, same art and sound as slice 1.

## Architecture

```
input (Bevy)  ──Action──▶  battle engine (pure Rust)  ──Vec<Event>──▶  event queue ──▶ animator (Bevy)
                                       ▲                                              │
                              enemy planner (pure)  ◀────── EnemyTurn phase ──────────┘
```

Two rules:

1. **Only the engine mutates battle state.** Bevy never touches unit
   positions, health or AP except by applying an `Action` and animating the
   resulting `Event`s.
2. **The engine never touches Bevy.** No `Entity`, no systems, no resources.
   Units are addressed by `UnitId(u32)`. The engine is tested with plain
   `#[test]`s and no `App`.

### Module layout

| Module | Responsibility | Depends on |
|---|---|---|
| `grid` | unchanged from slice 1 (cells, LOS, cover, A*) | — |
| `rules` | unchanged (`hit_chance`, `roll_hit`, `roll_damage`, `GameRng`); gains nothing | `grid` |
| `battle` | **new.** `Battle` state, `Action`, `Event`, `ActionError`, `apply`, UI queries (`reachable`, `path_to`, `hit_chance`, `visible_enemies`), `from_tiled` loader, tuning constants | `grid`, `rules`, `tiled` |
| `planner` | **new.** `plan_enemy_action(&Battle) -> Option<Action>` | `battle` |
| `map` | keeps `grid_from_tiled` / `spawns_from_tiled` (pure); loses its Bevy systems | `grid`, `tiled` |
| `phase` | **new.** `Phase` state, event queue resource, the phase driver (player input → animating → enemy turn → …) | `battle`, `planner` |
| `animator` | **new.** Drains the event queue at real-time pace: walks sprites per `Stepped`, holds on shots, tints the dead, emits `ShotFired` for tracers/audio | `battle` events |
| `orders` | rewritten: selection, hover, builds `Action`s, applies them through `phase` | `battle` |
| `render` | overlays extended: reach tint, hovered path + AP cost, hit-chance label, AP pips; tracers unchanged | `battle` queries |
| `ui` | turn/outcome text, End Turn hint | `phase` |
| `assets`, `camera`, `audio`, `debug`, `presentation` | unchanged apart from wiring | — |

Deleted: `unit.rs` (components move into `battle::Unit` plus a `UnitSprite(UnitId)` marker), `movement.rs`, `combat.rs`, `ai.rs`, `sim.rs`, `state.rs` (replaced by `phase.rs`), `test_support.rs` (the engine needs no App).

## The battle engine (`battle`)

### State

```rust
pub struct UnitId(pub u32);              // index into Battle::units, stable for the battle
pub enum Side { Player, Enemy }
pub struct Weapon { range: i32, damage_min: i32, damage_max: i32, ap_cost: i32 }
pub struct Unit { id, side, pos: GridPos, health: i32, health_max: i32, weapon, ap: i32, ap_max: i32, alive: bool }
pub enum Outcome { Victory, Defeat }
pub struct Battle { grid: Grid, units: Vec<Unit>, turn: Side, turn_number: u32, rng: ChaCha8Rng, outcome: Option<Outcome> }
```

Constructors: `Battle::new(grid, units, seed)` (tests) and
`Battle::from_tiled(&tiled::Map, seed)` (the game), which uses the existing
`map::grid_from_tiled` and `map::spawns_from_tiled`.

### Tuning constants (one block, `battle::tuning`)

| Constant | Value |
|---|---|
| `SOLDIER_AP` | 6 |
| `ENEMY_AP` | 5 |
| `MOVE_COST` (per cell, diagonal same) | 1 |
| rifle `ap_cost` (both sides this slice) | 3 |
| rifle `range` / damage | soldier 8 / 20–35, enemy 7 / 10–25 (unchanged from slice 1) |
| health | 100 both |

### Actions

```rust
pub enum Action {
    Move { unit: UnitId, path: Vec<GridPos> },   // path excludes the start cell
    Shoot { unit: UnitId, target: UnitId },
    EndTurn,
}
```

`apply(&mut self, action) -> Result<Vec<Event>, ActionError>` validates then
mutates. Validation failures return `ActionError` and leave the state
untouched:

`BattleOver`, `NotYourTurn`, `NoSuchUnit`, `UnitDead`, `NotEnoughAp { need, have }`,
`PathNotWalkable(GridPos)`, `PathNotContiguous`, `CellOccupied(GridPos)`,
`TargetNotVisible`, `OutOfRange { distance, range }`, `SameSide`,
`EndTurnOnlyOnPlayerTurn`.

### Events

```rust
pub enum Event {
    Stepped { unit, from, to },                         // one per cell
    Shot { shooter, target, hit: bool, damage: i32 },   // ordinary shot
    ReactionShot { shooter, target, hit: bool, damage: i32 },
    Died(UnitId),
    TurnEnded(Side),
    TurnStarted { side: Side, turn_number: u32 },
    BattleOver(Outcome),
}
```

Events are returned in the order they happened. `damage` is 0 on a miss.

### Rules

- **Turn start:** every living unit of the side whose turn begins gets
  `ap = ap_max`. AP is NOT reset at turn end, so unspent AP carries into the
  opponent's turn as the reaction budget.
- **Move:** costs `MOVE_COST` per cell. The whole path is validated up front
  (walkable, contiguous king-moves, no corner cutting per `grid::neighbours`,
  every cell unoccupied by a living unit). Then cells are applied one at a
  time: emit `Stepped`, deduct AP, run the reaction check (below). If the
  mover dies mid-path the remaining steps are dropped and the move ends there.
- **Shoot:** requires `ap >= weapon.ap_cost`, target alive, other side,
  `distance <= range`, `grid.has_line_of_sight`. Deduct AP, roll
  `rules::hit_chance(distance, range, grid.cover_against(target.pos, shooter.pos))`
  then `roll_hit` and `roll_damage` from the battle RNG, apply damage, emit
  `Shot`, then `Died` if health reached 0. Shooting never triggers reactions.
- **Reaction check** (after each `Stepped` of unit `m`): for every living unit
  `u` of the other side, in ascending `UnitId`, with `u.ap >= u.weapon.ap_cost`,
  `distance(u, m) <= u.weapon.range` and LOS to the mover's NEW cell: `u`
  fires one shot exactly as above but emitting `ReactionShot`. A unit may
  react more than once per enemy move if it still has AP. If the mover dies,
  stop checking further reactors.
- **End turn:** valid only on `turn == Player` (the enemy side ends its turn
  through the phase driver, which calls `end_enemy_turn()` — a separate
  method so the UI can never end the enemy turn). Emits `TurnEnded(side)`,
  advances `turn_number` when control returns to the player, resets the new
  side's AP, emits `TurnStarted`.
- **Outcome:** after every action, if a side that has any units has no living
  units, set `outcome` and append `BattleOver(outcome)`. Once set, every
  `apply` returns `ActionError::BattleOver`. A battle where only one side
  exists never ends (as in slice 1).
- **RNG:** advances only on shots. Same seed + same action sequence = same
  events. This is the replay guarantee the `--seed` flag now honours.

### Queries (read-only, for the UI and the planner)

- `reachable(unit) -> HashMap<GridPos, i32>`: Dijkstra from the unit's cell
  over walkable, unoccupied cells with `grid::neighbours` costs collapsed to
  `MOVE_COST` per step, bounded by the unit's AP. Excludes the start cell.
- `path_to(unit, goal) -> Option<Vec<GridPos>>`: A* (`grid::find_path`)
  filtered by occupancy; `None` if unreachable or costs more AP than the unit
  has.
- `hit_chance(shooter, target) -> Option<f32>`: `None` if no LOS or out of
  range, else the `rules::hit_chance` value; also returns the cover flag via
  `target_in_cover(shooter, target) -> bool`.
- `visible_enemies(unit) -> Vec<UnitId>`: living units of the other side with
  LOS and within `weapon.range`, nearest first.
- `units()`, `unit(id)`, `turn()`, `turn_number()`, `outcome()`, `grid()`.

## The enemy planner (`planner`)

`plan_enemy_action(&Battle) -> Option<Action>`. Pure; reads only the queries
above. `None` means "nothing useful left; end the turn".

Enemies are considered in ascending `UnitId`. For the first living enemy
with `ap > 0` that yields an action:

1. **Shoot** the nearest visible player if `ap >= ap_cost` and either the enemy
   is in cover against that player (`target_in_cover(player, enemy)`) or no
   cover cell qualifies under rule 2.
2. **Take cover:** among `reachable` cells whose cost leaves
   `ap - cost >= ap_cost`, that have LOS to the chosen player and give cover
   against it (`grid.cover_against(cell, player.pos)`), pick the cheapest
   (ties: lowest y then x). Emit `Move` along `path_to`.
3. **Advance:** if no player is visible, move along `path_to` the nearest
   player's cell, truncated to `ap - ap_cost` steps (so a shot is still
   possible after moving); if that leaves zero steps, skip.
4. Otherwise this enemy is done; try the next.

Because the planner is re-run before every action, reaction fire that kills
or wounds an enemy changes the rest of the turn naturally.

## The Bevy side

### Phase state (`phase`)

```rust
enum AppState { Loading, Battle }            // Battle replaces Playing/Paused
enum Phase { PlayerInput, Animating, EnemyTurn }   // sub-state, valid in AppState::Battle
```

Resources: `Battle`, `EventQueue(VecDeque<Event>)`, `BattleSeed(u64)`.

Driver:

- `PlayerInput`: `orders` may call `phase::submit(action)`, which applies it
  to the `Battle`, pushes the events, and switches to `Animating`. `EndTurn`
  is submitted the same way.
- `Animating`: the `animator` drains the queue. When empty: if
  `battle.turn() == Enemy` go to `EnemyTurn`, else back to `PlayerInput`. If
  `BattleOver` was seen, stay in a terminal `Animating`-empty state where only
  `R` works.
- `EnemyTurn`: call `plan_enemy_action`; if `Some(a)`, apply it, push events,
  go to `Animating`; if `None`, call `battle.end_enemy_turn()`, push events,
  go to `Animating` (which will return to `PlayerInput`).
- `R` (any phase, any time): rebuild `Battle::from_tiled` with `BattleSeed`,
  despawn and respawn unit sprites, clear the queue, `PlayerInput`.

### Animator

One system in `Update` while `Phase::Animating`. Pops events and plays them:

| Event | Animation |
|---|---|
| `Stepped` | lerp the unit's sprite from `from` to `to` at 160 px/s; rotate to face the step; next event when arrived |
| `Shot` / `ReactionShot` | rotate shooter to face target, emit `ShotFired { from, to, hit, reaction }` for tracer + audio, hold 0.25 s (0.4 s for reaction) |
| `Died` | tint sprite grey, drop z to 5 |
| `TurnEnded` / `TurnStarted` | update UI text, hold 0.5 s on `TurnStarted { Enemy }` |
| `BattleOver` | update UI text |

`ShotFired` keeps its slice 1 shape plus a `reaction: bool`; tracers draw
reaction shots in red instead of orange/yellow; audio plays the same clips.

### Input (`orders`), only in `Phase::PlayerInput`

- Left click / keys 1–4: select a soldier (players only, living only).
  Selection is a `Selected(UnitId)` resource, not a component.
- Hover: `GridPos::from_world(cursor)`. If a visible enemy is under the cursor
  (nearest sprite within pick radius), the hover target is that unit;
  otherwise the cell.
- Right click on an enemy: `submit(Shoot)` if `hit_chance` is `Some`.
- Right click on a reachable cell: `submit(Move { path: path_to(goal) })`.
- Enter: `submit(EndTurn)`. `R`: restart. Space: nothing.
- Invalid submissions (engine returns `Err`) are logged at `info` and ignored;
  the previews make them rare.

### Previews (`render`, gizmos)

- Reach tint: for the selected unit, `reachable()` cells drawn as translucent
  filled squares (`gizmos.rect_2d` outline plus a second inset rect is fine;
  no sprite needed).
- Hover path: `path_to(hover cell)` drawn as a polyline with the AP cost as
  `Text2d` at the goal.
- Hover enemy: `Text2d` near the target: `"72%  COVER"` or `"72%"`, or
  `"no LOS"` / `"out of range"`.
- AP pips: small ticks under each living unit's health bar, one per AP.
- Health bars, selection ring, tracers as in slice 1.

### UI text (`ui`)

Top-left: `YOUR TURN 3`, `ENEMY TURN`, `VICTORY`, `DEFEAT`. Bottom-left hint
updated to: `LMB select  hover = path/AP or hit%  RMB move/shoot  1-4 squad  Enter end turn  R restart  WASD pan  wheel zoom`.

## Testing

- **Engine (`battle`, plain tests, the bulk):** AP accounting for moves and
  shots; each `ActionError` variant is returned and the state is unchanged;
  a move emits one `Stepped` per cell; reaction fire triggers on the exact
  step that enters LOS, spends the reactor's AP, is skipped without AP or
  LOS, and stops a killed mover's remaining steps; cover halves hits over a
  seeded volley (port of slice 1's statistical test); `EndTurn` resets only
  the new side's AP; `BattleOver` fires once and locks the engine; a scripted
  battle from a fixed seed produces a fixed event log (replay guarantee).
  ASCII-map test helper: `Battle::from_ascii(art, seed)` where `P`/`E` mark
  units (test-only, `#[cfg(test)]`).
- **Queries:** `reachable` respects AP and occupancy; `path_to` returns `None`
  beyond AP; `hit_chance` is `None` without LOS.
- **Planner:** shoots when in cover; takes cover when a qualifying cell
  exists; advances when nothing is visible; returns `None` when spent.
- **Loader:** `Battle::from_tiled` on `assets/maps/mission01.tmx` yields 4
  soldiers + 3 enemies at the slice 1 positions.
- **Phase driver:** one headless Bevy test with an instant animator (drains
  the queue in one frame) that runs from `PlayerInput`, submits `EndTurn`,
  and asserts control returns to `PlayerInput` with `turn_number == 2`.
- **Bevy visuals/audio:** verified by playing and the `WT_SCREENSHOT` hook.

## Follow-up issues this slice closes

Superseded or covered by the rewrite: #14 fixed timestep, #16 mid-tile
strand, #17 AI thrash, #18 same cover cell, #19 order queue, #20 occupancy,
#27 multi-attacker test, #28 hotkey double-handling, #30 split map.rs.
Pulled in deliberately: #15 LOS symmetry (reaction fire makes asymmetry
visible; fix: `has_line_of_sight` accepts LOS if either direction's Bresenham
line is clear, with a symmetry test).

Still open after this slice: #21 cover-tile art, #22 register_type, #23
edge-scroll, #24 audio handles in loading, #25 run_if(resource_exists),
#26 map reload idempotency, #29 tracer ordering.

## Non-goals

Weapon variety, stances/overwatch orders, objectives, multiple maps, squad
persistence, save/undo, individual-initiative or continuous turn modes,
fog of war, animations beyond lerp + rotate.
