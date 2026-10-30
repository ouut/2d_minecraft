use std::collections::HashMap;
use macroquad::models::draw_mesh;
use macroquad::models::Mesh;
use macroquad::prelude::*;
use voxel_core::block::{BlockId, BlockRegistry};
use voxel_core::coords::{ChunkPos, WorldPos};
use voxel_core::traits::{MemoryWorld, WorldAccess};
use voxel_web::camera::PlayerCamera;
use voxel_web::hud::{render_hud, Hotbar};
use voxel_web::mesh::build_chunk_mesh;
use voxel_web::net_client::generate_demo_world;
use voxel_web::raycast::{can_place_block_at, draw_selection_box, raycast_voxel, MAX_RAY_DISTANCE};
use voxel_web::view_mode::ViewMode;

fn window_conf() -> Conf {
    Conf {
        window_title: String::from("Minecraft 3D (WebGL / Macroquad)"),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

fn rebuild_chunk(
    meshes: &mut HashMap<ChunkPos, Vec<Mesh>>,
    cpos: ChunkPos,
    world: &MemoryWorld,
    reg: &BlockRegistry,
) {
    let chunk_mesh = build_chunk_mesh(cpos, world, reg);
    meshes.insert(cpos, chunk_mesh);
}

fn rebuild_affected_chunks(
    meshes: &mut HashMap<ChunkPos, Vec<Mesh>>,
    pos: WorldPos,
    world: &MemoryWorld,
    reg: &BlockRegistry,
) {
    let (center_cpos, lx, ly, _) = pos.to_chunk_and_local();
    rebuild_chunk(meshes, center_cpos, world, reg);

    // Rebuild neighbor chunks if on border
    if lx == 0 {
        rebuild_chunk(meshes, ChunkPos::new(center_cpos.x - 1, center_cpos.y), world, reg);
    } else if lx == 15 {
        rebuild_chunk(meshes, ChunkPos::new(center_cpos.x + 1, center_cpos.y), world, reg);
    }

    if ly == 0 {
        rebuild_chunk(meshes, ChunkPos::new(center_cpos.x, center_cpos.y - 1), world, reg);
    } else if ly == 15 {
        rebuild_chunk(meshes, ChunkPos::new(center_cpos.x, center_cpos.y + 1), world, reg);
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let reg = BlockRegistry::new();
    let mut world = generate_demo_world();

    // Spawn player safely on surface near (0, 0)
    let spawn_z = (world.surface_z(0, 0) as f32) + 2.0;
    let mut player = PlayerCamera::new(vec3(0.5, 0.5, spawn_z));

    let mut view_mode = ViewMode::FirstPerson3D;
    let mut hotbar = Hotbar::new();

    // Initial mesh build for 3x3 chunks
    let mut chunk_meshes: HashMap<ChunkPos, Vec<Mesh>> = HashMap::new();
    for cy in -1..=1 {
        for cx in -1..=1 {
            let cpos = ChunkPos::new(cx, cy);
            rebuild_chunk(&mut chunk_meshes, cpos, &world, &reg);
        }
    }

    let mut last_mouse_pos = Vec2::from(mouse_position());

    loop {
        let dt = get_frame_time();

        // 1. Perspective mode toggle
        if is_key_pressed(KeyCode::V) {
            view_mode.toggle();
        }

        // 2. Mouse pointer lock toggle
        if is_key_pressed(KeyCode::Tab) || is_key_pressed(KeyCode::Escape) {
            player.toggle_mouse_lock();
        }
        if !player.mouse_locked && is_mouse_button_pressed(MouseButton::Left) {
            player.set_mouse_locked(true);
        }

        // 3. Hotbar slot selection (1-9)
        hotbar.handle_input();

        // 4. Movement input
        let mut forward = 0.0;
        let mut right = 0.0;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
            forward += 1.0;
        }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
            forward -= 1.0;
        }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
            right += 1.0;
        }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
            right -= 1.0;
        }

        let jump = is_key_down(KeyCode::Space);
        let sneak = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);

        // 5. Mouse view rotation & Orbit
        let current_mouse_pos = Vec2::from(mouse_position());
        let mouse_delta_pixels = current_mouse_pos - last_mouse_pos;
        last_mouse_pos = current_mouse_pos;

        match view_mode {
            ViewMode::FirstPerson3D => {
                // In first person, rotate player look when mouse is captured
                player.update_mouse_look(mouse_delta_pixels);
            }
            ViewMode::Isometric2D5 { .. } => {
                // In 2.5D, rotate camera orbit when dragging with Right Mouse Button or when locked
                if is_mouse_button_down(MouseButton::Right) || player.mouse_locked {
                    view_mode.handle_isometric_orbit(mouse_delta_pixels.x, mouse_delta_pixels.y);
                }
                // Zoom in/out with scroll wheel
                let wheel = mouse_wheel().1;
                if wheel.abs() > 0.01 {
                    view_mode.handle_zoom(wheel);
                }
            }
        }

        // 6. Physics and AABB terrain collision
        player.update_physics(dt, vec2(forward, right), jump, sneak, &world, &reg);

        // 7. Raycast targeting
        let ray_origin = player.eye_position();
        let ray_dir = player.front();
        let ray_hit = raycast_voxel(ray_origin, ray_dir, MAX_RAY_DISTANCE, &world, &reg);

        // 8. Mining (Left Click) & Placing (Right Click)
        if player.mouse_locked {
            if is_mouse_button_pressed(MouseButton::Left) {
                if let Some(hit) = ray_hit {
                    world.set_block(hit.hit_pos, BlockId::AIR, 0);
                    rebuild_affected_chunks(&mut chunk_meshes, hit.hit_pos, &world, &reg);
                }
            } else if is_mouse_button_pressed(MouseButton::Right) {
                if let Some(hit) = ray_hit {
                    let (dx, dy, dz) = hit.hit_face.normal();
                    let place_pos = hit.hit_pos.offset(dx, dy, dz);
                    if can_place_block_at(place_pos, player.position) {
                        world.set_block(place_pos, hotbar.current_block(), 0);
                        rebuild_affected_chunks(&mut chunk_meshes, place_pos, &world, &reg);
                    }
                }
            }
        }

        // 9. 3D World Rendering
        // Authentic Minecraft sunny sky blue
        clear_background(Color::new(0.53, 0.81, 0.98, 1.0));

        let cam = view_mode.build_camera_3d(&player);
        set_camera(&cam);

        // Draw voxel chunk meshes
        for meshes in chunk_meshes.values() {
            for mesh in meshes {
                draw_mesh(mesh);
            }
        }

        // Draw 3D wireframe outline on targeted block
        if let Some(hit) = ray_hit {
            draw_selection_box(hit.hit_pos);
        }

        // Draw player avatar in 2.5D Isometric view
        if view_mode.is_isometric() {
            let p = player.position;
            // Legs (Navy blue)
            draw_cube(vec3(p.x, p.y, p.z + 0.35), vec3(0.5, 0.35, 0.7), None, Color::new(0.1, 0.2, 0.5, 1.0));
            // Torso (Cyan shirt)
            draw_cube(vec3(p.x, p.y, p.z + 0.95), vec3(0.5, 0.35, 0.5), None, Color::new(0.0, 0.6, 0.8, 1.0));
            // Head (Skin tone)
            draw_cube(vec3(p.x, p.y, p.z + 1.45), vec3(0.4, 0.4, 0.4), None, Color::new(0.85, 0.68, 0.52, 1.0));
        }

        set_default_camera();

        // 10. 2D HUD Rendering
        render_hud(&player, view_mode, &hotbar);

        next_frame().await;
    }
}
