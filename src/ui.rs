//! Text overlay: PAUSED / VICTORY / DEFEAT plus a one-line controls hint.

use bevy::prelude::*;

use crate::battle::Battle;
use crate::battle::{Outcome, Side};
use crate::phase::AppState;

#[derive(Component)]
pub struct StatusText;

pub fn spawn_overlay(mut commands: Commands) {
    commands.spawn((
        Name::new("status text"),
        StatusText,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(40.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));
    commands.spawn((
        Name::new("controls hint"),
        Text::new(
            "LMB select  hover = path/AP or hit%  RMB move/shoot  1-4 squad  Enter end turn  R restart  WASD pan  wheel zoom",
        ),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(12.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));
}

pub fn update_overlay(
    state: Res<State<AppState>>,
    battle: Option<Res<Battle>>,
    mut text: Single<&mut Text, With<StatusText>>,
) {
    let label = match (state.get(), battle.as_deref()) {
        (AppState::Loading, _) | (_, None) => "Loading...".to_string(),
        (_, Some(b)) => match (b.outcome(), b.turn()) {
            (Some(Outcome::Victory), _) => "VICTORY".to_string(),
            (Some(Outcome::Defeat), _) => "DEFEAT".to_string(),
            (None, Side::Enemy) => "ENEMY TURN".to_string(),
            (None, Side::Player) => format!("YOUR TURN {}", b.turn_number()),
        },
    };
    if text.0 != label {
        text.0 = label;
    }
}
