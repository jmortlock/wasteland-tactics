//! Every handle the game needs, loaded up front during `AppState::Loading`.

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledMapAsset;
use bevy_kira_audio::AudioSource;

use crate::grid::Grid;
use crate::state::AppState;

#[derive(Resource)]
pub struct GameAssets {
    pub soldier: Handle<Image>,
    pub enemy: Handle<Image>,
    pub map: Handle<TiledMapAsset>,
    pub shot_sfx: Handle<AudioSource>,
    pub hit_sfx: Handle<AudioSource>,
}

pub fn load_assets(mut commands: Commands, server: Res<AssetServer>) {
    commands.insert_resource(GameAssets {
        soldier: server.load("sprites/soldier.png"),
        enemy: server.load("sprites/enemy.png"),
        map: server.load("maps/mission01.tmx"),
        shot_sfx: server.load("audio/shot.ogg"),
        hit_sfx: server.load("audio/hit.ogg"),
    });
}

/// Moves to `Playing` once the images are in and the map has produced a `Grid`.
pub fn check_loaded(
    server: Res<AssetServer>,
    assets: Res<GameAssets>,
    grid: Option<Res<Grid>>,
    mut next: ResMut<NextState<AppState>>,
) {
    let images_ready = server.is_loaded_with_dependencies(&assets.soldier)
        && server.is_loaded_with_dependencies(&assets.enemy);
    if images_ready && grid.is_some() {
        info!("assets loaded, starting mission");
        next.set(AppState::Playing);
    }
}
