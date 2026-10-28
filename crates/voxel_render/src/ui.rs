use macroquad::color::{Color, DARKGRAY, GREEN, LIGHTGRAY, RED, WHITE, YELLOW};
use macroquad::shapes::{draw_rectangle, draw_rectangle_lines};
use macroquad::text::draw_text;
use macroquad::texture::{draw_texture_ex, DrawTextureParams};
use macroquad::window::{screen_height, screen_width};
use voxel_core::block::BlockRegistry;
use voxel_core::coords::IsoCamera;
use voxel_core::traits::UiStateView;
use crate::atlas::IsoAtlas;

pub struct HudRenderer;

impl HudRenderer {
    pub fn render_hud(
        ui_state: &UiStateView,
        camera: &IsoCamera,
        registry: &BlockRegistry,
        atlas: &IsoAtlas,
        fps: i32,
        is_repl_open: bool,
    ) {
        let sw = screen_width();
        let sh = screen_height();

        // 1. Top HUD Bar
        let bar_h = 36.0;
        draw_rectangle(0.0, 0.0, sw, bar_h, Color::new(0.08, 0.08, 0.12, 0.92));
        draw_rectangle_lines(0.0, 0.0, sw, bar_h, 1.0, Color::new(0.2, 0.3, 0.4, 0.8));

        // Title and FPS
        draw_text(
            &format!("2D Minecraft (3D Voxel) | FPS: {}", fps),
            12.0,
            23.0,
            16.0,
            WHITE,
        );

        // Mode buttons: F1 Player, F2 Colony, F3 God
        let mode_names = ["F1: Player", "F2: Colony", "F3: God Edit"];
        let mut btn_x = 280.0;
        for (i, name) in mode_names.iter().enumerate() {
            let is_active = (ui_state.game_mode as usize) == i;
            let bg = if is_active {
                Color::new(0.2, 0.5, 0.9, 0.9)
            } else {
                Color::new(0.18, 0.2, 0.25, 0.8)
            };
            draw_rectangle(btn_x, 5.0, 95.0, 26.0, bg);
            draw_rectangle_lines(btn_x, 5.0, 95.0, 26.0, 1.0, LIGHTGRAY);
            draw_text(name, btn_x + 8.0, 22.0, 14.0, WHITE);
            btn_x += 105.0;
        }

        // Z-Slice Indicator
        draw_rectangle(btn_x, 5.0, 110.0, 26.0, Color::new(0.15, 0.2, 0.25, 0.8));
        draw_rectangle_lines(btn_x, 5.0, 110.0, 26.0, 1.0, LIGHTGRAY);
        draw_text(
            &format!("Slice Z: {} (Q/E)", camera.slice_z),
            btn_x + 8.0,
            22.0,
            14.0,
            YELLOW,
        );
        btn_x += 120.0;

        // Camera Rotation Indicator
        let rot_str = match camera.rotation {
            voxel_core::coords::CardinalDir::North => "Rot: 0° (N)",
            voxel_core::coords::CardinalDir::East => "Rot: 90° (E)",
            voxel_core::coords::CardinalDir::South => "Rot: 180° (S)",
            voxel_core::coords::CardinalDir::West => "Rot: 270° (W)",
        };
        draw_rectangle(btn_x, 5.0, 105.0, 26.0, Color::new(0.15, 0.2, 0.25, 0.8));
        draw_rectangle_lines(btn_x, 5.0, 105.0, 26.0, 1.0, LIGHTGRAY);
        draw_text(rot_str, btn_x + 8.0, 22.0, 14.0, GREEN);
        btn_x += 115.0;

        // F5 Reload Mods button
        draw_rectangle(btn_x, 5.0, 120.0, 26.0, Color::new(0.2, 0.4, 0.2, 0.8));
        draw_rectangle_lines(btn_x, 5.0, 120.0, 26.0, 1.0, GREEN);
        draw_text("F5: Reload Mods", btn_x + 8.0, 22.0, 14.0, WHITE);
        btn_x += 130.0;

        // REPL button
        let repl_bg = if is_repl_open {
            Color::new(0.7, 0.3, 0.1, 0.9)
        } else {
            Color::new(0.25, 0.2, 0.3, 0.8)
        };
        draw_rectangle(btn_x, 5.0, 110.0, 26.0, repl_bg);
        draw_rectangle_lines(btn_x, 5.0, 110.0, 26.0, 1.0, WHITE);
        draw_text("~: Scheme REPL", btn_x + 8.0, 22.0, 14.0, WHITE);

        // 2. Bottom Toolbar: Block Palette & Tools
        let bot_bar_h = 58.0;
        let bot_bar_y = sh - bot_bar_h;
        draw_rectangle(
            0.0,
            bot_bar_y,
            sw,
            bot_bar_h,
            Color::new(0.08, 0.08, 0.12, 0.94),
        );
        draw_rectangle_lines(
            0.0,
            bot_bar_y,
            sw,
            bot_bar_h,
            1.0,
            Color::new(0.2, 0.3, 0.4, 0.8),
        );

        // Display Block Palette (Built-ins + Mod blocks)
        let mut pal_x = 12.0;
        draw_text("Palette:", pal_x, bot_bar_y + 35.0, 14.0, LIGHTGRAY);
        pal_x += 60.0;

        let registered_blocks = registry.all_registered();
        for def in registered_blocks.iter().take(16) {
            let is_selected = def.id == ui_state.selected_block;
            let slot_bg = if is_selected {
                Color::new(0.3, 0.6, 0.9, 0.9)
            } else {
                Color::new(0.15, 0.18, 0.22, 0.8)
            };
            draw_rectangle(pal_x, bot_bar_y + 8.0, 42.0, 42.0, slot_bg);
            draw_rectangle_lines(pal_x, bot_bar_y + 8.0, 42.0, 42.0, 1.0, LIGHTGRAY);

            if let Some(tex) = atlas.get_texture(def.id) {
                draw_texture_ex(
                    tex,
                    pal_x + 5.0,
                    bot_bar_y + 10.0,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(macroquad::math::vec2(32.0, 32.0)),
                        ..Default::default()
                    },
                );
            }
            pal_x += 48.0;
        }

