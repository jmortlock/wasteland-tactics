//! Everything with a window: loading, map, sprites, camera, input, UI, audio, debug.

use bevy::prelude::*;

use crate::state::AppState;
use crate::{assets, camera, debug, map, render};

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::Loading),
            (assets::load_assets, map::spawn_map, camera::spawn_camera).chain(),
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
                camera::pan_camera,
                camera::zoom_camera,
                debug::screenshot_and_exit,
            ),
        );
    }
}
