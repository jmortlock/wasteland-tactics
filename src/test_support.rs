//! Helpers for headless ECS tests: a minimal App with the simulation, a fixed grid and a seed.

#![cfg(test)]

use std::time::Duration;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use crate::grid::{Grid, GridPos};
use crate::rules::GameRng;
use crate::sim::SimulationPlugin;
use crate::state::AppState;
use crate::unit::{Faction, unit_bundle};

pub fn headless_app(grid: Grid, seed: u64) -> App {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.insert_resource(Time::<()>::default());
    app.insert_resource(ButtonInput::<KeyCode>::default());
    app.insert_resource(grid);
    app.insert_resource(GameRng::seeded(seed));
    app.insert_state(AppState::Playing);
    app.add_plugins(SimulationPlugin);
    app
}

/// Advance the fake clock by `secs` and run one frame.
pub fn tick(app: &mut App, secs: f32) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f32(secs));
    app.update();
}

pub fn spawn_unit(app: &mut App, faction: Faction, pos: GridPos) -> Entity {
    app.world_mut().spawn(unit_bundle(faction, pos)).id()
}

pub fn set_order(app: &mut App, unit: Entity, order: crate::unit::Order) {
    let mut e = app.world_mut().entity_mut(unit);
    *e.get_mut::<crate::unit::Order>().unwrap() = order;
    e.remove::<crate::unit::Path>();
}

pub fn grid_pos(app: &App, unit: Entity) -> GridPos {
    *app.world().entity(unit).get::<GridPos>().unwrap()
}

pub fn order(app: &App, unit: Entity) -> crate::unit::Order {
    *app.world()
        .entity(unit)
        .get::<crate::unit::Order>()
        .unwrap()
}
