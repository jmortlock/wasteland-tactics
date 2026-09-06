//! Pure grid model: cells, coordinates, line of sight, cover and pathfinding.
//! No systems live here, only data and functions, so everything is unit-testable.

use bevy::prelude::*;

pub const TILE_SIZE: f32 = 64.0;

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug, Reflect)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

impl GridPos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// World-space centre of this cell. Origin is the bottom-left cell; y grows upward.
    pub fn to_world(self) -> Vec2 {
        Vec2::new(
            (self.x as f32 + 0.5) * TILE_SIZE,
            (self.y as f32 + 0.5) * TILE_SIZE,
        )
    }

    pub fn from_world(p: Vec2) -> Self {
        Self::new(
            (p.x / TILE_SIZE).floor() as i32,
            (p.y / TILE_SIZE).floor() as i32,
        )
    }

    /// Chebyshev (king-move) distance. Weapon range and sight range use this.
    pub fn distance(self, other: GridPos) -> i32 {
        (self.x - other.x).abs().max((self.y - other.y).abs())
    }
}

/// Which sides of a cell give cover to a unit standing in it.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct CoverSides {
    pub n: bool,
    pub e: bool,
    pub s: bool,
    pub w: bool,
}

impl CoverSides {
    /// Parses a Tiled property such as `"n"`, `"ne"` or `"n,e"`. Unknown characters are ignored.
    pub fn parse(s: &str) -> Self {
        let mut c = Self::default();
        for ch in s.chars() {
            match ch {
                'n' => c.n = true,
                'e' => c.e = true,
                's' => c.s = true,
                'w' => c.w = true,
                _ => {}
            }
        }
        c
    }

    pub fn any(self) -> bool {
        self.n || self.e || self.s || self.w
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub walkable: bool,
    pub blocks_sight: bool,
    pub cover: CoverSides,
}

impl Cell {
    const NO_COVER: CoverSides = CoverSides {
        n: false,
        e: false,
        s: false,
        w: false,
    };
    pub const FLOOR: Cell = Cell {
        walkable: true,
        blocks_sight: false,
        cover: Self::NO_COVER,
    };
    pub const WALL: Cell = Cell {
        walkable: false,
        blocks_sight: true,
        cover: Self::NO_COVER,
    };

    /// A walkable, see-through tile (sandbags, low wall) that shelters the given sides.
    pub fn cover(sides: CoverSides) -> Cell {
        Cell {
            walkable: true,
            blocks_sight: false,
            cover: sides,
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct Grid {
    width: i32,
    height: i32,
    cells: Vec<Cell>,
}

impl Grid {
    /// An all-floor grid.
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::FLOOR; (width * height) as usize],
        }
    }

    /// Builds a grid from ASCII art. The FIRST line is the TOP row (highest y).
    /// `#` wall, `.` floor, `^` `>` `v` `<` cover tile sheltering its north/east/south/west side.
    pub fn from_ascii(art: &str) -> Self {
        let rows: Vec<&str> = art
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::trim)
            .collect();
        let height = rows.len() as i32;
        let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
        let mut grid = Self::new(width, height);
        for (row_idx, row) in rows.iter().enumerate() {
            let y = height - 1 - row_idx as i32;
            for (x, ch) in row.chars().enumerate() {
                let cell = match ch {
                    '#' => Cell::WALL,
                    '^' => Cell::cover(CoverSides {
                        n: true,
                        ..default()
                    }),
                    '>' => Cell::cover(CoverSides {
                        e: true,
                        ..default()
                    }),
                    'v' => Cell::cover(CoverSides {
                        s: true,
                        ..default()
                    }),
                    '<' => Cell::cover(CoverSides {
                        w: true,
                        ..default()
                    }),
                    _ => Cell::FLOOR,
                };
                grid.set(GridPos::new(x as i32, y), cell);
            }
        }
        grid
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn in_bounds(&self, p: GridPos) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height
    }

    pub fn get(&self, p: GridPos) -> Option<&Cell> {
        self.in_bounds(p)
            .then(|| &self.cells[(p.y * self.width + p.x) as usize])
    }

    pub fn set(&mut self, p: GridPos, cell: Cell) {
        if self.in_bounds(p) {
            self.cells[(p.y * self.width + p.x) as usize] = cell;
        }
    }

    pub fn is_walkable(&self, p: GridPos) -> bool {
        self.get(p).is_some_and(|c| c.walkable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_first_line_is_top_row() {
        let g = Grid::from_ascii("#..\n...");
        assert_eq!((g.width(), g.height()), (3, 2));
        assert!(
            !g.is_walkable(GridPos::new(0, 1)),
            "wall is on the top row (y=1)"
        );
        assert!(g.is_walkable(GridPos::new(0, 0)));
        assert!(
            !g.is_walkable(GridPos::new(5, 5)),
            "out of bounds is not walkable"
        );
    }

    #[test]
    fn ascii_cover_glyphs() {
        let g = Grid::from_ascii("^>v<");
        assert_eq!(
            g.get(GridPos::new(0, 0)).unwrap().cover,
            CoverSides {
                n: true,
                ..default()
            }
        );
        assert_eq!(
            g.get(GridPos::new(1, 0)).unwrap().cover,
            CoverSides {
                e: true,
                ..default()
            }
        );
        assert_eq!(
            g.get(GridPos::new(2, 0)).unwrap().cover,
            CoverSides {
                s: true,
                ..default()
            }
        );
        assert_eq!(
            g.get(GridPos::new(3, 0)).unwrap().cover,
            CoverSides {
                w: true,
                ..default()
            }
        );
        assert!(
            g.is_walkable(GridPos::new(0, 0)),
            "cover tiles are walkable"
        );
        assert!(!g.get(GridPos::new(0, 0)).unwrap().blocks_sight);
    }

    #[test]
    fn world_round_trip() {
        let p = GridPos::new(3, 7);
        assert_eq!(p.to_world(), Vec2::new(3.5 * TILE_SIZE, 7.5 * TILE_SIZE));
        assert_eq!(GridPos::from_world(p.to_world()), p);
        assert_eq!(
            GridPos::from_world(Vec2::new(-1.0, 10.0)),
            GridPos::new(-1, 0)
        );
    }

    #[test]
    fn chebyshev_distance() {
        assert_eq!(GridPos::new(0, 0).distance(GridPos::new(3, -2)), 3);
    }

    #[test]
    fn cover_sides_parse() {
        assert_eq!(
            CoverSides::parse("n,e"),
            CoverSides {
                n: true,
                e: true,
                ..default()
            }
        );
        assert_eq!(
            CoverSides::parse("sw"),
            CoverSides {
                s: true,
                w: true,
                ..default()
            }
        );
        assert!(!CoverSides::parse("xyz").any());
    }
}
