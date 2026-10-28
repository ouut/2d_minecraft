use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use steel::steel_vm::engine::Engine;
use steel::steel_vm::register_fn::RegisterFn;
use voxel_core::block::{BlockDef, BlockId, BlockRegistry};
use voxel_core::coords::WorldPos;
use voxel_core::traits::{EngineCommand, JobType};

#[derive(Clone, Default)]
pub struct ScriptBridge {
    pub commands: Arc<Mutex<Vec<EngineCommand>>>,
    pub logs: Arc<Mutex<Vec<String>>>,
    pub custom_blocks: Arc<Mutex<Vec<BlockDef>>>,
    pub custom_tools: Arc<Mutex<Vec<(String, String)>>>,
    pub hooks: Arc<Mutex<HashMap<String, String>>>,
    pub registry_names: Arc<Mutex<HashMap<String, u8>>>,
}

pub struct ScriptEngine {
    vm: Engine,
    bridge: ScriptBridge,
}

impl Default for ScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptEngine {
    pub fn new() -> Self {
        let bridge = ScriptBridge::default();
        let mut vm = Engine::new();
        Self::register_api(&mut vm, &bridge);

        Self { vm, bridge }
    }

    fn register_api(vm: &mut Engine, bridge: &ScriptBridge) {
        // 1. mod-log
        let logs_c = bridge.logs.clone();
        let cmds_c = bridge.commands.clone();
        vm.register_fn("mod-log", move |msg: String| {
            if let Ok(mut l) = logs_c.lock() {
                l.push(msg.clone());
            }
            if let Ok(mut c) = cmds_c.lock() {
                c.push(EngineCommand::LogMessage { text: msg });
            }
        });

        // 2. register-block-raw!
        let blocks_c = bridge.custom_blocks.clone();
        vm.register_fn(
            "%register-block-internal!",
            move |name: String,
                  tr: isize,
                  tg: isize,
                  tb: isize,
                  lr: isize,
                  lg: isize,
                  lb: isize,
                  rr: isize,
                  rg: isize,
                  rb: isize,
                  light: isize,
                  solid: bool,
                  transparent: bool,
                  liquid: bool,
                  gravity: bool| {
                let def = BlockDef {
                    id: BlockId::AIR, // will be assigned on sync with BlockRegistry
                    name,
                    top_color: [tr as u8, tg as u8, tb as u8],
                    left_color: [lr as u8, lg as u8, lb as u8],
                    right_color: [rr as u8, rg as u8, rb as u8],
                    light_emission: light as u8,
                    is_solid: solid,
                    is_transparent: transparent,
                    is_liquid: liquid,
                    has_gravity: gravity,
                    is_ramp: false,
                    is_stair: false,
                    hardness: 1.0,
                };
                if let Ok(mut b) = blocks_c.lock() {
                    b.retain(|x| x.name != def.name);
                    b.push(def);
                }
            },
        );

        // 3. set-block!
        let cmds_c = bridge.commands.clone();
        let reg_c = bridge.registry_names.clone();
        vm.register_fn(
            "set-block!",
            move |x: isize, y: isize, z: isize, block_name: String| {
                let id = reg_c
                    .lock()
                    .ok()
                    .and_then(|r| r.get(&block_name).copied())
                    .map(BlockId)
                    .unwrap_or(BlockId::DIRT);

                if let Ok(mut c) = cmds_c.lock() {
                    c.push(EngineCommand::SetBlock {
                        pos: WorldPos::new(x as i32, y as i32, z as i32),
                        id,
                        meta: 0,
                        by_player_or_colonist: false,
                    });
                }
            },
        );

        // 4. fill-box!
        let cmds_c = bridge.commands.clone();
        let reg_c = bridge.registry_names.clone();
        vm.register_fn(
            "fill-box!",
            move |x1: isize,
                  y1: isize,
                  z1: isize,
                  x2: isize,
                  y2: isize,
                  z2: isize,
                  block_name: String| {
                let id = reg_c
                    .lock()
                    .ok()
                    .and_then(|r| r.get(&block_name).copied())
                    .map(BlockId)
                    .unwrap_or(BlockId::DIRT);

                if let Ok(mut c) = cmds_c.lock() {
                    c.push(EngineCommand::FillBox {
                        min: WorldPos::new(x1 as i32, y1 as i32, z1 as i32),
                        max: WorldPos::new(x2 as i32, y2 as i32, z2 as i32),
                        id,
                        hollow: false,
                    });
                }
            },
        );

        // 5. hollow-box!
        let cmds_c = bridge.commands.clone();
        let reg_c = bridge.registry_names.clone();
        vm.register_fn(
            "hollow-box!",
            move |x1: isize,
                  y1: isize,
                  z1: isize,
                  x2: isize,
                  y2: isize,
                  z2: isize,
                  block_name: String| {
                let id = reg_c
                    .lock()
                    .ok()
                    .and_then(|r| r.get(&block_name).copied())
                    .map(BlockId)
                    .unwrap_or(BlockId::DIRT);

                if let Ok(mut c) = cmds_c.lock() {
                    c.push(EngineCommand::FillBox {
                        min: WorldPos::new(x1 as i32, y1 as i32, z1 as i32),
                        max: WorldPos::new(x2 as i32, y2 as i32, z2 as i32),
                        id,
                        hollow: true,
                    });
                }
            },
        );

        // 6. spawn-fluid!
        let cmds_c = bridge.commands.clone();
        vm.register_fn(
            "spawn-fluid!",
            move |x: isize, y: isize, z: isize, fluid_name: String, level: isize| {
                let id = if fluid_name == "lava" {
                    BlockId::LAVA
                } else {
                    BlockId::WATER
                };
                if let Ok(mut c) = cmds_c.lock() {
                    c.push(EngineCommand::SpawnFluid {
                        pos: WorldPos::new(x as i32, y as i32, z as i32),
                        id,
                        level: level as u8,
                    });
                }
            },
        );

        // 7. spawn-colonist!
        let cmds_c = bridge.commands.clone();
        vm.register_fn(
            "spawn-colonist!",
            move |x: isize, y: isize, z: isize, name: String| {
                if let Ok(mut c) = cmds_c.lock() {
                    c.push(EngineCommand::SpawnColonist {
                        pos: WorldPos::new(x as i32, y as i32, z as i32),
                        name,
                    });
                }
            },
        );

        // 8. teleport-player!
        let cmds_c = bridge.commands.clone();
        vm.register_fn(
            "teleport-player!",
            move |x: isize, y: isize, z: isize| {
                if let Ok(mut c) = cmds_c.lock() {
                    c.push(EngineCommand::TeleportPlayer {
                        pos: WorldPos::new(x as i32, y as i32, z as i32),
                    });
                }
            },
        );

        // 9. designate-box!
        let cmds_c = bridge.commands.clone();
        let reg_c = bridge.registry_names.clone();
        vm.register_fn(
            "designate-box!",
            move |x1: isize,
                  y1: isize,
                  z1: isize,
                  x2: isize,
                  y2: isize,
                  z2: isize,
                  job_str: String,
                  block_name: String| {
                let job_type = match job_str.as_str() {
                    "mine" => JobType::Mine,
                    "channel" => JobType::Channel,
                    "build" => JobType::Build,
                    "chop" => JobType::Chop,
                    _ => JobType::Mine,
                };
                let id = reg_c
                    .lock()
                    .ok()
                    .and_then(|r| r.get(&block_name).copied())
                    .map(BlockId)
                    .unwrap_or(BlockId::AIR);

                if let Ok(mut c) = cmds_c.lock() {
                    c.push(EngineCommand::DesignateBox {
                        min: WorldPos::new(x1 as i32, y1 as i32, z1 as i32),
                        max: WorldPos::new(x2 as i32, y2 as i32, z2 as i32),
                        job_type,
                        build_block: id,
                    });
                }
            },
        );

        // 10. register-tool!
        let tools_c = bridge.custom_tools.clone();
        vm.register_fn("register-tool!", move |tool_name: String, fn_name: String| {
            if let Ok(mut t) = tools_c.lock() {
                t.retain(|(k, _)| k != &tool_name);
                t.push((tool_name, fn_name));
            }
        });

        // 11. register-hook!
        let hooks_c = bridge.hooks.clone();
        vm.register_fn("register-hook!", move |event_name: String, fn_name: String| {
            if let Ok(mut h) = hooks_c.lock() {
                h.insert(event_name, fn_name);
            }
        });

        // Run Prelude definition for flexible register-block!
        let prelude = r#"
            (define (register-block! name tr tg tb . rest)
              (if (= (length rest) 11)
                  (%register-block-internal! name tr tg tb
                    (car rest) (cadr rest) (caddr rest)
                    (cadddr rest) (list-ref rest 4) (list-ref rest 5)
                    (list-ref rest 6) (list-ref rest 7) (list-ref rest 8)
                    (list-ref rest 9) (list-ref rest 10))
                  ;; Fallback 9-arg variant: compute left/right from top RGB
                  (%register-block-internal! name tr tg tb
                    (max 0 (- tr 30)) (max 0 (- tg 30)) (max 0 (- tb 30))
                    (max 0 (- tr 60)) (max 0 (- tg 60)) (max 0 (- tb 60))
                    (car rest) (cadr rest) (caddr rest)
                    (cadddr rest) (list-ref rest 4))))
        "#;
        let _ = vm.run(prelude);
    }

