use noise::{Fbm, MultiFractal, NoiseFn, Perlin};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use voxel_core::block::BlockId;
use voxel_core::chunk::{Chunk, CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH};
use voxel_core::coords::ChunkPos;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Biome {
    Plains,
    Forest,
    Desert,
    SnowMountain,
    Ocean,
}

pub struct TerrainGenerator {
    pub seed: u32,
    height_noise: Fbm<Perlin>,
    roughness_noise: Fbm<Perlin>,
    temp_noise: Perlin,
    moisture_noise: Perlin,
    cave_noise1: Perlin,
    cave_noise2: Perlin,
    density_3d: Perlin,
}

impl TerrainGenerator {
    pub fn new(seed: u32) -> Self {
        let height_noise = Fbm::<Perlin>::new(seed)
            .set_octaves(4)
            .set_frequency(0.015)
            .set_persistence(0.5);

        let roughness_noise = Fbm::<Perlin>::new(seed.wrapping_add(101))
            .set_octaves(3)
            .set_frequency(0.04)
            .set_persistence(0.4);

        let temp_noise = Perlin::new(seed.wrapping_add(202));
        let moisture_noise = Perlin::new(seed.wrapping_add(303));
        let cave_noise1 = Perlin::new(seed.wrapping_add(404));
        let cave_noise2 = Perlin::new(seed.wrapping_add(505));
        let density_3d = Perlin::new(seed.wrapping_add(606));

        Self {
            seed,
            height_noise,
            roughness_noise,
            temp_noise,
            moisture_noise,
            cave_noise1,
            cave_noise2,
            density_3d,
        }
    }

    pub fn get_biome(&self, wx: f64, wy: f64) -> Biome {
        let temp = self.temp_noise.get([wx * 0.005, wy * 0.005]);
        let moisture = self.moisture_noise.get([wx * 0.005, wy * 0.005]);
        let base_h = self.height_noise.get([wx * 0.01, wy * 0.01]);

        if base_h < -0.3 {
            Biome::Ocean
        } else if temp > 0.35 && moisture < -0.1 {
            Biome::Desert
        } else if temp < -0.35 {
            Biome::SnowMountain
        } else if moisture > 0.15 {
            Biome::Forest
        } else {
            Biome::Plains
        }
    }

    pub fn get_base_height(&self, wx: f64, wy: f64, biome: Biome) -> i32 {
        let base = self.height_noise.get([wx * 0.015, wy * 0.015]);
        let rough = self.roughness_noise.get([wx * 0.04, wy * 0.04]);

        let (mid, amp) = match biome {
            Biome::Ocean => (16.0, 6.0),
            Biome::Plains => (26.0, 5.0),
            Biome::Forest => (28.0, 7.0),
            Biome::Desert => (24.0, 4.0),
            Biome::SnowMountain => (40.0, 14.0),
        };

        let h = mid + (base * 0.7 + rough * 0.3) * amp;
        (h.round() as i32).clamp(4, (CHUNK_HEIGHT - 6) as i32)
    }

