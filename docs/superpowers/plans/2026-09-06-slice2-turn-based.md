# Slice 2: Squad Turn-Based Core Loop — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the real-time-with-pause loop with a pure, deterministic squad turn-based battle engine (action points, End Turn, enemy turn, reaction fire), with Bevy reduced to input, previews and an event animator.

**Architecture:** A new `battle` module owns all battle state and exposes `apply(Action) -> Result<Vec<BattleEvent>, ActionError>` plus read-only queries; a `planner` module returns one enemy action at a time; a `phase` state machine (`Loading → PlayerInput ↔ Animating ↔ EnemyTurn`) applies actions and pushes events into a queue that the `animator` plays at real-time pace. Slice 1's `grid`, `rules` and the pure `map` loaders survive untouched; `unit`, `movement`, `combat`, `ai`, `sim`, `state` and `test_support` are deleted.

**Tech Stack:** Rust 1.96, Bevy 0.19.1 (pinned), bevy_ecs_tiled 0.13.4, bevy_kira_audio 0.26.0, pathfinding 4.15 (`astar`, `dijkstra_all`), rand 0.9 / rand_chacha 0.9, tiled 0.16.

**Spec:** `docs/superpowers/specs/2026-09-06-slice2-turn-based-design.md`

## Global Constraints

- Only the engine mutates battle state: Bevy code never writes unit position, health or AP except by `Battle::apply` / `Battle::end_enemy_turn` and animating the returned events.
- The engine has no Bevy dependency except the `Resource` derive marker on `Battle` (same convention as `Grid`). No `Entity`, systems or queries in `battle.rs` or `planner.rs`. Units are addressed by `UnitId(u32)`, which is the index into `Battle::units`.
- Tuning constants live only in `battle::tuning`: `SOLDIER_AP = 6`, `ENEMY_AP = 5`, `MOVE_COST = 1`, rifle `ap_cost = 3`, soldier weapon range 8 / damage 20–35, enemy weapon range 7 / damage 10–25, health 100.
- RNG advances only on shots. Same seed + same action sequence = same events.
- Grid coordinates unchanged: origin bottom-left, y up, `TILE_SIZE = 64.0`; sprites at z = 10, dead at z = 5, hover label at z = 20.
- Naming: the spec's `Event` enum is `BattleEvent` in code, to avoid clashing with Bevy's `Event` trait. The spec's `Phase` sub-state is folded into a flat `AppState { Loading, PlayerInput, Animating, EnemyTurn }` (same semantics, no SubStates machinery).
- Interim dead-code rule: new pure modules that are not yet wired (Tasks 1–8) carry `#![allow(dead_code)]` as their first line; Task 9 removes every one of them. Never re-add a crate-level allow.
- Deprecated `.or(...)` must not be used; use `.or_else(...)`. `#[allow(clippy::type_complexity)]` / `too_many_arguments` on Bevy systems are fine.
- CI enforces `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, the same with `--features inspector`, and `cargo test`. Run all four before every commit.
- Commit messages end with the trailers `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b`. Work on branch `slice-2`; push only that branch; merge to main happens at finishing-a-development-branch.
- Follow-up issues closed by this slice (add `Closes #N` lines in Task 11's commit): #14, #15, #16, #17, #18, #19, #20, #27, #28, #30.

---

## File structure

| File | Responsibility | Fate |
|---|---|---|
| `src/battle.rs` | engine: types, `Battle`, `apply`, reaction fire, queries, `from_tiled`, `from_ascii` (test) | create (Tasks 1–6) |
| `src/planner.rs` | `plan_enemy_action(&Battle) -> Option<Action>` | create (Task 7) |
| `src/phase.rs` | `AppState`, `EventQueue`, `BattleSeed`, `submit`, phase driver, restart | create (Task 8) |
| `src/animator.rs` | `UnitSprite`, `ShotFired`, `Animation`, `animate` system | create (Task 8) |
| `src/grid.rs` | unchanged except symmetric LOS | modify (Task 5) |
| `src/rules.rs` | unchanged; `GameRng` becomes test-only | modify (Task 9) |
| `src/map.rs` | pure loaders (`spawns_from_tiled` now returns `Side`); `spawn_map`, `on_map_created` build the `Battle` and unit sprites; `spawn_unit_sprites` helper | modify (Tasks 6, 9) |
| `src/assets.rs` | `check_loaded` waits for `Battle` | modify (Task 9) |
| `src/orders.rs` | selection, hover, builds actions, submits | rewrite (Task 9), hover (Task 10) |
| `src/render.rs` | overlays from `Battle`; tracers (red for reaction); previews | rewrite (Task 9), previews (Task 10) |
| `src/ui.rs` | turn/outcome text, hint | rewrite (Task 9) |
| `src/audio.rs` | reads `animator::ShotFired` | modify (Task 9) |
| `src/presentation.rs`, `src/main.rs` | wiring | rewrite (Task 9) |
| `src/unit.rs`, `movement.rs`, `combat.rs`, `ai.rs`, `sim.rs`, `state.rs`, `test_support.rs` | old sim | delete (Task 9) |
| `README.md`, `docs/superpowers/specs/2026-09-06-wasteland-tactics-design.md` | docs | modify (Task 11) |

---

### Task 1: Battle engine skeleton — types, construction, End Turn

**Files:**
- Create: `src/battle.rs`
- Modify: `src/main.rs` (add `mod battle;` alphabetically, after `mod audio;`)

**Interfaces:**
- Produces (all `pub` in `crate::battle`): `UnitId(pub u32)`; `Side { Player, Enemy }` + `Side::other()`; `Weapon { range, damage_min, damage_max, ap_cost }`; `Unit { id, side, pos, health, health_max, weapon, ap, ap_max, alive }` + `Unit::new(id, side, pos)`; `Outcome { Victory, Defeat }`; `Action { Move { unit, path }, Shoot { unit, target }, EndTurn }`; `BattleEvent { Stepped { unit, from, to }, Shot { shooter, target, hit, damage }, ReactionShot { … same fields … }, Died(UnitId), TurnEnded(Side), TurnStarted { side, turn_number }, BattleOver(Outcome) }`; `ActionError` (all variants listed in the spec plus `EmptyPath`); `Battle::new(grid, units, seed)`, `Battle::grid()`, `units()`, `unit(id)`, `living(side)`, `unit_at(pos)`, `turn()`, `turn_number()`, `outcome()`, `apply(action)`, `end_enemy_turn()`; `tuning` constants; test-only `Battle::from_ascii(art, seed)`.
- This task implements only `EndTurn` inside `apply`; `Move` and `Shoot` return `Err(ActionError::NotYourTurn)` placeholders that Tasks 2–3 replace — see Step 3, where the two arms are marked.

- [ ] **Step 1: Write the failing tests**

Create `src/battle.rs`:
```rust
//! The pure turn-based battle engine. No Bevy systems, no entities: units are
//! addressed by `UnitId`, which indexes `Battle::units`. Presentation applies
//! `Action`s and animates the returned `BattleEvent`s; nothing else mutates state.
#![allow(dead_code)] // removed in Task 9 when the engine is wired into the app

use bevy::prelude::Resource;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::grid::{Grid, GridPos};
use crate::rules;

pub mod tuning {
    use super::Weapon;
    pub const SOLDIER_AP: i32 = 6;
    pub const ENEMY_AP: i32 = 5;
    /// AP per cell moved, diagonals included.
    pub const MOVE_COST: i32 = 1;
    pub const RIFLE_AP_COST: i32 = 3;
    pub const HEALTH: i32 = 100;
    pub const SOLDIER_WEAPON: Weapon = Weapon {
        range: 8,
        damage_min: 20,
        damage_max: 35,
        ap_cost: RIFLE_AP_COST,
    };
    pub const ENEMY_WEAPON: Weapon = Weapon {
        range: 7,
        damage_min: 10,
        damage_max: 25,
        ap_cost: RIFLE_AP_COST,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_builder_assigns_ids_in_reading_order_and_full_ap() {
        let b = Battle::from_ascii("P.E\n.#.\nP..", 1);
        assert_eq!(b.units().len(), 3);
        assert_eq!(b.unit(UnitId(0)).unwrap().side, Side::Player);
        assert_eq!(b.unit(UnitId(0)).unwrap().pos, GridPos::new(0, 2));
        assert_eq!(b.unit(UnitId(1)).unwrap().side, Side::Enemy);
        assert_eq!(b.unit(UnitId(1)).unwrap().pos, GridPos::new(2, 2));
        assert_eq!(b.unit(UnitId(2)).unwrap().pos, GridPos::new(0, 0));
        assert_eq!(b.unit(UnitId(0)).unwrap().ap, tuning::SOLDIER_AP);
        assert_eq!(b.unit(UnitId(1)).unwrap().ap, tuning::ENEMY_AP);
        assert!(!b.grid().is_walkable(GridPos::new(1, 1)), "walls still parse");
        assert_eq!(b.turn(), Side::Player);
        assert_eq!(b.turn_number(), 1);
        assert_eq!(b.outcome(), None);
        assert_eq!(b.unit_at(GridPos::new(2, 2)).map(|u| u.id), Some(UnitId(1)));
        assert!(b.unit_at(GridPos::new(1, 0)).is_none());
    }

    #[test]
    fn end_turn_hands_control_to_the_enemy_and_back() {
        let mut b = Battle::from_ascii("P...E", 1);
        // Spend some player AP so we can see it is NOT refilled at the end of the player's turn.
        b.units[0].ap = 2;
        let events = b.apply(Action::EndTurn).unwrap();
        assert_eq!(
            events,
            vec![
                BattleEvent::TurnEnded(Side::Player),
                BattleEvent::TurnStarted {
                    side: Side::Enemy,
                    turn_number: 1
                }
            ]
        );
        assert_eq!(b.turn(), Side::Enemy);
        assert_eq!(b.unit(UnitId(0)).unwrap().ap, 2, "unspent AP carries into the enemy turn");
        assert_eq!(b.apply(Action::EndTurn), Err(ActionError::EndTurnOnlyOnPlayerTurn));

        b.units[1].ap = 0;
        let events = b.end_enemy_turn();
        assert_eq!(
            events,
            vec![
                BattleEvent::TurnEnded(Side::Enemy),
                BattleEvent::TurnStarted {
                    side: Side::Player,
                    turn_number: 2
                }
            ]
        );
        assert_eq!(b.turn(), Side::Player);
        assert_eq!(b.turn_number(), 2);
        assert_eq!(b.unit(UnitId(0)).unwrap().ap, tuning::SOLDIER_AP, "player AP refilled on their turn");
        assert_eq!(b.unit(UnitId(1)).unwrap().ap, 0, "enemy AP untouched until its turn");
        assert!(b.end_enemy_turn().is_empty(), "no-op on the player's turn");
    }

    #[test]
    fn dead_units_do_not_get_ap_and_are_not_at_their_cell() {
        let mut b = Battle::from_ascii("P.E", 1);
        b.units[1].alive = false;
        b.apply(Action::EndTurn).unwrap();
        assert_eq!(b.unit(UnitId(1)).unwrap().ap, tuning::ENEMY_AP, "unchanged, not refilled");
        assert!(b.unit_at(GridPos::new(2, 0)).is_none());
        assert_eq!(b.living(Side::Enemy).count(), 0);
        assert_eq!(b.unit(UnitId(7)), None);
    }
}
```
Add `mod battle;` to `src/main.rs` after `mod audio;`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test battle:: 2>&1 | tail -20`
Expected: compile errors (`Battle`, `UnitId`, … not found).

- [ ] **Step 3: Implement the skeleton**

Insert between the `tuning` module and the tests:
```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct UnitId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Player,
    Enemy,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::Player => Side::Enemy,
            Side::Enemy => Side::Player,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Weapon {
    /// Maximum Chebyshev distance in cells.
    pub range: i32,
    pub damage_min: i32,
    pub damage_max: i32,
    pub ap_cost: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    pub id: UnitId,
    pub side: Side,
    pub pos: GridPos,
    pub health: i32,
    pub health_max: i32,
    pub weapon: Weapon,
    pub ap: i32,
    pub ap_max: i32,
    pub alive: bool,
}

