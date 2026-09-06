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
}
