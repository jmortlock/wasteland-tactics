//! Everything with a window: loading, map, sprites, camera, input, overlays, UI, audio, debug.

use bevy::prelude::*;
use bevy_kira_audio::prelude::AudioApp;

use crate::animator::{self, Animation, ShotFired};
use crate::orders::Selected;
use crate::phase::{self, AppState};
use crate::{assets, audio, camera, debug, map, orders, render, ui};

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        debug::add_inspector(app);
        app.add_audio_channel::<audio::Sfx>();
        app.add_message::<ShotFired>();
        app.init_resource::<Animation>();
        app.init_resource::<Selected>();
        app.add_systems(
            OnEnter(AppState::Loading),
            (
                assets::load_assets,
                map::spawn_map,
                camera::spawn_camera,
                ui::spawn_overlay,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (map::on_map_created, assets::check_loaded)
                .chain()
                .run_if(in_state(AppState::Loading)),
        )
        .add_systems(
            Update,
            orders::player_input.run_if(in_state(AppState::PlayerInput)),
        )
        .add_systems(
            Update,
            (animator::animate, phase::after_animation)
                .chain()
                .run_if(in_state(AppState::Animating)),
        )
        .add_systems(
            Update,
            phase::drive_enemy_turn.run_if(in_state(AppState::EnemyTurn)),
        )
        .add_systems(
            Update,
            (
                render::draw_overlays.run_if(resource_exists::<crate::battle::Battle>),
                render::spawn_tracers,
                render::draw_tracers,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                camera::pan_camera,
                camera::zoom_camera,
                debug::screenshot_and_exit,
                ui::update_overlay,
                audio::play_shot_sounds,
                phase::restart,
            ),
        );
    }
}
