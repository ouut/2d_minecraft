#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BlockId(pub u8);

impl BlockId {
    pub const AIR: BlockId = BlockId(0);
    pub const GRASS: BlockId = BlockId(1);
    pub const DIRT: BlockId = BlockId(2);
    pub const STONE: BlockId = BlockId(3);
    pub const SAND: BlockId = BlockId(4);
    pub const GRAVEL: BlockId = BlockId(5);
    pub const SNOW: BlockId = BlockId(6);
    pub const CLAY: BlockId = BlockId(7);
    pub const BEDROCK: BlockId = BlockId(8);
    pub const COAL_ORE: BlockId = BlockId(9);
    pub const IRON_ORE: BlockId = BlockId(10);
    pub const GOLD_ORE: BlockId = BlockId(11);
    pub const DIAMOND_ORE: BlockId = BlockId(12);
    pub const WOOD_LOG: BlockId = BlockId(13);
    pub const LEAVES: BlockId = BlockId(14);
    pub const PLANKS: BlockId = BlockId(15);
    pub const COBBLESTONE: BlockId = BlockId(16);
    pub const BRICK: BlockId = BlockId(17);
    pub const GLASS: BlockId = BlockId(18);
    pub const RAMP: BlockId = BlockId(19);
    pub const STAIR: BlockId = BlockId(20);
    pub const LADDER: BlockId = BlockId(21);
    pub const TORCH: BlockId = BlockId(22);
    pub const GLOWSTONE: BlockId = BlockId(23);
    pub const WATER: BlockId = BlockId(24);
    pub const LAVA: BlockId = BlockId(25);
    pub const OBSIDIAN: BlockId = BlockId(26);

    pub const FIRST_CUSTOM_ID: u8 = 27;
    pub const MAX_ID: u8 = 254;

    #[inline]
    pub const fn is_air(self) -> bool {
        self.0 == 0
    }
}

impl From<u8> for BlockId {
    #[inline]
    fn from(val: u8) -> Self {
        BlockId(val)
    }
}

impl From<BlockId> for u8 {
    #[inline]
    fn from(id: BlockId) -> Self {
        id.0
    }
}

/// 4-byte packed Voxel structure for memory efficiency (64KB per 16x16x64 chunk)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Voxel {
    pub id: BlockId,
    pub meta: u8,
    pub light: u8,
    pub flags: u8,
}

impl Voxel {
    pub const AIR: Voxel = Voxel {
        id: BlockId::AIR,
        meta: 0,
        light: 0,
        flags: 0,
    };

    #[inline]
    pub const fn new(id: BlockId) -> Self {
        Voxel {
            id,
            meta: 0,
            light: 0,
            flags: 0,
        }
    }

    #[inline]
    pub const fn with_meta(mut self, meta: u8) -> Self {
        self.meta = meta;
        self
    }

    #[inline]
    pub fn sky_light(&self) -> u8 {
        (self.light >> 4) & 0x0F
    }

    #[inline]
    pub fn block_light(&self) -> u8 {
        self.light & 0x0F
    }

    #[inline]
    pub fn set_sky_light(&mut self, val: u8) {
        self.light = (self.light & 0x0F) | ((val & 0x0F) << 4);
    }

    #[inline]
    pub fn set_block_light(&mut self, val: u8) {
        self.light = (self.light & 0xF0) | (val & 0x0F);
    }

