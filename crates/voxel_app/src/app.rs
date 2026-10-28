use std::path::Path;
use macroquad::input::{
    get_char_pressed, is_key_down, is_key_pressed, is_mouse_button_down,
    is_mouse_button_pressed, is_mouse_button_released, mouse_position, mouse_wheel, KeyCode,
    MouseButton,
};
use macroquad::time::get_fps;
use macroquad::window::{clear_background, screen_height, screen_width};
use voxel_core::block::{BlockId, BlockRegistry};
use voxel_core::coords::{ChunkPos, IsoCamera, WorldPos};
use voxel_core::traits::{EngineCommand, GameMode, JobType, UiStateView, WorldAccess};
use voxel_ecs::EcsManager;
use voxel_render::{HudRenderer, IsoAtlas, WorldRenderer};
use voxel_script::ScriptEngine;
use voxel_sim::SimulationEngine;

use crate::world::ChunkWorld;

pub struct EngineApp {
    pub world: ChunkWorld,
    pub registry: BlockRegistry,
    pub sim: SimulationEngine,
    pub ecs: EcsManager,
    pub script: ScriptEngine,
    pub atlas: IsoAtlas,
    pub camera: IsoCamera,
    pub ui_state: UiStateView,
    pub is_repl_open: bool,
    pub box_drag_start: Option<WorldPos>,
    pub physics_timer: f32,
}

