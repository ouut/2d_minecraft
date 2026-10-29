use flecs_ecs::prelude::*;
use voxel_core::block::BlockId;
use voxel_core::coords::WorldPos;
use voxel_core::traits::{ActorKind, JobType};

#[derive(Component, Copy, Clone, Debug, PartialEq, Default)]
pub struct Position3D {
    pub pos: WorldPos,
}

#[derive(Component, Copy, Clone, Debug, PartialEq, Default)]
pub struct Velocity3D {
    pub vx: f32,
    pub vy: f32,
    pub vz: f32,
}

#[derive(Component, Clone, Debug, PartialEq)]
pub struct ActorComponent {
    pub kind: ActorKind,
    pub name: String,
    pub carrying: Option<BlockId>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum ColonistState {
    #[default]
    Idle,
    MovingToJob(WorldPos),
    WorkingOnJob {
        target: WorldPos,
        timer: f32,
        total_time: f32,
    },
}

#[derive(Component, Clone, Debug, PartialEq, Default)]
pub struct ColonistAI {
    pub state: ColonistState,
    pub assigned_job_idx: Option<usize>,
    pub assigned_job_entity: Option<Entity>,
    pub current_path: Vec<WorldPos>,
    pub path_step: usize,
    pub step_cooldown: f32,
}

#[derive(Component, Clone, Debug, PartialEq)]
pub struct PlayerController {
    pub reach: i32,
    pub move_cooldown: f32,
    pub action_cooldown: f32,
}

impl Default for PlayerController {
    fn default() -> Self {
        Self {
            reach: 5,
            move_cooldown: 0.0,
            action_cooldown: 0.0,
        }
    }
}

#[derive(Component, Clone, Debug, PartialEq)]
pub struct Inventory {
    pub slots: [Option<(BlockId, u32)>; 9],
    pub selected_slot: usize,
}

impl Default for Inventory {
    fn default() -> Self {
        let mut inv = Self {
            slots: [None; 9],
            selected_slot: 0,
        };
        inv.slots[0] = Some((BlockId::DIRT, 64));
        inv.slots[1] = Some((BlockId::COBBLESTONE, 64));
        inv.slots[2] = Some((BlockId::PLANKS, 64));
        inv.slots[3] = Some((BlockId::RAMP, 64));
        inv.slots[4] = Some((BlockId::STAIR, 64));
        inv.slots[5] = Some((BlockId::TORCH, 32));
        inv.slots[6] = Some((BlockId::WATER, 8));
        inv.slots[7] = Some((BlockId::SAND, 32));
        inv.slots[8] = Some((BlockId::GLASS, 32));
        inv
    }
}

// Flecs Relationship Tag Structs
#[derive(Component, Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct WorkingOn;

#[derive(Component, Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct Likes;

#[derive(Component, Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct Faction;

#[derive(Component, Clone, Debug, PartialEq)]
pub struct JobTask {
    pub pos: WorldPos,
    pub job_type: JobType,
    pub build_block: BlockId,
    pub job_idx: usize,
}
