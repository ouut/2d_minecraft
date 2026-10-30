use macroquad::models::{Mesh, Vertex};
use macroquad::prelude::*;
use voxel_core::block::{BlockId, BlockRegistry, Voxel};
use voxel_core::coords::{ChunkPos, WorldPos};
use voxel_core::chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH};
use voxel_core::traits::WorldAccess;

/// The 6 primary face directions of a cubic voxel.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FaceDir {
    Top,    // +Z (Up)
    Bottom, // -Z (Down)
    North,  // +Y
    South,  // -Y
    East,   // +X
    West,   // -X
}

impl FaceDir {
    pub const ALL: [FaceDir; 6] = [
        FaceDir::Top,
        FaceDir::Bottom,
        FaceDir::North,
        FaceDir::South,
        FaceDir::East,
        FaceDir::West,
    ];

    #[inline]
    pub const fn normal(self) -> (i32, i32, i32) {
        match self {
            FaceDir::Top => (0, 0, 1),
            FaceDir::Bottom => (0, 0, -1),
            FaceDir::North => (0, 1, 0),
            FaceDir::South => (0, -1, 0),
            FaceDir::East => (1, 0, 0),
            FaceDir::West => (-1, 0, 0),
        }
    }

    #[inline]
    pub const fn normal_f32(self) -> Vec3 {
        match self {
            FaceDir::Top => vec3(0.0, 0.0, 1.0),
            FaceDir::Bottom => vec3(0.0, 0.0, -1.0),
            FaceDir::North => vec3(0.0, 1.0, 0.0),
            FaceDir::South => vec3(0.0, -1.0, 0.0),
            FaceDir::East => vec3(1.0, 0.0, 0.0),
            FaceDir::West => vec3(-1.0, 0.0, 0.0),
        }
    }

    /// Directional lighting factor as defined by standard Minecraft lighting:
    /// Top (+Z): 1.0, North/South (+Y/-Y): 0.8, East/West (+X/-X): 0.7, Bottom (-Z): 0.5.
    #[inline]
    pub const fn dir_light(self) -> f32 {
        match self {
            FaceDir::Top => 1.0,
            FaceDir::North | FaceDir::South => 0.8,
            FaceDir::East | FaceDir::West => 0.7,
            FaceDir::Bottom => 0.5,
        }
    }
}

/// Ambient Occlusion brightness mapping for 4 levels (0: darkest, 3: fully unoccluded).
pub const AO_FACTORS: [f32; 4] = [0.45, 0.65, 0.82, 1.0];

/// Classic 4-level Minecraft Ambient Occlusion calculation.
/// Checks 2 adjacent side blocks and 1 corner block relative to the quad vertex.
#[inline]
pub fn calculate_ao(side1: bool, side2: bool, corner: bool) -> usize {
    match (side1, side2, corner) {
        (true, true, _) => 0,
        (true, false, false) | (false, true, false) => 1,
        (false, false, true) => 2,
        (true, false, true) | (false, true, true) => 1,
        _ => 3,
    }
}

/// Checks whether a block's face should be rendered against its neighbor.
/// Emits quad only when neighbor is air or transparent, avoiding hidden interior polygons.
#[inline]
pub fn is_face_visible(
    current_id: BlockId,
    neighbor_id: BlockId,
    reg: &BlockRegistry,
) -> bool {
    if current_id.is_air() {
        return false;
    }
    if neighbor_id.is_air() {
        return true;
    }
    // If neighbor is transparent:
    if reg.is_transparent(neighbor_id) {
        // Cull between identical transparent blocks (e.g. glass against glass)
        return current_id != neighbor_id;
    }
    false
}

