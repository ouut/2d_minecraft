use macroquad::prelude::*;
use voxel_core::block::BlockRegistry;
use voxel_core::coords::WorldPos;
use voxel_core::traits::WorldAccess;
use crate::camera::{PLAYER_HEIGHT, PLAYER_RADIUS};
use crate::mesh::FaceDir;

pub const MAX_RAY_DISTANCE: f32 = 6.0;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RaycastHit {
    pub hit_pos: WorldPos,
    pub hit_face: FaceDir,
    pub distance: f32,
}

/// 3D DDA (Digital Differential Analyzer) fast voxel traversal.
/// Casts a ray from `origin` in `dir` and finds the first solid voxel hit within `max_dist`.
pub fn raycast_voxel(
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
    world: &impl WorldAccess,
    reg: &BlockRegistry,
) -> Option<RaycastHit> {
    if dir.length_squared() < 1e-6 {
        return None;
    }
    let dir = dir.normalize();

    let mut x = origin.x.floor() as i32;
    let mut y = origin.y.floor() as i32;
    let mut z = origin.z.floor() as i32;

    // If starting inside a solid block
    let start_id = world.get_block_id(WorldPos::new(x, y, z));
    if !start_id.is_air() && reg.is_solid(start_id) {
        return Some(RaycastHit {
            hit_pos: WorldPos::new(x, y, z),
            hit_face: FaceDir::Top,
            distance: 0.0,
        });
    }

    let step_x = if dir.x > 0.0 { 1 } else { -1 };
    let step_y = if dir.y > 0.0 { 1 } else { -1 };
    let step_z = if dir.z > 0.0 { 1 } else { -1 };

    let t_delta_x = if dir.x.abs() > 1e-6 { (1.0 / dir.x).abs() } else { f32::INFINITY };
    let t_delta_y = if dir.y.abs() > 1e-6 { (1.0 / dir.y).abs() } else { f32::INFINITY };
    let t_delta_z = if dir.z.abs() > 1e-6 { (1.0 / dir.z).abs() } else { f32::INFINITY };

    let mut t_max_x = if dir.x > 0.0 {
        (x as f32 + 1.0 - origin.x) / dir.x
    } else if dir.x < 0.0 {
        (origin.x - x as f32) / (-dir.x)
    } else {
        f32::INFINITY
    };

    let mut t_max_y = if dir.y > 0.0 {
        (y as f32 + 1.0 - origin.y) / dir.y
    } else if dir.y < 0.0 {
        (origin.y - y as f32) / (-dir.y)
    } else {
        f32::INFINITY
    };

    let mut t_max_z = if dir.z > 0.0 {
        (z as f32 + 1.0 - origin.z) / dir.z
    } else if dir.z < 0.0 {
        (origin.z - z as f32) / (-dir.z)
    } else {
        f32::INFINITY
    };

    let mut current_dist = 0.0;
    let mut hit_face;

    while current_dist <= max_dist {
        if t_max_x < t_max_y && t_max_x < t_max_z {
            if t_max_x > max_dist {
                break;
            }
            x += step_x;
            current_dist = t_max_x;
            t_max_x += t_delta_x;
            hit_face = if step_x > 0 { FaceDir::West } else { FaceDir::East };
        } else if t_max_y < t_max_z {
            if t_max_y > max_dist {
                break;
            }
            y += step_y;
            current_dist = t_max_y;
            t_max_y += t_delta_y;
            hit_face = if step_y > 0 { FaceDir::South } else { FaceDir::North };
        } else {
            if t_max_z > max_dist {
                break;
            }
            z += step_z;
            current_dist = t_max_z;
            t_max_z += t_delta_z;
            hit_face = if step_z > 0 { FaceDir::Bottom } else { FaceDir::Top };
        }

        let cur_pos = WorldPos::new(x, y, z);
        let id = world.get_block_id(cur_pos);
        if !id.is_air() && reg.is_solid(id) {
            return Some(RaycastHit {
                hit_pos: cur_pos,
                hit_face,
                distance: current_dist,
            });
        }
    }

    None
}

