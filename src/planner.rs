//! Enemy turn planner: returns the single next action for the enemy side,
//! or `None` when no enemy has anything useful left to do (= end the turn).
//! Pure: reads only `Battle`'s public queries.
#![allow(dead_code)] // removed in Task 9

use crate::battle::{Action, Battle, Side, Unit, tuning};
use crate::grid::GridPos;

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
        let cover_path = if in_cover {
            None
        } else {
            best_cover_path(battle, enemy, target_pos)
        };
        if enemy.ap >= shot_cost && (in_cover || cover_path.is_none()) {
            return Some(Action::Shoot {
                unit: enemy.id,
                target,
            });
        }
        return cover_path.map(|path| Action::Move {
            unit: enemy.id,
            path,
        });
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
    (!path.is_empty()).then_some(Action::Move {
        unit: enemy.id,
        path,
    })
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
    candidates.sort_by_key(|t| (t.0, t.1, t.2));
    let (_, _, _, goal) = *candidates.first()?;
    battle.path_to(enemy.id, goal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::UnitId;

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
        b.units_mut_for_test()
            .push(Unit::new(UnitId(1), Side::Enemy, GridPos::new(4, 0)));
        b.units_mut_for_test()[1].ap = tuning::ENEMY_AP;
        assert_eq!(
            plan_enemy_action(&b),
            Some(Action::Shoot {
                unit: UnitId(1),
                target: UnitId(0)
            })
        );
    }

    #[test]
    fn takes_reachable_cover_facing_the_player_then_shoots() {
        // '<' at (4,1) shelters its west side from the player at (0,1). Enemy at (5,1) is 1 step away.
        let mut b = enemy_turn(".......\nP...<.E\n.......");
        let action = plan_enemy_action(&b).expect("moves");
        let Action::Move { unit, ref path } = action else {
            panic!("expected Move, got {action:?}")
        };
        assert_eq!(unit, UnitId(1));
        assert_eq!(path.len(), 2, "two cells from (6,1) to the cover at (4,1)");
        assert_eq!(path.last(), Some(&GridPos::new(4, 1)));
        b.apply(action).unwrap();
        assert_eq!(
            plan_enemy_action(&b),
            Some(Action::Shoot {
                unit: UnitId(1),
                target: UnitId(0)
            }),
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
            Some(Action::Shoot {
                unit: UnitId(1),
                target: UnitId(0)
            })
        );
    }

    #[test]
    fn advances_toward_an_unseen_player_keeping_a_shot_in_hand() {
        // Wall column at x=3 (all rows) except a gap... no gap: enemy cannot see or reach; so use a
        // long open row where the player is simply out of range (distance 10 > range 7).
        let b = enemy_turn("P.........E");
        let action = plan_enemy_action(&b).expect("advances");
        let Action::Move { unit, path } = action else {
            panic!("expected Move, got {action:?}")
        };
        assert_eq!(unit, UnitId(1));
        assert_eq!(
            path.len(),
            (tuning::ENEMY_AP - tuning::RIFLE_AP_COST) as usize,
            "5 AP - 3 for a shot = 2 steps"
        );
        assert_eq!(path.last(), Some(&GridPos::new(8, 0)));
    }

    #[test]
    fn skips_spent_enemies_and_ends_when_all_are_spent() {
        let mut b = enemy_turn("P...E\n....E");
        b.units_mut_for_test()[1].ap = 0;
        assert_eq!(
            plan_enemy_action(&b),
            Some(Action::Shoot {
                unit: UnitId(2),
                target: UnitId(0)
            })
        );
        b.units_mut_for_test()[2].ap = 0;
        assert_eq!(plan_enemy_action(&b), None);
    }
}
