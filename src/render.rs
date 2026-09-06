//! Gizmo overlays (selection ring, health bars, AP pips) and shot tracers, read from the `Battle`.
//! Also the planning previews: reach tint, hovered path with AP cost, hit chance.

use bevy::color::palettes::css::{LIME, ORANGE, RED, WHITE, YELLOW};
use bevy::prelude::*;

use crate::animator::{ShotFired, UnitSprite};
use crate::battle::{Battle, Side};
use crate::grid::TILE_SIZE;
use crate::orders::Hover;
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

/// World-space text that follows the hover (path cost or hit chance).
#[derive(Component)]
pub struct HoverLabel;

pub fn spawn_hover_label(mut commands: Commands) {
    commands.spawn((
        Name::new("hover label"),
        HoverLabel,
        Text2d::new(""),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_xyz(0.0, 0.0, 20.0),
        Visibility::Hidden,
    ));
}

/// Hides the hover label when the player's input phase ends, so no stale cost or hit chance lingers.
pub fn hide_hover_label(mut label: Single<&mut Visibility, With<HoverLabel>>) {
    **label = Visibility::Hidden;
}

/// Reachable cells, hovered path with AP cost, hit chance on a hovered enemy.
#[allow(clippy::type_complexity)]
pub fn draw_previews(
    mut gizmos: Gizmos,
    battle: Res<Battle>,
    selected: Res<Selected>,
    hover: Res<Hover>,
    mut label: Single<(&mut Text2d, &mut Transform, &mut Visibility), With<HoverLabel>>,
) {
    let (text, transform, visibility) = &mut *label;
    **visibility = Visibility::Hidden;
    let Some(unit) = selected.0 else {
        return;
    };
    if battle
        .unit(unit)
        .is_none_or(|u| !u.alive || u.side != Side::Player)
    {
        return;
    }

    let reach = battle.reachable(unit);
    for cell in reach.keys() {
        gizmos.rect_2d(
            Isometry2d::from_translation(cell.to_world()),
            Vec2::splat(TILE_SIZE - 10.0),
            Color::srgba(0.4, 0.8, 1.0, 0.35),
        );
    }

    let mut show = |what: String, at: Vec2| {
        text.0 = what;
        transform.translation.x = at.x;
        transform.translation.y = at.y;
        **visibility = Visibility::Visible;
    };

    if let Some(target) = hover.unit.filter(|t| *t != unit)
        && let Some(t) = battle
            .unit(target)
            .filter(|u| u.alive && u.side == Side::Enemy)
    {
        let at = t.pos.to_world() + Vec2::new(0.0, UNIT_SPRITE_SIZE * 0.95);
        match battle.hit_chance(unit, target) {
            Some(chance) => {
                let cover = if battle.target_in_cover(unit, target) {
                    "  COVER"
                } else {
                    ""
                };
                show(format!("{:.0}%{cover}", chance * 100.0), at);
            }
            None => {
                let s = battle.unit(unit).expect("checked");
                let why = if s.pos.distance(t.pos) > s.weapon.range {
                    "out of range"
                } else {
                    "no LOS"
                };
                show(why.to_string(), at);
            }
        }
        return;
    }

    if let Some(goal) = hover.cell
        && let Some(path) = battle.path_to(unit, goal)
    {
        let mut prev = battle.unit(unit).expect("checked").pos.to_world();
        for cell in &path {
            let here = cell.to_world();
            gizmos.line_2d(prev, here, Color::srgb(0.6, 0.9, 1.0));
            prev = here;
        }
        gizmos.circle_2d(Isometry2d::from_translation(prev), 6.0, LIME);
        let cost = path.len() as i32 * crate::battle::tuning::MOVE_COST;
        show(
            format!("{cost} AP"),
            prev + Vec2::new(0.0, TILE_SIZE * 0.55),
        );
    }
}
