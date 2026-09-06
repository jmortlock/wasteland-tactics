//! The pure turn-based battle engine. No Bevy systems, no entities: units are
//! addressed by `UnitId`, which indexes `Battle::units`. Presentation applies
//! `Action`s and animates the returned `BattleEvent`s; nothing else mutates state.
#![allow(dead_code)] // removed in Task 9 when the engine is wired into the app

use std::collections::HashMap;

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
    Move {
        unit: UnitId,
        path: Vec<GridPos>,
    },
    Shoot {
        unit: UnitId,
        target: UnitId,
    },
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
            Action::Shoot { unit, target } => {
                self.validate_shot(unit, target, false)?;
                self.fire(unit, target, false, &mut events);
            }
            Action::Move { unit, path } => self.walk(unit, &path, &mut events)?,
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
    fn fire(
        &mut self,
        shooter: UnitId,
        target: UnitId,
        reaction: bool,
        events: &mut Vec<BattleEvent>,
    ) {
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

    fn walk(
        &mut self,
        unit: UnitId,
        path: &[GridPos],
        events: &mut Vec<BattleEvent>,
    ) -> Result<(), ActionError> {
        self.validate_path(unit, path)?;
        for &cell in path {
            let from = self.units[unit.0 as usize].pos;
            {
                let u = &mut self.units[unit.0 as usize];
                u.pos = cell;
                u.ap -= tuning::MOVE_COST;
            }
            events.push(BattleEvent::Stepped {
                unit,
                from,
                to: cell,
            });
            self.react_to(unit, events);
            if !self.units[unit.0 as usize].alive {
                break;
            }
        }
        Ok(())
    }

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

    fn switch_turn(&mut self, events: &mut Vec<BattleEvent>) {
        events.push(BattleEvent::TurnEnded(self.turn));
        self.turn = self.turn.other();
        if self.turn == Side::Player {
            self.turn_number += 1;
        }
        for unit in self
            .units
            .iter_mut()
            .filter(|u| u.alive && u.side == self.turn)
        {
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
        assert!(
            !b.grid().is_walkable(GridPos::new(1, 1)),
            "walls still parse"
        );
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
        assert_eq!(
            b.unit(UnitId(0)).unwrap().ap,
            2,
            "unspent AP carries into the enemy turn"
        );
        assert_eq!(
            b.apply(Action::EndTurn),
            Err(ActionError::EndTurnOnlyOnPlayerTurn)
        );

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
        assert_eq!(
            b.unit(UnitId(0)).unwrap().ap,
            tuning::SOLDIER_AP,
            "player AP refilled on their turn"
        );
        assert_eq!(
            b.unit(UnitId(1)).unwrap().ap,
            0,
            "enemy AP untouched until its turn"
        );
        assert!(b.end_enemy_turn().is_empty(), "no-op on the player's turn");
    }

    #[test]
    fn dead_units_do_not_get_ap_and_are_not_at_their_cell() {
        let mut b = Battle::from_ascii("P.E", 1);
        b.units[1].alive = false;
        b.apply(Action::EndTurn).unwrap();
        assert_eq!(
            b.unit(UnitId(1)).unwrap().ap,
            tuning::ENEMY_AP,
            "unchanged, not refilled"
        );
        assert!(b.unit_at(GridPos::new(2, 0)).is_none());
        assert_eq!(b.living(Side::Enemy).count(), 0);
        assert_eq!(b.unit(UnitId(7)), None);
    }

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
        assert_eq!(
            b.unit(UnitId(0)).unwrap().ap,
            tuning::SOLDIER_AP - tuning::RIFLE_AP_COST
        );
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
        assert_eq!(
            shoot(&mut b, 0, 1),
            Err(ActionError::TargetNotVisible),
            "wall between (0,1) and (4,1)"
        );
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
            Err(ActionError::OutOfRange {
                distance: 10,
                range: 8
            })
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
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, BattleEvent::BattleOver(_)))
        );
        assert_eq!(b.outcome(), None);
    }

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
                BattleEvent::Stepped {
                    unit: UnitId(0),
                    from: GridPos::new(0, 0),
                    to: GridPos::new(1, 0)
                },
                BattleEvent::Stepped {
                    unit: UnitId(0),
                    from: GridPos::new(1, 0),
                    to: GridPos::new(2, 0)
                },
                BattleEvent::Stepped {
                    unit: UnitId(0),
                    from: GridPos::new(2, 0),
                    to: GridPos::new(3, 0)
                },
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
        assert_eq!(
            mv(&mut b, 0, &[(1, 2)]),
            Err(ActionError::PathNotWalkable(GridPos::new(1, 2)))
        );
        assert_eq!(
            mv(&mut b, 0, &[(2, 2)]),
            Err(ActionError::PathNotContiguous),
            "two cells away"
        );
        // (0,2)->(1,1) is a diagonal past the wall at (1,2): corner cutting is checked before occupancy.
        assert_eq!(
            mv(&mut b, 0, &[(1, 1)]),
            Err(ActionError::PathNotContiguous)
        );
        // (1,1)->(2,0) is a legal diagonal, but the enemy stands on (2,0).
        assert_eq!(
            mv(&mut b, 1, &[(2, 0)]),
            Err(ActionError::CellOccupied(GridPos::new(2, 0)))
        );
        assert_eq!(mv(&mut b, 2, &[(1, 0)]), Err(ActionError::NotYourTurn));
        assert_eq!(b.units, before.units);
    }

    #[test]
    fn no_corner_cutting_on_moves() {
        let mut b = Battle::from_ascii("#.\nP#", 1);
        assert_eq!(
            mv(&mut b, 0, &[(1, 1)]),
            Err(ActionError::PathNotContiguous)
        );
    }

    #[test]
    fn reachable_respects_ap_and_occupancy() {
        let mut b = Battle::from_ascii(".......\n..P.E..\n.......", 1);
        b.units[0].ap = 2;
        let reach = b.reachable(UnitId(0));
        assert_eq!(reach.get(&GridPos::new(3, 1)), Some(&1));
        assert_eq!(reach.get(&GridPos::new(0, 1)), Some(&2));
        assert_eq!(
            reach.get(&GridPos::new(5, 1)),
            None,
            "3 cells away with 2 AP"
        );
        assert_eq!(
            reach.get(&GridPos::new(4, 1)),
            None,
            "the enemy stands there"
        );
        assert_eq!(reach.get(&GridPos::new(2, 1)), None, "start cell excluded");
        // Every in-bounds cell within Chebyshev distance 2 of (2,1): x 0..=4, y 0..=2 = 15,
        // minus the start cell and the enemy's cell.
        assert_eq!(reach.len(), 13);
    }

    #[test]
    fn path_to_is_within_ap_and_avoids_units() {
        let b = Battle::from_ascii("P.E..", 1);
        assert_eq!(
            b.path_to(UnitId(0), GridPos::new(2, 0)),
            None,
            "occupied goal"
        );
        assert_eq!(
            b.path_to(UnitId(0), GridPos::new(3, 0)),
            None,
            "only route is through the enemy on a 1-row map"
        );
        let mut open = Battle::from_ascii("P....\n.....", 1);
        assert_eq!(
            open.path_to(UnitId(0), GridPos::new(2, 0)),
            Some(vec![GridPos::new(1, 0), GridPos::new(2, 0)])
        );
        open.units[0].ap = 1;
        assert_eq!(
            open.path_to(UnitId(0), GridPos::new(2, 0)),
            None,
            "costs 2, has 1"
        );
    }

    #[test]
    fn reaction_fire_triggers_on_the_step_that_enters_sight() {
        // Wall column at x=2 on rows 1-2; the enemy at (4,1) sits level with it, so (0,2) and (0,1)
        // are hidden and (0,0) is the first cell it can see. Player walks down the left column, then east.
        let mut b = Battle::from_ascii("P.#..\n..#.E\n.....", 7);
        b.apply(Action::EndTurn).unwrap(); // enemy has full AP (5) = one reaction shot
        b.end_enemy_turn(); // back to player, turn 2, enemy AP still 5
        assert_eq!(b.unit(UnitId(1)).unwrap().ap, tuning::ENEMY_AP);
        let events = mv(&mut b, 0, &[(0, 1), (0, 0), (1, 0)]).unwrap();
        let steps: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, BattleEvent::Stepped { .. }))
            .collect();
        assert_eq!(steps.len(), 3);
        let reactions: Vec<_> = events
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e, BattleEvent::ReactionShot { .. }))
            .collect();
        assert_eq!(
            reactions.len(),
            1,
            "one shot: 5 AP - 3 = 2, not enough for a second"
        );
        // The reaction comes right after the step into (0,0), which is the first cell with LOS to (4,0).
        let (idx, _) = reactions[0];
        assert_eq!(
            events[idx - 1],
            BattleEvent::Stepped {
                unit: UnitId(0),
                from: GridPos::new(0, 1),
                to: GridPos::new(0, 0)
            }
        );
        assert_eq!(
            b.unit(UnitId(1)).unwrap().ap,
            tuning::ENEMY_AP - tuning::RIFLE_AP_COST
        );
    }

    #[test]
    fn no_reaction_without_ap_or_line_of_sight() {
        let mut b = Battle::from_ascii("P...E", 7);
        b.units[1].ap = 2;
        let events = mv(&mut b, 0, &[(1, 0)]).unwrap();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, BattleEvent::ReactionShot { .. }))
        );
        let mut walled = Battle::from_ascii("P.#.E", 7);
        let events = mv(&mut walled, 0, &[(1, 0)]).unwrap();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, BattleEvent::ReactionShot { .. }))
        );
    }

    #[test]
    fn a_mover_killed_by_reaction_fire_stops_where_it_fell() {
        let mut b = Battle::from_ascii("P.........E", 2);
        b.units[0].health = 1;
        b.units[0].ap = 20;
        b.units[1].ap = 30; // a reaction on every in-range step (from (3,0) on); the first hit kills
        let path: Vec<(i32, i32)> = (1..=9).map(|x| (x, 0)).collect();
        let events = mv(&mut b, 0, &path).unwrap();
        let died_at = events
            .iter()
            .position(|e| *e == BattleEvent::Died(UnitId(0)))
            .expect("dies");
        let steps_after = events[died_at..]
            .iter()
            .filter(|e| matches!(e, BattleEvent::Stepped { .. }))
            .count();
        assert_eq!(steps_after, 0, "no more steps after death");
        assert_eq!(
            events.last(),
            Some(&BattleEvent::BattleOver(Outcome::Defeat))
        );
        assert!(!b.unit(UnitId(0)).unwrap().alive);
        assert!(
            b.unit(UnitId(0)).unwrap().pos.x < 9,
            "fell before the end of the path"
        );
    }

    #[test]
    fn cover_halves_reaction_and_direct_hits_over_a_seeded_volley() {
        // Shooter at (2,0) fires north at a target on (2,2). 'v' shelters the south side.
        fn damage_after(seed: u64, target_glyph: char) -> i32 {
            let art = format!("..{target_glyph}..\n.....\n..P..");
            let mut b = Battle::from_ascii(&art, seed);
            // Put the enemy on the cover cell by hand (from_ascii can't stack a unit on a glyph).
            b.units
                .push(Unit::new(UnitId(1), Side::Enemy, GridPos::new(2, 2)));
            b.units[1].health = 1_000_000;
            let mut total = 0;
            for _ in 0..400 {
                b.units[0].ap = 3;
                let ev = b
                    .apply(Action::Shoot {
                        unit: UnitId(0),
                        target: UnitId(1),
                    })
                    .unwrap();
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
        assert!(
            (wrong_side - open).abs() < open / 5,
            "wrong side {wrong_side} vs open {open}"
        );
    }

    #[test]
    fn same_seed_and_actions_replay_identically() {
        fn play(seed: u64) -> Vec<BattleEvent> {
            let mut b = Battle::from_ascii("P.....\n......\nE.....", seed);
            let mut log = Vec::new();
            log.extend(
                b.apply(Action::Move {
                    unit: UnitId(0),
                    path: vec![GridPos::new(1, 2), GridPos::new(2, 2)],
                })
                .unwrap(),
            );
            log.extend(
                b.apply(Action::Shoot {
                    unit: UnitId(0),
                    target: UnitId(1),
                })
                .unwrap(),
            );
            log.extend(b.apply(Action::EndTurn).unwrap());
            log.extend(
                b.apply(Action::Shoot {
                    unit: UnitId(1),
                    target: UnitId(0),
                })
                .unwrap_or_default(),
            );
            log
        }
        assert_eq!(play(42), play(42));
        assert_ne!(play(42), play(43), "different seeds diverge (hit rolls)");
    }
}
