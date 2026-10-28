pub mod lighting;
pub mod physics;

pub use lighting::*;
pub use physics::*;

use voxel_core::block::BlockRegistry;
use voxel_core::coords::WorldPos;
use voxel_core::traits::WorldAccess;

pub struct SimulationEngine;

impl SimulationEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn recompute_lighting(
        &self,
        world: &mut dyn WorldAccess,
        registry: &BlockRegistry,
        min: WorldPos,
        max: WorldPos,
    ) {
        lighting::recompute_lighting(world, registry, min, max);
    }

    pub fn step_physics(
        &self,
        world: &mut dyn WorldAccess,
        registry: &BlockRegistry,
        active_center: WorldPos,
        radius: i32,
    ) -> usize {
        physics::PhysicsEngine::step_simulation(world, registry, active_center, radius)
    }

    pub fn check_collapse(
        &self,
        world: &mut dyn WorldAccess,
        registry: &BlockRegistry,
        mined_pos: WorldPos,
    ) -> usize {
        physics::PhysicsEngine::check_structural_collapse(world, registry, mined_pos)
    }
}

impl Default for SimulationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxel_core::block::BlockId;
    use voxel_core::traits::MemoryWorld;

    #[test]
    fn test_lighting_propagation() {
        let mut world = MemoryWorld::new();
        let registry = BlockRegistry::new();

        // Place a torch
        let torch_pos = WorldPos::new(5, 5, 10);
        world.set_block(torch_pos, BlockId::TORCH, 0);

        recompute_lighting(
            &mut world,
            &registry,
            WorldPos::new(0, 0, 0),
            WorldPos::new(10, 10, 20),
        );

        let torch_v = world.get_voxel(torch_pos);
        assert_eq!(torch_v.block_light(), 14);

        let neighbor_v = world.get_voxel(torch_pos.offset(1, 0, 0));
        assert_eq!(neighbor_v.block_light(), 13);
    }

    #[test]
    fn test_sand_gravity() {
        let mut world = MemoryWorld::new();
        let registry = BlockRegistry::new();

        let sand_pos = WorldPos::new(5, 5, 10);
        world.set_block(sand_pos, BlockId::SAND, 0);

        let center = WorldPos::new(5, 5, 10);
        let changes = PhysicsEngine::step_simulation(&mut world, &registry, center, 5);

        assert!(changes > 0);
        assert_eq!(world.get_block_id(sand_pos), BlockId::AIR);
        assert_eq!(world.get_block_id(sand_pos.offset(0, 0, -1)), BlockId::SAND);
    }

    #[test]
    fn test_water_lava_interaction() {
        let mut world = MemoryWorld::new();
        let registry = BlockRegistry::new();

        let water_pos = WorldPos::new(5, 5, 10);
        let lava_pos = WorldPos::new(6, 5, 10);

        world.set_block(water_pos, BlockId::WATER, 7);
        world.set_block(lava_pos, BlockId::LAVA, 7);

        let changes = PhysicsEngine::step_simulation(&mut world, &registry, water_pos, 5);
        assert!(changes > 0);

        // Lava met water should solidify
        let solid_id = world.get_block_id(lava_pos);
        assert!(solid_id == BlockId::OBSIDIAN || solid_id == BlockId::COBBLESTONE);
    }
}
