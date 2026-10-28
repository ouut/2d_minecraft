use std::collections::HashMap;
use voxel_core::block::{BlockId, Voxel};
use voxel_core::chunk::{Chunk, CHUNK_HEIGHT};
use voxel_core::coords::{ChunkPos, WorldPos};
use voxel_core::traits::WorldAccess;
use voxel_gen::TerrainGenerator;

pub struct ChunkWorld {
    pub chunks: HashMap<ChunkPos, Chunk>,
    pub generator: TerrainGenerator,
}

impl ChunkWorld {
    pub fn new(seed: u32) -> Self {
        Self {
            chunks: HashMap::new(),
            generator: TerrainGenerator::new(seed),
        }
    }

    pub fn ensure_chunk(&mut self, pos: ChunkPos) {
        if !self.chunks.contains_key(&pos) {
            let chunk = self.generator.generate_chunk(pos);
            self.chunks.insert(pos, chunk);
        }
    }

    pub fn ensure_chunks_around(&mut self, center_chunk: ChunkPos, radius: i32) {
        for cx in (center_chunk.x - radius)..=(center_chunk.x + radius) {
            for cy in (center_chunk.y - radius)..=(center_chunk.y + radius) {
                self.ensure_chunk(ChunkPos::new(cx, cy));
            }
        }
    }
}

impl WorldAccess for ChunkWorld {
    fn get_voxel(&self, pos: WorldPos) -> Voxel {
        if !self.in_bounds(pos.z) {
            return Voxel::AIR;
        }
        let (cpos, lx, ly, lz) = pos.to_chunk_and_local();
        if let Some(chunk) = self.chunks.get(&cpos) {
            chunk.get(lx, ly, lz)
        } else {
            // Unloaded chunk returns air
            Voxel::AIR
        }
    }

    fn set_voxel(&mut self, pos: WorldPos, voxel: Voxel) -> bool {
        if !self.in_bounds(pos.z) {
            return false;
        }
        let (cpos, lx, ly, lz) = pos.to_chunk_and_local();
        self.ensure_chunk(cpos);
        let chunk = self.chunks.get_mut(&cpos).unwrap();
        chunk.set(lx, ly, lz, voxel)
    }

    fn surface_z(&self, x: i32, y: i32) -> i32 {
        for z in (0..CHUNK_HEIGHT as i32).rev() {
            let id = self.get_block_id(WorldPos::new(x, y, z));
            if !id.is_air() && id != BlockId::WATER && id != BlockId::LAVA {
                return z;
            }
        }
        20
    }
}
