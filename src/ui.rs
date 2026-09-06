//! Text overlay: PAUSED / VICTORY / DEFEAT plus a one-line controls hint.

use bevy::prelude::*;

use crate::state::{AppState, MissionOutcome};

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
        Text::new("LMB select  RMB move/attack  1-4 squad  Space pause  WASD pan  wheel zoom"),
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
    outcome: Option<Res<MissionOutcome>>,
    mut text: Single<&mut Text, With<StatusText>>,
) {
    let label = match (outcome.as_deref(), state.get()) {
        (Some(MissionOutcome::Victory), _) => "VICTORY",
        (Some(MissionOutcome::Defeat), _) => "DEFEAT",
        (None, AppState::Paused) => "PAUSED",
        (None, AppState::Loading) => "Loading...",
        (None, AppState::Playing) => "",
    };
    if text.0 != label {
        text.0 = label.to_string();
    }
}
