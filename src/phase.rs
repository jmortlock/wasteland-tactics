//! Battle phase state machine. Input applies actions during `PlayerInput`; every
//! action's events go through `Animating`; the enemy turn alternates planner + animation.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledMapAsset;

use crate::animator::{Animation, UnitSprite};
use crate::assets::GameAssets;
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

/// `R`: rebuild the battle from the map with the same seed and respawn the sprites.
#[allow(clippy::too_many_arguments)]
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
            (drain, after_animation)
                .chain()
                .run_if(in_state(AppState::Animating)),
        );
        app.add_systems(
            Update,
            drive_enemy_turn.run_if(in_state(AppState::EnemyTurn)),
        );
        app
    }

    fn state(app: &App) -> AppState {
        *app.world().resource::<State<AppState>>().get()
    }

    #[test]
    fn end_turn_runs_the_enemy_turn_and_returns_control() {
        let mut app = app("P.......E");
        // Same three steps as `submit`, done on the world's resources one at a time.
        let events = app
            .world_mut()
            .resource_mut::<Battle>()
            .apply(Action::EndTurn)
            .unwrap();
        app.world_mut()
            .resource_mut::<EventQueue>()
            .0
            .extend(events);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Animating);
        for _ in 0..60 {
            app.update();
            if state(&app) == AppState::PlayerInput
                && app.world().resource::<Battle>().turn_number() == 2
            {
                break;
            }
        }
        assert_eq!(state(&app), AppState::PlayerInput);
        assert_eq!(app.world().resource::<Battle>().turn_number(), 2);
        assert!(app.world().resource::<EventQueue>().0.is_empty());
        // The enemy did something on its turn (it saw the soldier at distance 8 > range 7, so it advanced).
        assert_ne!(
            app.world()
                .resource::<Battle>()
                .unit(crate::battle::UnitId(1))
                .unwrap()
                .pos,
            crate::grid::GridPos::new(8, 0)
        );
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
