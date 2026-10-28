pub mod components;
pub mod jobs;
pub mod pathfinding;
pub mod player;

pub use components::*;
pub use jobs::*;
pub use pathfinding::*;
pub use player::*;

use hecs::Entity;
use voxel_core::block::{BlockId, BlockRegistry};
use voxel_core::coords::{CardinalDir, WorldPos};
use voxel_core::traits::{
    ActorKind, ActorRenderView, EngineCommand, JobRenderView, JobType, WorldAccess,
};

pub struct EcsManager {
    world: hecs::World,
    pub job_manager: JobManager,
    player_entity: Option<Entity>,
}

impl Default for EcsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EcsManager {
    pub fn new() -> Self {
        Self {
            world: hecs::World::new(),
            job_manager: JobManager::new(),
            player_entity: None,
        }
    }

    pub fn spawn_player(&mut self, pos: WorldPos) -> Entity {
        let e = self.world.spawn((
            Position3D { pos },
            Velocity3D::default(),
            ActorComponent {
                kind: ActorKind::Player,
                name: "Steve (Player)".into(),
                carrying: None,
            },
            PlayerController::default(),
            Inventory::default(),
        ));
        self.player_entity = Some(e);
        e
    }

    pub fn spawn_colonist(&mut self, pos: WorldPos, name: String) -> Entity {
        self.world.spawn((
            Position3D { pos },
            Velocity3D::default(),
            ActorComponent {
                kind: ActorKind::Colonist,
                name,
                carrying: None,
            },
            ColonistAI::default(),
        ))
    }

    pub fn player_pos(&self) -> Option<WorldPos> {
        let e = self.player_entity?;
        self.world.get::<&Position3D>(e).ok().map(|p| p.pos)
    }

    pub fn teleport_player(&mut self, pos: WorldPos) {
        if let Some(e) = self.player_entity {
            if let Ok(mut p) = self.world.get::<&mut Position3D>(e) {
                p.pos = pos;
            }
        }
    }

    pub fn handle_player_input(
        &mut self,
        camera_rot: CardinalDir,
        input_dx: i32,
        input_dy: i32,
        world: &dyn WorldAccess,
        registry: &BlockRegistry,
    ) -> bool {
        if let Some(e) = self.player_entity {
            if let (Ok(mut pos), Ok(mut ctrl)) = (
                self.world.get::<&mut Position3D>(e),
                self.world.get::<&mut PlayerController>(e),
            ) {
                if ctrl.move_cooldown <= 0.0 {
                    return PlayerSystem::move_player(
                        &mut pos,
                        &mut ctrl,
                        camera_rot,
                        input_dx,
                        input_dy,
                        world,
                        registry,
                    );
                }
            }
        }
        false
    }

