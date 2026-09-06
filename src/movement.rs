//! Turns `Order::MoveTo` into a `Path`, then walks units along it.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::grid::{Grid, GridPos};
use crate::unit::{Dead, Order, Path, Speed};

/// Gives every unit with a fresh `MoveTo` order a `Path`, or cancels the order if there is none.
#[allow(clippy::type_complexity)]
pub fn plan_paths(
    mut commands: Commands,
    grid: Res<Grid>,
    mut units: Query<(Entity, &GridPos, &mut Order), (Without<Path>, Without<Dead>)>,
) {
    for (entity, pos, mut order) in &mut units {
        let Order::MoveTo(goal) = *order else {
            continue;
        };
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
#[allow(clippy::type_complexity)]
pub fn follow_paths(
    mut commands: Commands,
    time: Res<Time>,
    mut set: ParamSet<(
        Query<(Entity, &GridPos), Without<Dead>>,
        Query<
            (
                Entity,
                &mut GridPos,
                &mut Transform,
                &mut Path,
                &mut Order,
                &Speed,
            ),
            Without<Dead>,
        >,
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
        let t = app
            .world()
            .entity(u)
            .get::<bevy::prelude::Transform>()
            .unwrap();
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
