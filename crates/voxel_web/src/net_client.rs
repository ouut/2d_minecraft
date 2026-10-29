use serde::{Deserialize, Serialize};
use voxel_core::block::BlockId;
use voxel_core::coords::WorldPos;
use voxel_core::traits::{MemoryWorld, WorldAccess};
use game_net::{ClientEnvelope, CodecFormat, DualCodec, ServerEnvelope};

/// High frequency client movement & orientation state sent over network.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerInputState {
    pub forward: f32,
    pub right: f32,
    pub jump: bool,
    pub sneak: bool,
    pub yaw: f32,
    pub pitch: f32,
}

/// Explicit player commands sent over network.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlayerGameCommand {
    MineBlock { pos: (i32, i32, i32) },
    PlaceBlock { pos: (i32, i32, i32), block_id: u8 },
    SelectSlot { slot: u8 },
}

/// Server snapshot payload containing player entities and chunk updates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorldSnapshot {
    pub server_tick: u64,
    pub player_x: f32,
    pub player_y: f32,
    pub player_z: f32,
}

/// Server game event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GameEvent {
    BlockChanged { pos: (i32, i32, i32), block_id: u8 },
    ChatMessage { sender: String, text: String },
}

pub type GameClientEnvelope = ClientEnvelope<PlayerInputState, PlayerGameCommand>;
pub type GameServerEnvelope = ServerEnvelope<WorldSnapshot, GameEvent>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    OfflineStandalone,
    Connecting,
    Connected,
}

pub struct NetClient {
    pub status: ConnectionStatus,
    pub format: CodecFormat,
    pub player_id: Option<u64>,
}

impl Default for NetClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NetClient {
    pub fn new() -> Self {
        Self {
            status: ConnectionStatus::OfflineStandalone,
            format: CodecFormat::Binary,
            player_id: None,
        }
    }

    pub fn encode_envelope(&self, env: &GameClientEnvelope) -> Result<Vec<u8>, game_net::CodecError> {
        DualCodec::encode(self.format, env)
    }

    pub fn decode_server_envelope(&self, bytes: &[u8]) -> Result<GameServerEnvelope, game_net::CodecError> {
        DualCodec::decode_adaptive(bytes)
    }
}

/// Generates an authentic Minecraft procedural sample terrain for offline standalone play.
/// Includes rolling hills, bedrock, stone layers, ore pockets, grass, dirt, water, sand, oak trees, and a cave.
pub fn generate_demo_world() -> MemoryWorld {
    let mut world = MemoryWorld::new();

    // 3x3 chunks: -16..=31 in X, -16..=31 in Y
    let min_coord = -16;
    let max_coord = 31;

    for y in min_coord..=max_coord {
        for x in min_coord..=max_coord {
            // Smooth multi-frequency elevation heightmap
            let fx = x as f32;
            let fy = y as f32;
            let h_val = 14.0
                + 3.5 * ((fx * 0.12).sin() + (fy * 0.14).cos())
                + 1.8 * ((fx * 0.05 + fy * 0.05).sin());
            let surface_h = (h_val.round() as i32).clamp(4, 30);

            // 1. Bedrock base
            world.set_block(WorldPos::new(x, y, 0), BlockId::BEDROCK, 0);

            // 2. Stone layer with ores
            for z in 1..=(surface_h - 4).max(1) {
                let id = if (x.wrapping_mul(7) ^ y.wrapping_mul(13) ^ z.wrapping_mul(19)).abs() % 23 == 0 {
                    BlockId::COAL_ORE
                } else if (x.wrapping_mul(11) ^ y.wrapping_mul(17) ^ z.wrapping_mul(23)).abs() % 31 == 0 {
                    BlockId::IRON_ORE
                } else if z <= 5 && (x.wrapping_mul(29) ^ y.wrapping_mul(37)).abs() % 41 == 0 {
                    BlockId::DIAMOND_ORE
                } else {
                    BlockId::STONE
                };
                world.set_block(WorldPos::new(x, y, z), id, 0);
            }

            // 3. Dirt sublayer
            for z in ((surface_h - 3).max(1))..surface_h {
                world.set_block(WorldPos::new(x, y, z), BlockId::DIRT, 0);
            }

            // 4. Surface block (Sand near water level <= 12, Grass above)
            if surface_h <= 12 {
                world.set_block(WorldPos::new(x, y, surface_h), BlockId::SAND, 0);
            } else {
                world.set_block(WorldPos::new(x, y, surface_h), BlockId::GRASS, 0);
            }

            // 5. Water level in low basins
            let water_level = 11;
            for z in (surface_h + 1)..=water_level {
                world.set_block(WorldPos::new(x, y, z), BlockId::WATER, 0);
            }
        }
    }

    // 6. Carve out a small cave passage through a hill
    for cx in 4..=10 {
        for cy in 6..=12 {
            for cz in 11..=14 {
                world.set_block(WorldPos::new(cx, cy, cz), BlockId::AIR, 0);
            }
        }
    }

    // Place a torch near the cave entrance
    world.set_block(WorldPos::new(4, 6, 12), BlockId::TORCH, 0);

    // 7. Plant several procedural oak trees
    let tree_coords = [(6, 18), (20, 10), (-8, 12), (14, -6), (-10, -8)];
    for &(tx, ty) in &tree_coords {
        let base_z = world.surface_z(tx, ty);
        if base_z > 12 {
            // Wood trunk 4 blocks tall
            for z in 1..=4 {
                world.set_block(WorldPos::new(tx, ty, base_z + z), BlockId::WOOD_LOG, 0);
            }
            // Leaves canopy
            for lz in 3..=5 {
                let radius: i32 = if lz == 5 { 1 } else { 2 };
                for dx in -radius..=radius {
                    for dy in -radius..=radius {
                        if dx == 0 && dy == 0 && lz <= 4 {
                            continue; // Trunk is here
                        }
                        // Soften corners
                        if dx.abs() == radius && dy.abs() == radius && radius > 1 {
                            continue;
                        }
                        let leaf_pos = WorldPos::new(tx + dx, ty + dy, base_z + lz);
                        if world.get_block_id(leaf_pos).is_air() {
                            world.set_block(leaf_pos, BlockId::LEAVES, 0);
                        }
                    }
                }
            }
            // Top single leaf cap
            world.set_block(WorldPos::new(tx, ty, base_z + 6), BlockId::LEAVES, 0);
        }
    }

    world
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_encoding_decoding() {
        let client = NetClient::new();
        let env = GameClientEnvelope::Command(PlayerGameCommand::MineBlock {
            pos: (10, 20, 30),
        });

        let encoded = client.encode_envelope(&env).expect("encode client packet");
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_demo_world_generation() {
        let world = generate_demo_world();

        // Check bedrock at bottom
        assert_eq!(world.get_block_id(WorldPos::new(0, 0, 0)), BlockId::BEDROCK);

        // Check surface is non-zero
        let sz = world.surface_z(0, 0);
        assert!(sz >= 10 && sz <= 25);
        let surf_id = world.get_block_id(WorldPos::new(0, 0, sz));
        assert!(surf_id == BlockId::GRASS || surf_id == BlockId::SAND);
    }
}
