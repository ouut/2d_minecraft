use std::collections::{HashSet, VecDeque};
use voxel_core::block::{BlockId, BlockRegistry, Voxel};
use voxel_core::coords::WorldPos;
use voxel_core::traits::WorldAccess;

pub const MAX_CANTILEVER: usize = 5;

pub struct PhysicsEngine;

impl PhysicsEngine {
    /// Step fluid and gravity simulation within an active radius around active_center
    pub fn step_simulation(
        world: &mut dyn WorldAccess,
        registry: &BlockRegistry,
        active_center: WorldPos,
        radius: i32,
    ) -> usize {
        let mut changes = 0;
        let x_min = active_center.x - radius;
        let x_max = active_center.x + radius;
        let y_min = active_center.y - radius;
        let y_max = active_center.y + radius;
        let z_min = (active_center.z - radius).max(1);
        let z_max = (active_center.z + radius).min(63);

        // Process bottom to top for gravity, and top to bottom for fluids
        // 1. Gravity (Sand / Gravel)
        for z in z_min..=z_max {
            for x in x_min..=x_max {
                for y in y_min..=y_max {
                    let pos = WorldPos::new(x, y, z);
                    let v = world.get_voxel(pos);
                    let def = registry.get(v.id);

                    if let Some(d) = def {
                        if d.has_gravity && !d.is_liquid {
                            let below = pos.offset(0, 0, -1);
                            let below_v = world.get_voxel(below);

                            if below_v.id.is_air() || registry.is_liquid(below_v.id) {
                                // Fall straight down
                                world.set_voxel(below, v);
                                world.set_voxel(pos, Voxel::AIR);
                                changes += 1;
                            } else {
                                // Try sliding diagonally down (休止角滑落)
                                let slides = [
                                    (1, 0),
                                    (-1, 0),
                                    (0, 1),
                                    (0, -1),
                                ];
                                for (dx, dy) in slides {
                                    let diag = pos.offset(dx, dy, -1);
                                    let side = pos.offset(dx, dy, 0);
                                    if world.get_voxel(side).id.is_air()
                                        && world.get_voxel(diag).id.is_air()
                                    {
                                        world.set_voxel(diag, v);
                                        world.set_voxel(pos, Voxel::AIR);
                                        changes += 1;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Fluid Simulation (Water & Lava)
        for z in (z_min..=z_max).rev() {
            for x in x_min..=x_max {
                for y in y_min..=y_max {
                    let pos = WorldPos::new(x, y, z);
                    let v = world.get_voxel(pos);

                    if !registry.is_liquid(v.id) {
                        continue;
                    }

                    let is_water = v.id == BlockId::WATER;
                    let current_level = if v.meta == 0 { 7 } else { v.meta };

                    // Interaction check with adjacent fluids (Water meets Lava)
                    let cardinal = [
                        pos.offset(1, 0, 0),
                        pos.offset(-1, 0, 0),
                        pos.offset(0, 1, 0),
                        pos.offset(0, -1, 0),
                        pos.offset(0, 0, -1),
                    ];

                    let mut reacted = false;
                    for adj in cardinal {
                        let adj_v = world.get_voxel(adj);
                        if is_water && adj_v.id == BlockId::LAVA {
                            // Water touches Lava -> Cobblestone or Obsidian
                            let new_id = if adj_v.meta >= 7 || adj_v.meta == 0 {
                                BlockId::OBSIDIAN
                            } else {
                                BlockId::COBBLESTONE
                            };
                            world.set_block(adj, new_id, 0);
                            reacted = true;
                            changes += 1;
                        } else if !is_water && adj_v.id == BlockId::WATER {
                            let new_id = if v.meta >= 7 || v.meta == 0 {
                                BlockId::OBSIDIAN
                            } else {
                                BlockId::COBBLESTONE
                            };
                            world.set_block(pos, new_id, 0);
                            reacted = true;
                            changes += 1;
                            break;
                        }
                    }

                    if reacted {
                        continue;
                    }

                    // A. Flow straight down
                    let below = pos.offset(0, 0, -1);
                    if world.in_bounds(below.z) {
                        let below_v = world.get_voxel(below);
                        if below_v.id.is_air() {
                            world.set_block(below, v.id, 7);
                            changes += 1;
                            continue;
                        } else if below_v.id == v.id && below_v.meta < 7 {
                            world.set_block(below, v.id, 7);
                            changes += 1;
                            continue;
                        }
                    }

                    // B. Flow horizontally if below is solid or full liquid
                    if current_level > 1 {
                        let decay = if is_water { 1 } else { 2 };
                        let next_level = current_level.saturating_sub(decay);

                        if next_level >= 1 {
                            let spread_dirs = [
                                pos.offset(1, 0, 0),
                                pos.offset(-1, 0, 0),
                                pos.offset(0, 1, 0),
                                pos.offset(0, -1, 0),
                            ];

                            for adj in spread_dirs {
                                if !world.in_bounds(adj.z) {
                                    continue;
                                }
                                let adj_v = world.get_voxel(adj);
                                if adj_v.id.is_air() {
                                    world.set_block(adj, v.id, next_level);
                                    changes += 1;
                                } else if adj_v.id == v.id && adj_v.meta < next_level {
                                    world.set_block(adj, v.id, next_level);
                                    changes += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        changes
    }

    /// Check structural collapse around a mined/removed block
    pub fn check_structural_collapse(
        world: &mut dyn WorldAccess,
        registry: &BlockRegistry,
        mined_pos: WorldPos,
    ) -> usize {
        let mut collapsed_count = 0;

        // Check 6 neighbors of the mined position
        for neighbor in mined_pos.neighbors_6() {
            let v = world.get_voxel(neighbor);
            if !registry.is_solid(v.id) || v.id == BlockId::BEDROCK {
                continue;
            }

            // Test if neighbor has vertical/grounded support within MAX_CANTILEVER distance
            if !Self::is_supported(world, registry, neighbor) {
                // Collect connected floating cluster
                let cluster = Self::collect_unsupported_cluster(world, registry, neighbor);
                for p in cluster {
                    let old_v = world.get_voxel(p);
                    world.set_voxel(p, Voxel::AIR);

                    // Find ground below for debris landing
                    let mut drop_z = p.z - 1;
                    while drop_z > 0 && world.get_voxel(WorldPos::new(p.x, p.y, drop_z)).id.is_air() {
                        drop_z -= 1;
                    }
                    let land_pos = WorldPos::new(p.x, p.y, (drop_z + 1).min(p.z));
                    world.set_voxel(land_pos, old_v);
                    collapsed_count += 1;
                }
            }
        }

        collapsed_count
    }

    fn is_supported(world: &dyn WorldAccess, registry: &BlockRegistry, start: WorldPos) -> bool {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        queue.push_back((start, 0));
        visited.insert(start);

        while let Some((pos, dist)) = queue.pop_front() {
            // If connected to bedrock or grounded column
            if pos.z <= 1 {
                return true;
            }

            // Check if standing on solid ground
            let below = pos.offset(0, 0, -1);
            let below_v = world.get_voxel(below);
            if registry.is_solid(below_v.id) && below.z <= 2 {
                return true;
            }

            if dist >= MAX_CANTILEVER {
                continue;
            }

            for n in pos.neighbors_6() {
                if visited.contains(&n) {
                    continue;
                }
                let nv = world.get_voxel(n);
                if registry.is_solid(nv.id) {
                    visited.insert(n);
                    queue.push_back((n, dist + 1));
                }
            }
        }

        false
    }

    fn collect_unsupported_cluster(
        world: &dyn WorldAccess,
        registry: &BlockRegistry,
        start: WorldPos,
    ) -> Vec<WorldPos> {
        let mut cluster = Vec::new();
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        queue.push_back(start);
        visited.insert(start);

        while let Some(pos) = queue.pop_front() {
            cluster.push(pos);
            if cluster.len() > 64 {
                // Safeguard against cascading whole map
                break;
            }

            for n in pos.neighbors_6() {
                if visited.contains(&n) {
                    continue;
                }
                let nv = world.get_voxel(n);
                if registry.is_solid(nv.id) && nv.id != BlockId::BEDROCK {
                    visited.insert(n);
                    queue.push_back(n);
                }
            }
        }

        cluster
    }
}
