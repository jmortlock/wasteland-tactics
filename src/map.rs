//! Turns a Tiled map into the `Grid` resource and unit spawn list.
//! The pure functions here are tested by loading `assets/maps/mission01.tmx` from disk.

use ::tiled::PropertyValue;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

use crate::grid::{CoverSides, Grid, GridPos};
use crate::unit::Faction;

use crate::assets::GameAssets;
use crate::unit::unit_bundle;

/// Builds the walkability/sight/cover grid from every tile layer's tile properties.
/// Later layers override earlier ones for the same cell (walls layer sits above ground).
pub fn grid_from_tiled(map: &::tiled::Map) -> Grid {
    let (w, h) = (map.width as i32, map.height as i32);
    let mut grid = Grid::new(w, h);
    for layer in map.layers() {
        let Some(tiles) = layer.as_tile_layer() else {
            continue;
        };
        for tiled_y in 0..h {
            for x in 0..w {
                let Some(layer_tile) = tiles.get_tile(x, tiled_y) else {
                    continue;
                };
                let Some(tile) = layer_tile.get_tile() else {
                    continue;
                };
                let pos = GridPos::new(x, h - 1 - tiled_y);
                let mut cell = *grid.get(pos).expect("in bounds");
                if let Some(PropertyValue::BoolValue(b)) = tile.properties.get("walkable") {
                    cell.walkable = *b;
                }
                if let Some(PropertyValue::BoolValue(b)) = tile.properties.get("blocks_sight") {
                    cell.blocks_sight = *b;
                }
                if let Some(PropertyValue::StringValue(s)) = tile.properties.get("cover") {
                    cell.cover = CoverSides::parse(s);
                }
                grid.set(pos, cell);
            }
        }
    }
    grid
}

/// Reads point objects with a `faction` property from every object layer.
pub fn spawns_from_tiled(map: &::tiled::Map) -> Vec<(Faction, GridPos)> {
    let h = map.height as i32;
    let (tw, th) = (map.tile_width as f32, map.tile_height as f32);
    let mut out = Vec::new();
    for layer in map.layers() {
        let Some(objects) = layer.as_object_layer() else {
            continue;
        };
        for obj in objects.objects() {
            let faction = match obj.properties.get("faction") {
                Some(PropertyValue::StringValue(s)) if s == "player" => Faction::Player,
                Some(PropertyValue::StringValue(s)) if s == "enemy" => Faction::Enemy,
                _ => continue,
            };
            let pos = GridPos::new(
                (obj.x / tw).floor() as i32,
                h - 1 - (obj.y / th).floor() as i32,
            );
            out.push((faction, pos));
        }
    }
    out
}

/// Spawns the map entity anchored so grid cell (0,0) is the bottom-left tile at world (0,0).
/// Units are spawned at z=10 and the dead at z=5, so map layers must stay below that.
pub fn spawn_map(mut commands: Commands, assets: Res<GameAssets>) {
    commands.spawn((
        Name::new("map"),
        TiledMap(assets.map.clone()),
        TilemapAnchor::BottomLeft,
        // Layers 1 z-unit apart (default is 100, which would bury units at z=10 under the walls layer).
        TiledMapLayerZOffset(1.0),
    ));
}

/// When the map finishes loading: build the `Grid` and spawn the units from its object layer.
pub fn on_map_created(
    mut commands: Commands,
    mut events: MessageReader<TiledEvent<MapCreated>>,
    maps: Res<Assets<TiledMapAsset>>,
) {
    for event in events.read() {
        let Some(map) = event.get_map(&maps) else {
            continue;
        };
        let grid = grid_from_tiled(map);
        let spawns = spawns_from_tiled(map);
        info!(
            "map ready: {}x{} grid, {} spawns",
            grid.width(),
            grid.height(),
            spawns.len()
        );
        commands.insert_resource(grid);
        for (faction, pos) in spawns {
            commands.spawn(unit_bundle(faction, pos));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load() -> ::tiled::Map {
        ::tiled::Loader::new()
            .load_tmx_map("assets/maps/mission01.tmx")
            .expect("mission01.tmx loads")
    }

    #[test]
    fn grid_matches_the_generated_layout() {
        let grid = grid_from_tiled(&load());
        assert_eq!((grid.width(), grid.height()), (20, 15));
        assert!(
            !grid.is_walkable(GridPos::new(0, 0)),
            "bottom-left corner is wall"
        );
        assert!(
            !grid.is_walkable(GridPos::new(19, 14)),
            "top-right corner is wall"
        );
        assert!(grid.is_walkable(GridPos::new(1, 1)));
        assert!(grid.get(GridPos::new(0, 0)).unwrap().blocks_sight);
        // '=' on ASCII row 3 (from top), column 12 -> y = 14 - 3 = 11
        let sandbag = grid.get(GridPos::new(12, 11)).unwrap();
        assert!(sandbag.walkable && !sandbag.blocks_sight);
        assert_eq!(
            sandbag.cover,
            CoverSides {
                n: true,
                s: true,
                ..default()
            }
        );
        // '|' on row 6, column 3 -> y = 8
        assert_eq!(
            grid.get(GridPos::new(3, 8)).unwrap().cover,
            CoverSides {
                e: true,
                w: true,
                ..default()
            }
        );
    }

    #[test]
    fn spawns_come_from_the_object_layer() {
        let spawns = spawns_from_tiled(&load());
        let players = spawns.iter().filter(|(f, _)| *f == Faction::Player).count();
        let enemies = spawns.iter().filter(|(f, _)| *f == Faction::Enemy).count();
        assert_eq!((players, enemies), (4, 3));
        // 'P' on ASCII row 12, column 3 -> (3, 2)
        assert!(spawns.contains(&(Faction::Player, GridPos::new(3, 2))));
        // 'E' on row 2, column 16 -> (16, 12)
        assert!(spawns.contains(&(Faction::Enemy, GridPos::new(16, 12))));
    }
}
