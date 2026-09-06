//! Executes `Order::Attack`: closes distance if needed, then fires on cooldown.
//! Shots are instant-hit; `ShotFired` lets presentation draw and sound them.

use bevy::prelude::*;

use crate::grid::{Grid, GridPos};
use crate::rules::{self, GameRng};
use crate::sim::ShotFired;
use crate::unit::{AttackCooldown, Dead, Health, Order, Path, Weapon};

#[allow(clippy::type_complexity)]
pub fn resolve_attacks(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    mut rng: ResMut<GameRng>,
    mut shots: MessageWriter<ShotFired>,
    mut attackers: Query<
        (
            Entity,
            &GridPos,
            &Weapon,
            &mut AttackCooldown,
            &mut Order,
            &mut Transform,
            Option<&Path>,
        ),
        Without<Dead>,
    >,
    mut targets: Query<(&GridPos, &mut Health), Without<Dead>>,
) {
    let dt = time.delta_secs();
    for (entity, pos, weapon, mut cooldown, mut order, mut transform, path) in &mut attackers {
        cooldown.0 = (cooldown.0 - dt).max(0.0);
        let Order::Attack(target) = *order else {
            continue;
        };
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

#[cfg(test)]
mod tests {
    use crate::grid::{Grid, GridPos};
    use crate::test_support::*;
    use crate::unit::{Dead, Faction, Health, Order, Path};

    fn health(app: &bevy::prelude::App, e: bevy::prelude::Entity) -> i32 {
        app.world().entity(e).get::<Health>().unwrap().current
    }

    fn set_health(app: &mut bevy::prelude::App, e: bevy::prelude::Entity, hp: i32) {
        app.world_mut()
            .entity_mut(e)
            .get_mut::<Health>()
            .unwrap()
            .current = hp;
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
        assert!(
            health(&app, t) < 100_000 - 5 * 20,
            "many shots landed over 10 s"
        );
        assert_eq!(
            order(&app, a),
            Order::Attack(t),
            "keeps attacking a living target"
        );
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
        assert!(
            app.world().entity(a).get::<Path>().is_some(),
            "no LOS: starts walking"
        );
        for _ in 0..40 {
            tick(&mut app, 0.1);
        }
        assert!(health(&app, t) < 100_000, "eventually gets a shot off");
        assert!(
            app.world().entity(a).get::<Path>().is_none(),
            "stops walking once in range with LOS"
        );
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
        assert!(
            (wrong_side - open).abs() < open / 5,
            "cover facing away does nothing: {wrong_side} vs {open}"
        );
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
