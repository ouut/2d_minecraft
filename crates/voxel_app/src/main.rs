mod app;
mod world;

use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "2D Minecraft Engine (3D Voxel World + Steel Scheme Mods)".to_string(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut app = app::EngineApp::new();

    loop {
        let dt = get_frame_time().min(0.1);
        app.update(dt);
        app.render();

        next_frame().await;
    }
}