/// Base color for blocks and faces, supporting distinct grass top/side and wood log textures.
pub fn get_block_face_color(
    block_id: BlockId,
    face: FaceDir,
    reg: &BlockRegistry,
) -> [u8; 4] {
    match block_id {
        BlockId::GRASS => match face {
            FaceDir::Top => [86, 176, 56, 255],     // Lush tender green
            FaceDir::Bottom => [110, 77, 47, 255],  // Dirt brown
            _ => [120, 85, 50, 255],                // Grass side dirt
        },
        BlockId::DIRT => [134, 96, 67, 255],
        BlockId::STONE => [128, 128, 128, 255],
        BlockId::COBBLESTONE => [105, 105, 105, 255],
        BlockId::WOOD_LOG => match face {
            FaceDir::Top | FaceDir::Bottom => [168, 140, 93, 255], // Rings
            _ => [103, 77, 46, 255],                               // Bark
        },
        BlockId::LEAVES => [60, 140, 45, 230],
        BlockId::SAND => [219, 206, 153, 255],
        BlockId::GRAVEL => [136, 130, 125, 255],
        BlockId::SNOW => [245, 245, 250, 255],
        BlockId::CLAY => [160, 165, 175, 255],
        BlockId::BEDROCK => [45, 45, 45, 255],
        BlockId::COAL_ORE => [60, 60, 60, 255],
        BlockId::IRON_ORE => [180, 150, 130, 255],
        BlockId::GOLD_ORE => [210, 180, 60, 255],
        BlockId::DIAMOND_ORE => [90, 220, 220, 255],
        BlockId::PLANKS => [180, 140, 90, 255],
        BlockId::BRICK => [160, 75, 60, 255],
        BlockId::GLASS => [200, 230, 245, 160],
        BlockId::TORCH => [255, 195, 77, 255],
        BlockId::WATER => [64, 110, 220, 180],
        BlockId::LAVA => [230, 90, 20, 255],
        BlockId::OBSIDIAN => [25, 20, 35, 255],
        _ => {
            if let Some(def) = reg.get(block_id) {
                let [r, g, b] = match face {
                    FaceDir::Top => def.top_color,
                    FaceDir::Bottom => def.left_color,
                    FaceDir::North | FaceDir::South => def.left_color,
                    FaceDir::East | FaceDir::West => def.right_color,
                };
                let a = if def.is_transparent { 180 } else { 255 };
                [r, g, b, a]
            } else {
                [180, 180, 180, 255]
            }
        }
    }
}

/// Mesh building accumulator that splits into separate meshes if vertex count nears u16::MAX.
pub struct MeshBuilder {
    pub meshes: Vec<Mesh>,
    current_vertices: Vec<Vertex>,
    current_indices: Vec<u16>,
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self {
            meshes: Vec::new(),
            current_vertices: Vec::with_capacity(4096),
            current_indices: Vec::with_capacity(6144),
        }
    }

    pub fn finish(mut self) -> Vec<Mesh> {
        if !self.current_vertices.is_empty() {
            self.meshes.push(Mesh {
                vertices: self.current_vertices,
                indices: self.current_indices,
                texture: None,
            });
        }
        self.meshes
    }

    #[inline]
    fn ensure_capacity(&mut self, additional_vertices: usize, additional_indices: usize) {
        // Macroquad's internal QuadGl drawcall capacity defaults to 10,000 vertices and 5,000 indices.
        // If a single mesh passes >= 5,000 indices, QuadGl logs "geometry() exceeded max drawcall size, clamping".
        // Therefore, we safely split meshes at 3,000 vertices / 4,500 indices per sub-mesh.
        if self.current_vertices.len() + additional_vertices > 3000
            || self.current_indices.len() + additional_indices > 4500
        {
            let vertices = std::mem::replace(&mut self.current_vertices, Vec::with_capacity(1024));
            let indices = std::mem::replace(&mut self.current_indices, Vec::with_capacity(1536));
            self.meshes.push(Mesh {
                vertices,
                indices,
                texture: None,
            });
        }
    }

    /// Adds a single quad with anisotropic split fix and directional + AO lighting.
    pub fn add_quad(
        &mut self,
        positions: [Vec3; 4],
        face: FaceDir,
        base_color: [u8; 4],
        ao_levels: [usize; 4],
        light_level: f32,
    ) {
        self.ensure_capacity(4, 6);

        let base_idx = self.current_vertices.len() as u16;
        let dir_light = face.dir_light();
        let normal = face.normal_f32();
        let normal4 = vec4(normal.x, normal.y, normal.z, 0.0);

        let uvs = [
            vec2(0.0, 0.0),
            vec2(1.0, 0.0),
            vec2(1.0, 1.0),
            vec2(0.0, 1.0),
        ];

        for i in 0..4 {
            let ao_idx = ao_levels[i].clamp(0, 3);
            let ao_factor = AO_FACTORS[ao_idx];
            let light = dir_light * ao_factor * light_level;

            let r = ((base_color[0] as f32 * light).clamp(0.0, 255.0)) as u8;
            let g = ((base_color[1] as f32 * light).clamp(0.0, 255.0)) as u8;
            let b = ((base_color[2] as f32 * light).clamp(0.0, 255.0)) as u8;
            let a = base_color[3];

            self.current_vertices.push(Vertex {
                position: positions[i],
                uv: uvs[i],
                color: [r, g, b, a],
                normal: normal4,
            });
        }

        // Classic Minecraft Anisotropic Quad Split:
        // When ao00 + ao11 < ao01 + ao10, flip triangulation diagonal to prevent diagonal light discontinuity.
        let ao00 = ao_levels[0];
        let ao10 = ao_levels[1];
        let ao11 = ao_levels[2];
        let ao01 = ao_levels[3];

        if ao00 + ao11 < ao01 + ao10 {
            // Flipped diagonal: (1, 2, 3) and (1, 3, 0)
            self.current_indices.extend_from_slice(&[
                base_idx + 1,
                base_idx + 2,
                base_idx + 3,
                base_idx + 1,
                base_idx + 3,
                base_idx + 0,
            ]);
        } else {
            // Standard diagonal: (0, 1, 2) and (0, 2, 3)
            self.current_indices.extend_from_slice(&[
                base_idx + 0,
                base_idx + 1,
                base_idx + 2,
                base_idx + 0,
                base_idx + 2,
                base_idx + 3,
            ]);
        }
    }
}