    pub fn sync_registry(&self, registry: &BlockRegistry) {
        if let Ok(mut names) = self.bridge.registry_names.lock() {
            names.clear();
            for def in registry.all_registered() {
                names.insert(def.name.clone(), def.id.0);
            }
        }
    }

    pub fn apply_custom_blocks_to_registry(&self, registry: &mut BlockRegistry) -> bool {
        let mut changed = false;
        if let Ok(blocks) = self.bridge.custom_blocks.lock() {
            for b in blocks.iter() {
                let res = registry.register_custom(
                    b.name.clone(),
                    b.top_color,
                    b.left_color,
                    b.right_color,
                    b.light_emission,
                    b.is_solid,
                    b.is_transparent,
                    b.is_liquid,
                    b.has_gravity,
                );
                if res.is_ok() {
                    changed = true;
                }
            }
        }
        self.sync_registry(registry);
        changed
    }

    pub fn drain_commands(&self) -> Vec<EngineCommand> {
        if let Ok(mut c) = self.bridge.commands.lock() {
            std::mem::take(&mut *c)
        } else {
            Vec::new()
        }
    }

    pub fn drain_logs(&self) -> Vec<String> {
        if let Ok(mut l) = self.bridge.logs.lock() {
            std::mem::take(&mut *l)
        } else {
            Vec::new()
        }
    }

