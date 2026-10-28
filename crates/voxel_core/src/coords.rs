use crate::chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct WorldPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl WorldPos {
    #[inline]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        WorldPos { x, y, z }
    }

    #[inline]
    pub fn to_chunk_and_local(self) -> (ChunkPos, usize, usize, usize) {
        let cx = self.x.div_euclid(CHUNK_WIDTH as i32);
        let cy = self.y.div_euclid(CHUNK_DEPTH as i32);
        let lx = self.x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        let ly = self.y.rem_euclid(CHUNK_DEPTH as i32) as usize;
        let lz = self.z.clamp(0, (CHUNK_HEIGHT - 1) as i32) as usize;
        (ChunkPos { x: cx, y: cy }, lx, ly, lz)
    }

    #[inline]
    pub fn offset(self, dx: i32, dy: i32, dz: i32) -> Self {
        WorldPos {
            x: self.x + dx,
            y: self.y + dy,
            z: self.z + dz,
        }
    }

    #[inline]
    pub fn manhattan_dist(self, other: WorldPos) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs() + (self.z - other.z).abs()
    }

    #[inline]
    pub fn neighbors_6(self) -> [WorldPos; 6] {
        [
            self.offset(1, 0, 0),
            self.offset(-1, 0, 0),
            self.offset(0, 1, 0),
            self.offset(0, -1, 0),
            self.offset(0, 0, 1),
            self.offset(0, 0, -1),
        ]
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct ChunkPos {
    pub x: i32,
    pub y: i32,
}

impl ChunkPos {
    #[inline]
    pub const fn new(x: i32, y: i32) -> Self {
        ChunkPos { x, y }
    }

    #[inline]
    pub fn min_world_pos(self) -> WorldPos {
        WorldPos {
            x: self.x * CHUNK_WIDTH as i32,
            y: self.y * CHUNK_DEPTH as i32,
            z: 0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum CardinalDir {
    #[default]
    North = 0, // 0 deg
    East = 1,  // 90 deg
    South = 2, // 180 deg
    West = 3,  // 270 deg
}

impl CardinalDir {
    pub fn rotate_cw(self) -> Self {
        match self {
            CardinalDir::North => CardinalDir::East,
            CardinalDir::East => CardinalDir::South,
            CardinalDir::South => CardinalDir::West,
            CardinalDir::West => CardinalDir::North,
        }
    }

    pub fn rotate_ccw(self) -> Self {
        match self {
            CardinalDir::North => CardinalDir::West,
            CardinalDir::East => CardinalDir::North,
            CardinalDir::South => CardinalDir::East,
            CardinalDir::West => CardinalDir::South,
        }
    }

    pub fn to_index(self) -> usize {
        self as usize
    }

    pub fn from_index(idx: usize) -> Self {
        match idx % 4 {
            0 => CardinalDir::North,
            1 => CardinalDir::East,
            2 => CardinalDir::South,
            _ => CardinalDir::West,
        }
    }

    pub fn to_delta(self) -> (i32, i32) {
        match self {
            CardinalDir::North => (0, -1),
            CardinalDir::East => (1, 0),
            CardinalDir::South => (0, 1),
            CardinalDir::West => (-1, 0),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HitFace {
    Top,
    LeftFront,
    RightFront,
}

/// 2.5D Isometric Camera with 4-direction 90-degree rotation
#[derive(Clone, Debug)]
pub struct IsoCamera {
    pub focus_x: f32,
    pub focus_y: f32,
    pub zoom: f32,
    pub slice_z: i32,
    pub rotation: CardinalDir,
    pub tile_w: f32,
    pub tile_h: f32,
    pub tile_z: f32,
    pub max_visible_depth: i32,
}

impl Default for IsoCamera {
    fn default() -> Self {
        Self {
            focus_x: 0.0,
            focus_y: 0.0,
            zoom: 1.5,
            slice_z: 32,
            rotation: CardinalDir::North,
            tile_w: 32.0,
            tile_h: 16.0,
            tile_z: 16.0,
            max_visible_depth: 16,
        }
    }
}

impl IsoCamera {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn rotate_clockwise(&mut self) {
        self.rotation = self.rotation.rotate_cw();
    }

    pub fn rotate_counter_clockwise(&mut self) {
        self.rotation = self.rotation.rotate_ccw();
    }

    /// Convert relative horizontal world coords (dx, dy) to rotated view grid coords (rx, ry)
    #[inline]
    pub fn world_to_view_coords(&self, dx: f32, dy: f32) -> (f32, f32) {
        match self.rotation {
            CardinalDir::North => (dx, dy),
            CardinalDir::East => (-dy, dx),
            CardinalDir::South => (-dx, -dy),
            CardinalDir::West => (dy, -dx),
        }
    }

    /// Inverse: convert rotated view grid coords (rx, ry) back to relative world coords (dx, dy)
    #[inline]
    pub fn view_to_world_coords(&self, rx: f32, ry: f32) -> (f32, f32) {
        match self.rotation {
            CardinalDir::North => (rx, ry),
            CardinalDir::East => (ry, -rx),
            CardinalDir::South => (-rx, -ry),
            CardinalDir::West => (-ry, rx),
        }
    }

    /// Project 3D WorldPos to 2D screen coordinates (origin at top-left)
    pub fn world_to_screen(&self, pos: WorldPos, screen_w: f32, screen_h: f32) -> (f32, f32) {
        let (rx, ry) = self.world_to_view_coords(pos.x as f32 - self.focus_x, pos.y as f32 - self.focus_y);

        let half_w = self.tile_w * 0.5 * self.zoom;
        let half_h = self.tile_h * 0.5 * self.zoom;
        let z_step = self.tile_z * self.zoom;

        let screen_center_x = screen_w * 0.5;
        let screen_center_y = screen_h * 0.5;

        let sx = screen_center_x + (rx - ry) * half_w;
        let sy = screen_center_y + (rx + ry) * half_h - (pos.z - self.slice_z) as f32 * z_step;

        (sx, sy)
    }

    /// Unproject screen coordinate to world coordinate on the locked Z plane (z = slice_z)
    pub fn screen_to_slice_plane(&self, sx: f32, sy: f32, screen_w: f32, screen_h: f32) -> WorldPos {
        let half_w = self.tile_w * 0.5 * self.zoom;
        let half_h = self.tile_h * 0.5 * self.zoom;

        let screen_center_x = screen_w * 0.5;
        let screen_center_y = screen_h * 0.5;

        let dx_screen = sx - screen_center_x;
        let dy_screen = sy - screen_center_y;

        // dx_screen = (rx - ry) * half_w  =>  rx - ry = dx_screen / half_w
        // dy_screen = (rx + ry) * half_h  =>  rx + ry = dy_screen / half_h
        let term_sub = dx_screen / half_w;
        let term_add = dy_screen / half_h;

        let rx = (term_add + term_sub) * 0.5;
        let ry = (term_add - term_sub) * 0.5;

        let (dx, dy) = self.view_to_world_coords(rx, ry);

        let wx = (dx + self.focus_x).round() as i32;
        let wy = (dy + self.focus_y).round() as i32;

        WorldPos::new(wx, wy, self.slice_z)
    }
}