/// Generates face vertex quad coordinates and calculates AO factors for a voxel face.
pub fn emit_voxel_face(
    builder: &mut MeshBuilder,
    pos: WorldPos,
    face: FaceDir,
    block_id: BlockId,
    voxel: Voxel,
    world: &impl WorldAccess,
    reg: &BlockRegistry,
) {
    let (x, y, z) = (pos.x as f32, pos.y as f32, pos.z as f32);

    // Quad vertex coordinates in counter-clockwise order looking from outside
    let (positions, s1_s2_corner) = match face {
        FaceDir::Top => (
            [
                vec3(x, y, z + 1.0),
                vec3(x + 1.0, y, z + 1.0),
                vec3(x + 1.0, y + 1.0, z + 1.0),
                vec3(x, y + 1.0, z + 1.0),
            ],
            [
                // v0: -X, -Y
                ((-1, 0, 0), (0, -1, 0), (-1, -1, 0)),
                // v1: +X, -Y
                ((1, 0, 0), (0, -1, 0), (1, -1, 0)),
                // v2: +X, +Y
                ((1, 0, 0), (0, 1, 0), (1, 1, 0)),
                // v3: -X, +Y
                ((-1, 0, 0), (0, 1, 0), (-1, 1, 0)),
            ],
        ),
        FaceDir::Bottom => (
            [
                vec3(x, y + 1.0, z),
                vec3(x + 1.0, y + 1.0, z),
                vec3(x + 1.0, y, z),
                vec3(x, y, z),
            ],
            [
                // v0: -X, +Y
                ((-1, 0, 0), (0, 1, 0), (-1, 1, 0)),
                // v1: +X, +Y
                ((1, 0, 0), (0, 1, 0), (1, 1, 0)),
                // v2: +X, -Y
                ((1, 0, 0), (0, -1, 0), (1, -1, 0)),
                // v3: -X, -Y
                ((-1, 0, 0), (0, -1, 0), (-1, -1, 0)),
            ],
        ),
        FaceDir::North => (
            [
                vec3(x + 1.0, y + 1.0, z),
                vec3(x + 1.0, y + 1.0, z + 1.0),
                vec3(x, y + 1.0, z + 1.0),
                vec3(x, y + 1.0, z),
            ],
            [
                // v0: +X, -Z
                ((1, 0, 0), (0, 0, -1), (1, 0, -1)),
                // v1: +X, +Z
                ((1, 0, 0), (0, 0, 1), (1, 0, 1)),
                // v2: -X, +Z
                ((-1, 0, 0), (0, 0, 1), (-1, 0, 1)),
                // v3: -X, -Z
                ((-1, 0, 0), (0, 0, -1), (-1, 0, -1)),
            ],
        ),
        FaceDir::South => (
            [
                vec3(x, y, z),
                vec3(x, y, z + 1.0),
                vec3(x + 1.0, y, z + 1.0),
                vec3(x + 1.0, y, z),
            ],
            [
                // v0: -X, -Z
                ((-1, 0, 0), (0, 0, -1), (-1, 0, -1)),
                // v1: -X, +Z
                ((-1, 0, 0), (0, 0, 1), (-1, 0, 1)),
                // v2: +X, +Z
                ((1, 0, 0), (0, 0, 1), (1, 0, 1)),
                // v3: +X, -Z
                ((1, 0, 0), (0, 0, -1), (1, 0, -1)),
            ],
        ),
        FaceDir::East => (
            [
                vec3(x + 1.0, y + 1.0, z),
                vec3(x + 1.0, y + 1.0, z + 1.0),
                vec3(x + 1.0, y, z + 1.0),
                vec3(x + 1.0, y, z),
            ],
            [
                // v0: +Y, -Z
                ((0, 1, 0), (0, 0, -1), (0, 1, -1)),
                // v1: +Y, +Z
                ((0, 1, 0), (0, 0, 1), (0, 1, 1)),
                // v2: -Y, +Z
                ((0, -1, 0), (0, 0, 1), (0, -1, 1)),
                // v3: -Y, -Z
                ((0, -1, 0), (0, 0, -1), (0, -1, -1)),
            ],
        ),
        FaceDir::West => (
            [
                vec3(x, y, z),
                vec3(x, y, z + 1.0),
                vec3(x, y + 1.0, z + 1.0),
                vec3(x, y + 1.0, z),
            ],
            [
                // v0: -Y, -Z
                ((0, -1, 0), (0, 0, -1), (0, -1, -1)),
                // v1: -Y, +Z
                ((0, -1, 0), (0, 0, 1), (0, -1, 1)),
                // v2: +Y, +Z
                ((0, 1, 0), (0, 0, 1), (0, 1, 1)),
                // v3: +Y, -Z
                ((0, 1, 0), (0, 0, -1), (0, 1, -1)),
            ],
        ),
    };

    let norm = face.normal();
    let norm_pos = pos.offset(norm.0, norm.1, norm.2);

    let is_solid_block = |p: WorldPos| -> bool {
        let n_id = world.get_block_id(p);
        !n_id.is_air() && reg.is_solid(n_id)
    };

    let mut ao_levels = [3usize; 4];
    for i in 0..4 {
        let (s1, s2, cor) = s1_s2_corner[i];
        let side1 = is_solid_block(norm_pos.offset(s1.0, s1.1, s1.2));
        let side2 = is_solid_block(norm_pos.offset(s2.0, s2.1, s2.2));
        let corner = is_solid_block(norm_pos.offset(cor.0, cor.1, cor.2));
        ao_levels[i] = calculate_ao(side1, side2, corner);
    }

    // Dual-channel lighting factor (0.35 to 1.0)
    let neighbor_voxel = world.get_voxel(norm_pos);
    let max_light = voxel.max_light().max(neighbor_voxel.max_light());
    let light_level = if max_light > 0 {
        0.35 + 0.65 * (max_light as f32 / 15.0)
    } else {
        1.0 // Default full daylight if unlit
    };

    let base_color = get_block_face_color(block_id, face, reg);
    builder.add_quad(positions, face, base_color, ao_levels, light_level);
}

