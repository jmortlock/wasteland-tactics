//! Turns a Tiled map into the `Grid` resource and unit spawn list.
//! The pure functions here are tested by loading `assets/maps/mission01.tmx` from disk.

use ::tiled::PropertyValue;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

use crate::animator::UnitSprite;
use crate::assets::GameAssets;
use crate::battle::{Battle, Side};
use crate::grid::{CoverSides, Grid, GridPos};
use crate::phase::BattleSeed;
use crate::render::UNIT_SPRITE_SIZE;

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
pub fn spawns_from_tiled(map: &::tiled::Map) -> Vec<(Side, GridPos)> {
    let h = map.height as i32;
    let (tw, th) = (map.tile_width as f32, map.tile_height as f32);
    let mut out = Vec::new();
    for layer in map.layers() {
        let Some(objects) = layer.as_object_layer() else {
            continue;
        };
        for obj in objects.objects() {
            let side = match obj.properties.get("faction") {
                Some(PropertyValue::StringValue(s)) if s == "player" => Side::Player,
                Some(PropertyValue::StringValue(s)) if s == "enemy" => Side::Enemy,
                _ => continue,
            };
            let pos = GridPos::new(
                (obj.x / tw).floor() as i32,
                h - 1 - (obj.y / th).floor() as i32,
            );
            out.push((side, pos));
        }
    }
    out
}

/// Spawns the map entity anchored so grid cell (0,0) is the bottom-left tile at world (0,0).
/// Unit sprites sit at z=10 (dead at 5, hover label at 20), so map layers stay below that.
pub fn spawn_map(mut commands: Commands, assets: Res<GameAssets>) {
    commands.spawn((
        Name::new("map"),
        TiledMap(assets.map.clone()),
        TilemapAnchor::BottomLeft,
        TiledMapLayerZOffset(1.0),
    ));
}

/// When the map finishes loading: build the `Battle` resource and one sprite per unit.
pub fn on_map_created(
    mut commands: Commands,
    mut events: MessageReader<TiledEvent<MapCreated>>,
    maps: Res<Assets<TiledMapAsset>>,
    assets: Res<GameAssets>,
    seed: Res<BattleSeed>,
) {
    for event in events.read() {
        let Some(map) = event.get_map(&maps) else {
            continue;
        };
        let battle = Battle::from_tiled(map, seed.0);
        info!(
            "map ready: {}x{} grid, {} units, seed {}",
            battle.grid().width(),
            battle.grid().height(),
            battle.units().len(),
            seed.0
        );
        spawn_unit_sprites(&mut commands, &battle, &assets);
        commands.insert_resource(battle);
    }
}

/// One sprite entity per unit, tagged with its `UnitId`. Also used by restart.
pub fn spawn_unit_sprites(commands: &mut Commands, battle: &Battle, assets: &GameAssets) {
    for unit in battle.units() {
        let (name, image) = match unit.side {
            Side::Player => ("Soldier", assets.soldier.clone()),
            Side::Enemy => ("Raider", assets.enemy.clone()),
        };
        commands.spawn((
            Name::new(format!("{name} {}", unit.id.0)),
            UnitSprite(unit.id),
            Sprite {
                image,
                custom_size: Some(Vec2::splat(UNIT_SPRITE_SIZE)),
                ..default()
            },
            Transform::from_translation(unit.pos.to_world().extend(10.0)),
        ));
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
        let players = spawns.iter().filter(|(s, _)| *s == Side::Player).count();
        let enemies = spawns.iter().filter(|(s, _)| *s == Side::Enemy).count();
        assert_eq!((players, enemies), (4, 3));
        // 'P' on ASCII row 12, column 3 -> (3, 2)
        assert!(spawns.contains(&(Side::Player, GridPos::new(3, 2))));
        // 'E' on row 2, column 16 -> (16, 12)
        assert!(spawns.contains(&(Side::Enemy, GridPos::new(16, 12))));
    }
}
