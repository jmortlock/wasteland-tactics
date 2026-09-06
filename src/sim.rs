//! The headless-capable simulation: everything that changes game state.
//! Presentation (sprites, camera, input, UI, audio) lives in `presentation.rs`.

use bevy::prelude::*;

use crate::ai;
use crate::combat;
use crate::movement;
use crate::rules::GameRng;
use crate::state::{self, AppState};

/// Emitted once per shot so presentation can draw a tracer and play a sound.
#[derive(Message, Clone, Copy, Debug)]
pub struct ShotFired {
    pub from: Vec2,
    pub to: Vec2,
    pub hit: bool,
}

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ShotFired>();
        if !app.world().contains_resource::<GameRng>() {
            app.insert_resource(GameRng::seeded(0));
        }
        app.add_systems(
            Update,
            (
                ai::enemy_think,
                movement::plan_paths,
                movement::follow_paths,
                combat::resolve_attacks,
                combat::apply_death,
            )
                .chain()
                .run_if(in_state(AppState::Playing)),
        );
        app.add_systems(
            Update,
            state::toggle_pause
                .run_if(in_state(AppState::Playing).or_else(in_state(AppState::Paused))),
        );
    }
}
