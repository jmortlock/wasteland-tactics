//! Plays `BattleEvent`s from the queue at real-time pace: walks sprites cell by cell,
//! holds on shots, tints the dead. The only code that moves unit sprites.

use bevy::prelude::*;

use crate::battle::{BattleEvent, Side, UnitId};
use crate::phase::EventQueue;

pub const WALK_SPEED: f32 = 160.0;
pub const SHOT_HOLD: f32 = 0.25;
pub const REACTION_HOLD: f32 = 0.4;
pub const TURN_HOLD: f32 = 0.5;

/// Marks the sprite entity that draws a battle unit.
#[derive(Component, Clone, Copy, Debug)]
pub struct UnitSprite(pub UnitId);

/// Emitted once per shot so tracers and audio can react.
#[derive(Message, Clone, Copy, Debug)]
pub struct ShotFired {
    pub from: Vec2,
    pub to: Vec2,
    pub hit: bool,
    pub reaction: bool,
}

/// The event currently being played, if any.
#[derive(Resource, Default, Debug)]
pub struct Animation(Option<Playing>);

#[derive(Debug)]
enum Playing {
    Walk { unit: UnitId, to: Vec2 },
    Hold { remaining: f32 },
}

fn sprite_pos(
    sprites: &Query<(&UnitSprite, &mut Transform, &mut Sprite)>,
    id: UnitId,
) -> Option<Vec2> {
    sprites
        .iter()
        .find(|(s, _, _)| s.0 == id)
        .map(|(_, tf, _)| tf.translation.truncate())
}

fn face(transform: &mut Transform, from: Vec2, to: Vec2) {
    let delta = to - from;
    if delta.length_squared() > 0.001 {
        transform.rotation = Quat::from_rotation_z(delta.to_angle());
    }
}

pub fn animate(
    time: Res<Time>,
    mut queue: ResMut<EventQueue>,
    mut current: ResMut<Animation>,
    mut shots: MessageWriter<ShotFired>,
    mut sprites: Query<(&UnitSprite, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();

    // Advance whatever is in flight.
    if let Some(playing) = &mut current.0 {
        let done = match playing {
            Playing::Walk { unit, to } => {
                let mut arrived = true;
                for (sprite, mut transform, _) in &mut sprites {
                    if sprite.0 != *unit {
                        continue;
                    }
                    let here = transform.translation.truncate();
                    let delta = *to - here;
                    let step = WALK_SPEED * dt;
                    if delta.length() <= step {
                        transform.translation.x = to.x;
                        transform.translation.y = to.y;
                    } else {
                        let movement = delta.normalize() * step;
                        transform.translation.x += movement.x;
                        transform.translation.y += movement.y;
                        arrived = false;
                    }
                }
                arrived
            }
            Playing::Hold { remaining } => {
                *remaining -= dt;
                *remaining <= 0.0
            }
        };
        if !done {
            return;
        }
        current.0 = None;
    }

    // Start the next event.
    let Some(event) = queue.0.pop_front() else {
        return;
    };
    match event {
        BattleEvent::Stepped { unit, to, .. } => {
            let target = to.to_world();
            for (sprite, mut transform, _) in &mut sprites {
                if sprite.0 == unit {
                    let here = transform.translation.truncate();
                    face(&mut transform, here, target);
                }
            }
            current.0 = Some(Playing::Walk { unit, to: target });
        }
        BattleEvent::Shot {
            shooter,
            target,
            hit,
            ..
        }
        | BattleEvent::ReactionShot {
            shooter,
            target,
            hit,
            ..
        } => {
            let reaction = matches!(event, BattleEvent::ReactionShot { .. });
            let (Some(from), Some(to)) =
                (sprite_pos(&sprites, shooter), sprite_pos(&sprites, target))
            else {
                return;
            };
            for (sprite, mut transform, _) in &mut sprites {
                if sprite.0 == shooter {
                    face(&mut transform, from, to);
                }
            }
            shots.write(ShotFired {
                from,
                to,
                hit,
                reaction,
            });
            current.0 = Some(Playing::Hold {
                remaining: if reaction { REACTION_HOLD } else { SHOT_HOLD },
            });
        }
        BattleEvent::Died(unit) => {
            for (sprite, mut transform, mut image) in &mut sprites {
                if sprite.0 == unit {
                    image.color = Color::srgb(0.35, 0.35, 0.35);
                    transform.translation.z = 5.0;
                }
            }
        }
        BattleEvent::TurnStarted {
            side: Side::Enemy, ..
        } => {
            current.0 = Some(Playing::Hold {
                remaining: TURN_HOLD,
            });
        }
        BattleEvent::TurnStarted { .. }
        | BattleEvent::TurnEnded(_)
        | BattleEvent::BattleOver(_) => {}
    }
}
