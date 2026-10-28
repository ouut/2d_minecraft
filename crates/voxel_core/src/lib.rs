pub mod block;
pub mod chunk;
pub mod coords;
pub mod traits;

pub use block::*;
pub use chunk::*;
pub use coords::*;
pub use traits::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_registry() {
        let mut reg = BlockRegistry::new();
        assert_eq!(reg.get(BlockId::STONE).unwrap().name, "stone");
        assert!(reg.is_solid(BlockId::STONE));
        assert!(!reg.is_solid(BlockId::AIR));
        assert!(reg.is_liquid(BlockId::WATER));

        let custom_id = reg
            .register_custom(
                "mythril_ore".into(),
                [100, 220, 255],
                [80, 180, 220],
                [60, 140, 180],
                8,
                true,
                false,
                false,
                false,
            )
            .expect("register custom block");
        assert!(custom_id.0 >= BlockId::FIRST_CUSTOM_ID);
        assert_eq!(reg.get(custom_id).unwrap().name, "mythril_ore");
        assert_eq!(reg.light_emission(custom_id), 8);
    }

    #[test]
    fn test_chunk_operations() {
        let mut chunk = Chunk::new();
        assert_eq!(chunk.get(0, 0, 0), Voxel::AIR);
        assert!(chunk.set_block(0, 0, 0, BlockId::GRASS, 0));
        assert_eq!(chunk.get_block_id(0, 0, 0), BlockId::GRASS);
        assert!(chunk.mesh_dirty);
    }

    #[test]
    fn test_world_coords_conversion() {
        let p1 = WorldPos::new(0, 0, 10);
        let (c1, lx1, ly1, lz1) = p1.to_chunk_and_local();
        assert_eq!(c1, ChunkPos::new(0, 0));
        assert_eq!((lx1, ly1, lz1), (0, 0, 10));

        let p2 = WorldPos::new(-1, -1, 5);
        let (c2, lx2, ly2, lz2) = p2.to_chunk_and_local();
        assert_eq!(c2, ChunkPos::new(-1, -1));
        assert_eq!((lx2, ly2, lz2), (15, 15, 5));
    }

    #[test]
    fn test_iso_camera_projection() {
        let mut cam = IsoCamera::new();
        cam.focus_x = 0.0;
        cam.focus_y = 0.0;
        cam.slice_z = 20;

        let screen_w = 800.0;
        let screen_h = 600.0;

        let p = WorldPos::new(5, 5, 20);
        let (sx, sy) = cam.world_to_screen(p, screen_w, screen_h);
        let unprojected = cam.screen_to_slice_plane(sx, sy, screen_w, screen_h);
        assert_eq!(unprojected, p);

        cam.rotate_clockwise();
        assert_eq!(cam.rotation, CardinalDir::East);
        let (sx2, sy2) = cam.world_to_screen(p, screen_w, screen_h);
        let unprojected2 = cam.screen_to_slice_plane(sx2, sy2, screen_w, screen_h);
        assert_eq!(unprojected2, p);
    }

    #[test]
    fn test_memory_world() {
        let mut world = MemoryWorld::new();
        let p = WorldPos::new(10, 10, 5);
        assert_eq!(world.get_voxel(p), Voxel::AIR);

        world.set_block(p, BlockId::DIRT, 0);
        assert_eq!(world.get_block_id(p), BlockId::DIRT);
        assert_eq!(world.surface_z(10, 10), 5);
    }
}
