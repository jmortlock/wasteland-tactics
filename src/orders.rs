//! Player input during `PlayerInput`: selection, hover, and turning clicks into actions.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::animator::UnitSprite;
use crate::battle::{Action, Battle, Side, UnitId};
use crate::grid::GridPos;
use crate::phase::{self, AppState, EventQueue};
use crate::render::UNIT_SPRITE_SIZE;

const PICK_RADIUS: f32 = UNIT_SPRITE_SIZE * 0.6;

/// The soldier the player is giving orders to.
#[derive(Resource, Default, Debug)]
pub struct Selected(pub Option<UnitId>);

/// What the cursor is over this frame, for previews.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Hover {
    pub cell: Option<GridPos>,
    pub unit: Option<UnitId>,
}

pub fn track_hover(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    sprites: Query<(&UnitSprite, &Transform)>,
    battle: Res<Battle>,
    mut hover: ResMut<Hover>,
) {
    let (cam, cam_tf) = *camera;
    let Some(world) = cursor_world(&window, cam, cam_tf) else {
        *hover = Hover::default();
        return;
    };
    let cell = GridPos::from_world(world);
    hover.cell = battle.grid().in_bounds(cell).then_some(cell);
    hover.unit = unit_under(&battle, &sprites, world);
}

fn cursor_world(window: &Window, camera: &Camera, camera_tf: &GlobalTransform) -> Option<Vec2> {
    let cursor = window.cursor_position()?;
    camera.viewport_to_world_2d(camera_tf, cursor).ok()
}

/// Living unit whose sprite is under the cursor, nearest first.
fn unit_under(
    battle: &Battle,
    sprites: &Query<(&UnitSprite, &Transform)>,
    world: Vec2,
) -> Option<UnitId> {
    sprites
        .iter()
        .filter(|(s, _)| battle.unit(s.0).is_some_and(|u| u.alive))
        .map(|(s, tf)| (tf.translation.truncate().distance(world), s.0))
        .filter(|(d, _)| *d <= PICK_RADIUS)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

#[allow(clippy::too_many_arguments)]
pub fn player_input(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    sprites: Query<(&UnitSprite, &Transform)>,
    mut battle: ResMut<Battle>,
    mut queue: ResMut<EventQueue>,
    mut selected: ResMut<Selected>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Enter) {
        if let Err(err) = phase::submit(&mut battle, &mut queue, &mut next, Action::EndTurn) {
            info!("end turn rejected: {err:?}");
        }
        return;
    }

    // Number keys select the nth living soldier by unit id.
    let hotkeys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ];
    for (i, key) in hotkeys.iter().enumerate() {
        if keys.just_pressed(*key) {
            let soldiers: Vec<UnitId> = battle.living(Side::Player).map(|u| u.id).collect();
            if let Some(&id) = soldiers.get(i) {
                selected.0 = Some(id);
            }
            return;
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
    let under = unit_under(&battle, &sprites, world);

    if left {
        selected.0 = under.filter(|id| battle.unit(*id).is_some_and(|u| u.side == Side::Player));
        return;
    }

    let Some(unit) = selected.0 else {
        return;
    };
    let action = match under {
        Some(target) if battle.unit(target).is_some_and(|u| u.side == Side::Enemy) => {
            Action::Shoot { unit, target }
        }
        _ => {
            let goal = GridPos::from_world(world);
            let Some(path) = battle.path_to(unit, goal) else {
                return;
            };
            Action::Move { unit, path }
        }
    };
    if let Err(err) = phase::submit(&mut battle, &mut queue, &mut next, action) {
        info!("action rejected: {err:?}");
    }
}
