use crate::block::{BlockId, Voxel};

pub const CHUNK_WIDTH: usize = 16;
pub const CHUNK_DEPTH: usize = 16;
pub const CHUNK_HEIGHT: usize = 64;
pub const CHUNK_AREA: usize = CHUNK_WIDTH * CHUNK_DEPTH;
pub const CHUNK_VOLUME: usize = CHUNK_AREA * CHUNK_HEIGHT;

#[derive(Clone, Debug)]
pub struct Chunk {
    voxels: Box<[Voxel]>,
    pub mesh_dirty: bool,
    pub light_dirty: bool,
    pub sim_dirty: bool,
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunk {
    pub fn new() -> Self {
        let voxels = vec![Voxel::AIR; CHUNK_VOLUME].into_boxed_slice();
        Self {
            voxels,
            mesh_dirty: true,
            light_dirty: true,
            sim_dirty: true,
        }
    }

    #[inline]
    pub fn index(lx: usize, ly: usize, lz: usize) -> usize {
        lx + ly * CHUNK_WIDTH + lz * CHUNK_AREA
    }

    #[inline]
    pub fn in_bounds(lx: usize, ly: usize, lz: usize) -> bool {
        lx < CHUNK_WIDTH && ly < CHUNK_DEPTH && lz < CHUNK_HEIGHT
    }

    #[inline]
    pub fn get(&self, lx: usize, ly: usize, lz: usize) -> Voxel {
        if Self::in_bounds(lx, ly, lz) {
            self.voxels[Self::index(lx, ly, lz)]
        } else {
            Voxel::AIR
        }
    }

    #[inline]
    pub fn get_block_id(&self, lx: usize, ly: usize, lz: usize) -> BlockId {
        self.get(lx, ly, lz).id
    }

    #[inline]
    pub fn set(&mut self, lx: usize, ly: usize, lz: usize, voxel: Voxel) -> bool {
        if Self::in_bounds(lx, ly, lz) {
            let idx = Self::index(lx, ly, lz);
            if self.voxels[idx] != voxel {
                self.voxels[idx] = voxel;
                self.mesh_dirty = true;
                self.light_dirty = true;
                self.sim_dirty = true;
            }
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn set_block(&mut self, lx: usize, ly: usize, lz: usize, id: BlockId, meta: u8) -> bool {
        if Self::in_bounds(lx, ly, lz) {
            let idx = Self::index(lx, ly, lz);
            let mut v = self.voxels[idx];
            if v.id != id || v.meta != meta {
                v.id = id;
                v.meta = meta;
                self.voxels[idx] = v;
                self.mesh_dirty = true;
                self.light_dirty = true;
                self.sim_dirty = true;
            }
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn voxels(&self) -> &[Voxel] {
        &self.voxels
    }

    #[inline]
    pub fn voxels_mut(&mut self) -> &mut [Voxel] {
        &mut self.voxels
    }
}
