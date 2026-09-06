//! Top-level app state and the pause toggle. Mission end is added in a later task.

use bevy::prelude::*;

use crate::unit::{Dead, Faction};

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
    outcome: Option<Res<MissionOutcome>>,
) {
    if outcome.is_some() {
        return;
    }
    if !keys.just_pressed(KeyCode::Space) {
        return;
    }
    match state.get() {
        AppState::Playing => next.set(AppState::Paused),
        AppState::Paused => next.set(AppState::Playing),
        AppState::Loading => {}
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Grid, GridPos};
    use crate::test_support::*;
    use crate::unit::{Faction, Health, Order};

    fn kill(app: &mut App, e: Entity) {
        app.world_mut()
            .entity_mut(e)
            .get_mut::<Health>()
            .unwrap()
            .current = 0;
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
        assert_eq!(
            *app.world().resource::<MissionOutcome>(),
            MissionOutcome::Victory
        );
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Paused
        );
    }

    #[test]
    fn all_players_dead_is_defeat() {
        let mut app = headless_app(Grid::new(4, 1), 1);
        let p = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        let _e = spawn_unit(&mut app, Faction::Enemy, GridPos::new(3, 0));
        kill(&mut app, p);
        tick(&mut app, 0.1);
        assert_eq!(
            *app.world().resource::<MissionOutcome>(),
            MissionOutcome::Defeat
        );
    }

    #[test]
    fn a_map_with_only_one_faction_never_ends() {
        let mut app = headless_app(Grid::new(4, 1), 1);
        let _p = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        tick(&mut app, 0.1);
        assert!(app.world().get_resource::<MissionOutcome>().is_none());
    }

    #[test]
    fn pause_freezes_the_sim_but_keeps_orders_until_resume() {
        let mut app = headless_app(Grid::new(10, 1), 1);
        let u = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Paused);
        tick(&mut app, 0.1); // transition applies
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Paused
        );
        set_order(&mut app, u, Order::MoveTo(GridPos::new(3, 0)));
        for _ in 0..10 {
            tick(&mut app, 0.1);
        }
        assert_eq!(
            grid_pos(&app, u),
            GridPos::new(0, 0),
            "nothing moves while paused"
        );
        assert_eq!(
            order(&app, u),
            Order::MoveTo(GridPos::new(3, 0)),
            "the order survives the pause"
        );
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Playing);
        for _ in 0..15 {
            tick(&mut app, 0.1);
        }
        assert_eq!(
            grid_pos(&app, u),
            GridPos::new(3, 0),
            "resumes and completes the order"
        );
    }

    #[test]
    fn space_does_not_resume_a_finished_mission() {
        let mut app = headless_app(Grid::new(4, 1), 1);
        let _p = spawn_unit(&mut app, Faction::Player, GridPos::new(0, 0));
        let e = spawn_unit(&mut app, Faction::Enemy, GridPos::new(3, 0));
        kill(&mut app, e);
        tick(&mut app, 0.1);
        tick(&mut app, 0.1);
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Paused
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        tick(&mut app, 0.1);
        tick(&mut app, 0.1);
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Paused
        );
    }
}
