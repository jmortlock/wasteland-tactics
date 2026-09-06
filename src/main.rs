mod ai;
mod animator;
mod assets;
mod audio;
mod battle;
mod camera;
mod combat;
mod debug;
mod grid;
mod map;
mod movement;
mod orders;
mod phase;
mod planner;
mod presentation;
mod render;
mod rules;
mod sim;
mod state;
mod test_support;
mod ui;
mod unit;

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledPlugin;

use crate::presentation::PresentationPlugin;
use crate::rules::GameRng;
use crate::sim::SimulationPlugin;
use crate::state::AppState;

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
        .insert_resource(GameRng::seeded(seed))
        .add_plugins((SimulationPlugin, PresentationPlugin))
        .run();
}

/// `--seed N` makes a battle reproducible.
fn seed_from_args() -> Option<u64> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == "--seed" {
            return args.next()?.parse().ok();
        }
    }
    None
}
