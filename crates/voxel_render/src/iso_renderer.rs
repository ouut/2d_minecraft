use macroquad::color::{Color, GREEN, WHITE, YELLOW};
use macroquad::shapes::{draw_line, draw_rectangle_lines};
use macroquad::text::draw_text;
use macroquad::texture::draw_texture_ex;
use macroquad::texture::DrawTextureParams;
use macroquad::window::{screen_height, screen_width};
use voxel_core::block::BlockRegistry;
use voxel_core::coords::{IsoCamera, WorldPos};
use voxel_core::traits::{ActorKind, ActorRenderView, JobRenderView, JobType, WorldAccess};
use crate::atlas::IsoAtlas;

pub struct WorldRenderer;

impl WorldRenderer {
    pub fn render_world(
        world: &dyn WorldAccess,
        registry: &BlockRegistry,
        atlas: &IsoAtlas,
        camera: &IsoCamera,
        actors: &[ActorRenderView],
        jobs: &[JobRenderView],
        hover_pos: Option<WorldPos>,
        box_selection: Option<(WorldPos, WorldPos)>,
    ) {
        let sw = screen_width();
        let sh = screen_height();

        // Visible range around camera focus
        let radius = 18;
        let rx_min = -radius;
        let rx_max = radius;
        let ry_min = -radius;
        let ry_max = radius;

        let z_min = (camera.slice_z - camera.max_visible_depth).max(0);
        let z_max = camera.slice_z;

        // Group into (depth_key = rx + ry) for Back-to-Front painter's algorithm
        let min_depth = rx_min + ry_min;
        let max_depth = rx_max + ry_max;

        let half_w = camera.tile_w * 0.5 * camera.zoom;
        let half_h = camera.tile_h * 0.5 * camera.zoom;
        let tile_render_w = camera.tile_w * camera.zoom;
        let tile_render_h = camera.tile_w * camera.zoom; // 32x32 texture

        for depth in min_depth..=max_depth {
            for z in z_min..=z_max {
                for rx in rx_min..=rx_max {
                    let ry = depth - rx;
                    if ry < ry_min || ry > ry_max {
                        continue;
                    }

                    let (dx, dy) = camera.view_to_world_coords(rx as f32, ry as f32);
                    let wx = (dx + camera.focus_x).round() as i32;
                    let wy = (dy + camera.focus_y).round() as i32;
                    let pos = WorldPos::new(wx, wy, z);

                    let voxel = world.get_voxel(pos);
                    if voxel.id.is_air() {
                        continue;
                    }

                    // Occlusion Culling: check top, front-left, front-right in view
                    if z + 1 <= camera.slice_z {
                        let top_v = world.get_voxel(pos.offset(0, 0, 1));
                        if registry.is_solid(top_v.id) && !registry.is_transparent(top_v.id) {
                            // Relative front neighbors in current view
                            let (fl_dx, fl_dy) = camera.view_to_world_coords(1.0, 0.0);
                            let (fr_dx, fr_dy) = camera.view_to_world_coords(0.0, 1.0);
                            let fl_v = world.get_voxel(pos.offset(fl_dx as i32, fl_dy as i32, 0));
                            let fr_v = world.get_voxel(pos.offset(fr_dx as i32, fr_dy as i32, 0));

                            if registry.is_solid(fl_v.id)
                                && !registry.is_transparent(fl_v.id)
                                && registry.is_solid(fr_v.id)
                                && !registry.is_transparent(fr_v.id)
                            {
                                // Completely hidden block
                                continue;
                            }
                        }
                    }

                    // Lighting & Depth Fog
                    let delta_z = (camera.slice_z - z) as f32;
                    let depth_fog = (1.0 - delta_z * 0.045).clamp(0.25, 1.0);

                    let light_val = (voxel.max_light().max(2) as f32 / 15.0).clamp(0.15, 1.0);
                    let intensity = depth_fog * light_val;

                    let tint_r = (intensity * 0.9).min(1.0);
                    let tint_g = (intensity * 0.95).min(1.0);
                    let tint_b = intensity.min(1.0);
                    let tint = Color::new(tint_r, tint_g, tint_b, 1.0);

                    // Screen position
                    let (sx, sy) = camera.world_to_screen(pos, sw, sh);
                    let draw_x = sx - half_w;
                    let draw_y = sy - half_h;

                    if let Some(tex) = atlas.get_texture(voxel.id) {
                        draw_texture_ex(
                            tex,
                            draw_x,
                            draw_y,
                            tint,
                            DrawTextureParams {
                                dest_size: Some(macroquad::math::vec2(tile_render_w, tile_render_h)),
                                ..Default::default()
                            },
                        );
                    }
                }
            }
        }

        // Draw Job Designations
        for job in jobs {
            if (camera.slice_z - job.pos.z).abs() <= 3 {
                let (sx, sy) = camera.world_to_screen(job.pos, sw, sh);
                let col = match job.job_type {
                    JobType::Mine => Color::new(1.0, 0.2, 0.2, 0.6),
                    JobType::Channel => Color::new(0.2, 0.8, 1.0, 0.6),
                    JobType::Build => Color::new(0.2, 1.0, 0.3, 0.6),
                    JobType::Chop => Color::new(1.0, 0.8, 0.2, 0.6),
                };
                draw_rectangle_lines(sx - half_w, sy - half_h, half_w * 2.0, half_h * 2.0, 2.0, col);
            }
        }

        // Draw Actors (Player & Colonists)
        for actor in actors {
            if (camera.slice_z - actor.pos.z).abs() <= 4 {
                let (sx, sy) = camera.world_to_screen(actor.pos, sw, sh);
                let (avatar_col, label_col) = match actor.kind {
                    ActorKind::Player => (Color::new(0.2, 0.8, 1.0, 1.0), YELLOW),
                    ActorKind::Colonist => (Color::new(0.9, 0.6, 0.2, 1.0), WHITE),
                };

                // Body diamond / circle
                macroquad::shapes::draw_circle(sx, sy, 6.0 * camera.zoom, avatar_col);
                macroquad::shapes::draw_circle_lines(sx, sy, 6.0 * camera.zoom, 1.5, WHITE);

                // Name tag
                let font_size = (12.0 * camera.zoom).clamp(10.0, 18.0);
                draw_text(&actor.name, sx - 25.0, sy - 10.0 * camera.zoom, font_size, label_col);

                if let Some(carried) = actor.carrying {
                    if let Some(tex) = atlas.get_texture(carried) {
                        draw_texture_ex(
                            tex,
                            sx + 5.0,
                            sy - 16.0,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(macroquad::math::vec2(12.0, 12.0)),
                                ..Default::default()
                            },
                        );
                    }
                }
            }
        }

        // Draw Hover indicator
        if let Some(h) = hover_pos {
            let (sx, sy) = camera.world_to_screen(h, sw, sh);
            let hw = half_w;
            let hh = half_h;
            // Draw diamond outline
            draw_line(sx, sy - hh, sx + hw, sy, 2.0, YELLOW);
            draw_line(sx + hw, sy, sx, sy + hh, 2.0, YELLOW);
            draw_line(sx, sy + hh, sx - hw, sy, 2.0, YELLOW);
            draw_line(sx - hw, sy, sx, sy - hh, 2.0, YELLOW);
        }

        // Draw Box Selection
        if let Some((p1, p2)) = box_selection {
            let (sx1, sy1) = camera.world_to_screen(p1, sw, sh);
            let (sx2, sy2) = camera.world_to_screen(p2, sw, sh);
            let min_x = sx1.min(sx2) - half_w;
            let min_y = sy1.min(sy2) - half_h;
            let w = (sx1 - sx2).abs() + half_w * 2.0;
            let h = (sy1 - sy2).abs() + half_h * 2.0;
            draw_rectangle_lines(min_x, min_y, w, h, 2.0, GREEN);
        }
    }
}