    #[inline]
    pub fn max_light(&self) -> u8 {
        let sky = self.sky_light();
        let blk = self.block_light();
        if sky > blk { sky } else { blk }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockDef {
    pub id: BlockId,
    pub name: String,
    pub top_color: [u8; 3],
    pub left_color: [u8; 3],
    pub right_color: [u8; 3],
    pub light_emission: u8,
    pub is_solid: bool,
    pub is_transparent: bool,
    pub is_liquid: bool,
    pub has_gravity: bool,
    pub is_ramp: bool,
    pub is_stair: bool,
    pub hardness: f32,
}

impl Default for BlockDef {
    fn default() -> Self {
        Self {
            id: BlockId::AIR,
            name: "air".to_string(),
            top_color: [0, 0, 0],
            left_color: [0, 0, 0],
            right_color: [0, 0, 0],
            light_emission: 0,
            is_solid: false,
            is_transparent: true,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BlockRegistry {
    defs: Vec<Option<BlockDef>>,
    name_to_id: std::collections::HashMap<String, BlockId>,
    next_custom_id: u8,
}

impl BlockRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            defs: vec![None; 256],
            name_to_id: std::collections::HashMap::new(),
            next_custom_id: BlockId::FIRST_CUSTOM_ID,
        };
        reg.register_builtins();
        reg
    }

    fn register_builtin(&mut self, def: BlockDef) {
        let id = def.id;
        self.name_to_id.insert(def.name.clone(), id);
        self.defs[id.0 as usize] = Some(def);
    }

    fn register_builtins(&mut self) {
        self.register_builtin(BlockDef {
            id: BlockId::AIR,
            name: "air".into(),
            top_color: [0, 0, 0],
            left_color: [0, 0, 0],
            right_color: [0, 0, 0],
            light_emission: 0,
            is_solid: false,
            is_transparent: true,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 0.0,
        });

        self.register_builtin(BlockDef {
            id: BlockId::GRASS,
            name: "grass".into(),
            top_color: [86, 176, 56],
            left_color: [110, 77, 47],
            right_color: [86, 60, 36],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 0.6,
        });

        self.register_builtin(BlockDef {
            id: BlockId::DIRT,
            name: "dirt".into(),
            top_color: [134, 96, 67],
            left_color: [110, 77, 47],
            right_color: [86, 60, 36],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 0.5,
        });

        self.register_builtin(BlockDef {
            id: BlockId::STONE,
            name: "stone".into(),
            top_color: [150, 150, 150],
            left_color: [120, 120, 120],
            right_color: [95, 95, 95],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 1.5,
        });

        self.register_builtin(BlockDef {
            id: BlockId::SAND,
            name: "sand".into(),
            top_color: [222, 206, 149],
            left_color: [195, 178, 123],
            right_color: [168, 152, 100],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: true,
            is_ramp: false,
            is_stair: false,
            hardness: 0.5,
        });

        self.register_builtin(BlockDef {
            id: BlockId::GRAVEL,
            name: "gravel".into(),
            top_color: [136, 126, 126],
            left_color: [110, 100, 100],
            right_color: [90, 80, 80],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: true,
            is_ramp: false,
            is_stair: false,
            hardness: 0.6,
        });

        self.register_builtin(BlockDef {
            id: BlockId::SNOW,
            name: "snow".into(),
            top_color: [240, 245, 250],
            left_color: [210, 220, 230],
            right_color: [180, 195, 210],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 0.2,
        });

        self.register_builtin(BlockDef {
            id: BlockId::CLAY,
            name: "clay".into(),
            top_color: [160, 166, 179],
            left_color: [140, 146, 159],
            right_color: [120, 126, 139],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 0.6,
        });

        self.register_builtin(BlockDef {
            id: BlockId::BEDROCK,
            name: "bedrock".into(),
            top_color: [50, 50, 50],
            left_color: [35, 35, 35],
            right_color: [20, 20, 20],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: -1.0,
        });

        self.register_builtin(BlockDef {
            id: BlockId::COAL_ORE,
            name: "coal_ore".into(),
            top_color: [130, 130, 130],
            left_color: [100, 100, 100],
            right_color: [80, 80, 80],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 3.0,
        });

        self.register_builtin(BlockDef {
            id: BlockId::IRON_ORE,
            name: "iron_ore".into(),
            top_color: [170, 150, 140],
            left_color: [140, 120, 110],
            right_color: [110, 95, 85],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 3.0,
        });

        self.register_builtin(BlockDef {
            id: BlockId::GOLD_ORE,
            name: "gold_ore".into(),
            top_color: [220, 200, 80],
            left_color: [180, 160, 60],
            right_color: [140, 120, 40],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 3.0,
        });

        self.register_builtin(BlockDef {
            id: BlockId::DIAMOND_ORE,
            name: "diamond_ore".into(),
            top_color: [90, 220, 230],
            left_color: [70, 180, 190],
            right_color: [50, 140, 150],
            light_emission: 2,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 3.0,
        });

        self.register_builtin(BlockDef {
            id: BlockId::WOOD_LOG,
            name: "wood_log".into(),
            top_color: [160, 125, 75],
            left_color: [105, 75, 45],
            right_color: [80, 55, 30],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 2.0,
        });

        self.register_builtin(BlockDef {
            id: BlockId::LEAVES,
            name: "leaves".into(),
            top_color: [50, 140, 40],
            left_color: [40, 115, 30],
            right_color: [30, 90, 20],
            light_emission: 0,
            is_solid: true,
            is_transparent: true,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 0.2,
        });

        self.register_builtin(BlockDef {
            id: BlockId::PLANKS,
            name: "planks".into(),
            top_color: [180, 140, 90],
            left_color: [150, 115, 70],
            right_color: [120, 90, 50],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 2.0,
        });

        self.register_builtin(BlockDef {
            id: BlockId::COBBLESTONE,
            name: "cobblestone".into(),
            top_color: [128, 128, 128],
            left_color: [100, 100, 100],
            right_color: [80, 80, 80],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 2.0,
        });

        self.register_builtin(BlockDef {
            id: BlockId::BRICK,
            name: "brick".into(),
            top_color: [180, 90, 75],
            left_color: [150, 70, 55],
            right_color: [120, 55, 40],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 2.0,
        });

        self.register_builtin(BlockDef {
            id: BlockId::GLASS,
            name: "glass".into(),
            top_color: [200, 230, 255],
            left_color: [170, 205, 235],
            right_color: [140, 180, 215],
            light_emission: 0,
            is_solid: true,
            is_transparent: true,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 0.3,
        });

        self.register_builtin(BlockDef {
            id: BlockId::RAMP,
            name: "ramp".into(),
            top_color: [140, 140, 140],
            left_color: [110, 110, 110],
            right_color: [85, 85, 85],
            light_emission: 0,
            is_solid: true,
            is_transparent: true,
            is_liquid: false,
            has_gravity: false,
            is_ramp: true,
            is_stair: false,
            hardness: 1.5,
        });

        self.register_builtin(BlockDef {
            id: BlockId::STAIR,
            name: "stair".into(),
            top_color: [160, 130, 85],
            left_color: [130, 105, 65],
            right_color: [100, 80, 45],
            light_emission: 0,
            is_solid: true,
            is_transparent: true,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: true,
            hardness: 1.5,
        });

        self.register_builtin(BlockDef {
            id: BlockId::LADDER,
            name: "ladder".into(),
            top_color: [150, 110, 60],
            left_color: [120, 85, 40],
            right_color: [90, 65, 30],
            light_emission: 0,
            is_solid: false,
            is_transparent: true,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 0.4,
        });

        self.register_builtin(BlockDef {
            id: BlockId::TORCH,
            name: "torch".into(),
            top_color: [255, 220, 100],
            left_color: [230, 160, 50],
            right_color: [180, 100, 30],
            light_emission: 14,
            is_solid: false,
            is_transparent: true,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 0.1,
        });

        self.register_builtin(BlockDef {
            id: BlockId::GLOWSTONE,
            name: "glowstone".into(),
            top_color: [255, 235, 140],
            left_color: [230, 200, 100],
            right_color: [200, 170, 70],
            light_emission: 15,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 0.3,
        });

        self.register_builtin(BlockDef {
            id: BlockId::WATER,
            name: "water".into(),
            top_color: [40, 110, 220],
            left_color: [30, 90, 190],
            right_color: [20, 70, 160],
            light_emission: 0,
            is_solid: false,
            is_transparent: true,
            is_liquid: true,
            has_gravity: true,
            is_ramp: false,
            is_stair: false,
            hardness: 100.0,
        });

        self.register_builtin(BlockDef {
            id: BlockId::LAVA,
            name: "lava".into(),
            top_color: [255, 100, 10],
            left_color: [220, 70, 5],
            right_color: [180, 40, 0],
            light_emission: 15,
            is_solid: false,
            is_transparent: false,
            is_liquid: true,
            has_gravity: true,
            is_ramp: false,
            is_stair: false,
            hardness: 100.0,
        });

        self.register_builtin(BlockDef {
            id: BlockId::OBSIDIAN,
            name: "obsidian".into(),
            top_color: [30, 20, 45],
            left_color: [20, 12, 32],
            right_color: [12, 8, 20],
            light_emission: 0,
            is_solid: true,
            is_transparent: false,
            is_liquid: false,
            has_gravity: false,
            is_ramp: false,
            is_stair: false,
            hardness: 50.0,
        });
    }

    pub fn get(&self, id: BlockId) -> Option<&BlockDef> {
        self.defs.get(id.0 as usize).and_then(|opt| opt.as_ref())
    }

    pub fn get_by_name(&self, name: &str) -> Option<&BlockDef> {
        self.name_to_id.get(name).and_then(|id| self.get(*id))
    }

    pub fn id_by_name(&self, name: &str) -> Option<BlockId> {
        self.name_to_id.get(name).copied()
    }

    pub fn is_solid(&self, id: BlockId) -> bool {
        self.get(id).map_or(false, |d| d.is_solid)
    }

    pub fn is_transparent(&self, id: BlockId) -> bool {
        self.get(id).map_or(true, |d| d.is_transparent)
    }

    pub fn is_liquid(&self, id: BlockId) -> bool {
        self.get(id).map_or(false, |d| d.is_liquid)
    }

    pub fn light_emission(&self, id: BlockId) -> u8 {
        self.get(id).map_or(0, |d| d.light_emission)
    }

    pub fn register_custom(
        &mut self,
        name: String,
        top_color: [u8; 3],
        left_color: [u8; 3],
        right_color: [u8; 3],
        light_emission: u8,
        is_solid: bool,
        is_transparent: bool,
        is_liquid: bool,
        has_gravity: bool,
    ) -> Result<BlockId, String> {
        if let Some(&existing_id) = self.name_to_id.get(&name) {
            let def = BlockDef {
                id: existing_id,
                name: name.clone(),
                top_color,
                left_color,
                right_color,
                light_emission,
                is_solid,
                is_transparent,
                is_liquid,
                has_gravity,
                is_ramp: false,
                is_stair: false,
                hardness: 1.0,
            };
            self.defs[existing_id.0 as usize] = Some(def);
            return Ok(existing_id);
        }

        if self.next_custom_id > BlockId::MAX_ID {
            return Err("Custom block ID limit (254) reached".into());
        }

        let id = BlockId(self.next_custom_id);
        self.next_custom_id += 1;

        let def = BlockDef {
            id,
            name: name.clone(),
            top_color,
            left_color,
            right_color,
            light_emission,
            is_solid,
            is_transparent,
            is_liquid,
            has_gravity,
            is_ramp: false,
            is_stair: false,
            hardness: 1.0,
        };

        self.name_to_id.insert(name, id);
        self.defs[id.0 as usize] = Some(def);
        Ok(id)
    }

    pub fn all_registered(&self) -> Vec<&BlockDef> {
        self.defs.iter().filter_map(|d| d.as_ref()).collect()
    }
}
