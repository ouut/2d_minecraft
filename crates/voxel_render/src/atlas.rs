use std::collections::HashMap;
use macroquad::texture::{Image, Texture2D};
use voxel_core::block::{BlockDef, BlockId, BlockRegistry};

pub const TILE_W: u16 = 32;
pub const TILE_H: u16 = 32; // 16 for top diamond + 16 for vertical wall

#[derive(Clone, Debug)]
pub struct TileImage {
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

impl TileImage {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            rgba: vec![0; (width as usize) * (height as usize) * 4],
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u16, y: u16, r: u8, g: u8, b: u8, a: u8) {
        if x < self.width && y < self.height {
            let idx = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
            self.rgba[idx] = r;
            self.rgba[idx + 1] = g;
            self.rgba[idx + 2] = b;
            self.rgba[idx + 3] = a;
        }
    }

    pub fn to_macroquad_image(&self) -> Image {
        Image {
            bytes: self.rgba.clone(),
            width: self.width,
            height: self.height,
        }
    }
}

pub struct IsoAtlas {
    pub tiles: HashMap<BlockId, TileImage>,
    pub textures: HashMap<BlockId, Texture2D>,
    pub is_gpu_ready: bool,
}

impl Default for IsoAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl IsoAtlas {
    pub fn new() -> Self {
        Self {
            tiles: HashMap::new(),
            textures: HashMap::new(),
            is_gpu_ready: false,
        }
    }

    pub fn build_all(&mut self, registry: &BlockRegistry) {
        self.tiles.clear();
        for def in registry.all_registered() {
            let tile = Self::generate_block_tile(def);
            self.tiles.insert(def.id, tile);
        }
    }

    pub fn upload_to_gpu(&mut self) {
        self.textures.clear();
        for (id, tile) in &self.tiles {
            let img = tile.to_macroquad_image();
            let tex = Texture2D::from_image(&img);
            self.textures.insert(*id, tex);
        }
        self.is_gpu_ready = true;
    }

    pub fn rebuild(&mut self, registry: &BlockRegistry) {
        self.build_all(registry);
        if self.is_gpu_ready {
            self.upload_to_gpu();
        }
    }

    pub fn get_texture(&self, id: BlockId) -> Option<&Texture2D> {
        self.textures.get(&id)
    }

    /// Generate procedural 2.5D pixel-art isometric tile (32x32)
    pub fn generate_block_tile(def: &BlockDef) -> TileImage {
        let mut img = TileImage::new(TILE_W, TILE_H);
        if def.id.is_air() {
            return img;
        }

        let [tr, tg, tb] = def.top_color;
        let [lr, lg, lb] = def.left_color;
        let [rr, rg, rb] = def.right_color;
        let alpha = if def.is_liquid { 190 } else if def.id == BlockId::GLASS { 160 } else { 255 };

        // 1. Top Face: 2:1 Rhombus (y from 0 to 15, center at (16, 8))
        for y in 0..16u16 {
            // Half-width at line y
            let dx = if y < 8 {
                (y + 1) * 2
            } else {
                (15 - y + 1) * 2
            };

            let start_x = 16u16.saturating_sub(dx);
            let end_x = (16u16 + dx).min(31);

            for x in start_x..=end_x {
                // Procedural noise variation
                let noise = (((x * 17) ^ (y * 31)) % 15) as i16 - 7;
                let r = (tr as i16 + noise).clamp(0, 255) as u8;
                let g = (tg as i16 + noise).clamp(0, 255) as u8;
                let b = (tb as i16 + noise).clamp(0, 255) as u8;

                // Subtle top edge highlight
                let is_edge = x == start_x || x == end_x || y == 0;
                let (fr, fg, fb) = if is_edge {
                    (
                        r.saturating_add(25),
                        g.saturating_add(25),
                        b.saturating_add(25),
                    )
                } else {
                    (r, g, b)
                };

                img.set_pixel(x, y, fr, fg, fb, alpha);
            }
        }

        // 2. Left-Front Face (x from 0 to 15, y from top diamond bottom down 16 pixels)
        for x in 0..16u16 {
            let top_edge_y = 8 + (x / 2);
            let bot_edge_y = top_edge_y + 15;

            for y in top_edge_y..=bot_edge_y {
                let noise = (((x * 23) ^ (y * 41)) % 13) as i16 - 6;
                let r = (lr as i16 + noise).clamp(0, 255) as u8;
                let g = (lg as i16 + noise).clamp(0, 255) as u8;
                let b = (lb as i16 + noise).clamp(0, 255) as u8;

                let is_bot = y == bot_edge_y || x == 0;
                let (fr, fg, fb) = if is_bot {
                    (
                        r.saturating_sub(30),
                        g.saturating_sub(30),
                        b.saturating_sub(30),
                    )
                } else {
                    (r, g, b)
                };

                img.set_pixel(x, y, fr, fg, fb, alpha);
            }
        }

        // 3. Right-Front Face (x from 16 to 31, y from top diamond bottom down 16 pixels)
        for x in 16..32u16 {
            let top_edge_y = 15 - ((x - 16) / 2);
            let bot_edge_y = top_edge_y + 15;

            for y in top_edge_y..=bot_edge_y {
                let noise = (((x * 29) ^ (y * 37)) % 13) as i16 - 6;
                let r = (rr as i16 + noise).clamp(0, 255) as u8;
                let g = (rg as i16 + noise).clamp(0, 255) as u8;
                let b = (rb as i16 + noise).clamp(0, 255) as u8;

                let is_bot = y == bot_edge_y || x == 31;
                let (fr, fg, fb) = if is_bot {
                    (
                        r.saturating_sub(30),
                        g.saturating_sub(30),
                        b.saturating_sub(30),
                    )
                } else {
                    (r, g, b)
                };

                img.set_pixel(x, y, fr, fg, fb, alpha);
            }
        }

        // Special details for Torch
        if def.id == BlockId::TORCH {
            // Clear outer areas, draw glowing flame at center
            for y in 0..10u16 {
                for x in 14..18u16 {
                    img.set_pixel(x, y, 255, 230, 80, 255);
                }
            }
        }

        img
    }
}
