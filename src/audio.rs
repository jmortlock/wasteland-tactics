//! Plays a sound for every shot. Uses a dedicated SFX channel so music can be added later.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;

use crate::animator::ShotFired;
use crate::assets::GameAssets;

/// Marker for the SFX channel resource `AudioChannel<Sfx>`.
#[derive(Resource)]
pub struct Sfx;

pub fn play_shot_sounds(
    mut shots: MessageReader<ShotFired>,
    assets: Res<GameAssets>,
    sfx: Res<AudioChannel<Sfx>>,
) {
    for shot in shots.read() {
        sfx.play(assets.shot_sfx.clone());
        if shot.hit {
            sfx.play(assets.hit_sfx.clone());
        }
    }
}
