//! Visual representation of units: sprites, death tint, gizmo overlays, tracers.

use bevy::color::palettes::css::{LIME, ORANGE, RED, WHITE, YELLOW};
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::sim::ShotFired;
use crate::unit::{Dead, Faction, Health, Order, Selected};

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

#[derive(Component)]
pub struct Tracer {
    pub from: Vec2,
    pub to: Vec2,
    pub hit: bool,
    pub ttl: f32,
}

/// Selection rings, health bars and move-target markers, drawn with gizmos every frame.
#[allow(clippy::type_complexity)]
pub fn draw_overlays(
    mut gizmos: Gizmos,
    units: Query<(
        &Transform,
        &Health,
        &Faction,
        &Order,
        Option<&Selected>,
        Option<&Dead>,
    )>,
) {
    for (transform, health, faction, order, selected, dead) in &units {
        if dead.is_some() {
            continue;
        }
        let p = transform.translation.truncate();
        if selected.is_some() {
            gizmos.circle_2d(
                Isometry2d::from_translation(p),
                UNIT_SPRITE_SIZE * 0.6,
                WHITE,
            );
            if let Order::MoveTo(goal) = order {
                gizmos.circle_2d(Isometry2d::from_translation(goal.to_world()), 8.0, LIME);
            }
        }
        // Health bar just above the sprite.
        let width = UNIT_SPRITE_SIZE;
        let left = p + Vec2::new(-width / 2.0, UNIT_SPRITE_SIZE * 0.65);
        let frac = (health.current.max(0) as f32 / health.max as f32).clamp(0.0, 1.0);
        let colour = match faction {
            Faction::Player => LIME,
            Faction::Enemy => RED,
        };
        gizmos.line_2d(left, left + Vec2::X * width, Color::srgb(0.2, 0.2, 0.2));
        gizmos.line_2d(left, left + Vec2::X * width * frac, colour);
    }
}

pub fn spawn_tracers(mut commands: Commands, mut shots: MessageReader<ShotFired>) {
    for shot in shots.read() {
        commands.spawn(Tracer {
            from: shot.from,
            to: shot.to,
            hit: shot.hit,
            ttl: 0.12,
        });
    }
}

pub fn draw_tracers(
    mut commands: Commands,
    time: Res<Time>,
    mut gizmos: Gizmos,
    mut tracers: Query<(Entity, &mut Tracer)>,
) {
    for (entity, mut tracer) in &mut tracers {
        tracer.ttl -= time.delta_secs();
        if tracer.ttl <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let colour = if tracer.hit { ORANGE } else { YELLOW };
        gizmos.line_2d(tracer.from, tracer.to, colour);
    }
}