/// Builds full 3D meshes for a single chunk.
pub fn build_chunk_mesh(
    chunk_pos: ChunkPos,
    world: &impl WorldAccess,
    reg: &BlockRegistry,
) -> Vec<Mesh> {
    let mut builder = MeshBuilder::new();
    let min_world = chunk_pos.min_world_pos();

    for lz in 0..CHUNK_HEIGHT {
        let gz = lz as i32;
        for ly in 0..CHUNK_DEPTH {
            let gy = min_world.y + ly as i32;
            for lx in 0..CHUNK_WIDTH {
                let gx = min_world.x + lx as i32;
                let pos = WorldPos::new(gx, gy, gz);
                let voxel = world.get_voxel(pos);
                let block_id = voxel.id;

                if block_id.is_air() {
                    continue;
                }

                // Check 6 adjacent faces for face culling
                for &face in &FaceDir::ALL {
                    let (dx, dy, dz) = face.normal();
                    let neighbor_pos = pos.offset(dx, dy, dz);
                    let neighbor_id = world.get_block_id(neighbor_pos);

                    if is_face_visible(block_id, neighbor_id, reg) {
                        emit_voxel_face(
                            &mut builder,
                            pos,
                            face,
                            block_id,
                            voxel,
                            world,
                            reg,
                        );
                    }
                }
            }
        }
    }

    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxel_core::traits::MemoryWorld;

    #[test]
    fn test_face_dir_properties() {
        assert_eq!(FaceDir::Top.normal(), (0, 0, 1));
        assert_eq!(FaceDir::Bottom.normal(), (0, 0, -1));
        assert_eq!(FaceDir::North.normal(), (0, 1, 0));
        assert_eq!(FaceDir::South.normal(), (0, -1, 0));
        assert_eq!(FaceDir::East.normal(), (1, 0, 0));
        assert_eq!(FaceDir::West.normal(), (-1, 0, 0));

        assert_eq!(FaceDir::Top.dir_light(), 1.0);
        assert_eq!(FaceDir::Bottom.dir_light(), 0.5);
        assert_eq!(FaceDir::North.dir_light(), 0.8);
        assert_eq!(FaceDir::South.dir_light(), 0.8);
        assert_eq!(FaceDir::East.dir_light(), 0.7);
        assert_eq!(FaceDir::West.dir_light(), 0.7);
    }

    #[test]
    fn test_calculate_ao() {
        // Both sides occluded -> Level 0
        assert_eq!(calculate_ao(true, true, false), 0);
        assert_eq!(calculate_ao(true, true, true), 0);

        // One side occluded -> Level 1
        assert_eq!(calculate_ao(true, false, false), 1);
        assert_eq!(calculate_ao(false, true, false), 1);

        // Only corner occluded -> Level 2
        assert_eq!(calculate_ao(false, false, true), 2);

        // No occlusion -> Level 3
        assert_eq!(calculate_ao(false, false, false), 3);
    }

    #[test]
    fn test_face_culling() {
        let reg = BlockRegistry::new();

        // Stone adjacent to air -> face is visible
        assert!(is_face_visible(BlockId::STONE, BlockId::AIR, &reg));

        // Stone adjacent to solid stone -> face is culled
        assert!(!is_face_visible(BlockId::STONE, BlockId::STONE, &reg));

        // Stone adjacent to transparent glass -> face is visible
        assert!(is_face_visible(BlockId::STONE, BlockId::GLASS, &reg));

        // Glass adjacent to glass -> face between them is culled
        assert!(!is_face_visible(BlockId::GLASS, BlockId::GLASS, &reg));
    }

    #[test]
    fn test_chunk_mesh_building() {
        let reg = BlockRegistry::new();
        let mut world = MemoryWorld::new();

        // Single isolated stone block
        let p = WorldPos::new(0, 0, 10);
        world.set_block(p, BlockId::STONE, 0);

        let meshes = build_chunk_mesh(ChunkPos::new(0, 0), &world, &reg);
        assert_eq!(meshes.len(), 1);
        // An isolated cube has 6 visible faces: 6 * 4 = 24 vertices, 6 * 6 = 36 indices
        assert_eq!(meshes[0].vertices.len(), 24);
        assert_eq!(meshes[0].indices.len(), 36);

        // Place another block adjacent to it (+X)
        world.set_block(p.offset(1, 0, 0), BlockId::STONE, 0);
        let meshes2 = build_chunk_mesh(ChunkPos::new(0, 0), &world, &reg);
        // 2 cubes sharing a face: 12 - 2 = 10 visible faces: 10 * 4 = 40 vertices, 60 indices
        assert_eq!(meshes2[0].vertices.len(), 40);
        assert_eq!(meshes2[0].indices.len(), 60);
    }
}
