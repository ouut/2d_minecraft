use macroquad::camera::Camera3D;
use macroquad::prelude::*;
use crate::camera::PlayerCamera;

/// Dual view mode switching between immersive 3D first-person and classic 2.5D isometric view.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    FirstPerson3D,
    Isometric2D5,
}

impl ViewMode {
    pub fn toggle(&mut self) {
        *self = match *self {
            ViewMode::FirstPerson3D => ViewMode::Isometric2D5,
            ViewMode::Isometric2D5 => ViewMode::FirstPerson3D,
        };
    }

    pub fn name(&self) -> &'static str {
        match self {
            ViewMode::FirstPerson3D => "3D First-Person",
            ViewMode::Isometric2D5 => "2.5D Isometric",
        }
    }

    /// Computes the Macroquad Camera3D configured with `up: vec3(0.0, 0.0, 1.0)`.
    pub fn build_camera_3d(&self, player: &PlayerCamera) -> Camera3D {
        match self {
            ViewMode::FirstPerson3D => {
                let eye = player.eye_position();
                let target = eye + player.front();
                Camera3D {
                    position: eye,
                    target,
                    up: vec3(0.0, 0.0, 1.0),
                    fovy: 75.0_f32.to_radians(),
                    aspect: None,
                    projection: Projection::Perspective,
                    ..Default::default()
                }
            }
            ViewMode::Isometric2D5 => {
                // Classic 2.5D isometric viewpoint offset +vec3(12.0, 12.0, 16.0) looking towards player torso
                let offset = vec3(12.0, 12.0, 16.0);
                let cam_pos = player.position + offset;
                let target = player.position + vec3(0.0, 0.0, 0.8);
                Camera3D {
                    position: cam_pos,
                    target,
                    up: vec3(0.0, 0.0, 1.0),
                    fovy: 45.0_f32.to_radians(),
                    aspect: None,
                    projection: Projection::Perspective,
                    ..Default::default()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_mode_toggle() {
        let mut mode = ViewMode::FirstPerson3D;
        assert_eq!(mode, ViewMode::FirstPerson3D);
        mode.toggle();
        assert_eq!(mode, ViewMode::Isometric2D5);
        mode.toggle();
        assert_eq!(mode, ViewMode::FirstPerson3D);
    }

    #[test]
    fn test_camera_configurations() {
        let player = PlayerCamera::new(vec3(5.0, 5.0, 10.0));

        let fp_cam = ViewMode::FirstPerson3D.build_camera_3d(&player);
        assert_eq!(fp_cam.up, vec3(0.0, 0.0, 1.0));
        assert_eq!(fp_cam.position, vec3(5.0, 5.0, 11.62));

        let iso_cam = ViewMode::Isometric2D5.build_camera_3d(&player);
        assert_eq!(iso_cam.up, vec3(0.0, 0.0, 1.0));
        assert_eq!(iso_cam.position, vec3(17.0, 17.0, 26.0));
        assert_eq!(iso_cam.target, vec3(5.0, 5.0, 10.8));
    }
}
