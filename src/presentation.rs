//! Everything with a window: loading, map, sprites, camera, input, UI, audio, debug.

use bevy::prelude::*;
use bevy_kira_audio::prelude::AudioApp;

use crate::state::AppState;
use crate::{assets, audio, camera, debug, map, orders, render, ui};

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_channel::<audio::Sfx>();
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
            (
                render::attach_sprites,
                render::tint_dead,
                render::draw_overlays,
                render::spawn_tracers,
                render::draw_tracers,
                camera::pan_camera,
                camera::zoom_camera,
                debug::screenshot_and_exit,
                ui::update_overlay,
                audio::play_shot_sounds,
            ),
        )
        .add_systems(
            Update,
            orders::player_input
                .run_if(in_state(AppState::Playing).or_else(in_state(AppState::Paused))),
        );
    }
}
