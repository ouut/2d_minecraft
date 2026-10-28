use std::collections::HashMap;
use crate::block::{BlockId, Voxel};
use crate::chunk::{Chunk, CHUNK_HEIGHT};
use crate::coords::{ChunkPos, WorldPos};

pub trait WorldAccess {
    fn get_voxel(&self, pos: WorldPos) -> Voxel;
    fn set_voxel(&mut self, pos: WorldPos, voxel: Voxel) -> bool;
    
    #[inline]
    fn get_block_id(&self, pos: WorldPos) -> BlockId {
        self.get_voxel(pos).id
    }

    #[inline]
    fn set_block(&mut self, pos: WorldPos, id: BlockId, meta: u8) -> bool {
        let mut v = self.get_voxel(pos);
        v.id = id;
        v.meta = meta;
        self.set_voxel(pos, v)
    }

    fn surface_z(&self, x: i32, y: i32) -> i32;

    #[inline]
    fn in_bounds(&self, z: i32) -> bool {
        (0..CHUNK_HEIGHT as i32).contains(&z)
    }
}

/// In-memory World implementing WorldAccess for testing and standalone simulation
#[derive(Clone, Debug, Default)]
pub struct MemoryWorld {
    chunks: HashMap<ChunkPos, Chunk>,
}

impl MemoryWorld {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
        }
    }

    pub fn insert_chunk(&mut self, pos: ChunkPos, chunk: Chunk) {
        self.chunks.insert(pos, chunk);
    }

    pub fn get_chunk(&self, pos: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&pos)
    }

    pub fn get_chunk_mut(&mut self, pos: ChunkPos) -> Option<&mut Chunk> {
        self.chunks.get_mut(&pos)
    }

    pub fn chunks(&self) -> &HashMap<ChunkPos, Chunk> {
        &self.chunks
    }
}

impl WorldAccess for MemoryWorld {
    fn get_voxel(&self, pos: WorldPos) -> Voxel {
        if !self.in_bounds(pos.z) {
            return Voxel::AIR;
        }
        let (cpos, lx, ly, lz) = pos.to_chunk_and_local();
        if let Some(chunk) = self.chunks.get(&cpos) {
            chunk.get(lx, ly, lz)
        } else {
            Voxel::AIR
        }
    }

    fn set_voxel(&mut self, pos: WorldPos, voxel: Voxel) -> bool {
        if !self.in_bounds(pos.z) {
            return false;
        }
        let (cpos, lx, ly, lz) = pos.to_chunk_and_local();
        let chunk = self.chunks.entry(cpos).or_insert_with(Chunk::new);
        chunk.set(lx, ly, lz, voxel)
    }

    fn surface_z(&self, x: i32, y: i32) -> i32 {
        for z in (0..CHUNK_HEIGHT as i32).rev() {
            let v = self.get_voxel(WorldPos::new(x, y, z));
            if !v.id.is_air() {
                return z;
            }
        }
        0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum GameMode {
    #[default]
    Player = 0,
    Colony = 1,
    GodEditor = 2,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum JobType {
    #[default]
    Mine,
    Channel,
    Build,
    Chop,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ActorKind {
    Player,
    Colonist,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JobDesignation {
    pub pos: WorldPos,
    pub job_type: JobType,
    pub build_block: BlockId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActorRenderView {
    pub id: u32,
    pub pos: WorldPos,
    pub kind: ActorKind,
    pub name: String,
    pub carrying: Option<BlockId>,
    pub busy_progress: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JobRenderView {
    pub pos: WorldPos,
    pub job_type: JobType,
    pub build_block: BlockId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiStateView {
    pub game_mode: GameMode,
    pub slice_z: i32,
    pub selected_block: BlockId,
    pub selected_tool: String,
    pub mod_tools: Vec<String>,
    pub repl_logs: Vec<String>,
    pub repl_input: String,
    pub status_msg: String,
}

impl Default for UiStateView {
    fn default() -> Self {
        Self {
            game_mode: GameMode::Player,
            slice_z: 32,
            selected_block: BlockId::DIRT,
            selected_tool: "mine".into(),
            mod_tools: Vec::new(),
            repl_logs: Vec::new(),
            repl_input: String::new(),
            status_msg: "Ready".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EngineCommand {
    SetBlock {
        pos: WorldPos,
        id: BlockId,
        meta: u8,
        by_player_or_colonist: bool,
    },
    FillBox {
        min: WorldPos,
        max: WorldPos,
        id: BlockId,
        hollow: bool,
    },
    SpawnFluid {
        pos: WorldPos,
        id: BlockId,
        level: u8,
    },
    SpawnColonist {
        pos: WorldPos,
        name: String,
    },
    TeleportPlayer {
        pos: WorldPos,
    },
    SetSliceZ {
        slice_z: i32,
    },
    DesignateBox {
        min: WorldPos,
        max: WorldPos,
        job_type: JobType,
        build_block: BlockId,
    },
    InvokeModTool {
        tool_name: String,
        pos: WorldPos,
    },
    LogMessage {
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum SimEvent {
    BlockPlaced { pos: WorldPos, id: BlockId },
    BlockMined { pos: WorldPos, old_id: BlockId },
    BlockInteracted { pos: WorldPos, id: BlockId },
    AtlasRebuildNeeded,
}
