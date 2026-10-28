use voxel_core::block::{BlockId, BlockRegistry};
use voxel_core::coords::{CardinalDir, WorldPos};
use voxel_core::traits::{EngineCommand, WorldAccess};
use crate::components::{Inventory, PlayerController, Position3D};
use crate::pathfinding::is_standable;

pub struct PlayerSystem;

impl PlayerSystem {
    pub fn move_player(
        pos: &mut Position3D,
        ctrl: &mut PlayerController,
        camera_rot: CardinalDir,
        input_dx: i32,
        input_dy: i32,
        world: &dyn WorldAccess,
        registry: &BlockRegistry,
    ) -> bool {
        if input_dx == 0 && input_dy == 0 {
            return false;
        }

        // Rotate input movement by camera rotation
        let (move_dx, move_dy) = match camera_rot {
            CardinalDir::North => (input_dx, input_dy),
            CardinalDir::East => (-input_dy, input_dx),
            CardinalDir::South => (-input_dx, -input_dy),
            CardinalDir::West => (input_dy, -input_dx),
        };

        let curr = pos.pos;
        let target_same_z = curr.offset(move_dx, move_dy, 0);

        // 1. Try same-Z movement
        if is_standable(world, registry, target_same_z) {
            pos.pos = target_same_z;
            ctrl.move_cooldown = 0.15;
            return true;
        }

        // 2. Try auto-climb Ramp up (+1 Z)
        let target_up = curr.offset(move_dx, move_dy, 1);
        let curr_v = world.get_voxel(curr);
        let target_v = world.get_voxel(target_same_z);

        if (target_v.id == BlockId::RAMP
            || target_v.id == BlockId::STAIR
            || curr_v.id == BlockId::STAIR
            || curr_v.id == BlockId::LADDER)
            && is_standable(world, registry, target_up)
        {
            pos.pos = target_up;
            ctrl.move_cooldown = 0.2;
            return true;
        }

        // 3. Try step down (-1 Z)
        let target_down = curr.offset(move_dx, move_dy, -1);
        if is_standable(world, registry, target_down) {
            pos.pos = target_down;
            ctrl.move_cooldown = 0.15;
            return true;
        }

        false
    }

    pub fn player_action(
        pos: &Position3D,
        inv: &mut Inventory,
        ctrl: &mut PlayerController,
        target_pos: WorldPos,
        is_mine: bool,
        world: &dyn WorldAccess,
        _registry: &BlockRegistry,
    ) -> Option<EngineCommand> {
        if pos.pos.manhattan_dist(target_pos) > ctrl.reach {
            return None;
        }

        if is_mine {
            let target_v = world.get_voxel(target_pos);
            if !target_v.id.is_air() && target_v.id != BlockId::BEDROCK {
                ctrl.action_cooldown = 0.2;
                return Some(EngineCommand::SetBlock {
                    pos: target_pos,
                    id: BlockId::AIR,
                    meta: 0,
                    by_player_or_colonist: true,
                });
            }
        } else {
            // Place block from inventory
            if let Some((block_id, count)) = inv.slots[inv.selected_slot].as_mut() {
                if *count > 0 {
                    let target_v = world.get_voxel(target_pos);
                    if target_v.id.is_air() || target_v.id == BlockId::WATER {
                        let id_to_place = *block_id;
                        *count -= 1;
                        if *count == 0 {
                            inv.slots[inv.selected_slot] = None;
                        }
                        ctrl.action_cooldown = 0.2;
                        return Some(EngineCommand::SetBlock {
                            pos: target_pos,
                            id: id_to_place,
                            meta: 0,
                            by_player_or_colonist: true,
                        });
                    }
                }
            }
        }

        None
    }
}
