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
    /// Only built via `Grid::from_ascii`, which is test-only.
    #[cfg(test)]
    pub const WALL: Cell = Cell {
        walkable: false,
        blocks_sight: true,
        cover: Self::NO_COVER,
    };

    /// A walkable, see-through tile (sandbags, low wall) that shelters the given sides.
    /// Only built via `Grid::from_ascii`, which is test-only.
    #[cfg(test)]
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

/// Bresenham line from `a` to `b`, inclusive of both ends, one cell per step.
pub fn line(a: GridPos, b: GridPos) -> Vec<GridPos> {
    let (mut x, mut y) = (a.x, a.y);
    let dx = (b.x - a.x).abs();
    let dy = -(b.y - a.y).abs();
    let sx = if a.x < b.x { 1 } else { -1 };
    let sy = if a.y < b.y { 1 } else { -1 };
    let mut err = dx + dy;
    let mut out = vec![a];
    while (x, y) != (b.x, b.y) {
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        out.push(GridPos::new(x, y));
    }
    out
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
    /// Test-only: production maps load from Tiled via `map.rs`.
    #[cfg(test)]
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

    /// True when no sight-blocking cell lies strictly between `a` and `b`.
    pub fn has_line_of_sight(&self, a: GridPos, b: GridPos) -> bool {
        let cells = line(a, b);
        cells[1..cells.len().saturating_sub(1).max(1)]
            .iter()
            .all(|p| !self.get(*p).is_some_and(|c| c.blocks_sight))
    }

    /// True when the last step of the shooter→target line enters the target's cell
    /// through a side that cell shelters. Diagonal entries count either side.
    pub fn cover_against(&self, target: GridPos, shooter: GridPos) -> bool {
        let Some(cell) = self.get(target) else {
            return false;
        };
        if !cell.cover.any() {
            return false;
        }
        let cells = line(shooter, target);
        if cells.len() < 2 {
            return false;
        }
        let prev = cells[cells.len() - 2];
        let dx = target.x - prev.x; // >0 : shot travelling east, enters through the west side
        let dy = target.y - prev.y; // >0 : shot travelling north, enters through the south side
        (dy < 0 && cell.cover.n)
            || (dy > 0 && cell.cover.s)
            || (dx < 0 && cell.cover.e)
            || (dx > 0 && cell.cover.w)
    }

    /// Walkable king-move neighbours with costs (10 orthogonal, 14 diagonal).
    /// Diagonals are only allowed when both adjacent orthogonal cells are walkable.
    pub fn neighbours(&self, p: GridPos) -> Vec<(GridPos, u32)> {
        let mut out = Vec::with_capacity(8);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let n = GridPos::new(p.x + dx, p.y + dy);
                if !self.is_walkable(n) {
                    continue;
                }
                let diagonal = dx != 0 && dy != 0;
                if diagonal
                    && !(self.is_walkable(GridPos::new(p.x + dx, p.y))
                        && self.is_walkable(GridPos::new(p.x, p.y + dy)))
                {
                    continue;
                }
                out.push((n, if diagonal { 14 } else { 10 }));
            }
        }
        out
    }

    /// A* path from `from` to `to`, excluding `from`, ending at `to`.
    pub fn find_path(&self, from: GridPos, to: GridPos) -> Option<Vec<GridPos>> {
        if !self.is_walkable(to) {
            return None;
        }
        let (mut path, _cost) = pathfinding::directed::astar::astar(
            &from,
            |p| self.neighbours(*p),
            |p| p.distance(to) as u32 * 10,
            |p| *p == to,
        )?;
        path.remove(0);
        Some(path)
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

    #[test]
    fn bresenham_line_is_inclusive_and_connected() {
        let l = line(GridPos::new(0, 0), GridPos::new(4, 2));
        assert_eq!(l.first(), Some(&GridPos::new(0, 0)));
        assert_eq!(l.last(), Some(&GridPos::new(4, 2)));
        assert_eq!(l.len(), 5, "one cell per x step on a shallow line");
        for w in l.windows(2) {
            assert_eq!(
                w[0].distance(w[1]),
                1,
                "consecutive cells are king-adjacent"
            );
        }
        assert_eq!(
            line(GridPos::new(2, 2), GridPos::new(2, 2)),
            vec![GridPos::new(2, 2)]
        );
    }

    #[test]
    fn walls_block_sight_but_cover_does_not() {
        let g = Grid::from_ascii(".#.\n...\n.^.");
        // top row y=2: (0,2) . (1,2) # (2,2) .
        assert!(
            !g.has_line_of_sight(GridPos::new(0, 2), GridPos::new(2, 2)),
            "wall between"
        );
        assert!(
            g.has_line_of_sight(GridPos::new(0, 0), GridPos::new(2, 0)),
            "cover between"
        );
        assert!(g.has_line_of_sight(GridPos::new(0, 1), GridPos::new(2, 1)));
        assert!(
            g.has_line_of_sight(GridPos::new(0, 2), GridPos::new(1, 2)),
            "endpoints never block"
        );
    }

    #[test]
    fn cover_is_directional() {
        let g = Grid::from_ascii("...\n.^.\n...");
        let target = GridPos::new(1, 1); // sheltered on its north side
        assert!(
            g.cover_against(target, GridPos::new(1, 2)),
            "shot from north"
        );
        assert!(
            !g.cover_against(target, GridPos::new(1, 0)),
            "shot from south"
        );
        assert!(
            !g.cover_against(target, GridPos::new(0, 1)),
            "shot from west"
        );
        assert!(
            g.cover_against(target, GridPos::new(0, 2)),
            "diagonal from north-west crosses the north side"
        );
        assert!(
            !g.cover_against(GridPos::new(0, 0), GridPos::new(2, 2)),
            "plain floor gives no cover"
        );
    }

    #[test]
    fn path_straight_corridor() {
        let g = Grid::from_ascii("#####\n#...#\n#####");
        let p = g.find_path(GridPos::new(1, 1), GridPos::new(3, 1)).unwrap();
        assert_eq!(p, vec![GridPos::new(2, 1), GridPos::new(3, 1)]);
        assert_eq!(
            g.find_path(GridPos::new(1, 1), GridPos::new(1, 1)),
            Some(vec![])
        );
    }

    #[test]
    fn path_goes_around_walls() {
        let g = Grid::from_ascii("...\n.#.\n...");
        let p = g.find_path(GridPos::new(0, 0), GridPos::new(2, 2)).unwrap();
        assert_eq!(p.last(), Some(&GridPos::new(2, 2)));
        assert!(p.iter().all(|c| g.is_walkable(*c)));
        assert_eq!(
            p.len(),
            4,
            "no corner cutting past the centre wall: e.g. (1,0) (2,0) (2,1) (2,2)"
        );
    }

    #[test]
    fn no_corner_cutting() {
        // Moving (0,0)->(1,1) diagonally would squeeze between two walls; must go the long way or fail.
        let g = Grid::from_ascii("#.\n.#");
        assert_eq!(g.find_path(GridPos::new(0, 0), GridPos::new(1, 1)), None);
    }

    #[test]
    fn unreachable_or_unwalkable_goal_is_none() {
        let g = Grid::from_ascii(".#.\n.#.\n.#.");
        assert_eq!(g.find_path(GridPos::new(0, 0), GridPos::new(2, 0)), None);
        assert_eq!(
            g.find_path(GridPos::new(0, 0), GridPos::new(1, 0)),
            None,
            "goal is a wall"
        );
    }
}
