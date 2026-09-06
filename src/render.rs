//! Visual representation of units: sprites, death tint. Gizmo overlays are added in Task 12.

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::unit::{Dead, Faction};

pub const UNIT_SPRITE_SIZE: f32 = 48.0;

pub fn attach_sprites(
    mut commands: Commands,
    assets: Res<GameAssets>,
    units: Query<(Entity, &Faction), Added<Faction>>,
) {
    for (entity, faction) in &units {
        let image = match faction {
            Faction::Player => assets.soldier.clone(),
            Faction::Enemy => assets.enemy.clone(),
        };
        commands.entity(entity).insert(Sprite {
            image,
            custom_size: Some(Vec2::splat(UNIT_SPRITE_SIZE)),
            ..default()
        });
    }
}

pub fn tint_dead(mut units: Query<(&mut Sprite, &mut Transform), Added<Dead>>) {
    for (mut sprite, mut transform) in &mut units {
        sprite.color = Color::srgb(0.35, 0.35, 0.35);
        transform.translation.z = 5.0; // under the living
    }
}
