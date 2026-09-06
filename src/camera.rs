//! 2D camera with keyboard pan and wheel zoom.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

const PAN_SPEED: f32 = 600.0;

pub fn spawn_camera(mut commands: Commands) {
    // Centre of the 20x15 map (1280 x 960 px). Scale 1.4 fits the full map height
    // into a 720 px tall window with a small margin; the wheel still zooms from there.
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 1.4,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(640.0, 480.0, 0.0),
    ));
}

pub fn pan_camera(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut camera: Single<&mut Transform, With<Camera2d>>,
) {
    let mut dir = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dir.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dir.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        dir.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        dir.x += 1.0;
    }
    if dir != Vec2::ZERO {
        let step = dir.normalize() * PAN_SPEED * time.delta_secs();
        camera.translation.x += step.x;
        camera.translation.y += step.y;
    }
}

pub fn zoom_camera(
    mut wheel: MessageReader<MouseWheel>,
    mut projection: Single<&mut Projection, With<Camera2d>>,
) {
    for event in wheel.read() {
        if let Projection::Orthographic(ortho) = &mut **projection {
            let factor = if event.y > 0.0 { 0.9 } else { 1.1 };
            ortho.scale = (ortho.scale * factor).clamp(0.4, 3.0);
        }
    }
}
