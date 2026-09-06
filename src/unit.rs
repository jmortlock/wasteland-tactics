//! Components shared by soldiers and enemies, and the bundle that spawns one.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::grid::GridPos;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Reflect)]
pub enum Faction {
    Player,
    Enemy,
}

#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct Health {
    pub current: i32,
    pub max: i32,
}

impl Health {
    pub fn is_dead(&self) -> bool {
        self.current <= 0
    }
}

#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct Weapon {
    /// Maximum Chebyshev distance in cells.
    pub range: i32,
    pub damage_min: i32,
    pub damage_max: i32,
    pub cooldown_secs: f32,
}

/// Seconds until this unit may fire again. Zero means ready.
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
pub struct AttackCooldown(pub f32);

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default, Reflect)]
pub enum Order {
    #[default]
    None,
    MoveTo(GridPos),
    Attack(Entity),
}

/// Movement speed in world pixels per second.
#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct Speed(pub f32);

/// How far (cells) this unit notices enemies. Used by AI only.
#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct SightRange(pub i32);

/// Remaining cells to walk through, front first. Present only while moving.
#[derive(Component, Debug, Default)]
pub struct Path(pub VecDeque<GridPos>);

#[derive(Component, Debug, Default)]
pub struct Dead;

#[derive(Component, Debug, Default)]
pub struct Selected;

pub fn unit_bundle(faction: Faction, pos: GridPos) -> impl Bundle {
    let (name, weapon) = match faction {
        Faction::Player => (
            "Soldier",
            Weapon {
                range: 8,
                damage_min: 20,
                damage_max: 35,
                cooldown_secs: 0.8,
            },
        ),
        Faction::Enemy => (
            "Raider",
            Weapon {
                range: 7,
                damage_min: 10,
                damage_max: 25,
                cooldown_secs: 1.0,
            },
        ),
    };
    (
        Name::new(name),
        faction,
        pos,
        Health {
            current: 100,
            max: 100,
        },
        weapon,
        AttackCooldown(0.0),
        Order::None,
        Speed(160.0),
        SightRange(9),
        Transform::from_translation(pos.to_world().extend(10.0)),
    )
}
