use std::collections::VecDeque;
use voxel_core::block::{BlockId, BlockRegistry};
use voxel_core::chunk::CHUNK_HEIGHT;
use voxel_core::coords::WorldPos;
use voxel_core::traits::WorldAccess;

pub fn recompute_lighting(
    world: &mut dyn WorldAccess,
    registry: &BlockRegistry,
    min: WorldPos,
    max: WorldPos,
) {
    let mut sky_queue: VecDeque<WorldPos> = VecDeque::new();
    let mut block_queue: VecDeque<WorldPos> = VecDeque::new();

    let x_min = min.x.min(max.x);
    let x_max = min.x.max(max.x);
    let y_min = min.y.min(max.y);
    let y_max = min.y.max(max.y);

    // 1. Vertical SkyLight downward casting
    for x in x_min..=x_max {
        for y in y_min..=y_max {
            let mut current_sky = 15u8;
            for z in (0..CHUNK_HEIGHT as i32).rev() {
                let p = WorldPos::new(x, y, z);
                let mut v = world.get_voxel(p);

                let emission = registry.light_emission(v.id);
                if emission > 0 {
                    v.set_block_light(emission);
                    block_queue.push_back(p);
                } else {
                    v.set_block_light(0);
                }

                if registry.is_solid(v.id) {
                    if v.id == BlockId::LEAVES {
                        current_sky = current_sky.saturating_sub(2);
                        v.set_sky_light(current_sky);
                        if current_sky > 0 {
                            sky_queue.push_back(p);
                        }
                    } else {
                        current_sky = 0;
                        v.set_sky_light(0);
                    }
                } else if registry.is_liquid(v.id) {
                    current_sky = current_sky.saturating_sub(2);
                    v.set_sky_light(current_sky);
                    if current_sky > 0 {
                        sky_queue.push_back(p);
                    }
                } else {
                    v.set_sky_light(current_sky);
                    if current_sky > 0 {
                        sky_queue.push_back(p);
                    }
                }

                world.set_voxel(p, v);
            }
        }
    }

    // 2. BFS Spread SkyLight
    while let Some(pos) = sky_queue.pop_front() {
        let v = world.get_voxel(pos);
        let light = v.sky_light();
        if light <= 1 {
            continue;
        }

        for npos in pos.neighbors_6() {
            if !world.in_bounds(npos.z) {
                continue;
            }
            let nv = world.get_voxel(npos);
            if registry.is_solid(nv.id) && nv.id != BlockId::LEAVES {
                continue;
            }

            let attenuation = if nv.id == BlockId::LEAVES || registry.is_liquid(nv.id) {
                2
            } else {
                1
            };

            let next_light = light.saturating_sub(attenuation);
            if next_light > nv.sky_light() {
                let mut updated = nv;
                updated.set_sky_light(next_light);
                world.set_voxel(npos, updated);
                sky_queue.push_back(npos);
            }
        }
    }

    // 3. BFS Spread BlockLight
    while let Some(pos) = block_queue.pop_front() {
        let v = world.get_voxel(pos);
        let light = v.block_light();
        if light <= 1 {
            continue;
        }

        for npos in pos.neighbors_6() {
            if !world.in_bounds(npos.z) {
                continue;
            }
            let nv = world.get_voxel(npos);
            if registry.is_solid(nv.id) && nv.id != BlockId::LEAVES {
                continue;
            }

            let attenuation = if nv.id == BlockId::LEAVES || registry.is_liquid(nv.id) {
                2
            } else {
                1
            };

            let next_light = light.saturating_sub(attenuation);
            if next_light > nv.block_light() {
                let mut updated = nv;
                updated.set_block_light(next_light);
                world.set_voxel(npos, updated);
                block_queue.push_back(npos);
            }
        }
    }
}