impl Unit {
    pub fn new(id: UnitId, side: Side, pos: GridPos) -> Unit {
        let (ap_max, weapon) = match side {
            Side::Player => (tuning::SOLDIER_AP, tuning::SOLDIER_WEAPON),
            Side::Enemy => (tuning::ENEMY_AP, tuning::ENEMY_WEAPON),
        };
        Unit {
            id,
            side,
            pos,
            health: tuning::HEALTH,
            health_max: tuning::HEALTH,
            weapon,
            ap: ap_max,
            ap_max,
            alive: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Victory,
    Defeat,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// `path` excludes the unit's current cell and ends at the destination.
    Move { unit: UnitId, path: Vec<GridPos> },
    Shoot { unit: UnitId, target: UnitId },
    EndTurn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattleEvent {
    Stepped {
        unit: UnitId,
        from: GridPos,
        to: GridPos,
    },
    Shot {
        shooter: UnitId,
        target: UnitId,
        hit: bool,
        damage: i32,
    },
    ReactionShot {
        shooter: UnitId,
        target: UnitId,
        hit: bool,
        damage: i32,
    },
    Died(UnitId),
    TurnEnded(Side),
    TurnStarted {
        side: Side,
        turn_number: u32,
    },
    BattleOver(Outcome),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionError {
    BattleOver,
    NotYourTurn,
    NoSuchUnit,
    UnitDead,
    NotEnoughAp { need: i32, have: i32 },
    EmptyPath,
    PathNotWalkable(GridPos),
    PathNotContiguous,
    CellOccupied(GridPos),
    TargetNotVisible,
    OutOfRange { distance: i32, range: i32 },
    SameSide,
    EndTurnOnlyOnPlayerTurn,
}

/// All battle state. Mutated only through `apply` and `end_enemy_turn`.
#[derive(Resource, Clone, Debug)]
pub struct Battle {
    grid: Grid,
    units: Vec<Unit>,
    turn: Side,
    turn_number: u32,
    rng: ChaCha8Rng,
    outcome: Option<Outcome>,
}

impl Battle {
    /// `units` must be ordered so that `units[i].id == UnitId(i)`.
    pub fn new(grid: Grid, units: Vec<Unit>, seed: u64) -> Battle {
        debug_assert!(units.iter().enumerate().all(|(i, u)| u.id.0 as usize == i));
        Battle {
            grid,
            units,
            turn: Side::Player,
            turn_number: 1,
            rng: ChaCha8Rng::seed_from_u64(seed),
            outcome: None,
        }
    }

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    pub fn units(&self) -> &[Unit] {
        &self.units
    }

    pub fn unit(&self, id: UnitId) -> Option<&Unit> {
        self.units.get(id.0 as usize)
    }

    pub fn living(&self, side: Side) -> impl Iterator<Item = &Unit> {
        self.units.iter().filter(move |u| u.alive && u.side == side)
    }

    /// The living unit standing on `pos`, if any.
    pub fn unit_at(&self, pos: GridPos) -> Option<&Unit> {
        self.units.iter().find(|u| u.alive && u.pos == pos)
    }

    pub fn turn(&self) -> Side {
        self.turn
    }

    pub fn turn_number(&self) -> u32 {
        self.turn_number
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    fn living_unit(&self, id: UnitId) -> Result<&Unit, ActionError> {
        let unit = self.unit(id).ok_or(ActionError::NoSuchUnit)?;
        if !unit.alive {
            return Err(ActionError::UnitDead);
        }
        Ok(unit)
    }

    /// Validates and applies one action. On `Err` the state is unchanged.
    pub fn apply(&mut self, action: Action) -> Result<Vec<BattleEvent>, ActionError> {
        if self.outcome.is_some() {
            return Err(ActionError::BattleOver);
        }
        let mut events = Vec::new();
        match action {
            Action::EndTurn => {
                if self.turn != Side::Player {
                    return Err(ActionError::EndTurnOnlyOnPlayerTurn);
                }
                self.switch_turn(&mut events);
            }
            // Task 2 replaces this arm.
            Action::Shoot { .. } => return Err(ActionError::NotYourTurn),
            // Task 3 replaces this arm.
            Action::Move { .. } => return Err(ActionError::NotYourTurn),
        }
        self.check_outcome(&mut events);
        Ok(events)
    }

    /// Ends the enemy turn. Only the phase driver calls this; the UI cannot.
    /// A no-op (empty vec) when it is not the enemy's turn or the battle is over.
    pub fn end_enemy_turn(&mut self) -> Vec<BattleEvent> {
        let mut events = Vec::new();
        if self.turn == Side::Enemy && self.outcome.is_none() {
            self.switch_turn(&mut events);
        }
        events
    }

    fn switch_turn(&mut self, events: &mut Vec<BattleEvent>) {
        events.push(BattleEvent::TurnEnded(self.turn));
        self.turn = self.turn.other();
        if self.turn == Side::Player {
            self.turn_number += 1;
        }
        for unit in self.units.iter_mut().filter(|u| u.alive && u.side == self.turn) {
            unit.ap = unit.ap_max;
        }
        events.push(BattleEvent::TurnStarted {
            side: self.turn,
            turn_number: self.turn_number,
        });
    }

    fn wiped_out(&self, side: Side) -> bool {
        let any = self.units.iter().any(|u| u.side == side);
        any && !self.units.iter().any(|u| u.side == side && u.alive)
    }

    fn check_outcome(&mut self, events: &mut Vec<BattleEvent>) {
        if self.outcome.is_some() {
            return;
        }
        let outcome = if self.wiped_out(Side::Player) {
            Outcome::Defeat
        } else if self.wiped_out(Side::Enemy) {
            Outcome::Victory
        } else {
            return;
        };
        self.outcome = Some(outcome);
        events.push(BattleEvent::BattleOver(outcome));
    }
}

#[cfg(test)]
impl Battle {
    /// Test builder. Grid glyphs as `Grid::from_ascii`, plus `P` (soldier) and `E`
    /// (enemy) standing on floor. Unit ids follow reading order: top row first, left to right.
    pub fn from_ascii(art: &str, seed: u64) -> Battle {
        let grid_art: String = art
            .chars()
            .map(|c| if c == 'P' || c == 'E' { '.' } else { c })
            .collect();
        let grid = Grid::from_ascii(&grid_art);
        let rows: Vec<&str> = art
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::trim)
            .collect();
        let height = rows.len() as i32;
        let mut units = Vec::new();
        for (row_idx, row) in rows.iter().enumerate() {
            let y = height - 1 - row_idx as i32;
            for (x, ch) in row.chars().enumerate() {
                let side = match ch {
                    'P' => Side::Player,
                    'E' => Side::Enemy,
                    _ => continue,
                };
                let id = UnitId(units.len() as u32);
                units.push(Unit::new(id, side, GridPos::new(x as i32, y)));
            }
        }
        Battle::new(grid, units, seed)
    }
}
```
Note: `Grid::from_ascii`, `Cell::WALL` and `Cell::cover` are already `#[cfg(test)]` in `grid.rs`; this test-only impl block is what keeps them reachable. The `std::collections::HashMap` import is added in Task 3, when `reachable` needs it.

- [ ] **Step 4: Run tests**

Run: `cargo test battle:: 2>&1 | tail -8`
Expected: 3 passed. Then `cargo clippy --all-targets -- -D warnings 2>&1 | tail -3` clean and `cargo fmt --all`.

- [ ] **Step 5: Commit**

```bash
git add src/battle.rs src/main.rs
git commit -m "Add battle engine skeleton: units, turns, End Turn

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 2: Shooting, death and battle outcome

**Files:**
- Modify: `src/battle.rs`

**Interfaces:**
- Produces: `Action::Shoot` fully implemented in `apply`; private `validate_shot(shooter, target, reaction) -> Result<(), ActionError>` and `fire(shooter, target, reaction, events)` reused by reaction fire in Task 4.

- [ ] **Step 1: Write the failing tests** (append inside `mod tests`)

```rust
    fn shoot(b: &mut Battle, shooter: u32, target: u32) -> Result<Vec<BattleEvent>, ActionError> {
        b.apply(Action::Shoot {
            unit: UnitId(shooter),
            target: UnitId(target),
        })
    }

    #[test]
    fn shooting_costs_ap_rolls_damage_and_emits_shot() {
        let mut b = Battle::from_ascii("P.E", 3);
        let events = shoot(&mut b, 0, 1).unwrap();
        assert_eq!(events.len(), 1);
        let BattleEvent::Shot {
            shooter,
            target,
            hit,
            damage,
        } = events[0]
        else {
            panic!("expected Shot, got {:?}", events[0]);
        };
        assert_eq!((shooter, target), (UnitId(0), UnitId(1)));
        assert_eq!(hit, damage > 0, "damage is 0 exactly when the shot missed");
        assert_eq!(b.unit(UnitId(0)).unwrap().ap, tuning::SOLDIER_AP - tuning::RIFLE_AP_COST);
        assert_eq!(b.unit(UnitId(1)).unwrap().health, tuning::HEALTH - damage);
        // Second shot allowed (6 - 3 = 3 AP left), third is not.
        shoot(&mut b, 0, 1).unwrap();
        assert_eq!(
            shoot(&mut b, 0, 1),
            Err(ActionError::NotEnoughAp { need: 3, have: 0 })
        );
    }

    #[test]
    fn shot_validation_errors_leave_state_untouched() {
        let mut b = Battle::from_ascii("P.#.E\nP....", 1);
        let before = b.clone();
        assert_eq!(shoot(&mut b, 0, 1), Err(ActionError::TargetNotVisible), "wall between (0,1) and (4,1)");
        assert_eq!(shoot(&mut b, 0, 2), Err(ActionError::SameSide));
        assert_eq!(shoot(&mut b, 1, 0), Err(ActionError::NotYourTurn));
        assert_eq!(shoot(&mut b, 9, 0), Err(ActionError::NoSuchUnit));
        b.units[2].alive = false;
        assert_eq!(shoot(&mut b, 0, 2), Err(ActionError::UnitDead));
        b.units[2].alive = true;
        assert_eq!(b.units, before.units);
        assert_eq!(b.turn(), before.turn());

        let mut far = Battle::from_ascii("P.........E", 1); // distance 10 > range 8
        assert_eq!(
            shoot(&mut far, 0, 1),
            Err(ActionError::OutOfRange { distance: 10, range: 8 })
        );
    }

    #[test]
    fn killing_the_last_enemy_ends_the_battle_and_locks_it() {
        let mut b = Battle::from_ascii("PE", 5);
        b.units[1].health = 1;
        // Keep shooting until it dies; with 0.9 hit chance at distance 1 this takes a shot or two.
        let mut all = Vec::new();
        loop {
            match shoot(&mut b, 0, 1) {
                Ok(ev) => all.extend(ev),
                Err(ActionError::NotEnoughAp { .. }) => {
                    b.apply(Action::EndTurn).unwrap();
                    b.end_enemy_turn();
                }
                Err(e) => panic!("{e:?}"),
            }
            if b.outcome().is_some() {
                break;
            }
        }
        assert!(all.contains(&BattleEvent::Died(UnitId(1))));
        assert_eq!(all.last(), Some(&BattleEvent::BattleOver(Outcome::Victory)));
        assert!(!b.unit(UnitId(1)).unwrap().alive);
        assert_eq!(b.apply(Action::EndTurn), Err(ActionError::BattleOver));
        assert!(b.end_enemy_turn().is_empty());
    }

    #[test]
    fn a_battle_with_one_side_never_ends() {
        let mut b = Battle::from_ascii("P.P", 1);
        let events = b.apply(Action::EndTurn).unwrap();
        assert!(!events.iter().any(|e| matches!(e, BattleEvent::BattleOver(_))));
        assert_eq!(b.outcome(), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test battle:: 2>&1 | tail -20`
Expected: the new tests fail (`NotYourTurn` where `Ok`/other errors expected).

- [ ] **Step 3: Implement**

Replace the `Action::Shoot { .. }` arm in `apply` with:
```rust
            Action::Shoot { unit, target } => {
                self.validate_shot(unit, target, false)?;
                self.fire(unit, target, false, &mut events);
            }
```
Add to `impl Battle` (above `switch_turn`):
```rust
    /// Shared by ordinary and reaction shots. Reaction shots skip the turn check.
    fn validate_shot(
        &self,
        shooter: UnitId,
        target: UnitId,
        reaction: bool,
    ) -> Result<(), ActionError> {
        let s = self.living_unit(shooter)?;
        let t = self.living_unit(target)?;
        if !reaction && s.side != self.turn {
            return Err(ActionError::NotYourTurn);
        }
        if s.side == t.side {
            return Err(ActionError::SameSide);
        }
        if s.ap < s.weapon.ap_cost {
            return Err(ActionError::NotEnoughAp {
                need: s.weapon.ap_cost,
                have: s.ap,
            });
        }
        let distance = s.pos.distance(t.pos);
        if distance > s.weapon.range {
            return Err(ActionError::OutOfRange {
                distance,
                range: s.weapon.range,
            });
        }
        if !self.grid.has_line_of_sight(s.pos, t.pos) {
            return Err(ActionError::TargetNotVisible);
        }
        Ok(())
    }

    /// Resolves an already-validated shot: spends AP, rolls, applies damage, emits events.
    fn fire(&mut self, shooter: UnitId, target: UnitId, reaction: bool, events: &mut Vec<BattleEvent>) {
        let (shooter_pos, weapon) = {
            let s = &self.units[shooter.0 as usize];
            (s.pos, s.weapon)
        };
        let target_pos = self.units[target.0 as usize].pos;
        let distance = shooter_pos.distance(target_pos);
        let in_cover = self.grid.cover_against(target_pos, shooter_pos);
        let chance = rules::hit_chance(distance, weapon.range, in_cover);
        let hit = rules::roll_hit(&mut self.rng, chance);
        let damage = if hit {
            rules::roll_damage(&mut self.rng, weapon.damage_min, weapon.damage_max)
        } else {
            0
        };
        self.units[shooter.0 as usize].ap -= weapon.ap_cost;
        let t = &mut self.units[target.0 as usize];
        t.health -= damage;
        events.push(if reaction {
            BattleEvent::ReactionShot {
                shooter,
                target,
                hit,
                damage,
            }
        } else {
            BattleEvent::Shot {
                shooter,
                target,
                hit,
                damage,
            }
        });
        if t.health <= 0 && t.alive {
            t.alive = false;
            events.push(BattleEvent::Died(target));
        }
    }
```

- [ ] **Step 4: Run tests, clippy, fmt**

Run: `cargo test battle:: 2>&1 | tail -8` → 7 passed. Clippy clean, fmt.

- [ ] **Step 5: Commit**

```bash
git add src/battle.rs
git commit -m "Add shooting, death and battle outcome to the engine

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 3: Movement and the reach/path queries

**Files:**
- Modify: `src/battle.rs`

**Interfaces:**
- Produces: `Action::Move` implemented (one `Stepped` per cell, `MOVE_COST` AP each, full up-front validation); `Battle::reachable(unit) -> HashMap<GridPos, i32>` (cell → AP cost, within AP, occupancy-aware, excludes start); `Battle::path_to(unit, goal) -> Option<Vec<GridPos>>` (`None` if unreachable, occupied, or over AP); private `walk`, `validate_path`, `step_successors`, and `react_to` stub (Task 4 fills it).

- [ ] **Step 1: Write the failing tests** (append inside `mod tests`)

```rust
    fn mv(b: &mut Battle, unit: u32, path: &[(i32, i32)]) -> Result<Vec<BattleEvent>, ActionError> {
        b.apply(Action::Move {
            unit: UnitId(unit),
            path: path.iter().map(|&(x, y)| GridPos::new(x, y)).collect(),
        })
    }

    #[test]
    fn moving_emits_one_step_per_cell_and_spends_ap() {
        let mut b = Battle::from_ascii("P....", 1);
        let events = mv(&mut b, 0, &[(1, 0), (2, 0), (3, 0)]).unwrap();
        assert_eq!(
            events,
            vec![
                BattleEvent::Stepped { unit: UnitId(0), from: GridPos::new(0, 0), to: GridPos::new(1, 0) },
                BattleEvent::Stepped { unit: UnitId(0), from: GridPos::new(1, 0), to: GridPos::new(2, 0) },
                BattleEvent::Stepped { unit: UnitId(0), from: GridPos::new(2, 0), to: GridPos::new(3, 0) },
            ]
        );
        assert_eq!(b.unit(UnitId(0)).unwrap().pos, GridPos::new(3, 0));
        assert_eq!(b.unit(UnitId(0)).unwrap().ap, tuning::SOLDIER_AP - 3);
        assert_eq!(
            mv(&mut b, 0, &[(4, 0), (3, 0), (4, 0), (3, 0)]),
            Err(ActionError::NotEnoughAp { need: 4, have: 3 })
        );
    }

    #[test]
    fn move_validation_errors_leave_state_untouched() {
        let mut b = Battle::from_ascii("P#.\n.P.\n..E", 1);
        let before = b.clone();
        assert_eq!(mv(&mut b, 0, &[]), Err(ActionError::EmptyPath));
        assert_eq!(mv(&mut b, 0, &[(1, 2)]), Err(ActionError::PathNotWalkable(GridPos::new(1, 2))));
        assert_eq!(mv(&mut b, 0, &[(2, 2)]), Err(ActionError::PathNotContiguous), "two cells away");
        // (0,2)->(1,1) is a diagonal past the wall at (1,2): corner cutting is checked before occupancy.
        assert_eq!(mv(&mut b, 0, &[(1, 1)]), Err(ActionError::PathNotContiguous));
        // (1,1)->(2,0) is a legal diagonal, but the enemy stands on (2,0).
        assert_eq!(mv(&mut b, 1, &[(2, 0)]), Err(ActionError::CellOccupied(GridPos::new(2, 0))));
        assert_eq!(mv(&mut b, 2, &[(1, 0)]), Err(ActionError::NotYourTurn));
        assert_eq!(b.units, before.units);
    }

    #[test]
    fn no_corner_cutting_on_moves() {
        let mut b = Battle::from_ascii("#.\nP#", 1);
        assert_eq!(mv(&mut b, 0, &[(1, 1)]), Err(ActionError::PathNotContiguous));
    }

    #[test]
    fn reachable_respects_ap_and_occupancy() {
        let mut b = Battle::from_ascii(".......\n..P.E..\n.......", 1);
        b.units[0].ap = 2;
        let reach = b.reachable(UnitId(0));
        assert_eq!(reach.get(&GridPos::new(3, 1)), Some(&1));
        assert_eq!(reach.get(&GridPos::new(0, 1)), Some(&2));
        assert_eq!(reach.get(&GridPos::new(5, 1)), None, "3 cells away with 2 AP");
        assert_eq!(reach.get(&GridPos::new(4, 1)), None, "the enemy stands there");
        assert_eq!(reach.get(&GridPos::new(2, 1)), None, "start cell excluded");
        // Every in-bounds cell within Chebyshev distance 2 of (2,1): x 0..=4, y 0..=2 = 15,
        // minus the start cell and the enemy's cell.
        assert_eq!(reach.len(), 13);
    }

    #[test]
    fn path_to_is_within_ap_and_avoids_units() {
        let b = Battle::from_ascii("P.E..", 1);
        assert_eq!(b.path_to(UnitId(0), GridPos::new(2, 0)), None, "occupied goal");
        assert_eq!(b.path_to(UnitId(0), GridPos::new(3, 0)), None, "only route is through the enemy on a 1-row map");
        let mut open = Battle::from_ascii("P....\n.....", 1);
        assert_eq!(
            open.path_to(UnitId(0), GridPos::new(2, 0)),
            Some(vec![GridPos::new(1, 0), GridPos::new(2, 0)])
        );
        open.units[0].ap = 1;
        assert_eq!(open.path_to(UnitId(0), GridPos::new(2, 0)), None, "costs 2, has 1");
    }
```
- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test battle:: 2>&1 | tail -20`
Expected: compile error (`reachable`, `path_to` missing) — that is the RED.

- [ ] **Step 3: Implement**

Add `use std::collections::HashMap;` after the inner attribute. Replace the `Action::Move { .. }` arm with:
```rust
            Action::Move { unit, path } => self.walk(unit, &path, &mut events)?,
```
Add to `impl Battle`:
```rust
    fn validate_path(&self, unit: UnitId, path: &[GridPos]) -> Result<(), ActionError> {
        let u = self.living_unit(unit)?;
        if u.side != self.turn {
            return Err(ActionError::NotYourTurn);
        }
        if path.is_empty() {
            return Err(ActionError::EmptyPath);
        }
        let need = path.len() as i32 * tuning::MOVE_COST;
        if u.ap < need {
            return Err(ActionError::NotEnoughAp { need, have: u.ap });
        }
        let mut prev = u.pos;
        for &cell in path {
            if !self.grid.is_walkable(cell) {
                return Err(ActionError::PathNotWalkable(cell));
            }
            if prev.distance(cell) != 1 {
                return Err(ActionError::PathNotContiguous);
            }
            let (dx, dy) = (cell.x - prev.x, cell.y - prev.y);
            let diagonal = dx != 0 && dy != 0;
            if diagonal
                && !(self.grid.is_walkable(GridPos::new(prev.x + dx, prev.y))
                    && self.grid.is_walkable(GridPos::new(prev.x, prev.y + dy)))
            {
                return Err(ActionError::PathNotContiguous);
            }
            if self.unit_at(cell).is_some() {
                return Err(ActionError::CellOccupied(cell));
            }
            prev = cell;
        }
        Ok(())
    }

    fn walk(&mut self, unit: UnitId, path: &[GridPos], events: &mut Vec<BattleEvent>) -> Result<(), ActionError> {
        self.validate_path(unit, path)?;
        for &cell in path {
            let from = self.units[unit.0 as usize].pos;
            {
                let u = &mut self.units[unit.0 as usize];
                u.pos = cell;
                u.ap -= tuning::MOVE_COST;
            }
            events.push(BattleEvent::Stepped { unit, from, to: cell });
            self.react_to(unit, events);
            if !self.units[unit.0 as usize].alive {
                break;
            }
        }
        Ok(())
    }

    /// Reaction fire after a step. Filled in by Task 4.
    fn react_to(&mut self, _mover: UnitId, _events: &mut Vec<BattleEvent>) {}

    /// King-move neighbours that are walkable, corner-safe and unoccupied, unit cost.
    fn step_successors(&self, p: GridPos) -> Vec<(GridPos, i32)> {
        self.grid
            .neighbours(p)
            .into_iter()
            .filter(|(n, _)| self.unit_at(*n).is_none())
            .map(|(n, _)| (n, 1))
            .collect()
    }

    /// Every cell the unit can reach this turn, mapped to its AP cost. Excludes the start cell.
    pub fn reachable(&self, unit: UnitId) -> HashMap<GridPos, i32> {
        let Some(u) = self.unit(unit).filter(|u| u.alive) else {
            return HashMap::new();
        };
        let budget = u.ap / tuning::MOVE_COST;
        pathfinding::directed::dijkstra::dijkstra_all(&u.pos, |p| self.step_successors(*p))
            .into_iter()
            .filter(|(_, (_, steps))| *steps <= budget)
            .map(|(cell, (_, steps))| (cell, steps * tuning::MOVE_COST))
            .collect()
    }

    /// Cheapest path to `goal` avoiding units, or `None` if unreachable or over the unit's AP.
    pub fn path_to(&self, unit: UnitId, goal: GridPos) -> Option<Vec<GridPos>> {
        let u = self.unit(unit).filter(|u| u.alive)?;
        if goal == u.pos || self.unit_at(goal).is_some() || !self.grid.is_walkable(goal) {
            return None;
        }
        let (mut path, steps) = pathfinding::directed::astar::astar(
            &u.pos,
            |p| self.step_successors(*p),
            |p| p.distance(goal),
            |p| *p == goal,
        )?;
        path.remove(0);
        (steps * tuning::MOVE_COST <= u.ap).then_some(path)
    }
```
`dijkstra_all` returns every reachable node except the start, with `(parent, cost)`.

- [ ] **Step 4: Run tests, clippy, fmt**

Run: `cargo test battle:: 2>&1 | tail -8` → 12 passed.

- [ ] **Step 5: Commit**

```bash
git add src/battle.rs
git commit -m "Add movement, reach and path queries to the engine

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 4: Reaction fire, the cover volley, and the replay guarantee

**Files:**
- Modify: `src/battle.rs`

**Interfaces:**
- Produces: `react_to` implemented per spec (every living unit of the other side, ascending id, with `ap >= ap_cost`, in range, with LOS to the mover's new cell, fires one `ReactionShot`; stop when the mover dies).

- [ ] **Step 1: Write the failing tests** (append inside `mod tests`)

```rust
    #[test]
    fn reaction_fire_triggers_on_the_step_that_enters_sight() {
        // Wall column at x=2 on rows 1-2; the enemy at (4,1) sits level with it, so (0,2) and (0,1)
        // are hidden and (0,0) is the first cell it can see. Player walks down the left column, then east.
        let mut b = Battle::from_ascii("P.#..\n..#.E\n.....", 7);
        b.apply(Action::EndTurn).unwrap(); // enemy has full AP (5) = one reaction shot
        b.end_enemy_turn(); // back to player, turn 2, enemy AP still 5
        assert_eq!(b.unit(UnitId(1)).unwrap().ap, tuning::ENEMY_AP);
        let events = mv(&mut b, 0, &[(0, 1), (0, 0), (1, 0)]).unwrap();
        let steps: Vec<_> = events.iter().filter(|e| matches!(e, BattleEvent::Stepped { .. })).collect();
        assert_eq!(steps.len(), 3);
        let reactions: Vec<_> = events
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e, BattleEvent::ReactionShot { .. }))
            .collect();
        assert_eq!(reactions.len(), 1, "one shot: 5 AP - 3 = 2, not enough for a second");
        // The reaction comes right after the step into (0,0), which is the first cell with LOS to (4,0).
        let (idx, _) = reactions[0];
        assert_eq!(
            events[idx - 1],
            BattleEvent::Stepped { unit: UnitId(0), from: GridPos::new(0, 1), to: GridPos::new(0, 0) }
        );
        assert_eq!(b.unit(UnitId(1)).unwrap().ap, tuning::ENEMY_AP - tuning::RIFLE_AP_COST);
    }

    #[test]
    fn no_reaction_without_ap_or_line_of_sight() {
        let mut b = Battle::from_ascii("P...E", 7);
        b.units[1].ap = 2;
        let events = mv(&mut b, 0, &[(1, 0)]).unwrap();
        assert!(!events.iter().any(|e| matches!(e, BattleEvent::ReactionShot { .. })));
        let mut walled = Battle::from_ascii("P.#.E", 7);
        let events = mv(&mut walled, 0, &[(1, 0)]).unwrap();
        assert!(!events.iter().any(|e| matches!(e, BattleEvent::ReactionShot { .. })));
    }

    #[test]
    fn a_mover_killed_by_reaction_fire_stops_where_it_fell() {
        let mut b = Battle::from_ascii("P.........E", 2);
        b.units[0].health = 1;
        b.units[0].ap = 20;
        b.units[1].ap = 30; // a reaction on every in-range step (from (3,0) on); the first hit kills
        let path: Vec<(i32, i32)> = (1..=9).map(|x| (x, 0)).collect();
        let events = mv(&mut b, 0, &path).unwrap();
        let died_at = events.iter().position(|e| *e == BattleEvent::Died(UnitId(0))).expect("dies");
        let steps_after = events[died_at..].iter().filter(|e| matches!(e, BattleEvent::Stepped { .. })).count();
        assert_eq!(steps_after, 0, "no more steps after death");
        assert_eq!(events.last(), Some(&BattleEvent::BattleOver(Outcome::Defeat)));
        assert!(!b.unit(UnitId(0)).unwrap().alive);
        assert!(b.unit(UnitId(0)).unwrap().pos.x < 9, "fell before the end of the path");
    }

    #[test]
    fn cover_halves_reaction_and_direct_hits_over_a_seeded_volley() {
        // Shooter at (2,0) fires north at a target on (2,2). 'v' shelters the south side.
        fn damage_after(seed: u64, target_glyph: char) -> i32 {
            let art = format!("..{target_glyph}..\n.....\n..P..");
            let mut b = Battle::from_ascii(&art, seed);
            // Put the enemy on the cover cell by hand (from_ascii can't stack a unit on a glyph).
            b.units.push(Unit::new(UnitId(1), Side::Enemy, GridPos::new(2, 2)));
            b.units[1].health = 1_000_000;
            let mut total = 0;
            for _ in 0..400 {
                b.units[0].ap = 3;
                let ev = b.apply(Action::Shoot { unit: UnitId(0), target: UnitId(1) }).unwrap();
                if let BattleEvent::Shot { damage, .. } = ev[0] {
                    total += damage;
                }
            }
            total
        }
        let open = damage_after(11, '.');
        let covered = damage_after(11, 'v');
        let wrong_side = damage_after(11, '^');
        assert!(covered < open * 7 / 10, "cover {covered} vs open {open}");
        assert!((wrong_side - open).abs() < open / 5, "wrong side {wrong_side} vs open {open}");
    }

    #[test]
    fn same_seed_and_actions_replay_identically() {
        fn play(seed: u64) -> Vec<BattleEvent> {
            let mut b = Battle::from_ascii("P.....\n......\nE.....", seed);
            let mut log = Vec::new();
            log.extend(b.apply(Action::Move { unit: UnitId(0), path: vec![GridPos::new(1, 2), GridPos::new(2, 2)] }).unwrap());
            log.extend(b.apply(Action::Shoot { unit: UnitId(0), target: UnitId(1) }).unwrap());
            log.extend(b.apply(Action::EndTurn).unwrap());
            log.extend(b.apply(Action::Shoot { unit: UnitId(1), target: UnitId(0) }).unwrap_or_default());
            log
        }
        assert_eq!(play(42), play(42));
        assert_ne!(play(42), play(43), "different seeds diverge (hit rolls)");
    }
```
Note for the last test: the enemy shot at the end is on the enemy's turn (`EndTurn` switched it), so `Shoot` by unit 1 is valid; `unwrap_or_default` only guards the unlikely case the soldier already died.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test battle::reaction 2>&1 | tail -12` and `cargo test battle::a_mover` — the reaction tests fail (no `ReactionShot` events).

- [ ] **Step 3: Implement**

Replace the `react_to` stub with:
```rust
    /// After `mover` steps: every living unit of the other side, in ascending id order,
    /// with enough AP, in range and with line of sight to the mover's new cell fires once.
    /// Stops as soon as the mover dies.
    fn react_to(&mut self, mover: UnitId, events: &mut Vec<BattleEvent>) {
        let mover_side = self.units[mover.0 as usize].side;
        let reactors: Vec<UnitId> = self
            .units
            .iter()
            .filter(|u| u.alive && u.side != mover_side)
            .map(|u| u.id)
            .collect();
        for reactor in reactors {
            if !self.units[mover.0 as usize].alive {
                break;
            }
            if self.validate_shot(reactor, mover, true).is_ok() {
                self.fire(reactor, mover, true, events);
            }
        }
    }
```

- [ ] **Step 4: Run tests, clippy, fmt**

Run: `cargo test battle:: 2>&1 | tail -8` → 17 passed. If `cover_halves_...` fails, check the `from_ascii` glyph row: the target row is the TOP line, so `'v'` at (2,2) shelters its south side and the shooter at (2,0) is south. Do not change thresholds.

- [ ] **Step 5: Commit**

```bash
git add src/battle.rs
git commit -m "Add reaction fire with unspent AP and the replay guarantee

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 5: Targeting queries and symmetric line of sight

**Files:**
- Modify: `src/battle.rs`, `src/grid.rs`

**Interfaces:**
- Produces: `Battle::hit_chance(shooter, target) -> Option<f32>`, `Battle::target_in_cover(shooter, target) -> bool`, `Battle::visible_enemies(unit) -> Vec<UnitId>` (nearest first, ties by id); `Grid::has_line_of_sight` becomes symmetric (closes #15).

- [ ] **Step 1: Write the failing tests**

In `src/grid.rs` `mod tests`:
```rust
    #[test]
    fn line_of_sight_is_symmetric() {
        // A single diagonal wall: Bresenham from one end passes through it, from the other it does not.
        let g = Grid::from_ascii("...\n.#.\n...");
        for (a, b) in [
            (GridPos::new(0, 0), GridPos::new(2, 1)),
            (GridPos::new(0, 2), GridPos::new(2, 1)),
            (GridPos::new(0, 0), GridPos::new(2, 2)),
        ] {
            assert_eq!(g.has_line_of_sight(a, b), g.has_line_of_sight(b, a), "{a:?} <-> {b:?}");
        }
        assert!(g.has_line_of_sight(GridPos::new(0, 0), GridPos::new(2, 1)), "clear in at least one direction");
        assert!(!g.has_line_of_sight(GridPos::new(0, 1), GridPos::new(2, 1)), "straight through the wall stays blocked");
    }
```
In `src/battle.rs` `mod tests`:
```rust
    #[test]
    fn targeting_queries() {
        let b = Battle::from_ascii("P.#E\n....\n..vE", 1);
        // Unit ids: 0 P(0,2), 1 E(3,2), 2 E(3,0). The 'v' at (2,0) is cover but no one stands on it.
        assert_eq!(b.hit_chance(UnitId(0), UnitId(1)), None, "wall at (2,2) blocks");
        let chance = b.hit_chance(UnitId(0), UnitId(2)).expect("visible diagonal");
        assert!((chance - rules::hit_chance(3, 8, false)).abs() < 1e-6);
        assert!(!b.target_in_cover(UnitId(0), UnitId(2)));
        assert_eq!(b.visible_enemies(UnitId(0)), vec![UnitId(2)]);
        assert_eq!(b.hit_chance(UnitId(1), UnitId(2)), None, "same side");
        let mut far = Battle::from_ascii("P.........E", 1);
        assert_eq!(far.hit_chance(UnitId(0), UnitId(1)), None, "out of range");
        assert!(far.visible_enemies(UnitId(0)).is_empty());
        far.units[1].alive = false;
        assert_eq!(far.hit_chance(UnitId(0), UnitId(1)), None, "dead");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test line_of_sight_is_symmetric 2>&1 | tail -6` → FAIL (asymmetric). `cargo test battle::targeting 2>&1 | tail -6` → compile error.

- [ ] **Step 3: Implement**

`src/grid.rs`: replace `has_line_of_sight` with:
```rust
    /// True when no sight-blocking cell lies strictly between `a` and `b`, in either
    /// Bresenham direction (the raster line is not symmetric; visibility must be).
    pub fn has_line_of_sight(&self, a: GridPos, b: GridPos) -> bool {
        self.clear_between(a, b) || self.clear_between(b, a)
    }

    fn clear_between(&self, a: GridPos, b: GridPos) -> bool {
        let cells = line(a, b);
        cells[1..cells.len().saturating_sub(1).max(1)]
            .iter()
            .all(|p| !self.get(*p).is_some_and(|c| c.blocks_sight))
    }
```
`src/battle.rs`, in `impl Battle`:
```rust
    /// Hit probability for a direct shot, or `None` when the shot is impossible
    /// (dead, same side, out of range, no line of sight).
    pub fn hit_chance(&self, shooter: UnitId, target: UnitId) -> Option<f32> {
        let s = self.unit(shooter).filter(|u| u.alive)?;
        let t = self.unit(target).filter(|u| u.alive)?;
        if s.side == t.side {
            return None;
        }
        let distance = s.pos.distance(t.pos);
        if distance > s.weapon.range || !self.grid.has_line_of_sight(s.pos, t.pos) {
            return None;
        }
        Some(rules::hit_chance(distance, s.weapon.range, self.target_in_cover(shooter, target)))
    }

    pub fn target_in_cover(&self, shooter: UnitId, target: UnitId) -> bool {
        match (self.unit(shooter), self.unit(target)) {
            (Some(s), Some(t)) => self.grid.cover_against(t.pos, s.pos),
            _ => false,
        }
    }

    /// Living units of the other side this unit could shoot right now, nearest first.
    pub fn visible_enemies(&self, unit: UnitId) -> Vec<UnitId> {
        let Some(u) = self.unit(unit).filter(|u| u.alive) else {
            return Vec::new();
        };
        let mut out: Vec<(i32, UnitId)> = self
            .living(u.side.other())
            .filter(|t| self.hit_chance(unit, t.id).is_some())
            .map(|t| (u.pos.distance(t.pos), t.id))
            .collect();
        out.sort();
        out.into_iter().map(|(_, id)| id).collect()
    }
```

- [ ] **Step 4: Run the whole suite, clippy, fmt**

Run: `cargo test 2>&1 | tail -4` → all pass (35 old + 18 battle + 1 grid = 54). Existing combat/ai tests still pass with symmetric LOS (their walls are straight lines).

- [ ] **Step 5: Commit**

```bash
git add src/battle.rs src/grid.rs
git commit -m "Add targeting queries and make line of sight symmetric

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 6: Loading a battle from the Tiled map

**Files:**
- Modify: `src/battle.rs`, `src/map.rs`

**Interfaces:**
- Produces: `Battle::from_tiled(map: &tiled::Map, seed: u64) -> Battle`; `map::spawns_from_tiled` now returns `Vec<(battle::Side, GridPos)>` (the old `on_map_created` maps `Side` back to `unit::Faction` locally until Task 9 deletes it).

- [ ] **Step 1: Write the failing test** (in `src/battle.rs` `mod tests`)

```rust
    #[test]
    fn from_tiled_loads_mission01() {
        let map = ::tiled::Loader::new()
            .load_tmx_map("assets/maps/mission01.tmx")
            .expect("mission01.tmx loads");
        let b = Battle::from_tiled(&map, 9);
        assert_eq!((b.grid().width(), b.grid().height()), (20, 15));
        assert_eq!(b.living(Side::Player).count(), 4);
        assert_eq!(b.living(Side::Enemy).count(), 3);
        assert!(b.units().iter().enumerate().all(|(i, u)| u.id == UnitId(i as u32)));
        assert!(b.units().iter().any(|u| u.side == Side::Player && u.pos == GridPos::new(3, 2)));
        assert!(b.units().iter().any(|u| u.side == Side::Enemy && u.pos == GridPos::new(16, 12)));
        assert!(b.units().iter().all(|u| u.ap == u.ap_max && u.alive));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test battle::from_tiled 2>&1 | tail -6` → compile error.

- [ ] **Step 3: Implement**

`src/map.rs`: change the import `use crate::unit::Faction;` to `use crate::battle::Side;`, change `spawns_from_tiled`'s return type to `Vec<(Side, GridPos)>` and its two match arms to `Side::Player` / `Side::Enemy`. In `on_map_created`, change the spawn loop to:
```rust
        for (side, pos) in spawns {
            let faction = match side {
                Side::Player => Faction::Player,
                Side::Enemy => Faction::Enemy,
            };
            commands.spawn(unit_bundle(faction, pos));
        }
```
and keep `use crate::unit::{unit_bundle, Faction};` (merge the two `crate::unit` imports into one line). In `map.rs` tests, replace `Faction::Player`/`Faction::Enemy` with `Side::Player`/`Side::Enemy`.

`src/battle.rs`, in `impl Battle` (the non-test one), after `new`:
```rust
    /// Builds the battle from a Tiled map: tile properties give the grid, the `spawns`
    /// object layer gives the units, in the order Tiled lists them.
    pub fn from_tiled(map: &::tiled::Map, seed: u64) -> Battle {
        let grid = crate::map::grid_from_tiled(map);
        let units = crate::map::spawns_from_tiled(map)
            .into_iter()
            .enumerate()
            .map(|(i, (side, pos))| Unit::new(UnitId(i as u32), side, pos))
            .collect();
        Battle::new(grid, units, seed)
    }
```

- [ ] **Step 4: Run the whole suite, clippy, fmt**

Run: `cargo test 2>&1 | tail -4` → 55 passed.

- [ ] **Step 5: Commit**

```bash
git add src/battle.rs src/map.rs
git commit -m "Load a battle from the Tiled map; spawns carry battle::Side

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 7: The enemy planner

**Files:**
- Create: `src/planner.rs`
- Modify: `src/main.rs` (`mod planner;` after `mod orders;`)

**Interfaces:**
- Consumes: `Battle` queries (`turn`, `outcome`, `living`, `unit`, `unit_at`, `visible_enemies`, `reachable`, `path_to`, `grid().cover_against/has_line_of_sight/find_path`).
- Produces: `planner::plan_enemy_action(&Battle) -> Option<Action>`.

- [ ] **Step 1: Write the failing tests**

Create `src/planner.rs`:
```rust
//! Enemy turn planner: returns the single next action for the enemy side,
//! or `None` when no enemy has anything useful left to do (= end the turn).
//! Pure: reads only `Battle`'s public queries.
#![allow(dead_code)] // removed in Task 9

use crate::battle::{Action, Battle, Side, Unit, UnitId, tuning};
use crate::grid::GridPos;

#[cfg(test)]
mod tests {
    use super::*;

    fn enemy_turn(art: &str) -> Battle {
        let mut b = Battle::from_ascii(art, 1);
        b.apply(Action::EndTurn).unwrap();
        b
    }

    #[test]
    fn none_on_the_player_turn_or_when_over() {
        let b = Battle::from_ascii("P.E", 1);
        assert_eq!(plan_enemy_action(&b), None);
    }

    #[test]
    fn shoots_when_already_in_cover_against_the_target() {
        // Enemy stands on '<' which shelters its west side; player is west.
        let mut b = enemy_turn("P...<");
        b.units.push(Unit::new(UnitId(1), Side::Enemy, GridPos::new(4, 0)));
        b.units[1].ap = tuning::ENEMY_AP;
        assert_eq!(
            plan_enemy_action(&b),
            Some(Action::Shoot { unit: UnitId(1), target: UnitId(0) })
        );
    }

    #[test]
    fn takes_reachable_cover_facing_the_player_then_shoots() {
        // '<' at (4,1) shelters its west side from the player at (0,1). Enemy at (5,1) is 1 step away.
        let mut b = enemy_turn(".......\nP...<.E\n.......");
        let action = plan_enemy_action(&b).expect("moves");
        let Action::Move { unit, ref path } = action else { panic!("expected Move, got {action:?}") };
        assert_eq!(unit, UnitId(1));
        assert_eq!(path.len(), 2, "two cells from (6,1) to the cover at (4,1)");
        assert_eq!(path.last(), Some(&GridPos::new(4, 1)));
        b.apply(action).unwrap();
        assert_eq!(
            plan_enemy_action(&b),
            Some(Action::Shoot { unit: UnitId(1), target: UnitId(0) }),
            "now in cover with 5-2=3 AP: shoot"
        );
        b.apply(plan_enemy_action(&b).unwrap()).unwrap();
        assert_eq!(plan_enemy_action(&b), None, "0 AP left: end turn");
    }

    #[test]
    fn shoots_directly_when_no_cover_is_reachable() {
        let b = enemy_turn("P....E");
        assert_eq!(
            plan_enemy_action(&b),
            Some(Action::Shoot { unit: UnitId(1), target: UnitId(0) })
        );
    }

    #[test]
    fn advances_toward_an_unseen_player_keeping_a_shot_in_hand() {
        // Wall column at x=3 (all rows) except a gap... no gap: enemy cannot see or reach; so use a
        // long open row where the player is simply out of range (distance 10 > range 7).
        let b = enemy_turn("P.........E");
        let action = plan_enemy_action(&b).expect("advances");
        let Action::Move { unit, path } = action else { panic!("expected Move, got {action:?}") };
        assert_eq!(unit, UnitId(1));
        assert_eq!(path.len(), (tuning::ENEMY_AP - tuning::RIFLE_AP_COST) as usize, "5 AP - 3 for a shot = 2 steps");
        assert_eq!(path.last(), Some(&GridPos::new(8, 0)));
    }

    #[test]
    fn skips_spent_enemies_and_ends_when_all_are_spent() {
        let mut b = enemy_turn("P...E\n....E");
        b.units[1].ap = 0;
        assert_eq!(
            plan_enemy_action(&b),
            Some(Action::Shoot { unit: UnitId(2), target: UnitId(0) })
        );
        b.units[2].ap = 0;
        assert_eq!(plan_enemy_action(&b), None);
    }
}
```
Add `mod planner;` to `src/main.rs` after `mod orders;`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test planner:: 2>&1 | tail -6` → compile error (`plan_enemy_action` missing).

- [ ] **Step 3: Implement** (insert above the tests)

```rust
pub fn plan_enemy_action(battle: &Battle) -> Option<Action> {
    if battle.turn() != Side::Enemy || battle.outcome().is_some() {
        return None;
    }
    battle
        .living(Side::Enemy)
        .filter(|e| e.ap > 0)
        .find_map(|e| plan_for(battle, e))
}

fn plan_for(battle: &Battle, enemy: &Unit) -> Option<Action> {
    let shot_cost = enemy.weapon.ap_cost;
    if let Some(&target) = battle.visible_enemies(enemy.id).first() {
        let target_pos = battle.unit(target)?.pos;
        let in_cover = battle.grid().cover_against(enemy.pos, target_pos);
        let cover_path = if in_cover { None } else { best_cover_path(battle, enemy, target_pos) };
        if enemy.ap >= shot_cost && (in_cover || cover_path.is_none()) {
            return Some(Action::Shoot { unit: enemy.id, target });
        }
        return cover_path.map(|path| Action::Move { unit: enemy.id, path });
    }
    // Nothing visible: close in on the nearest soldier, keeping enough AP for one shot.
    let nearest = battle
        .living(Side::Player)
        .min_by_key(|p| (enemy.pos.distance(p.pos), p.id))?;
    let steps = (enemy.ap - shot_cost) / tuning::MOVE_COST;
    if steps <= 0 {
        return None;
    }
    let full = battle.grid().find_path(enemy.pos, nearest.pos)?;
    let mut path = Vec::new();
    for cell in full.into_iter().take(steps as usize) {
        if battle.unit_at(cell).is_some() {
            break;
        }
        path.push(cell);
    }
    (!path.is_empty()).then_some(Action::Move { unit: enemy.id, path })
}

/// Cheapest reachable cell that shelters the enemy from `target_pos`, keeps line of sight
/// and range to it, and leaves AP for a shot. Ties: lowest y, then lowest x.
fn best_cover_path(battle: &Battle, enemy: &Unit, target_pos: GridPos) -> Option<Vec<GridPos>> {
    let grid = battle.grid();
    let mut candidates: Vec<(i32, i32, i32, GridPos)> = battle
        .reachable(enemy.id)
        .into_iter()
        .filter(|(cell, cost)| {
            enemy.ap - cost >= enemy.weapon.ap_cost
                && cell.distance(target_pos) <= enemy.weapon.range
                && grid.cover_against(*cell, target_pos)
                && grid.has_line_of_sight(*cell, target_pos)
        })
        .map(|(cell, cost)| (cost, cell.y, cell.x, cell))
        .collect();
    candidates.sort();
    let (_, _, _, goal) = *candidates.first()?;
    battle.path_to(enemy.id, goal)
}
```

- [ ] **Step 4: Run tests, clippy, fmt**

Run: `cargo test planner:: 2>&1 | tail -8` → 6 passed.

- [ ] **Step 5: Commit**

```bash
git add src/planner.rs src/main.rs
git commit -m "Add the enemy turn planner

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 8: Phase driver and animator (not yet wired)

**Files:**
- Create: `src/phase.rs`, `src/animator.rs`
- Modify: `src/main.rs` (`mod animator;` after `mod ai;`; `mod phase;` after `mod orders;`)

**Interfaces:**
- Produces:
  - `phase::AppState { Loading, PlayerInput, Animating, EnemyTurn }` (States, default Loading) — NOTE: this coexists with the old `state::AppState` until Task 9; nothing initialises it yet.
  - `phase::EventQueue(pub VecDeque<BattleEvent>)`, `phase::BattleSeed(pub u64)` resources.
  - `phase::submit(battle, queue, next, action) -> Result<(), ActionError>`.
  - Systems `phase::after_animation` (in `Animating`), `phase::drive_enemy_turn` (in `EnemyTurn`).
  - `animator::UnitSprite(pub UnitId)` component; `animator::ShotFired { from, to, hit, reaction }` message; `animator::Animation` resource; `animator::animate` system; constants `WALK_SPEED = 160.0`, `SHOT_HOLD = 0.25`, `REACTION_HOLD = 0.4`, `TURN_HOLD = 0.5`.

- [ ] **Step 1: Write `src/phase.rs` with its failing test**

```rust
//! Battle phase state machine. Input applies actions during `PlayerInput`; every
//! action's events go through `Animating`; the enemy turn alternates planner + animation.
#![allow(dead_code)] // removed in Task 9

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::battle::{Action, ActionError, Battle, BattleEvent, Side};
use crate::planner::plan_enemy_action;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    Loading,
    PlayerInput,
    Animating,
    EnemyTurn,
}

/// Events waiting to be animated, in order.
#[derive(Resource, Default, Debug)]
pub struct EventQueue(pub VecDeque<BattleEvent>);

#[derive(Resource, Clone, Copy, Debug)]
pub struct BattleSeed(pub u64);

/// Applies a player action; on success queues its events and moves to `Animating`.
pub fn submit(
    battle: &mut Battle,
    queue: &mut EventQueue,
    next: &mut NextState<AppState>,
    action: Action,
) -> Result<(), ActionError> {
    let events = battle.apply(action)?;
    queue.0.extend(events);
    next.set(AppState::Animating);
    Ok(())
}

/// Once the queue is drained: hand control to whichever side's turn it is.
/// A finished battle stays in `Animating` with an empty queue until `R` restarts it.
pub fn after_animation(
    battle: Res<Battle>,
    queue: Res<EventQueue>,
    mut next: ResMut<NextState<AppState>>,
) {
    if !queue.0.is_empty() || battle.outcome().is_some() {
        return;
    }
    next.set(match battle.turn() {
        Side::Enemy => AppState::EnemyTurn,
        Side::Player => AppState::PlayerInput,
    });
}

/// One planner step per visit: apply the next enemy action (or end the enemy turn),
/// queue the events, animate. Runs exactly once per entry into `EnemyTurn`.
pub fn drive_enemy_turn(
    mut battle: ResMut<Battle>,
    mut queue: ResMut<EventQueue>,
    mut next: ResMut<NextState<AppState>>,
) {
    let events = match plan_enemy_action(&battle) {
        Some(action) => match battle.apply(action.clone()) {
            Ok(events) => events,
            Err(err) => {
                warn!("planner produced an invalid action {action:?}: {err:?}; ending enemy turn");
                battle.end_enemy_turn()
            }
        },
        None => battle.end_enemy_turn(),
    };
    queue.0.extend(events);
    next.set(AppState::Animating);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    /// Stand-in for the animator: drains the queue instantly.
    fn drain(mut queue: ResMut<EventQueue>) {
        queue.0.clear();
    }

    fn app(art: &str) -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.insert_resource(Battle::from_ascii(art, 1));
        app.init_resource::<EventQueue>();
        app.insert_state(AppState::PlayerInput);
        app.add_systems(
            Update,
            (drain, after_animation).chain().run_if(in_state(AppState::Animating)),
        );
        app.add_systems(Update, drive_enemy_turn.run_if(in_state(AppState::EnemyTurn)));
        app
    }

    fn state(app: &App) -> AppState {
        *app.world().resource::<State<AppState>>().get()
    }

    #[test]
    fn end_turn_runs_the_enemy_turn_and_returns_control() {
        let mut app = app("P.......E");
        // Same three steps as `submit`, done on the world's resources one at a time.
        let events = app.world_mut().resource_mut::<Battle>().apply(Action::EndTurn).unwrap();
        app.world_mut().resource_mut::<EventQueue>().0.extend(events);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Animating);
        for _ in 0..60 {
            app.update();
            if state(&app) == AppState::PlayerInput && app.world().resource::<Battle>().turn_number() == 2 {
                break;
            }
        }
        assert_eq!(state(&app), AppState::PlayerInput);
        assert_eq!(app.world().resource::<Battle>().turn_number(), 2);
        assert!(app.world().resource::<EventQueue>().0.is_empty());
        // The enemy did something on its turn (it saw the soldier at distance 8 > range 7, so it advanced).
        assert_ne!(app.world().resource::<Battle>().unit(crate::battle::UnitId(1)).unwrap().pos, crate::grid::GridPos::new(8, 0));
    }

    #[test]
    fn a_finished_battle_stays_in_animating() {
        let mut app = app("P");
        app.world_mut()
            .resource_mut::<Battle>()
            .force_outcome_for_test(crate::battle::Outcome::Defeat);
        app.insert_state(AppState::Animating);
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(state(&app), AppState::Animating);
    }
}
```
The second test needs a test-only mutator on `Battle`: add to the `#[cfg(test)] impl Battle` block in `battle.rs`:
```rust
    pub fn force_outcome_for_test(&mut self, outcome: Outcome) {
        self.outcome = Some(outcome);
    }
```

- [ ] **Step 2: Write `src/animator.rs`** (no unit test; verified in Task 9 by play)

```rust
//! Plays `BattleEvent`s from the queue at real-time pace: walks sprites cell by cell,
//! holds on shots, tints the dead. The only code that moves unit sprites.
#![allow(dead_code)] // removed in Task 9

use bevy::prelude::*;

use crate::battle::{BattleEvent, Side, UnitId};
use crate::phase::EventQueue;

pub const WALK_SPEED: f32 = 160.0;
pub const SHOT_HOLD: f32 = 0.25;
pub const REACTION_HOLD: f32 = 0.4;
pub const TURN_HOLD: f32 = 0.5;

/// Marks the sprite entity that draws a battle unit.
#[derive(Component, Clone, Copy, Debug)]
pub struct UnitSprite(pub UnitId);

/// Emitted once per shot so tracers and audio can react.
#[derive(Message, Clone, Copy, Debug)]
pub struct ShotFired {
    pub from: Vec2,
    pub to: Vec2,
    pub hit: bool,
    pub reaction: bool,
}

/// The event currently being played, if any.
#[derive(Resource, Default, Debug)]
pub struct Animation(Option<Playing>);

#[derive(Debug)]
enum Playing {
    Walk { unit: UnitId, to: Vec2 },
    Hold { remaining: f32 },
}

fn sprite_pos(sprites: &Query<(&UnitSprite, &mut Transform, &mut Sprite)>, id: UnitId) -> Option<Vec2> {
    sprites
        .iter()
        .find(|(s, _, _)| s.0 == id)
        .map(|(_, tf, _)| tf.translation.truncate())
}

fn face(transform: &mut Transform, from: Vec2, to: Vec2) {
    let delta = to - from;
    if delta.length_squared() > 0.001 {
        transform.rotation = Quat::from_rotation_z(delta.to_angle());
    }
}

pub fn animate(
    time: Res<Time>,
    mut queue: ResMut<EventQueue>,
    mut current: ResMut<Animation>,
    mut shots: MessageWriter<ShotFired>,
    mut sprites: Query<(&UnitSprite, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();

    // Advance whatever is in flight.
    if let Some(playing) = &mut current.0 {
        let done = match playing {
            Playing::Walk { unit, to } => {
                let mut arrived = true;
                for (sprite, mut transform, _) in &mut sprites {
                    if sprite.0 != *unit {
                        continue;
                    }
                    let here = transform.translation.truncate();
                    let delta = *to - here;
                    let step = WALK_SPEED * dt;
                    if delta.length() <= step {
                        transform.translation.x = to.x;
                        transform.translation.y = to.y;
                    } else {
                        let movement = delta.normalize() * step;
                        transform.translation.x += movement.x;
                        transform.translation.y += movement.y;
                        arrived = false;
                    }
                }
                arrived
            }
            Playing::Hold { remaining } => {
                *remaining -= dt;
                *remaining <= 0.0
            }
        };
        if !done {
            return;
        }
        current.0 = None;
    }

    // Start the next event.
    let Some(event) = queue.0.pop_front() else {
        return;
    };
    match event {
        BattleEvent::Stepped { unit, to, .. } => {
            let target = to.to_world();
            for (sprite, mut transform, _) in &mut sprites {
                if sprite.0 == unit {
                    let here = transform.translation.truncate();
                    face(&mut transform, here, target);
                }
            }
            current.0 = Some(Playing::Walk { unit, to: target });
        }
        BattleEvent::Shot { shooter, target, hit, .. } | BattleEvent::ReactionShot { shooter, target, hit, .. } => {
            let reaction = matches!(event, BattleEvent::ReactionShot { .. });
            let (Some(from), Some(to)) = (sprite_pos(&sprites, shooter), sprite_pos(&sprites, target)) else {
                return;
            };
            for (sprite, mut transform, _) in &mut sprites {
                if sprite.0 == shooter {
                    face(&mut transform, from, to);
                }
            }
            shots.write(ShotFired { from, to, hit, reaction });
            current.0 = Some(Playing::Hold {
                remaining: if reaction { REACTION_HOLD } else { SHOT_HOLD },
            });
        }
        BattleEvent::Died(unit) => {
            for (sprite, mut transform, mut image) in &mut sprites {
                if sprite.0 == unit {
                    image.color = Color::srgb(0.35, 0.35, 0.35);
                    transform.translation.z = 5.0;
                }
            }
        }
        BattleEvent::TurnStarted { side: Side::Enemy, .. } => {
            current.0 = Some(Playing::Hold { remaining: TURN_HOLD });
        }
        BattleEvent::TurnStarted { .. } | BattleEvent::TurnEnded(_) | BattleEvent::BattleOver(_) => {}
    }
}
```

- [ ] **Step 3: Add the mods, run tests, clippy, fmt**

Add `mod animator;` (after `mod ai;`) and `mod phase;` (after `mod orders;`) to `src/main.rs`.
Run: `cargo test phase:: 2>&1 | tail -8` → 2 passed; `cargo test 2>&1 | tail -3` → 57 passed. Clippy clean (the two new modules have the module-level allow).

- [ ] **Step 4: Commit**

```bash
git add src/phase.rs src/animator.rs src/battle.rs src/main.rs
git commit -m "Add the phase driver and event animator

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 9: The switch — wire the engine in, delete the old simulation

**Files:**
- Delete: `src/unit.rs`, `src/movement.rs`, `src/combat.rs`, `src/ai.rs`, `src/sim.rs`, `src/state.rs`, `src/test_support.rs`
- Modify: `src/map.rs`, `src/assets.rs`, `src/rules.rs`, `src/audio.rs`, `src/battle.rs`, `src/planner.rs`, `src/phase.rs`, `src/animator.rs`, `src/main.rs`
- Rewrite: `src/orders.rs`, `src/render.rs`, `src/ui.rs`, `src/presentation.rs`

**Interfaces:**
- Consumes everything from Tasks 1–8.
- Produces: a playable turn-based game. `orders::Selected(pub Option<UnitId>)` resource; `map::spawn_unit_sprites(commands, battle, assets)`; `phase::restart` system.

- [ ] **Step 1: Delete the old modules and the interim allows**

```bash
git rm -q src/unit.rs src/movement.rs src/combat.rs src/ai.rs src/sim.rs src/state.rs src/test_support.rs
sed -i '/^#!\[allow(dead_code)\]/d' src/battle.rs src/planner.rs src/phase.rs src/animator.rs
```
In `src/rules.rs`, put `#[cfg(test)]` on the `GameRng` struct AND its `impl` block (only tests use it now).

- [ ] **Step 2: `src/main.rs`**

```rust
mod animator;
mod assets;
mod audio;
mod battle;
mod camera;
mod debug;
mod grid;
mod map;
mod orders;
mod phase;
mod planner;
mod presentation;
mod render;
mod rules;
mod ui;

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledPlugin;

use crate::phase::{AppState, BattleSeed, EventQueue};
use crate::presentation::PresentationPlugin;

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
        .insert_resource(BattleSeed(seed))
        .init_resource::<EventQueue>()
        .add_plugins(PresentationPlugin)
        .run();
}

/// `--seed N` replays the same dice: with identical inputs the battle is identical.
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

- [ ] **Step 3: `src/map.rs` — build the battle and its sprites**

Replace the imports and the two systems (keep `grid_from_tiled`, `spawns_from_tiled` and the tests):
```rust
use ::tiled::PropertyValue;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

use crate::animator::UnitSprite;
use crate::assets::GameAssets;
use crate::battle::{Battle, Side};
use crate::grid::{CoverSides, Grid, GridPos};
use crate::phase::BattleSeed;
use crate::render::UNIT_SPRITE_SIZE;
```
```rust
/// Spawns the map entity anchored so grid cell (0,0) is the bottom-left tile at world (0,0).
/// Unit sprites sit at z=10 (dead at 5, hover label at 20), so map layers stay below that.
pub fn spawn_map(mut commands: Commands, assets: Res<GameAssets>) {
    commands.spawn((
        Name::new("map"),
        TiledMap(assets.map.clone()),
        TilemapAnchor::BottomLeft,
        TiledMapLayerZOffset(1.0),
    ));
}

/// When the map finishes loading: build the `Battle` resource and one sprite per unit.
pub fn on_map_created(
    mut commands: Commands,
    mut events: MessageReader<TiledEvent<MapCreated>>,
    maps: Res<Assets<TiledMapAsset>>,
    assets: Res<GameAssets>,
    seed: Res<BattleSeed>,
) {
    for event in events.read() {
        let Some(map) = event.get_map(&maps) else {
            continue;
        };
        let battle = Battle::from_tiled(map, seed.0);
        info!(
            "map ready: {}x{} grid, {} units, seed {}",
            battle.grid().width(),
            battle.grid().height(),
            battle.units().len(),
            seed.0
        );
        spawn_unit_sprites(&mut commands, &battle, &assets);
        commands.insert_resource(battle);
    }
}

/// One sprite entity per unit, tagged with its `UnitId`. Also used by restart.
pub fn spawn_unit_sprites(commands: &mut Commands, battle: &Battle, assets: &GameAssets) {
    for unit in battle.units() {
        let (name, image) = match unit.side {
            Side::Player => ("Soldier", assets.soldier.clone()),
            Side::Enemy => ("Raider", assets.enemy.clone()),
        };
        commands.spawn((
            Name::new(format!("{name} {}", unit.id.0)),
            UnitSprite(unit.id),
            Sprite {
                image,
                custom_size: Some(Vec2::splat(UNIT_SPRITE_SIZE)),
                ..default()
            },
            Transform::from_translation(unit.pos.to_world().extend(10.0)),
        ));
    }
}
```

- [ ] **Step 4: `src/assets.rs`**

Replace the `Grid`/`AppState` imports with `use crate::battle::Battle;` and `use crate::phase::AppState;`, and `check_loaded` with:
```rust
/// Moves to the player's first turn once the images are in and the map produced a `Battle`.
pub fn check_loaded(
    server: Res<AssetServer>,
    assets: Res<GameAssets>,
    battle: Option<Res<Battle>>,
    mut next: ResMut<NextState<AppState>>,
) {
    let images_ready = server.is_loaded_with_dependencies(&assets.soldier)
        && server.is_loaded_with_dependencies(&assets.enemy);
    if images_ready && battle.is_some() {
        info!("assets loaded, your turn");
        next.set(AppState::PlayerInput);
    }
}
```

- [ ] **Step 5: `src/phase.rs` — add restart**

Add imports `use crate::assets::GameAssets; use crate::animator::{Animation, UnitSprite}; use bevy_ecs_tiled::prelude::TiledMapAsset;` and the system:
```rust
/// `R`: rebuild the battle from the map with the same seed and respawn the sprites.
pub fn restart(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    battle: Option<ResMut<Battle>>,
    assets: Option<Res<GameAssets>>,
    maps: Res<Assets<TiledMapAsset>>,
    seed: Res<BattleSeed>,
    mut queue: ResMut<EventQueue>,
    mut animation: ResMut<Animation>,
    sprites: Query<Entity, With<UnitSprite>>,
    mut next: ResMut<NextState<AppState>>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    let (Some(mut battle), Some(assets)) = (battle, assets) else {
        return;
    };
    let Some(map) = maps.get(&assets.map).map(|asset| &asset.map) else {
        return;
    };
    for entity in &sprites {
        commands.entity(entity).despawn();
    }
    *battle = Battle::from_tiled(map, seed.0);
    queue.0.clear();
    *animation = Animation::default();
    crate::map::spawn_unit_sprites(&mut commands, &battle, &assets);
    info!("battle restarted with seed {}", seed.0);
    next.set(AppState::PlayerInput);
}
```
`Animation` needs `Default` (it has it) and its inner field is private to `animator`; `Animation::default()` is enough.

- [ ] **Step 6: `src/orders.rs` — rewrite**

```rust
//! Player input during `PlayerInput`: selection, hover, and turning clicks into actions.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::animator::UnitSprite;
use crate::battle::{Action, Battle, Side, UnitId};
use crate::grid::GridPos;
use crate::phase::{self, AppState, EventQueue};
use crate::render::UNIT_SPRITE_SIZE;

const PICK_RADIUS: f32 = UNIT_SPRITE_SIZE * 0.6;

/// The soldier the player is giving orders to.
#[derive(Resource, Default, Debug)]
pub struct Selected(pub Option<UnitId>);

fn cursor_world(window: &Window, camera: &Camera, camera_tf: &GlobalTransform) -> Option<Vec2> {
    let cursor = window.cursor_position()?;
    camera.viewport_to_world_2d(camera_tf, cursor).ok()
}

/// Living unit whose sprite is under the cursor, nearest first.
fn unit_under(
    battle: &Battle,
    sprites: &Query<(&UnitSprite, &Transform)>,
    world: Vec2,
) -> Option<UnitId> {
    sprites
        .iter()
        .filter(|(s, _)| battle.unit(s.0).is_some_and(|u| u.alive))
        .map(|(s, tf)| (tf.translation.truncate().distance(world), s.0))
        .filter(|(d, _)| *d <= PICK_RADIUS)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

#[allow(clippy::too_many_arguments)]
pub fn player_input(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    sprites: Query<(&UnitSprite, &Transform)>,
    mut battle: ResMut<Battle>,
    mut queue: ResMut<EventQueue>,
    mut selected: ResMut<Selected>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Enter) {
        if let Err(err) = phase::submit(&mut battle, &mut queue, &mut next, Action::EndTurn) {
            info!("end turn rejected: {err:?}");
        }
        return;
    }

    // Number keys select the nth living soldier by unit id.
    let hotkeys = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];
    for (i, key) in hotkeys.iter().enumerate() {
        if keys.just_pressed(*key) {
            let soldiers: Vec<UnitId> = battle.living(Side::Player).map(|u| u.id).collect();
            if let Some(&id) = soldiers.get(i) {
                selected.0 = Some(id);
            }
            return;
        }
    }

    let left = mouse.just_pressed(MouseButton::Left);
    let right = mouse.just_pressed(MouseButton::Right);
    if !left && !right {
        return;
    }
    let (cam, cam_tf) = *camera;
    let Some(world) = cursor_world(&window, cam, cam_tf) else {
        return;
    };
    let under = unit_under(&battle, &sprites, world);

    if left {
        selected.0 = under.filter(|id| battle.unit(*id).is_some_and(|u| u.side == Side::Player));
        return;
    }

    let Some(unit) = selected.0 else {
        return;
    };
    let action = match under {
        Some(target) if battle.unit(target).is_some_and(|u| u.side == Side::Enemy) => {
            Action::Shoot { unit, target }
        }
        _ => {
            let goal = GridPos::from_world(world);
            let Some(path) = battle.path_to(unit, goal) else {
                return;
            };
            Action::Move { unit, path }
        }
    };
    if let Err(err) = phase::submit(&mut battle, &mut queue, &mut next, action) {
        info!("action rejected: {err:?}");
    }
}
```

- [ ] **Step 7: `src/render.rs` — rewrite overlays and tracers over the battle**

```rust
//! Gizmo overlays (selection ring, health bars, AP pips) and shot tracers, read from the `Battle`.
//! Planning previews (reach, path, hit chance) are added in Task 10.

use bevy::color::palettes::css::{LIME, ORANGE, RED, WHITE, YELLOW};
use bevy::prelude::*;

use crate::animator::{ShotFired, UnitSprite};
use crate::battle::{Battle, Side};
use crate::orders::Selected;

pub const UNIT_SPRITE_SIZE: f32 = 48.0;

#[derive(Component)]
pub struct Tracer {
    pub from: Vec2,
    pub to: Vec2,
    pub hit: bool,
    pub reaction: bool,
    pub ttl: f32,
}

/// Selection ring, health bar and AP pips over every living unit.
pub fn draw_overlays(
    mut gizmos: Gizmos,
    battle: Res<Battle>,
    selected: Res<Selected>,
    sprites: Query<(&UnitSprite, &Transform)>,
) {
    for (sprite, transform) in &sprites {
        let Some(unit) = battle.unit(sprite.0).filter(|u| u.alive) else {
            continue;
        };
        let p = transform.translation.truncate();
        if selected.0 == Some(unit.id) {
            gizmos.circle_2d(Isometry2d::from_translation(p), UNIT_SPRITE_SIZE * 0.6, WHITE);
        }
        let width = UNIT_SPRITE_SIZE;
        let left = p + Vec2::new(-width / 2.0, UNIT_SPRITE_SIZE * 0.65);
        let frac = (unit.health.max(0) as f32 / unit.health_max as f32).clamp(0.0, 1.0);
        let colour = match unit.side {
            Side::Player => LIME,
            Side::Enemy => RED,
        };
        gizmos.line_2d(left, left + Vec2::X * width, Color::srgb(0.2, 0.2, 0.2));
        gizmos.line_2d(left, left + Vec2::X * width * frac, colour);
        // AP pips: one short tick per remaining AP, under the health bar.
        let pip = width / unit.ap_max.max(1) as f32;
        let row = left + Vec2::new(0.0, -5.0);
        for i in 0..unit.ap.max(0) {
            let x0 = row + Vec2::X * (i as f32 * pip + 1.0);
            gizmos.line_2d(x0, x0 + Vec2::X * (pip - 2.0), Color::srgb(0.9, 0.9, 0.5));
        }
    }
}

pub fn spawn_tracers(mut commands: Commands, mut shots: MessageReader<ShotFired>) {
    for shot in shots.read() {
        commands.spawn(Tracer {
            from: shot.from,
            to: shot.to,
            hit: shot.hit,
            reaction: shot.reaction,
            ttl: if shot.reaction { 0.2 } else { 0.12 },
        });
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
        let colour = match (tracer.reaction, tracer.hit) {
            (true, _) => RED,
            (false, true) => ORANGE,
            (false, false) => YELLOW,
        };
        gizmos.line_2d(tracer.from, tracer.to, colour);
    }
}
```

- [ ] **Step 8: `src/ui.rs` — rewrite**

Change the import to `use crate::battle::{Outcome, Side}; use crate::battle::Battle; use crate::phase::AppState;`, the hint text to
`"LMB select  hover = path/AP or hit%  RMB move/shoot  1-4 squad  Enter end turn  R restart  WASD pan  wheel zoom"`, and `update_overlay` to:
```rust
pub fn update_overlay(
    state: Res<State<AppState>>,
    battle: Option<Res<Battle>>,
    mut text: Single<&mut Text, With<StatusText>>,
) {
    let label = match (state.get(), battle.as_deref()) {
        (AppState::Loading, _) | (_, None) => "Loading...".to_string(),
        (_, Some(b)) => match (b.outcome(), b.turn()) {
            (Some(Outcome::Victory), _) => "VICTORY".to_string(),
            (Some(Outcome::Defeat), _) => "DEFEAT".to_string(),
            (None, Side::Enemy) => "ENEMY TURN".to_string(),
            (None, Side::Player) => format!("YOUR TURN {}", b.turn_number()),
        },
    };
    if text.0 != label {
        text.0 = label;
    }
}
```

- [ ] **Step 9: `src/audio.rs`** — change `use crate::sim::ShotFired;` to `use crate::animator::ShotFired;`. Nothing else.

- [ ] **Step 10: `src/presentation.rs` — rewrite**

```rust
//! Everything with a window: loading, map, sprites, camera, input, overlays, UI, audio, debug.

use bevy::prelude::*;
use bevy_kira_audio::prelude::AudioApp;

use crate::animator::{self, Animation, ShotFired};
use crate::orders::Selected;
use crate::phase::{self, AppState};
use crate::{assets, audio, camera, debug, map, orders, render, ui};

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        debug::add_inspector(app);
        app.add_audio_channel::<audio::Sfx>();
        app.add_message::<ShotFired>();
        app.init_resource::<Animation>();
        app.init_resource::<Selected>();
        app.add_systems(
            OnEnter(AppState::Loading),
            (
                assets::load_assets,
                map::spawn_map,
                camera::spawn_camera,
                ui::spawn_overlay,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (map::on_map_created, assets::check_loaded)
                .chain()
                .run_if(in_state(AppState::Loading)),
        )
        .add_systems(
            Update,
            orders::player_input.run_if(in_state(AppState::PlayerInput)),
        )
        .add_systems(
            Update,
            (animator::animate, phase::after_animation)
                .chain()
                .run_if(in_state(AppState::Animating)),
        )
        .add_systems(
            Update,
            phase::drive_enemy_turn.run_if(in_state(AppState::EnemyTurn)),
        )
        .add_systems(
            Update,
            (
                render::draw_overlays.run_if(resource_exists::<crate::battle::Battle>),
                render::spawn_tracers,
                render::draw_tracers,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                camera::pan_camera,
                camera::zoom_camera,
                debug::screenshot_and_exit,
                ui::update_overlay,
                audio::play_shot_sounds,
                phase::restart,
            ),
        );
    }
}
```

- [ ] **Step 11: Build, test, lint, and play**

```bash
cargo test 2>&1 | tail -4        # expect: grid 13, rules 4, map 2, battle 19, planner 6, phase 2 = 46
cargo clippy --all-targets -- -D warnings 2>&1 | tail -3
cargo clippy --all-targets --features inspector -- -D warnings 2>&1 | tail -3
cargo fmt --all --check
WT_SCREENSHOT=/tmp/claude-1000/-home-mortlock-src/302ad263-77ce-466c-8f05-4a78fe90fc83/scratchpad/shot-s2-9.png \
  timeout 120 cargo run --features dynamic -- --seed 1 2>&1 | grep -E "map ready|your turn|panicked|ERROR"
```
Expected: log `map ready: 20x15 grid, 7 units, seed 1` then `assets loaded, your turn`; the screenshot shows the map, all seven units with health bars, AP pips under each, and `YOUR TURN 1` top-left. Nothing moves on its own (it is the player's turn). Then run without the env var and play: select a soldier, right-click a cell within a few tiles (it walks, AP pips drop), Enter (enemies act one at a time, `ENEMY TURN` shows, reaction shots draw red), `R` restarts. If anything panics, the log line is the finding; do not tune constants.

- [ ] **Step 12: Commit**

```bash
git add -A
git commit -m "Switch the game to the turn-based engine; delete the real-time simulation

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 10: Planning previews — reach, path cost, hit chance

**Files:**
- Modify: `src/orders.rs` (hover), `src/render.rs` (previews, hover label), `src/presentation.rs` (wiring)

**Interfaces:**
- Produces: `orders::Hover { cell: Option<GridPos>, unit: Option<UnitId> }` resource updated every frame in `PlayerInput` by `orders::track_hover`; `render::HoverLabel` component + `render::spawn_hover_label` (OnEnter Loading) + `render::draw_previews` system.

- [ ] **Step 1: Hover tracking in `src/orders.rs`**

Add:
```rust
/// What the cursor is over this frame, for previews.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Hover {
    pub cell: Option<GridPos>,
    pub unit: Option<UnitId>,
}

pub fn track_hover(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    sprites: Query<(&UnitSprite, &Transform)>,
    battle: Res<Battle>,
    mut hover: ResMut<Hover>,
) {
    let (cam, cam_tf) = *camera;
    let Some(world) = cursor_world(&window, cam, cam_tf) else {
        *hover = Hover::default();
        return;
    };
    let cell = GridPos::from_world(world);
    hover.cell = battle.grid().in_bounds(cell).then_some(cell);
    hover.unit = unit_under(&battle, &sprites, world);
}
```

- [ ] **Step 2: Previews in `src/render.rs`**

Add imports `use crate::grid::TILE_SIZE; use crate::orders::Hover;` and:
```rust
/// World-space text that follows the hover (path cost or hit chance).
#[derive(Component)]
pub struct HoverLabel;

pub fn spawn_hover_label(mut commands: Commands) {
    commands.spawn((
        Name::new("hover label"),
        HoverLabel,
        Text2d::new(""),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_xyz(0.0, 0.0, 20.0),
        Visibility::Hidden,
    ));
}

/// Reachable cells, hovered path with AP cost, hit chance on a hovered enemy.
pub fn draw_previews(
    mut gizmos: Gizmos,
    battle: Res<Battle>,
    selected: Res<Selected>,
    hover: Res<Hover>,
    mut label: Single<(&mut Text2d, &mut Transform, &mut Visibility), With<HoverLabel>>,
) {
    let (text, transform, visibility) = &mut *label;
    **visibility = Visibility::Hidden;
    let Some(unit) = selected.0 else {
        return;
    };
    if battle.unit(unit).is_none_or(|u| !u.alive || u.side != Side::Player) {
        return;
    }

    let reach = battle.reachable(unit);
    for cell in reach.keys() {
        gizmos.rect_2d(
            Isometry2d::from_translation(cell.to_world()),
            Vec2::splat(TILE_SIZE - 10.0),
            Color::srgba(0.4, 0.8, 1.0, 0.35),
        );
    }

    let mut show = |what: String, at: Vec2| {
        text.0 = what;
        transform.translation.x = at.x;
        transform.translation.y = at.y;
        **visibility = Visibility::Visible;
    };

    if let Some(target) = hover.unit.filter(|t| *t != unit) {
        if let Some(t) = battle.unit(target).filter(|u| u.alive && u.side == Side::Enemy) {
            let at = t.pos.to_world() + Vec2::new(0.0, UNIT_SPRITE_SIZE * 0.95);
            match battle.hit_chance(unit, target) {
                Some(chance) => {
                    let cover = if battle.target_in_cover(unit, target) { "  COVER" } else { "" };
                    show(format!("{:.0}%{cover}", chance * 100.0), at);
                }
                None => {
                    let s = battle.unit(unit).expect("checked");
                    let why = if s.pos.distance(t.pos) > s.weapon.range { "out of range" } else { "no LOS" };
                    show(why.to_string(), at);
                }
            }
            return;
        }
    }

    if let Some(goal) = hover.cell {
        if let Some(path) = battle.path_to(unit, goal) {
            let mut prev = battle.unit(unit).expect("checked").pos.to_world();
            for cell in &path {
                let here = cell.to_world();
                gizmos.line_2d(prev, here, Color::srgb(0.6, 0.9, 1.0));
                prev = here;
            }
            gizmos.circle_2d(Isometry2d::from_translation(prev), 6.0, LIME);
            let cost = path.len() as i32 * crate::battle::tuning::MOVE_COST;
            show(format!("{cost} AP"), prev + Vec2::new(0.0, TILE_SIZE * 0.55));
        }
    }
}
```

- [ ] **Step 3: Wire in `src/presentation.rs`**

Add `use crate::orders::Hover;`, `app.init_resource::<Hover>();`, append `render::spawn_hover_label` to the `OnEnter(AppState::Loading)` chain, change the player-input registration to
`(orders::track_hover, orders::player_input).chain().run_if(in_state(AppState::PlayerInput))`, and add `render::draw_previews.run_if(in_state(AppState::PlayerInput))` to the overlay chain after `render::draw_overlays`.

- [ ] **Step 4: Build, lint, screenshot, play**

```bash
cargo test 2>&1 | tail -3 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -2 && cargo fmt --all --check
WT_SCREENSHOT=/tmp/claude-1000/-home-mortlock-src/302ad263-77ce-466c-8f05-4a78fe90fc83/scratchpad/shot-s2-10.png \
  timeout 120 cargo run --features dynamic -- --seed 1 2>&1 | grep -E "panicked|ERROR"
```
The screenshot has nothing selected, so it looks like Task 9's. Then play: press `1` — a ring appears on soldier 0 and a diamond of translucent squares tints the cells it can reach (6 AP); hover a cell — a light path and `N AP` label; hover a robot from a soldier with line of sight — `NN%` or `NN%  COVER`; hover one behind a wall — `no LOS`. Report what you saw; the human confirms.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "Add reach, path-cost and hit-chance previews

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
```

---

### Task 11: Docs, issue closure, CI

**Files:**
- Modify: `README.md`, `docs/superpowers/specs/2026-09-06-wasteland-tactics-design.md` (one-line status note at the top)

- [ ] **Step 1: README**

Replace the Controls paragraph with:
```markdown
Squad turn-based: each soldier has action points (1 per cell moved, 3 per shot).
Left click or keys 1–4 select a soldier; hovering shows reachable cells, the path
and its AP cost, or the hit chance on an enemy. Right click moves or shoots.
Enter ends your turn; enemies then act. Unspent AP fires reaction shots at enemies
that move into view during their turn. R restarts, WASD pans, mouse wheel zooms.
```
Change the seed line's comment to `# same seed + same clicks = same battle`, and the Status section to:
```markdown
Slice 2 (squad turn-based core loop) — see `docs/superpowers/specs/2026-09-06-slice2-turn-based-design.md`.
```
At the top of the slice 1 spec, under the title, add: `> Superseded on the core loop by `2026-09-06-slice2-turn-based-design.md` (squad turn-based). Rules, assets and layout still apply.`

- [ ] **Step 2: Final checks, commit, push, CI**

```bash
cargo test 2>&1 | tail -3
cargo clippy --all-targets -- -D warnings 2>&1 | tail -2
cargo clippy --all-targets --features inspector -- -D warnings 2>&1 | tail -2
cargo fmt --all --check
git add -A
git commit -m "Document the turn-based loop; close superseded follow-ups

Closes #14
Closes #15
Closes #16
Closes #17
Closes #18
Closes #19
Closes #20
Closes #27
Closes #28
Closes #30

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014oNqx4puogf1FiC3PpPA3b"
git push -u origin slice-2
gh auth switch --user jmortlock
gh run list --repo jmortlock/wasteland-tactics --branch slice-2 --limit 1
# then: gh run watch <id> --repo jmortlock/wasteland-tactics --exit-status
gh auth switch --user jmortlock_lmg
```
Expected: CI green on `slice-2`. Issues close when the branch merges to main.

---

## Not in this plan

Weapon variety, stances, objectives, more maps, persistence, save/undo, fog of war. Open follow-ups after this slice: #21 cover-tile art, #22 register_type, #23 edge-scroll, #24 audio handles in loading, #25 (partly done: overlays are gated on `Battle`), #26 map reload idempotency, #29 tracer ordering (now chained — close it too if the reviewer agrees).
