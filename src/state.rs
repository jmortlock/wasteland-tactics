//! Top-level app state and the pause toggle. Mission end is added in a later task.

use bevy::prelude::*;

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
) {
    if !keys.just_pressed(KeyCode::Space) {
        return;
    }
    match state.get() {
        AppState::Playing => next.set(AppState::Paused),
        AppState::Paused => next.set(AppState::Playing),
        AppState::Loading => {}
    }
}

use crate::unit::{Dead, Faction};

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
    use crate::unit::{Faction, Health};

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
}
