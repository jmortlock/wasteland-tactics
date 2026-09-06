// Modules are wired up task by task; remove this once every public item has a consumer (planned for Task 14).
#![allow(dead_code)]

mod combat;
mod grid;
mod movement;
mod rules;
mod sim;
mod state;
mod test_support;
mod unit;

use bevy::prelude::*;

fn main() {
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
                }),
        )
        .add_systems(Startup, spawn_camera)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