        // Display Mod Custom Tools buttons
        if !ui_state.mod_tools.is_empty() {
            pal_x += 16.0;
            draw_text("Mod Tools:", pal_x, bot_bar_y + 35.0, 14.0, YELLOW);
            pal_x += 75.0;

            for tool in &ui_state.mod_tools {
                let is_sel = &ui_state.selected_tool == tool;
                let bg = if is_sel {
                    Color::new(0.8, 0.4, 0.1, 0.9)
                } else {
                    Color::new(0.25, 0.22, 0.3, 0.8)
                };
                let w = (tool.len() * 8 + 16) as f32;
                draw_rectangle(pal_x, bot_bar_y + 12.0, w, 34.0, bg);
                draw_rectangle_lines(pal_x, bot_bar_y + 12.0, w, 34.0, 1.0, YELLOW);
                draw_text(tool, pal_x + 8.0, bot_bar_y + 34.0, 13.0, WHITE);
                pal_x += w + 8.0;
            }
        }

        // Status message
        draw_text(
            &ui_state.status_msg,
            16.0,
            bot_bar_y - 12.0,
            14.0,
            Color::new(0.8, 0.9, 1.0, 0.9),
        );

        // 3. Scheme Interactive REPL Console Overlay
        if is_repl_open {
            let repl_w = sw * 0.75;
            let repl_h = 240.0;
            let repl_x = (sw - repl_w) * 0.5;
            let repl_y = bot_bar_y - repl_h - 10.0;

            // Background
            draw_rectangle(
                repl_x,
                repl_y,
                repl_w,
                repl_h,
                Color::new(0.05, 0.06, 0.09, 0.95),
            );
            draw_rectangle_lines(repl_x, repl_y, repl_w, repl_h, 1.5, GREEN);

            // Title
            draw_text(
                "Steel Scheme Interactive REPL Console (Type Scheme expression and press Enter)",
                repl_x + 10.0,
                repl_y + 20.0,
                14.0,
                GREEN,
            );
            draw_rectangle(
                repl_x + 8.0,
                repl_y + 26.0,
                repl_w - 16.0,
                1.0,
                DARKGRAY,
            );

            // Log messages
            let mut line_y = repl_y + 44.0;
            let log_slice = if ui_state.repl_logs.len() > 9 {
                &ui_state.repl_logs[ui_state.repl_logs.len() - 9..]
            } else {
                &ui_state.repl_logs[..]
            };

            for line in log_slice {
                let col = if line.starts_with("Error") {
                    RED
                } else if line.starts_with("=>") {
                    YELLOW
                } else {
                    LIGHTGRAY
                };
                draw_text(line, repl_x + 12.0, line_y, 13.0, col);
                line_y += 18.0;
            }

            // Input prompt line at bottom of REPL
            let input_y = repl_y + repl_h - 14.0;
            draw_rectangle(
                repl_x + 8.0,
                input_y - 18.0,
                repl_w - 16.0,
                24.0,
                Color::new(0.12, 0.14, 0.18, 0.9),
            );
            draw_text(
                &format!("steel> {}_", ui_state.repl_input),
                repl_x + 14.0,
                input_y,
                14.0,
                WHITE,
            );
        }
    }
}
