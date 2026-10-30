use macroquad::prelude::*;
use voxel_core::block::BlockId;
use crate::camera::PlayerCamera;
use crate::view_mode::ViewMode;

#[derive(Copy, Clone, Debug)]
pub struct HotbarItem {
    pub id: BlockId,
    pub name: &'static str,
    pub color: Color,
}

pub struct Hotbar {
    pub selected_slot: usize,
    pub items: [HotbarItem; 9],
}

impl Default for Hotbar {
    fn default() -> Self {
        Self::new()
    }
}

impl Hotbar {
    pub fn new() -> Self {
        Self {
            selected_slot: 0,
            items: [
                HotbarItem { id: BlockId::GRASS, name: "Grass", color: Color::from_rgba(86, 176, 56, 255) },
                HotbarItem { id: BlockId::DIRT, name: "Dirt", color: Color::from_rgba(134, 96, 67, 255) },
                HotbarItem { id: BlockId::STONE, name: "Stone", color: Color::from_rgba(128, 128, 128, 255) },
                HotbarItem { id: BlockId::COBBLESTONE, name: "Cobble", color: Color::from_rgba(105, 105, 105, 255) },
                HotbarItem { id: BlockId::WOOD_LOG, name: "Wood", color: Color::from_rgba(103, 77, 46, 255) },
                HotbarItem { id: BlockId::LEAVES, name: "Leaves", color: Color::from_rgba(60, 140, 45, 255) },
                HotbarItem { id: BlockId::SAND, name: "Sand", color: Color::from_rgba(219, 206, 153, 255) },
                HotbarItem { id: BlockId::GLASS, name: "Glass", color: Color::from_rgba(200, 230, 245, 200) },
                HotbarItem { id: BlockId::TORCH, name: "Torch", color: Color::from_rgba(255, 195, 77, 255) },
            ],
        }
    }

    #[inline]
    pub fn current_block(&self) -> BlockId {
        self.items[self.selected_slot].id
    }

    #[inline]
    pub fn current_name(&self) -> &'static str {
        self.items[self.selected_slot].name
    }

    pub fn handle_input(&mut self) {
        if is_key_pressed(KeyCode::Key1) { self.selected_slot = 0; }
        if is_key_pressed(KeyCode::Key2) { self.selected_slot = 1; }
        if is_key_pressed(KeyCode::Key3) { self.selected_slot = 2; }
        if is_key_pressed(KeyCode::Key4) { self.selected_slot = 3; }
        if is_key_pressed(KeyCode::Key5) { self.selected_slot = 4; }
        if is_key_pressed(KeyCode::Key6) { self.selected_slot = 5; }
        if is_key_pressed(KeyCode::Key7) { self.selected_slot = 6; }
        if is_key_pressed(KeyCode::Key8) { self.selected_slot = 7; }
        if is_key_pressed(KeyCode::Key9) { self.selected_slot = 8; }
    }
}

/// Renders the 2D HUD (Crosshair, Info Panel, and Hotbar) on top of the 3D scene.
pub fn render_hud(player: &PlayerCamera, view_mode: ViewMode, hotbar: &Hotbar) {
    let screen_w = screen_width();
    let screen_h = screen_height();

    // 1. Center Crosshair (in First-Person mode)
    if view_mode.is_first_person() {
        let cx = screen_w * 0.5;
        let cy = screen_h * 0.5;
        let cross_size = 9.0;
        let thickness = 2.0;

        // Shadow outline
        draw_rectangle(cx - cross_size - 1.0, cy - thickness * 0.5 - 1.0, cross_size * 2.0 + 2.0, thickness + 2.0, Color::new(0.0, 0.0, 0.0, 0.5));
        draw_rectangle(cx - thickness * 0.5 - 1.0, cy - cross_size - 1.0, thickness + 2.0, cross_size * 2.0 + 2.0, Color::new(0.0, 0.0, 0.0, 0.5));

        // White crosshair lines
        draw_rectangle(cx - cross_size, cy - thickness * 0.5, cross_size * 2.0, thickness, WHITE);
        draw_rectangle(cx - thickness * 0.5, cy - cross_size, thickness, cross_size * 2.0, WHITE);
    }

    // 2. Top-Left Diagnostics and Controls Overlay
    let panel_x = 12.0;
    let panel_y = 12.0;
    let panel_w = 460.0;
    let panel_h = 100.0;

    draw_rectangle(panel_x, panel_y, panel_w, panel_h, Color::new(0.0, 0.0, 0.0, 0.65));
    draw_rectangle_lines(panel_x, panel_y, panel_w, panel_h, 1.5, Color::new(0.4, 0.4, 0.4, 0.8));

    let fps = get_fps();
    let pos = player.position;
    let mode_str = match view_mode {
        ViewMode::FirstPerson3D => "3D First-Person",
        ViewMode::Isometric2D5 { .. } => "2.5D Isometric",
    };
    let held_str = hotbar.current_name();
    let lock_str = if player.mouse_locked { "LOCKED (Tab/Esc to unlock)" } else { "UNLOCKED (Click to lock)" };

    draw_text(&format!("Minecraft 3D (Macroquad WebGL) | FPS: {}", fps), panel_x + 10.0, panel_y + 22.0, 18.0, YELLOW);
    draw_text(&format!("XYZ: {:.2} / {:.2} / {:.2} | View: {} [V]", pos.x, pos.y, pos.z, mode_str), panel_x + 10.0, panel_y + 44.0, 16.0, WHITE);
    draw_text(&format!("Held: {} [1-9] | Mouse: {}", held_str, lock_str), panel_x + 10.0, panel_y + 66.0, 16.0, LIGHTGRAY);
    draw_text("[WASD] Move | [Space] Jump | [V] Toggle 3D/2.5D | [RMB] Orbit", panel_x + 10.0, panel_y + 88.0, 14.0, GREEN);

    // 3. Bottom Hotbar (9 items)
    let slot_size = 46.0;
    let slot_gap = 6.0;
    let total_w = 9.0 * slot_size + 8.0 * slot_gap;
    let start_x = (screen_w - total_w) * 0.5;
    let start_y = screen_h - slot_size - 16.0;

    for i in 0..9 {
        let x = start_x + i as f32 * (slot_size + slot_gap);
        let y = start_y;
        let is_selected = i == hotbar.selected_slot;

        // Slot background
        let bg_color = if is_selected {
            Color::new(0.25, 0.25, 0.3, 0.9)
        } else {
            Color::new(0.12, 0.12, 0.12, 0.8)
        };
        draw_rectangle(x, y, slot_size, slot_size, bg_color);

        // Block color icon
        let item = &hotbar.items[i];
        let pad = 8.0;
        draw_rectangle(x + pad, y + pad, slot_size - 2.0 * pad, slot_size - 2.0 * pad, item.color);
        draw_rectangle_lines(x + pad, y + pad, slot_size - 2.0 * pad, slot_size - 2.0 * pad, 1.0, BLACK);

        // Slot number label
        let slot_num = format!("{}", i + 1);
        draw_text(&slot_num, x + 4.0, y + 14.0, 14.0, WHITE);

        // Border
        if is_selected {
            draw_rectangle_lines(x - 2.0, y - 2.0, slot_size + 4.0, slot_size + 4.0, 3.0, GOLD);
        } else {
            draw_rectangle_lines(x, y, slot_size, slot_size, 1.5, Color::new(0.3, 0.3, 0.3, 0.8));
        }
    }
}
