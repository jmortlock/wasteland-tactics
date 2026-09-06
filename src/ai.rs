//! Enemy behaviour: idle → see a player → take nearby cover → shoot.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::grid::{Grid, GridPos};
use crate::unit::{Dead, Faction, Order, Path, SightRange};

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