impl EngineApp {
    pub fn new() -> Self {
        let mut registry = BlockRegistry::new();
        let mut script = ScriptEngine::new();

        // Load official mods from /workspace/2d_minecraft/mods
        let mods_dir = Path::new("/workspace/2d_minecraft/mods");
        let _ = script.load_mods_from_dir(mods_dir, &mut registry);

        let mut atlas = IsoAtlas::new();
        atlas.build_all(&registry);

        let mut world = ChunkWorld::new(1337);
        let center_chunk = ChunkPos::new(0, 0);
        world.ensure_chunks_around(center_chunk, 2);

        let mut ecs = EcsManager::new();
        let spawn_surface = world.surface_z(0, 0);
        let player_spawn = WorldPos::new(0, 0, spawn_surface + 1);
        ecs.spawn_player(player_spawn);

        // Spawn 3 starter dwarf colonists
        ecs.spawn_colonist(WorldPos::new(2, 1, world.surface_z(2, 1) + 1), "Urist (Miner)".into());
        ecs.spawn_colonist(WorldPos::new(-2, 1, world.surface_z(-2, 1) + 1), "Darin (Builder)".into());
        ecs.spawn_colonist(WorldPos::new(0, 3, world.surface_z(0, 3) + 1), "Brokk (Engineer)".into());

        let sim = SimulationEngine::new();
        // Initial lighting pass
        sim.recompute_lighting(
            &mut world,
            &registry,
            WorldPos::new(-20, -20, 0),
            WorldPos::new(20, 20, 60),
        );

        let mut camera = IsoCamera::new();
        camera.focus_x = player_spawn.x as f32;
        camera.focus_y = player_spawn.y as f32;
        camera.slice_z = player_spawn.z + 1;

        let mut ui_state = UiStateView::default();
        ui_state.mod_tools = script.get_custom_tool_names();
        ui_state.selected_tool = ui_state
            .mod_tools
            .first()
            .cloned()
            .unwrap_or_else(|| "Build Watchtower".into());
        ui_state.status_msg = "Engine Initialized. Mods Loaded. Press F1/F2/F3 to switch modes.".into();

        Self {
            world,
            registry,
            sim,
            ecs,
            script,
            atlas,
            camera,
            ui_state,
            is_repl_open: false,
            box_drag_start: None,
            physics_timer: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.handle_input(dt);

        // Update ECS
        let ecs_cmds = self.ecs.update(dt, &self.world, &self.registry);
        self.execute_commands(ecs_cmds);

        // Periodic fluid and physics step (every 0.25s)
        self.physics_timer += dt;
        if self.physics_timer >= 0.25 {
            self.physics_timer = 0.0;
            let focus = WorldPos::new(
                self.camera.focus_x as i32,
                self.camera.focus_y as i32,
                self.camera.slice_z,
            );
            self.sim.step_physics(&mut self.world, &self.registry, focus, 24);
        }

        // Camera follow in Player mode
        if self.ui_state.game_mode == GameMode::Player {
            if let Some(ppos) = self.ecs.player_pos() {
                self.camera.focus_x = ppos.x as f32;
                self.camera.focus_y = ppos.y as f32;
                self.camera.slice_z = ppos.z + 1;
            }
        }

        // Drain logs from script
        let script_logs = self.script.drain_logs();
        for log in script_logs {
            self.ui_state.repl_logs.push(log);
        }

        // Ensure dynamic chunks generated around camera
        let (cx, cy) = (
            (self.camera.focus_x / 16.0).floor() as i32,
            (self.camera.focus_y / 16.0).floor() as i32,
        );
        self.world.ensure_chunks_around(ChunkPos::new(cx, cy), 1);
    }

    fn handle_input(&mut self, dt: f32) {
        // Mode Switch Keys
        if is_key_pressed(KeyCode::F1) {
            self.ui_state.game_mode = GameMode::Player;
            self.ui_state.status_msg = "Switched to Player Mode (WASD to move, Left/Right click)".into();
        } else if is_key_pressed(KeyCode::F2) {
            self.ui_state.game_mode = GameMode::Colony;
            self.ui_state.status_msg = "Switched to Colony Mode (Drag box to designate jobs)".into();
        } else if is_key_pressed(KeyCode::F3) {
            self.ui_state.game_mode = GameMode::GodEditor;
            self.ui_state.status_msg = "Switched to God Editor Mode (Use palette / Mod Tools)".into();
        }

        // Camera 90-degree rotation (R key)
        if is_key_pressed(KeyCode::R) {
            self.camera.rotate_clockwise();
        }

        // Z-Slice adjustments (Q/E or mouse wheel)
        if is_key_pressed(KeyCode::Q) {
            self.camera.slice_z = (self.camera.slice_z - 1).max(1);
        } else if is_key_pressed(KeyCode::E) {
            self.camera.slice_z = (self.camera.slice_z + 1).min(63);
        }
        let wheel = mouse_wheel().1;
        if wheel > 0.5 {
            self.camera.slice_z = (self.camera.slice_z + 1).min(63);
        } else if wheel < -0.5 {
            self.camera.slice_z = (self.camera.slice_z - 1).max(1);
        }

        // F5 Hot Reload Scheme Mods
        if is_key_pressed(KeyCode::F5) {
            let mods_dir = Path::new("/workspace/2d_minecraft/mods");
            match self.script.load_mods_from_dir(mods_dir, &mut self.registry) {
                Ok(loaded) => {
                    self.atlas.rebuild(&self.registry);
                    self.ui_state.mod_tools = self.script.get_custom_tool_names();
                    self.ui_state.status_msg = format!("Mods reloaded successfully: {:?}", loaded);
                    self.ui_state.repl_logs.push(format!("[Hot-Reload] {:?}", loaded));
                }
                Err(e) => {
                    self.ui_state.status_msg = format!("Mod reload error: {}", e);
                    self.ui_state.repl_logs.push(format!("[Hot-Reload Error] {}", e));
                }
            }
        }

        // Scheme REPL Console toggle (`~` or F12)
        if is_key_pressed(KeyCode::GraveAccent) || is_key_pressed(KeyCode::F12) {
            self.is_repl_open = !self.is_repl_open;
        }

        if self.is_repl_open {
            self.handle_repl_typing();
            return;
        }

        // Game interaction when REPL closed
        let (mx, my) = mouse_position();
        let hovered_pos = self.camera.screen_to_slice_plane(mx, my, screen_width(), screen_height());

        // Camera panning in Colony / God Editor
        if self.ui_state.game_mode != GameMode::Player {
            let pan_speed = 35.0 * dt;
            if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
                self.camera.focus_x -= pan_speed;
            }
            if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
                self.camera.focus_x += pan_speed;
            }
            if is_key_down(KeyCode::Up) || is_key_down(KeyCode::W) {
                self.camera.focus_y -= pan_speed;
            }
            if is_key_down(KeyCode::Down) || is_key_down(KeyCode::S) {
                self.camera.focus_y += pan_speed;
            }
        }

