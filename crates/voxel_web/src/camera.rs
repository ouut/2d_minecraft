use macroquad::prelude::*;
use voxel_core::block::BlockRegistry;
use voxel_core::coords::WorldPos;
use voxel_core::traits::WorldAccess;

/// Player bounding box half-width (total width = 0.6) and total height = 1.8.
pub const PLAYER_RADIUS: f32 = 0.3;
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const EYE_HEIGHT: f32 = 1.62;

pub const WALK_SPEED: f32 = 4.3;
pub const SNEAK_SPEED: f32 = 1.6;
pub const JUMP_VELOCITY: f32 = 8.4;
pub const GRAVITY: f32 = 24.0;
pub const MOUSE_SENSITIVITY: f32 = 0.003;

#[derive(Clone, Debug)]
pub struct PlayerCamera {
    pub position: Vec3, // Feet position
    pub velocity: Vec3,
    pub yaw: f32,       // Radians in XY plane
    pub pitch: f32,     // Elevation radians (-89 deg to +89 deg)
    pub on_ground: bool,
    pub mouse_locked: bool,
}

impl Default for PlayerCamera {
    fn default() -> Self {
        Self::new(vec3(0.0, 0.0, 20.0))
    }
}

impl PlayerCamera {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            mouse_locked: false,
        }
    }

    #[inline]
    pub fn eye_position(&self) -> Vec3 {
        vec3(self.position.x, self.position.y, self.position.z + EYE_HEIGHT)
    }

    #[inline]
    pub fn front(&self) -> Vec3 {
        vec3(
            self.yaw.cos() * self.pitch.cos(),
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
        )
        .normalize()
    }

    #[inline]
    pub fn right(&self) -> Vec3 {
        vec3(self.yaw.sin(), -self.yaw.cos(), 0.0)
    }

    #[inline]
    pub fn forward_horizontal(&self) -> Vec3 {
        vec3(self.yaw.cos(), self.yaw.sin(), 0.0)
    }

    pub fn set_mouse_locked(&mut self, locked: bool) {
        self.mouse_locked = locked;
        set_cursor_grab(locked);
        show_mouse(!locked);
    }

    pub fn toggle_mouse_lock(&mut self) {
        let new_state = !self.mouse_locked;
        self.set_mouse_locked(new_state);
    }

    /// Handles mouse look rotation.
    pub fn update_mouse_look(&mut self, mouse_delta: Vec2) {
        if !self.mouse_locked {
            return;
        }

        self.yaw += mouse_delta.x * MOUSE_SENSITIVITY;
        // Subtract Y delta so moving mouse up tilts camera upward
        self.pitch -= mouse_delta.y * MOUSE_SENSITIVITY;

        let max_pitch = 89.0_f32.to_radians();
        self.pitch = self.pitch.clamp(-max_pitch, max_pitch);
    }

    /// Checks if a bounding box collides with solid voxels in the world.
    pub fn is_colliding(
        min: Vec3,
        max: Vec3,
        world: &impl WorldAccess,
        reg: &BlockRegistry,
    ) -> bool {
        let x_start = (min.x.floor() as i32).max(-1000);
        let x_end = (max.x.floor() as i32).min(1000);
        let y_start = (min.y.floor() as i32).max(-1000);
        let y_end = (max.y.floor() as i32).min(1000);
        let z_start = (min.z.floor() as i32).max(0);
        let z_end = (max.z.floor() as i32).min(63);

        for z in z_start..=z_end {
            for y in y_start..=y_end {
                for x in x_start..=x_end {
                    let id = world.get_block_id(WorldPos::new(x, y, z));
                    if !id.is_air() && reg.is_solid(id) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Updates movement, gravity, and ground AABB collisions with voxel terrain.
    pub fn update_physics(
        &mut self,
        dt: f32,
        input_move: Vec2, // (forward, right) in [-1..1]
        jump: bool,
        sneak: bool,
        world: &impl WorldAccess,
        reg: &BlockRegistry,
    ) {
        let dt = dt.min(0.05); // Cap delta time to prevent tunneling

        // 1. Horizontal movement velocity
        let speed = if sneak { SNEAK_SPEED } else { WALK_SPEED };
        let mut move_dir = Vec3::ZERO;
        if input_move.length_squared() > 0.001 {
            let norm_input = input_move.normalize();
            move_dir += self.forward_horizontal() * norm_input.x;
            move_dir += self.right() * norm_input.y;
        }

        let target_vx = move_dir.x * speed;
        let target_vy = move_dir.y * speed;

        // Smooth horizontal acceleration / friction
        let accel = if self.on_ground { 15.0 } else { 5.0 };
        self.velocity.x += (target_vx - self.velocity.x) * (accel * dt).min(1.0);
        self.velocity.y += (target_vy - self.velocity.y) * (accel * dt).min(1.0);

        // 2. Jump & Gravity
        if jump && self.on_ground {
            self.velocity.z = JUMP_VELOCITY;
            self.on_ground = false;
        }

        self.velocity.z -= GRAVITY * dt;
        self.velocity.z = self.velocity.z.max(-40.0); // Terminal velocity

        // 3. Axis-separated AABB collision resolution
        // Z movement
        let dz = self.velocity.z * dt;
        let mut new_z = self.position.z + dz;
        self.on_ground = false;

        if dz <= 0.0 {
            // Check landing on floor
            let foot_min = vec3(
                self.position.x - PLAYER_RADIUS,
                self.position.y - PLAYER_RADIUS,
                new_z,
            );
            let foot_max = vec3(
                self.position.x + PLAYER_RADIUS,
                self.position.y + PLAYER_RADIUS,
                new_z + 0.1,
            );

            if Self::is_colliding(foot_min, foot_max, world, reg) {
                // Find highest block top directly below feet
                let block_z = new_z.floor() as i32;
                let top_z = (block_z + 1) as f32;
                if self.position.z >= top_z - 0.2 {
                    new_z = top_z;
                    self.velocity.z = 0.0;
                    self.on_ground = true;
                }
            }
        } else {
            // Check ceiling collision
            let head_min = vec3(
                self.position.x - PLAYER_RADIUS,
                self.position.y - PLAYER_RADIUS,
                new_z + PLAYER_HEIGHT - 0.1,
            );
            let head_max = vec3(
                self.position.x + PLAYER_RADIUS,
                self.position.y + PLAYER_RADIUS,
                new_z + PLAYER_HEIGHT,
            );

            if Self::is_colliding(head_min, head_max, world, reg) {
                let ceiling_bottom = (new_z + PLAYER_HEIGHT).floor();
                new_z = ceiling_bottom - PLAYER_HEIGHT;
                self.velocity.z = 0.0;
            }
        }
        self.position.z = new_z;

        // X movement with sliding
        let dx = self.velocity.x * dt;
        if dx.abs() > 0.0001 {
            let next_x = self.position.x + dx;
            let aabb_min = vec3(next_x - PLAYER_RADIUS, self.position.y - PLAYER_RADIUS, self.position.z);
            let aabb_max = vec3(next_x + PLAYER_RADIUS, self.position.y + PLAYER_RADIUS, self.position.z + PLAYER_HEIGHT);
            if !Self::is_colliding(aabb_min, aabb_max, world, reg) {
                self.position.x = next_x;
            } else {
                self.velocity.x = 0.0;
            }
        }

        // Y movement with sliding
        let dy = self.velocity.y * dt;
        if dy.abs() > 0.0001 {
            let next_y = self.position.y + dy;
            let aabb_min = vec3(self.position.x - PLAYER_RADIUS, next_y - PLAYER_RADIUS, self.position.z);
            let aabb_max = vec3(self.position.x + PLAYER_RADIUS, next_y + PLAYER_RADIUS, self.position.z + PLAYER_HEIGHT);
            if !Self::is_colliding(aabb_min, aabb_max, world, reg) {
                self.position.y = next_y;
            } else {
                self.velocity.y = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxel_core::block::BlockId;
    use voxel_core::traits::MemoryWorld;

    #[test]
    fn test_camera_vectors() {
        let mut cam = PlayerCamera::new(vec3(0.0, 0.0, 10.0));
        assert_eq!(cam.eye_position(), vec3(0.0, 0.0, 11.62));

        // Looking along +X
        cam.yaw = 0.0;
        cam.pitch = 0.0;
        let f = cam.front();
        assert!((f.x - 1.0).abs() < 1e-4);
        assert!(f.y.abs() < 1e-4);
        assert!(f.z.abs() < 1e-4);

        // Looking along +Y
        cam.yaw = std::f32::consts::FRAC_PI_2;
        let f_north = cam.front();
        assert!(f_north.x.abs() < 1e-4);
        assert!((f_north.y - 1.0).abs() < 1e-4);

        // Looking up (+Z)
        cam.pitch = std::f32::consts::FRAC_PI_2 - 0.01;
        let f_up = cam.front();
        assert!(f_up.z > 0.99);
    }

    #[test]
    fn test_camera_gravity_and_ground_landing() {
        let reg = BlockRegistry::new();
        let mut world = MemoryWorld::new();

        // Solid floor at z = 5
        for x in -2..=2 {
            for y in -2..=2 {
                world.set_block(WorldPos::new(x, y, 5), BlockId::STONE, 0);
            }
        }

        // Spawn player in air at z = 8
        let mut cam = PlayerCamera::new(vec3(0.0, 0.0, 8.0));
        assert!(!cam.on_ground);

        // Step physics until player lands on the floor (top of block 5 is z = 6)
        for _ in 0..100 {
            cam.update_physics(0.02, Vec2::ZERO, false, false, &world, &reg);
            if cam.on_ground {
                break;
            }
        }

        assert!(cam.on_ground);
        assert!((cam.position.z - 6.0).abs() < 0.05);
        assert!(cam.velocity.z.abs() < 1e-3);
    }
}