/// Checks whether placing a block at `place_pos` would intersect the player's bounding box.
pub fn can_place_block_at(place_pos: WorldPos, player_pos: Vec3) -> bool {
    let bx_min = place_pos.x as f32;
    let bx_max = (place_pos.x + 1) as f32;
    let by_min = place_pos.y as f32;
    let by_max = (place_pos.y + 1) as f32;
    let bz_min = place_pos.z as f32;
    let bz_max = (place_pos.z + 1) as f32;

    let px_min = player_pos.x - PLAYER_RADIUS;
    let px_max = player_pos.x + PLAYER_RADIUS;
    let py_min = player_pos.y - PLAYER_RADIUS;
    let py_max = player_pos.y + PLAYER_RADIUS;
    let pz_min = player_pos.z;
    let pz_max = player_pos.z + PLAYER_HEIGHT;

    // Check AABB intersection
    let overlap_x = px_min < bx_max && px_max > bx_min;
    let overlap_y = py_min < by_max && py_max > by_min;
    let overlap_z = pz_min < bz_max && pz_max > bz_min;

    !(overlap_x && overlap_y && overlap_z)
}

/// Draws an authentic 3D wireframe highlight around the targeted voxel.
pub fn draw_selection_box(pos: WorldPos) {
    let (x, y, z) = (pos.x as f32, pos.y as f32, pos.z as f32);
    let offset = -0.001;
    let size = 1.002;
    let c = Color::new(0.08, 0.08, 0.08, 0.85);

    let p000 = vec3(x + offset, y + offset, z + offset);
    let p100 = vec3(x + size, y + offset, z + offset);
    let p110 = vec3(x + size, y + size, z + offset);
    let p010 = vec3(x + offset, y + size, z + offset);

    let p001 = vec3(x + offset, y + offset, z + size);
    let p101 = vec3(x + size, y + offset, z + size);
    let p111 = vec3(x + size, y + size, z + size);
    let p011 = vec3(x + offset, y + size, z + size);

    // Bottom face 4 edges
    draw_line_3d(p000, p100, c);
    draw_line_3d(p100, p110, c);
    draw_line_3d(p110, p010, c);
    draw_line_3d(p010, p000, c);

    // Top face 4 edges
    draw_line_3d(p001, p101, c);
    draw_line_3d(p101, p111, c);
    draw_line_3d(p111, p011, c);
    draw_line_3d(p011, p001, c);

    // 4 Vertical connecting edges
    draw_line_3d(p000, p001, c);
    draw_line_3d(p100, p101, c);
    draw_line_3d(p110, p111, c);
    draw_line_3d(p010, p011, c);
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxel_core::block::BlockId;
    use voxel_core::traits::MemoryWorld;

    #[test]
    fn test_raycast_voxel_hits() {
        let reg = BlockRegistry::new();
        let mut world = MemoryWorld::new();

        // Floor block at (0, 0, 5)
        let target = WorldPos::new(0, 0, 5);
        world.set_block(target, BlockId::STONE, 0);

        // Raycast straight down from (0.5, 0.5, 7.0)
        let hit = raycast_voxel(
            vec3(0.5, 0.5, 7.0),
            vec3(0.0, 0.0, -1.0),
            5.0,
            &world,
            &reg,
        );

        assert!(hit.is_some());
        let h = hit.unwrap();
        assert_eq!(h.hit_pos, target);
        assert_eq!(h.hit_face, FaceDir::Top);
        assert!((h.distance - 1.0).abs() < 1e-3);
    }

    #[test]
    fn test_raycast_side_hit() {
        let reg = BlockRegistry::new();
        let mut world = MemoryWorld::new();

        // Wall block at (5, 0, 5)
        let target = WorldPos::new(5, 0, 5);
        world.set_block(target, BlockId::STONE, 0);

        // Raycast East (+X) from (2.5, 0.5, 5.5)
        let hit = raycast_voxel(
            vec3(2.5, 0.5, 5.5),
            vec3(1.0, 0.0, 0.0),
            5.0,
            &world,
            &reg,
        );

        assert!(hit.is_some());
        let h = hit.unwrap();
        assert_eq!(h.hit_pos, target);
        assert_eq!(h.hit_face, FaceDir::West);
    }

    #[test]
    fn test_can_place_block_at() {
        let player_pos = vec3(0.0, 0.0, 5.0); // Footprint (-0.3..0.3, -0.3..0.3, 5.0..6.8)

        // Block at (0, 0, 5) overlaps player feet -> false
        assert!(!can_place_block_at(WorldPos::new(0, 0, 5), player_pos));

        // Block at (0, 0, 6) overlaps player torso -> false
        assert!(!can_place_block_at(WorldPos::new(0, 0, 6), player_pos));

        // Block at (0, 0, 4) is below feet -> true
        assert!(can_place_block_at(WorldPos::new(0, 0, 4), player_pos));

        // Block at (2, 0, 5) is 2 blocks away -> true
        assert!(can_place_block_at(WorldPos::new(2, 0, 5), player_pos));
    }
}
