mod animator;
mod assets;
mod audio;
mod battle;
mod camera;
mod debug;
mod grid;
mod map;
mod orders;
mod phase;
mod planner;
mod presentation;
mod render;
mod rules;
mod ui;

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledPlugin;

use crate::phase::{AppState, BattleSeed, EventQueue};
use crate::presentation::PresentationPlugin;

fn main() {
    let seed = seed_from_args().unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    });
    println!("seed {seed}");

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Wasteland Tactics".into(),
                        ..default()
                    }),
                    ..default()
                })
                .disable::<bevy::audio::AudioPlugin>(),
        )
        .add_plugins((TiledPlugin::default(), bevy_kira_audio::AudioPlugin))
        .init_state::<AppState>()
        .insert_resource(BattleSeed(seed))
        .init_resource::<EventQueue>()
        .add_plugins(PresentationPlugin)
        .run();
}

/// `--seed N` replays the same dice: with identical inputs the battle is identical.
fn seed_from_args() -> Option<u64> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == "--seed" {
            return args.next()?.parse().ok();
        }
    }
    None
}
