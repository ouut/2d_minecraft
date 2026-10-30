use macroquad::camera::Camera3D;
use macroquad::prelude::*;
use crate::camera::PlayerCamera;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ViewMode {
    FirstPerson3D,
    Isometric2D5 {
        /// Orbit angle in radians around the target (default 45 degrees / pi/4)
        orbit_yaw: f32,
        /// Pitch angle above the horizon (default ~35.264 degrees for true isometric)
        orbit_pitch: f32,
        /// Distance from target
        distance: f32,
    },
}

impl Default for ViewMode {
    fn default() -> Self {
        ViewMode::FirstPerson3D
    }
}

impl ViewMode {
    pub fn toggle(&mut self) {
        *self = match *self {
            ViewMode::FirstPerson3D => ViewMode::Isometric2D5 {
                orbit_yaw: std::f32::consts::FRAC_PI_4,
                orbit_pitch: 0.6154, // ~35.264 degrees (true isometric)
                distance: 24.0,
            },
            ViewMode::Isometric2D5 { .. } => ViewMode::FirstPerson3D,
        };
    }

    pub fn is_isometric(&self) -> bool {
        matches!(self, ViewMode::Isometric2D5 { .. })
    }

    pub fn is_first_person(&self) -> bool {
        matches!(self, ViewMode::FirstPerson3D)
    }

    pub fn handle_isometric_orbit(&mut self, dx: f32, dy: f32) {
        if let ViewMode::Isometric2D5 { ref mut orbit_yaw, ref mut orbit_pitch, .. } = *self {
            *orbit_yaw += dx * 0.005;
            *orbit_pitch = (*orbit_pitch - dy * 0.005).clamp(0.1, 1.5);
        }
    }

    pub fn handle_zoom(&mut self, delta_wheel: f32) {
        if let ViewMode::Isometric2D5 { ref mut distance, .. } = *self {
            *distance = (*distance - delta_wheel * 2.0).clamp(6.0, 60.0);
        }
    }

    /// Computes the Macroquad Camera3D configured with `up: vec3(0.0, 0.0, 1.0)`.
    pub fn build_camera_3d(&self, player: &PlayerCamera) -> Camera3D {
        match *self {
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
            ViewMode::Isometric2D5 { orbit_yaw, orbit_pitch, distance } => {
                // Spherical coordinate offset based on orbit angles
                let offset_x = distance * orbit_pitch.cos() * orbit_yaw.cos();
                let offset_y = distance * orbit_pitch.cos() * orbit_yaw.sin();
                let offset_z = distance * orbit_pitch.sin();

                let target = player.position + vec3(0.0, 0.0, 0.8);
                let cam_pos = target + vec3(offset_x, offset_y, offset_z);

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
        assert!(matches!(mode, ViewMode::Isometric2D5 { .. }));
        mode.toggle();
        assert_eq!(mode, ViewMode::FirstPerson3D);
    }

    #[test]
    fn test_camera_configurations() {
        let player = PlayerCamera::new(vec3(5.0, 5.0, 10.0));

        let fp_cam = ViewMode::FirstPerson3D.build_camera_3d(&player);
        assert_eq!(fp_cam.up, vec3(0.0, 0.0, 1.0));
        assert_eq!(fp_cam.position, vec3(5.0, 5.0, 11.62));

        let mut iso_mode = ViewMode::FirstPerson3D;
        iso_mode.toggle();
        let iso_cam = iso_mode.build_camera_3d(&player);
        assert_eq!(iso_cam.up, vec3(0.0, 0.0, 1.0));
        assert_eq!(iso_cam.target, vec3(5.0, 5.0, 10.8));
    }
}