    pub fn handle_player_action(
        &mut self,
        target_pos: WorldPos,
        is_mine: bool,
        world: &dyn WorldAccess,
        registry: &BlockRegistry,
    ) -> Option<EngineCommand> {
        let e = self.player_entity?;
        let (pos, mut inv, mut ctrl) = (
            *self.world.get::<&Position3D>(e).ok()?,
            self.world.get::<&mut Inventory>(e).ok()?,
            self.world.get::<&mut PlayerController>(e).ok()?,
        );

        if ctrl.action_cooldown <= 0.0 {
            PlayerSystem::player_action(
                &pos,
                &mut inv,
                &mut ctrl,
                target_pos,
                is_mine,
                world,
                registry,
            )
        } else {
            None
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        world: &dyn WorldAccess,
        registry: &BlockRegistry,
    ) -> Vec<EngineCommand> {
        let mut commands = Vec::new();

        // Update player cooldowns
        if let Some(e) = self.player_entity {
            if let Ok(mut ctrl) = self.world.get::<&mut PlayerController>(e) {
                ctrl.move_cooldown = (ctrl.move_cooldown - dt).max(0.0);
                ctrl.action_cooldown = (ctrl.action_cooldown - dt).max(0.0);
            }
        }

        // Update Colonist AI
        for (pos, ai, actor) in self
            .world
            .query_mut::<(&mut Position3D, &mut ColonistAI, &mut ActorComponent)>()
        {
            ai.step_cooldown = (ai.step_cooldown - dt).max(0.0);

            match ai.state.clone() {
                ColonistState::Idle => {
                    // Try to pick a job from job manager
                    if let Some((job_idx, job)) = self.job_manager.find_closest_job(pos.pos) {
                        ai.assigned_job_idx = Some(job_idx);
                        // Find path to adjacent tile of job
                        let target_stand = job.pos;
                        if let Some(path) =
                            find_path_3d(world, registry, pos.pos, target_stand, 300)
                        {
                            ai.current_path = path;
                            ai.path_step = 0;
                            ai.state = ColonistState::MovingToJob(job.pos);
                        }
                    }
                }
                ColonistState::MovingToJob(job_pos) => {
                    let job_target = job_pos;
                    if pos.pos.manhattan_dist(job_target) <= 1 {
                        // Arrived at job
                        ai.state = ColonistState::WorkingOnJob {
                            target: job_target,
                            timer: 0.0,
                            total_time: 1.5,
                        };
                    } else if ai.path_step < ai.current_path.len() {
                        if ai.step_cooldown <= 0.0 {
                            pos.pos = ai.current_path[ai.path_step];
                            ai.path_step += 1;
                            ai.step_cooldown = 0.25;
                        }
                    } else {
                        // Path ended but not near job; repath or drop
                        ai.state = ColonistState::Idle;
                    }
                }
                ColonistState::WorkingOnJob {
                    target,
                    mut timer,
                    total_time,
                } => {
                    timer += dt;
                    if timer >= total_time {
                        let completed_target = target;
                        if let Some(job_idx) = ai.assigned_job_idx {
                            if let Some(job) = self.job_manager.complete_job(job_idx) {
                                match job.job_type {
                                    JobType::Mine | JobType::Channel | JobType::Chop => {
                                        commands.push(EngineCommand::SetBlock {
                                            pos: completed_target,
                                            id: BlockId::AIR,
                                            meta: 0,
                                            by_player_or_colonist: true,
                                        });
                                        actor.carrying = Some(BlockId::COBBLESTONE);
                                    }
                                    JobType::Build => {
                                        commands.push(EngineCommand::SetBlock {
                                            pos: completed_target,
                                            id: job.build_block,
                                            meta: 0,
                                            by_player_or_colonist: true,
                                        });
                                        actor.carrying = None;
                                    }
                                }
                            }
                        }
                        ai.assigned_job_idx = None;
                        ai.state = ColonistState::Idle;
                    } else {
                        ai.state = ColonistState::WorkingOnJob {
                            target,
                            timer,
                            total_time,
                        };
                    }
                }
            }
        }

        commands
    }

    pub fn get_actor_render_views(&self) -> Vec<ActorRenderView> {
        let mut views = Vec::new();
        let mut id = 0;
        for (pos, actor) in self.world.query::<(&Position3D, &ActorComponent)>().iter() {
            views.push(ActorRenderView {
                id,
                pos: pos.pos,
                kind: actor.kind,
                name: actor.name.clone(),
                carrying: actor.carrying,
                busy_progress: 0.0,
            });
            id += 1;
        }
        views
    }

    pub fn get_job_render_views(&self) -> Vec<JobRenderView> {
        self.job_manager.get_job_render_views()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxel_core::traits::MemoryWorld;

    #[test]
    fn test_3d_pathfinding_flat() {
        let mut world = MemoryWorld::new();
        let registry = BlockRegistry::new();

        // Create a 10x10 flat surface at z=4
        for x in 0..10 {
            for y in 0..10 {
                world.set_block(WorldPos::new(x, y, 4), BlockId::STONE, 0);
            }
        }

        let start = WorldPos::new(1, 1, 5);
        let goal = WorldPos::new(8, 8, 5);

        let path = find_path_3d(&world, &registry, start, goal, 500);
        assert!(path.is_some());
        let p = path.unwrap();
        assert_eq!(p.first().copied(), Some(start));
        assert!(p.last().unwrap().manhattan_dist(goal) <= 1);
    }

    #[test]
    fn test_3d_pathfinding_ramp_climb() {
        let mut world = MemoryWorld::new();
        let registry = BlockRegistry::new();

        // Ground at z=4
        for x in 0..10 {
            for y in 0..10 {
                world.set_block(WorldPos::new(x, y, 4), BlockId::STONE, 0);
            }
        }

        // Higher terrace at z=5
        world.set_block(WorldPos::new(5, 5, 5), BlockId::STONE, 0);
        // Ramp at (4, 5, 5) leading up to (5, 5, 6)
        world.set_block(WorldPos::new(4, 5, 5), BlockId::RAMP, 1);

        let start = WorldPos::new(2, 5, 5);
        let goal = WorldPos::new(5, 5, 6);

        let path = find_path_3d(&world, &registry, start, goal, 500);
        assert!(path.is_some(), "Should find path climbing ramp to terrace");
    }

    #[test]
    fn test_colonist_job_execution() {
        let mut world = MemoryWorld::new();
        let registry = BlockRegistry::new();

        for x in 0..10 {
            for y in 0..10 {
                world.set_block(WorldPos::new(x, y, 4), BlockId::STONE, 0);
            }
        }

        let mut ecs = EcsManager::new();
        ecs.spawn_colonist(WorldPos::new(1, 1, 5), "Urist (Dwarf)".into());

        // Designate a mine job
        let target = WorldPos::new(3, 3, 5);
        ecs.job_manager.designate_box(target, target, JobType::Mine, BlockId::AIR);

        // Step simulation
        for _ in 0..20 {
            let cmds = ecs.update(0.3, &world, &registry);
            for c in cmds {
                if let EngineCommand::SetBlock { pos, id, .. } = c {
                    assert_eq!(pos, target);
                    assert_eq!(id, BlockId::AIR);
                }
            }
        }
    }
}