        match self.ui_state.game_mode {
            GameMode::Player => {
                let mut dx = 0;
                let mut dy = 0;
                if is_key_pressed(KeyCode::A) || is_key_pressed(KeyCode::Left) {
                    dx -= 1;
                }
                if is_key_pressed(KeyCode::D) || is_key_pressed(KeyCode::Right) {
                    dx += 1;
                }
                if is_key_pressed(KeyCode::W) || is_key_pressed(KeyCode::Up) {
                    dy -= 1;
                }
                if is_key_pressed(KeyCode::S) || is_key_pressed(KeyCode::Down) {
                    dy += 1;
                }

                if dx != 0 || dy != 0 {
                    self.ecs.handle_player_input(
                        self.camera.rotation,
                        dx,
                        dy,
                        &self.world,
                        &self.registry,
                    );
                }

                // Left click to mine
                if is_mouse_button_pressed(MouseButton::Left) && my > 40.0 && my < screen_height() - 60.0 {
                    if let Some(cmd) = self.ecs.handle_player_action(
                        hovered_pos,
                        true,
                        &self.world,
                        &self.registry,
                    ) {
                        self.execute_commands(vec![cmd]);
                    }
                }

                // Right click to place
                if is_mouse_button_pressed(MouseButton::Right) && my > 40.0 && my < screen_height() - 60.0 {
                    if let Some(cmd) = self.ecs.handle_player_action(
                        hovered_pos,
                        false,
                        &self.world,
                        &self.registry,
                    ) {
                        self.execute_commands(vec![cmd]);
                    }
                }
            }
            GameMode::Colony => {
                // Drag box designation
                if is_mouse_button_pressed(MouseButton::Left) && my > 40.0 && my < screen_height() - 60.0 {
                    self.box_drag_start = Some(hovered_pos);
                }

                if is_mouse_button_released(MouseButton::Left) {
                    if let Some(start_pos) = self.box_drag_start.take() {
                        let job_type = match self.ui_state.selected_tool.as_str() {
                            "channel" => JobType::Channel,
                            "build" => JobType::Build,
                            "chop" => JobType::Chop,
                            _ => JobType::Mine,
                        };
                        self.ecs.job_manager.designate_box(
                            start_pos,
                            hovered_pos,
                            job_type,
                            self.ui_state.selected_block,
                        );
                        self.ui_state.status_msg = format!("Designated {:?} box from {:?} to {:?}", job_type, start_pos, hovered_pos);
                    }
                }

                if is_mouse_button_pressed(MouseButton::Right) {
                    self.ecs.job_manager.remove_job_at(hovered_pos);
                    self.box_drag_start = None;
                }
            }
            GameMode::GodEditor => {
                // Execute Mod Tool or Paint Palette Block
                if is_mouse_button_pressed(MouseButton::Left) && my > 40.0 && my < screen_height() - 60.0 {
                    if self.ui_state.mod_tools.contains(&self.ui_state.selected_tool) {
                        // Trigger Scheme Mod Tool
                        let tool_name = self.ui_state.selected_tool.clone();
                        match self.script.trigger_tool(&tool_name, hovered_pos) {
                            Ok(cmds) => {
                                self.execute_commands(cmds);
                                self.ui_state.status_msg = format!("Executed Mod Tool '{}'", tool_name);
                            }
                            Err(e) => {
                                self.ui_state.status_msg = format!("Mod Tool Error: {}", e);
                            }
                        }
                    } else {
                        // Paint selected block
                        self.execute_commands(vec![EngineCommand::SetBlock {
                            pos: hovered_pos,
                            id: self.ui_state.selected_block,
                            meta: 0,
                            by_player_or_colonist: false,
                        }]);
                    }
                }

                if is_mouse_button_down(MouseButton::Right) && my > 40.0 && my < screen_height() - 60.0 {
                    self.execute_commands(vec![EngineCommand::SetBlock {
                        pos: hovered_pos,
                        id: BlockId::AIR,
                        meta: 0,
                        by_player_or_colonist: false,
                    }]);
                }
            }
        }
    }

    fn handle_repl_typing(&mut self) {
        while let Some(c) = get_char_pressed() {
            if c != '`' && c != '~' && !c.is_control() {
                self.ui_state.repl_input.push(c);
            }
        }

        if is_key_pressed(KeyCode::Backspace) {
            self.ui_state.repl_input.pop();
        }

        if is_key_pressed(KeyCode::Enter) {
            let expr = self.ui_state.repl_input.trim().to_string();
            if !expr.is_empty() {
                self.ui_state.repl_logs.push(format!("> {}", expr));
                match self.script.eval_repl(&expr) {
                    Ok(val) => {
                        self.ui_state.repl_logs.push(val);
                        let cmds = self.script.drain_commands();
                        self.execute_commands(cmds);
                        // Check if new custom blocks were added
                        if self.script.apply_custom_blocks_to_registry(&mut self.registry) {
                            self.atlas.rebuild(&self.registry);
                        }
                    }
                    Err(err) => {
                        self.ui_state.repl_logs.push(err);
                    }
                }
                self.ui_state.repl_input.clear();
            }
        }

        if is_key_pressed(KeyCode::Escape) {
            self.is_repl_open = false;
        }
    }

    pub fn execute_commands(&mut self, commands: Vec<EngineCommand>) {
        for cmd in commands {
            match cmd {
                EngineCommand::SetBlock { pos, id, meta, by_player_or_colonist } => {
                    let old_id = self.world.get_block_id(pos);
                    self.world.set_block(pos, id, meta);

                    // Recompute local lighting
                    self.sim.recompute_lighting(
                        &mut self.world,
                        &self.registry,
                        pos.offset(-8, -8, -8),
                        pos.offset(8, 8, 8),
                    );

                    // If a block was mined, check structural collapse
                    if by_player_or_colonist && id.is_air() && !old_id.is_air() {
                        self.sim.check_collapse(&mut self.world, &self.registry, pos);
                    }
                }
                EngineCommand::FillBox { min, max, id, hollow } => {
                    let x_min = min.x.min(max.x);
                    let x_max = min.x.max(max.x);
                    let y_min = min.y.min(max.y);
                    let y_max = min.y.max(max.y);
                    let z_min = min.z.min(max.z);
                    let z_max = min.z.max(max.z);

                    for x in x_min..=x_max {
                        for y in y_min..=y_max {
                            for z in z_min..=z_max {
                                let is_edge = x == x_min || x == x_max || y == y_min || y == y_max || z == z_min || z == z_max;
                                if !hollow || is_edge {
                                    self.world.set_block(WorldPos::new(x, y, z), id, 0);
                                }
                            }
                        }
                    }
                    self.sim.recompute_lighting(
                        &mut self.world,
                        &self.registry,
                        min.offset(-4, -4, -4),
                        max.offset(4, 4, 4),
                    );
                }
                EngineCommand::SpawnFluid { pos, id, level } => {
                    self.world.set_block(pos, id, level);
                }
                EngineCommand::SpawnColonist { pos, name } => {
                    self.ecs.spawn_colonist(pos, name);
                }
                EngineCommand::TeleportPlayer { pos } => {
                    self.ecs.teleport_player(pos);
                }
                EngineCommand::SetSliceZ { slice_z } => {
                    self.camera.slice_z = slice_z;
                }
                EngineCommand::DesignateBox { min, max, job_type, build_block } => {
                    self.ecs.job_manager.designate_box(min, max, job_type, build_block);
                }
                EngineCommand::InvokeModTool { tool_name, pos } => {
                    let _ = self.script.trigger_tool(&tool_name, pos);
                }
                EngineCommand::LogMessage { text } => {
                    self.ui_state.repl_logs.push(text);
                }
            }
        }
    }

    pub fn render(&mut self) {
        clear_background(macroquad::color::Color::new(0.06, 0.07, 0.1, 1.0));

        if !self.atlas.is_gpu_ready {
            self.atlas.upload_to_gpu();
        }

        let actors = self.ecs.get_actor_render_views();
        let jobs = self.ecs.get_job_render_views();

        let (mx, my) = mouse_position();
        let hovered_pos = self.camera.screen_to_slice_plane(mx, my, screen_width(), screen_height());
        let box_sel = self.box_drag_start.map(|s| (s, hovered_pos));

        WorldRenderer::render_world(
            &self.world,
            &self.registry,
            &self.atlas,
            &self.camera,
            &actors,
            &jobs,
            Some(hovered_pos),
            box_sel,
        );

        HudRenderer::render_hud(
            &self.ui_state,
            &self.camera,
            &self.registry,
            &self.atlas,
            get_fps(),
            self.is_repl_open,
        );
    }
}

impl Default for EngineApp {
    fn default() -> Self {
        Self::new()
    }
}
