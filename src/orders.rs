//! Player input: selection and issuing orders to selected soldiers.
//! Runs while Playing or Paused so orders can be queued during a pause.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::grid::{Grid, GridPos};
use crate::render::UNIT_SPRITE_SIZE;
use crate::unit::{Dead, Faction, Order, Path, Selected};

const PICK_RADIUS: f32 = UNIT_SPRITE_SIZE * 0.6;

fn cursor_world(window: &Window, camera: &Camera, camera_tf: &GlobalTransform) -> Option<Vec2> {
    let cursor = window.cursor_position()?;
    camera.viewport_to_world_2d(camera_tf, cursor).ok()
}

#[allow(clippy::too_many_arguments)]
pub fn player_input(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    grid: Res<Grid>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    units: Query<(Entity, &Faction, &Transform, Option<&Selected>), Without<Dead>>,
    mut orders: Query<&mut Order>,
) {
    // Number keys select the nth living soldier in spawn order (entity index).
    let hotkeys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ];
    for (i, key) in hotkeys.iter().enumerate() {
        if keys.just_pressed(*key) {
            let mut soldiers: Vec<Entity> = units
                .iter()
                .filter(|(_, f, _, _)| **f == Faction::Player)
                .map(|(e, ..)| e)
                .collect();
            soldiers.sort_by_key(|e| e.index());
            if let Some(&chosen) = soldiers.get(i) {
                for (e, _, _, selected) in &units {
                    if selected.is_some() {
                        commands.entity(e).remove::<Selected>();
                    }
                }
                commands.entity(chosen).insert(Selected);
            }
        }
    }

    let left = mouse.just_pressed(MouseButton::Left);
    let right = mouse.just_pressed(MouseButton::Right);
    if !left && !right {
        return;
    }
    let (cam, cam_tf) = *camera;
    let Some(world) = cursor_world(&window, cam, cam_tf) else {
        return;
    };

    let under_cursor = units
        .iter()
        .filter(|(_, _, tf, _)| tf.translation.truncate().distance(world) <= PICK_RADIUS)
        .min_by(|a, b| {
            let da = a.2.translation.truncate().distance(world);
            let db = b.2.translation.truncate().distance(world);
            da.total_cmp(&db)
        });

    if left {
        let additive = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        if !additive {
            for (e, _, _, selected) in &units {
                if selected.is_some() {
                    commands.entity(e).remove::<Selected>();
                }
            }
        }
        if let Some((e, Faction::Player, _, _)) = under_cursor {
            commands.entity(e).insert(Selected);
        }
        return;
    }

    // Right click: attack an enemy under the cursor, otherwise move to the cell.
    let new_order = match under_cursor {
        Some((target, Faction::Enemy, _, _)) => Order::Attack(target),
        _ => {
            let cell = GridPos::from_world(world);
            if !grid.is_walkable(cell) {
                return;
            }
            Order::MoveTo(cell)
        }
    };
    for (e, faction, _, selected) in &units {
        if *faction == Faction::Player && selected.is_some() {
            if let Ok(mut order) = orders.get_mut(e) {
                *order = new_order;
            }
            commands.entity(e).remove::<Path>();
        }
    }
}
