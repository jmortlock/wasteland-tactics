//! Gizmo overlays (selection ring, health bars, AP pips) and shot tracers, read from the `Battle`.
//! Planning previews (reach, path, hit chance) are added in Task 10.

use bevy::color::palettes::css::{LIME, ORANGE, RED, WHITE, YELLOW};
use bevy::prelude::*;

use crate::animator::{ShotFired, UnitSprite};
use crate::battle::{Battle, Side};
use crate::orders::Selected;

pub const UNIT_SPRITE_SIZE: f32 = 48.0;

#[derive(Component)]
pub struct Tracer {
    pub from: Vec2,
    pub to: Vec2,
    pub hit: bool,
    pub reaction: bool,
    pub ttl: f32,
}

/// Selection ring, health bar and AP pips over every living unit.
pub fn draw_overlays(
    mut gizmos: Gizmos,
    battle: Res<Battle>,
    selected: Res<Selected>,
    sprites: Query<(&UnitSprite, &Transform)>,
) {
    for (sprite, transform) in &sprites {
        let Some(unit) = battle.unit(sprite.0).filter(|u| u.alive) else {
            continue;
        };
        let p = transform.translation.truncate();
        if selected.0 == Some(unit.id) {
            gizmos.circle_2d(
                Isometry2d::from_translation(p),
                UNIT_SPRITE_SIZE * 0.6,
                WHITE,
            );
        }
        let width = UNIT_SPRITE_SIZE;
        let left = p + Vec2::new(-width / 2.0, UNIT_SPRITE_SIZE * 0.65);
        let frac = (unit.health.max(0) as f32 / unit.health_max as f32).clamp(0.0, 1.0);
        let colour = match unit.side {
            Side::Player => LIME,
            Side::Enemy => RED,
        };
        gizmos.line_2d(left, left + Vec2::X * width, Color::srgb(0.2, 0.2, 0.2));
        gizmos.line_2d(left, left + Vec2::X * width * frac, colour);
        // AP pips: one short tick per remaining AP, under the health bar.
        let pip = width / unit.ap_max.max(1) as f32;
        let row = left + Vec2::new(0.0, -5.0);
        for i in 0..unit.ap.max(0) {
            let x0 = row + Vec2::X * (i as f32 * pip + 1.0);
            gizmos.line_2d(x0, x0 + Vec2::X * (pip - 2.0), Color::srgb(0.9, 0.9, 0.5));
        }
    }
}

pub fn spawn_tracers(mut commands: Commands, mut shots: MessageReader<ShotFired>) {
    for shot in shots.read() {
        commands.spawn(Tracer {
            from: shot.from,
            to: shot.to,
            hit: shot.hit,
            reaction: shot.reaction,
            ttl: if shot.reaction { 0.2 } else { 0.12 },
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
        let colour = match (tracer.reaction, tracer.hit) {
            (true, _) => RED,
            (false, true) => ORANGE,
            (false, false) => YELLOW,
        };
        gizmos.line_2d(tracer.from, tracer.to, colour);
    }
}