    pub fn get_custom_tool_names(&self) -> Vec<String> {
        if let Ok(t) = self.bridge.custom_tools.lock() {
            t.iter().map(|(name, _)| name.clone()).collect()
        } else {
            Vec::new()
        }
    }

    pub fn eval_repl(&mut self, expr: &str) -> Result<String, String> {
        match self.vm.run(expr.to_string()) {
            Ok(vals) => {
                if vals.is_empty() {
                    Ok("=> ()".into())
                } else {
                    let out = vals
                        .iter()
                        .map(|v| format!("{:?}", v))
                        .collect::<Vec<_>>()
                        .join(" ");
                    Ok(format!("=> {}", out))
                }
            }
            Err(e) => Err(format!("Error: {:?}", e)),
        }
    }

    pub fn load_mods_from_dir(
        &mut self,
        mods_dir: &Path,
        registry: &mut BlockRegistry,
    ) -> Result<Vec<String>, String> {
        self.sync_registry(registry);
        let mut loaded = Vec::new();

        if !mods_dir.exists() {
            return Ok(loaded);
        }

        let mut paths: Vec<_> = std::fs::read_dir(mods_dir)
            .map_err(|e| format!("Failed to read mods dir: {}", e))?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.extension().map_or(false, |ext| ext == "scm"))
            .collect();

        paths.sort();

        for path in paths {
            let filename = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let code = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", filename, e))?;

            self.vm
                .run(code)
                .map_err(|e| format!("Error in mod {}: {:?}", filename, e))?;

            loaded.push(filename);
        }

        self.apply_custom_blocks_to_registry(registry);
        Ok(loaded)
    }

    pub fn trigger_tool(&mut self, tool_name: &str, pos: WorldPos) -> Result<Vec<EngineCommand>, String> {
        let fn_name = {
            let tools = self.bridge.custom_tools.lock().map_err(|_| "Lock error")?;
            tools
                .iter()
                .find(|(name, _)| name == tool_name)
                .map(|(_, f)| f.clone())
        };

        if let Some(func) = fn_name {
            let code = format!("({} {} {} {})", func, pos.x, pos.y, pos.z);
            let _ = self.vm.run(code).map_err(|e| format!("Tool execution error: {:?}", e))?;
            Ok(self.drain_commands())
        } else {
            Err(format!("Tool '{}' not found", tool_name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_engine_basic_primitives() {
        let mut engine = ScriptEngine::new();
        let reg = BlockRegistry::new();
        engine.sync_registry(&reg);

        let code = r#"
            (mod-log "Hello Scheme Mod!")
            (set-block! 10 20 5 "stone")
            (spawn-colonist! 10 20 6 "Urist")
        "#;
        let res = engine.eval_repl(code);
        assert!(res.is_ok());

        let cmds = engine.drain_commands();
        assert_eq!(cmds.len(), 3);
        assert!(matches!(cmds[0], EngineCommand::LogMessage { .. }));
        assert!(matches!(cmds[1], EngineCommand::SetBlock { .. }));
        assert!(matches!(cmds[2], EngineCommand::SpawnColonist { .. }));
    }

    #[test]
    fn test_load_official_sample_mods() {
        let mut engine = ScriptEngine::new();
        let mut reg = BlockRegistry::new();

        let mods_dir = Path::new("/workspace/2d_minecraft/mods");
        let loaded = engine.load_mods_from_dir(mods_dir, &mut reg).expect("load mods");
        assert!(loaded.len() >= 3);

        // Check custom blocks registered
        assert!(reg.get_by_name("mythril_ore").is_some());
        assert!(reg.get_by_name("aether_crystal").is_some());
        assert!(reg.get_by_name("quicksand").is_some());

        // Check tools registered
        let tools = engine.get_custom_tool_names();
        assert!(tools.contains(&"Build Watchtower".to_string()));
        assert!(tools.contains(&"Meteor Strike".to_string()));

        // Trigger custom tool
        let tool_cmds = engine.trigger_tool("Build Watchtower", WorldPos::new(20, 20, 5)).expect("trigger tool");
        assert!(!tool_cmds.is_empty(), "Watchtower tool should produce commands");
    }
}
