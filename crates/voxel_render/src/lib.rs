pub mod atlas;
pub mod iso_renderer;
pub mod ui;

pub use atlas::*;
pub use iso_renderer::*;
pub use ui::*;

#[cfg(test)]
mod tests {
    use super::*;
    use voxel_core::block::BlockRegistry;

    #[test]
    fn test_atlas_generation_cpu() {
        let registry = BlockRegistry::new();
        let mut atlas = IsoAtlas::new();
        atlas.build_all(&registry);

        assert!(!atlas.tiles.is_empty());
        let grass_tile = atlas.tiles.get(&voxel_core::block::BlockId::GRASS);
        assert!(grass_tile.is_some());
        let tile = grass_tile.unwrap();
        assert_eq!(tile.width, 32);
        assert_eq!(tile.height, 32);
        assert_eq!(tile.rgba.len(), 32 * 32 * 4);
    }
}