    pub fn generate_chunk(&self, chunk_pos: ChunkPos) -> Chunk {
        let mut chunk = Chunk::new();
        let min_pos = chunk_pos.min_world_pos();
        let mut rng = StdRng::seed_from_u64(
            (self.seed as u64)
                ^ ((chunk_pos.x as u64) << 32)
                ^ ((chunk_pos.y as u64) & 0xFFFF_FFFF),
        );

        let sea_level = 20;

        // 1. Terrain base fill & 3D density & Caves
        for lx in 0..CHUNK_WIDTH {
            for ly in 0..CHUNK_DEPTH {
                let wx = (min_pos.x + lx as i32) as f64;
                let wy = (min_pos.y + ly as i32) as f64;
                let biome = self.get_biome(wx, wy);
                let surface_z = self.get_base_height(wx, wy, biome);

                // Bedrock at z=0
                chunk.set_block(lx, ly, 0, BlockId::BEDROCK, 0);

                for lz in 1..CHUNK_HEIGHT {
                    let wz = lz as i32;

                    // 3D density noise for cliffs and overhangs
                    let d3 = self.density_3d.get([wx * 0.04, wy * 0.04, wz as f64 * 0.05]);
                    let height_diff = surface_z - wz;
                    let is_solid = (height_diff as f64 + d3 * 4.0) >= 0.0;

                    if is_solid {
                        // Check 3D cave carving (Cheese & Spaghetti)
                        let c1 = self.cave_noise1.get([wx * 0.04, wy * 0.04, wz as f64 * 0.06]);
                        let c2 = self.cave_noise2.get([wx * 0.04, wy * 0.04, wz as f64 * 0.06]);
                        let is_cave = wz < surface_z - 2 && c1.abs() < 0.08 && c2.abs() < 0.08;

                        if is_cave {
                            if wz <= 7 {
                                // Deep lava lakes
                                chunk.set_block(lx, ly, lz, BlockId::LAVA, 7);
                            } else {
                                chunk.set_block(lx, ly, lz, BlockId::AIR, 0);
                            }
                        } else {
                            // Solid material by depth
                            let block_id = if wz == surface_z {
                                match biome {
                                    Biome::Desert => BlockId::SAND,
                                    Biome::SnowMountain => BlockId::SNOW,
                                    Biome::Ocean => {
                                        if wz <= sea_level {
                                            BlockId::SAND
                                        } else {
                                            BlockId::GRASS
                                        }
                                    }
                                    _ => BlockId::GRASS,
                                }
                            } else if wz >= surface_z - 3 {
                                match biome {
                                    Biome::Desert => BlockId::SAND,
                                    Biome::SnowMountain => BlockId::STONE,
                                    Biome::Ocean => BlockId::CLAY,
                                    _ => BlockId::DIRT,
                                }
                            } else {
                                // Subsurface stone and ore generation
                                let ore_roll: f32 = rng.gen();
                                if wz <= 12 && ore_roll < 0.015 {
                                    BlockId::DIAMOND_ORE
                                } else if wz <= 20 && ore_roll < 0.03 {
                                    BlockId::GOLD_ORE
                                } else if wz <= 35 && ore_roll < 0.06 {
                                    BlockId::IRON_ORE
                                } else if wz <= 48 && ore_roll < 0.10 {
                                    BlockId::COAL_ORE
                                } else if ore_roll < 0.12 {
                                    BlockId::GRAVEL
                                } else {
                                    BlockId::STONE
                                }
                            };
                            chunk.set_block(lx, ly, lz, block_id, 0);
                        }
                    } else if wz <= sea_level {
                        // Ocean / river water
                        chunk.set_block(lx, ly, lz, BlockId::WATER, 7);
                    }
                }
            }
        }

        // 2. Natural slope ramps generation
        for lx in 0..CHUNK_WIDTH {
            for ly in 0..CHUNK_DEPTH {
                // Find top solid block at (lx, ly)
                let mut top_z = -1;
                for lz in (1..CHUNK_HEIGHT).rev() {
                    let id = chunk.get_block_id(lx, ly, lz);
                    if !id.is_air() && id != BlockId::WATER && id != BlockId::LAVA {
                        top_z = lz as i32;
                        break;
                    }
                }

                if top_z > 1 && top_z < (CHUNK_HEIGHT - 2) as i32 {
                    // Check if adjacent tile is 1 block higher within chunk
                    let checks = [
                        (lx as i32 + 1, ly as i32, 1),  // East
                        (lx as i32 - 1, ly as i32, 3),  // West
                        (lx as i32, ly as i32 + 1, 2),  // South
                        (lx as i32, ly as i32 - 1, 0),  // North
                    ];

                    for (nx, ny, dir_meta) in checks {
                        if nx >= 0 && nx < CHUNK_WIDTH as i32 && ny >= 0 && ny < CHUNK_DEPTH as i32 {
                            let neighbor_solid = chunk.get_block_id(nx as usize, ny as usize, (top_z + 1) as usize);
                            let above_us = chunk.get_block_id(lx, ly, (top_z + 1) as usize);
                            if !neighbor_solid.is_air() && above_us.is_air() {
                                // We can place a ramp on top of our block leaning towards neighbor
                                if rng.gen_bool(0.4) {
                                    chunk.set_block(lx, ly, (top_z + 1) as usize, BlockId::RAMP, dir_meta);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Tree decoration for forest / plains
        for lx in 2..(CHUNK_WIDTH - 2) {
            for ly in 2..(CHUNK_DEPTH - 2) {
                let wx = (min_pos.x + lx as i32) as f64;
                let wy = (min_pos.y + ly as i32) as f64;
                let biome = self.get_biome(wx, wy);

                let tree_chance = match biome {
                    Biome::Forest => 0.04,
                    Biome::Plains => 0.008,
                    _ => 0.0,
                };

                if rng.gen_bool(tree_chance) {
                    // Find surface block
                    let mut ground_z = -1;
                    for lz in (1..CHUNK_HEIGHT - 7).rev() {
                        let id = chunk.get_block_id(lx, ly, lz);
                        if id == BlockId::GRASS || id == BlockId::DIRT {
                            ground_z = lz as i32;
                            break;
                        }
                    }

                    if ground_z > 0 && ground_z < (CHUNK_HEIGHT - 7) as i32 {
                        let trunk_h = rng.gen_range(4..=5);
                        // Trunk
                        for dz in 1..=trunk_h {
                            chunk.set_block(lx, ly, (ground_z + dz) as usize, BlockId::WOOD_LOG, 0);
                        }
                        let top_z = ground_z + trunk_h;
                        // Leaves canopy
                        for dx in -2..=2 {
                            for dy in -2..=2 {
                                for dz in -1..=1 {
                                    if dx * dx + dy * dy + dz * dz <= 5 {
                                        let leaf_x = (lx as i32 + dx) as usize;
                                        let leaf_y = (ly as i32 + dy) as usize;
                                        let leaf_z = (top_z + dz) as usize;
                                        if leaf_z < CHUNK_HEIGHT {
                                            let current = chunk.get_block_id(leaf_x, leaf_y, leaf_z);
                                            if current.is_air() {
                                                chunk.set_block(leaf_x, leaf_y, leaf_z, BlockId::LEAVES, 0);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        chunk
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_generation_deterministic() {
        let gen = TerrainGenerator::new(42);
        let c1 = gen.generate_chunk(ChunkPos::new(0, 0));
        let c2 = gen.generate_chunk(ChunkPos::new(0, 0));

        assert_eq!(c1.voxels(), c2.voxels());
        assert_eq!(c1.get_block_id(0, 0, 0), BlockId::BEDROCK);
    }

    #[test]
    fn test_terrain_has_solid_surface() {
        let gen = TerrainGenerator::new(12345);
        let chunk = gen.generate_chunk(ChunkPos::new(2, 3));

        let mut has_solid = false;
        for z in 1..CHUNK_HEIGHT {
            let id = chunk.get_block_id(8, 8, z);
            if id == BlockId::GRASS || id == BlockId::STONE || id == BlockId::SAND || id == BlockId::WATER {
                has_solid = true;
                break;
            }
        }
        assert!(has_solid, "Chunk should have non-empty surface blocks");
    }
}
