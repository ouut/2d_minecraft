pub mod components;
pub mod jobs;
pub mod pathfinding;
pub mod player;

pub use components::*;
pub use jobs::*;
pub use pathfinding::*;
pub use player::*;

use flecs_ecs::prelude::*;
pub use flecs_ecs::core::{Entity, EntityView};

use voxel_core::block::{BlockId, BlockRegistry};
use voxel_core::coords::{CardinalDir, WorldPos};
use voxel_core::traits::{
    ActorKind, ActorRenderView, EngineCommand, JobRenderView, JobType, WorldAccess,
};

pub struct EcsManager {
    world: World,
    pub job_manager: JobManager,
    player_entity: Option<Entity>,
    base_dwarf: Entity,
    base_player: Entity,
}

impl Default for EcsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EcsManager {
    pub fn new() -> Self {
        let world = World::new();

        // 1. Configure component traits (inheritance for prefabs, cascading on delete)
        world
            .component::<Position3D>()
            .add_trait::<(flecs::OnInstantiate, flecs::Inherit)>();
        world
            .component::<Velocity3D>()
            .add_trait::<(flecs::OnInstantiate, flecs::Inherit)>();
        world
            .component::<ActorComponent>()
            .add_trait::<(flecs::OnInstantiate, flecs::Inherit)>();
        world
            .component::<ColonistAI>()
            .add_trait::<(flecs::OnInstantiate, flecs::Inherit)>();
        world
            .component::<PlayerController>()
            .add_trait::<(flecs::OnInstantiate, flecs::Inherit)>();
        world
            .component::<Inventory>()
            .add_trait::<(flecs::OnInstantiate, flecs::Inherit)>();
        world
            .component::<WorkingOn>()
            .add_trait::<(flecs::OnDeleteTarget, flecs::Remove)>();

        // 2. Create BaseDwarf prefab
        let base_dwarf = world
            .prefab_named("BaseDwarf")
            .set(Position3D {
                pos: WorldPos::new(0, 0, 0),
            })
            .set(Velocity3D::default())
            .set(ActorComponent {
                kind: ActorKind::Colonist,
                name: "Colonist".into(),
                carrying: None,
            })
            .set(ColonistAI::default())
            .id();

        // 3. Create BasePlayer prefab
        let base_player = world
            .prefab_named("BasePlayer")
            .set(Position3D {
                pos: WorldPos::new(0, 0, 0),
            })
            .set(Velocity3D::default())
            .set(ActorComponent {
                kind: ActorKind::Player,
                name: "Steve (Player)".into(),
                carrying: None,
            })
            .set(PlayerController::default())
            .set(Inventory::default())
            .id();

        Self {
            world,
            job_manager: JobManager::new(),
            player_entity: None,
            base_dwarf,
            base_player,
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn spawn_player(&mut self, pos: WorldPos) -> EntityView<'_> {
        let e = self
            .world
            .entity()
            .is_a(self.base_player)
            .set(Position3D { pos })
            .set(Velocity3D::default())
            .set(ActorComponent {
                kind: ActorKind::Player,
                name: "Steve (Player)".into(),
                carrying: None,
            })
            .set(PlayerController::default())
            .set(Inventory::default());
        self.player_entity = Some(e.id());
        e
    }

    pub fn spawn_colonist(&mut self, pos: WorldPos, name: String) -> EntityView<'_> {
        self.world
            .entity()
            .is_a(self.base_dwarf)
            .set(Position3D { pos })
            .set(Velocity3D::default())
            .set(ActorComponent {
                kind: ActorKind::Colonist,
                name,
                carrying: None,
            })
            .set(ColonistAI::default())
    }

    pub fn player_pos(&self) -> Option<WorldPos> {
        let e = self.player_entity?;
        if !self.world.is_alive(e) {
            return None;
        }
        let view = self.world.entity_from_id(e);
        view.try_cloned::<&Position3D>().map(|p| p.pos)
    }

    pub fn teleport_player(&mut self, pos: WorldPos) {
        if let Some(e) = self.player_entity {
            if self.world.is_alive(e) {
                let view = self.world.entity_from_id(e);
                view.try_get::<&mut Position3D>(|p| {
                    p.pos = pos;
                });
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
        let Some(e) = self.player_entity else {
            return false;
        };
        if !self.world.is_alive(e) {
            return false;
        }
        let view = self.world.entity_from_id(e);
        let mut moved = false;
        view.try_get::<(&mut Position3D, &mut PlayerController)>(|(pos, ctrl)| {
            if ctrl.move_cooldown <= 0.0 {
                moved = PlayerSystem::move_player(
                    pos,
                    ctrl,
                    camera_rot,
                    input_dx,
                    input_dy,
                    world,
                    registry,
                );
            }
        });
        moved
    }

    pub fn handle_player_action(
        &mut self,
        target_pos: WorldPos,
        is_mine: bool,
        world: &dyn WorldAccess,
        registry: &BlockRegistry,
    ) -> Option<EngineCommand> {
        let e = self.player_entity?;
        if !self.world.is_alive(e) {
            return None;
        }
        let view = self.world.entity_from_id(e);
        let mut cmd = None;
        view.try_get::<(&Position3D, &mut Inventory, &mut PlayerController)>(
            |(pos, inv, ctrl)| {
                if ctrl.action_cooldown <= 0.0 {
                    cmd = PlayerSystem::player_action(
                        pos,
                        inv,
                        ctrl,
                        target_pos,
                        is_mine,
                        world,
                        registry,
                    );
                }
            },
        );
        cmd
    }

    pub fn update(
        &mut self,
        dt: f32,
        world: &dyn WorldAccess,
        registry: &BlockRegistry,
    ) -> Vec<EngineCommand> {
        let mut commands = Vec::new();

        // 1. Update player cooldowns
        if let Some(e) = self.player_entity {
            if self.world.is_alive(e) {
                let view = self.world.entity_from_id(e);
                view.try_get::<&mut PlayerController>(|ctrl| {
                    ctrl.move_cooldown = (ctrl.move_cooldown - dt).max(0.0);
                    ctrl.action_cooldown = (ctrl.action_cooldown - dt).max(0.0);
                });
            }
        }

        // 2. Collect Colonist AI entities to safely update them
        let mut colonist_ids = Vec::new();
        self.world.each_entity::<&ColonistAI>(|entity, _| {
            colonist_ids.push(entity.id());
        });

        // 3. Update each colonist
        for cid in colonist_ids {
            if !self.world.is_alive(cid) {
                continue;
            }

            let colonist_view = self.world.entity_from_id(cid);
            let mut current_pos = None;
            let mut current_ai = None;
            let mut current_actor = None;

            colonist_view.try_get::<(&Position3D, &ColonistAI, &ActorComponent)>(
                |(pos, ai, actor)| {
                    current_pos = Some(*pos);
                    current_ai = Some(ai.clone());
                    current_actor = Some(actor.clone());
                },
            );

            let (Some(mut pos), Some(mut ai), Some(mut actor)) =
                (current_pos, current_ai, current_actor)
            else {
                continue;
            };

            ai.step_cooldown = (ai.step_cooldown - dt).max(0.0);

            match ai.state.clone() {
                ColonistState::Idle => {
                    // Try to pick a job from job manager
                    if let Some((job_idx, job)) = self.job_manager.find_closest_job(pos.pos) {
                        let target_stand = job.pos;
                        if let Some(path) =
                            find_path_3d(world, registry, pos.pos, target_stand, 300)
                        {
                            ai.assigned_job_idx = Some(job_idx);
                            ai.current_path = path;
                            ai.path_step = 0;
                            ai.state = ColonistState::MovingToJob(job.pos);

                            // Flecs relationship: Colonist claims job entity
                            let job_entity = self.world.entity().set(JobTask {
                                pos: job.pos,
                                job_type: job.job_type,
                                build_block: job.build_block,
                                job_idx,
                            });
                            colonist_view.add((WorkingOn, job_entity));
                            ai.assigned_job_entity = Some(job_entity.id());
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
                        if let Some(je_id) = ai.assigned_job_entity.take() {
                            if self.world.is_alive(je_id) {
                                let je = self.world.entity_from_id(je_id);
                                colonist_view.remove((WorkingOn, je));
                                je.destruct();
                            }
                        }
                        ai.assigned_job_idx = None;
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

                        // Remove Flecs WorkingOn relationship and destroy job entity
                        if let Some(je_id) = ai.assigned_job_entity.take() {
                            if self.world.is_alive(je_id) {
                                let je = self.world.entity_from_id(je_id);
                                colonist_view.remove((WorkingOn, je));
                                je.destruct();
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

            // Write back updated colonist state
            colonist_view.set(pos);
            colonist_view.set(ai);
            colonist_view.set(actor);
        }

        commands
    }

    pub fn get_actor_render_views(&self) -> Vec<ActorRenderView> {
        let mut views = Vec::new();
        let mut id = 0;
        self.world
            .each_entity::<(&Position3D, &ActorComponent)>(|entity, (pos, actor)| {
                let busy_progress = entity.try_get::<&ColonistAI>(|ai| match &ai.state {
                    ColonistState::WorkingOnJob {
                        timer,
                        total_time,
                        ..
                    } if *total_time > 0.0 => (timer / total_time).clamp(0.0, 1.0),
                    _ => 0.0,
                }).unwrap_or(0.0);

                views.push(ActorRenderView {
                    id,
                    pos: pos.pos,
                    kind: actor.kind,
                    name: actor.name.clone(),
                    carrying: actor.carrying,
                    busy_progress,
                });
                id += 1;
            });
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
        let mut job_done = false;
        for _ in 0..30 {
            let cmds = ecs.update(0.3, &world, &registry);
            for c in cmds {
                if let EngineCommand::SetBlock { pos, id, .. } = c {
                    assert_eq!(pos, target);
                    assert_eq!(id, BlockId::AIR);
                    job_done = true;
                }
            }
        }
        assert!(job_done, "Colonist should execute and complete the designated mining job");
    }

    #[test]
    fn test_flecs_relationship_and_prefabs() {
        let mut ecs = EcsManager::new();
        let colonist_id = ecs
            .spawn_colonist(WorldPos::new(0, 0, 1), "Test Dwarf".into())
            .id();
        let player_id = ecs.spawn_player(WorldPos::new(5, 5, 1)).id();

        let colonist = ecs.world().entity_from_id(colonist_id);
        let player = ecs.world().entity_from_id(player_id);

        // Colonist inherits from base_dwarf prefab
        assert!(colonist.has((flecs::IsA, ecs.base_dwarf)));
        // Player inherits from base_player prefab
        assert!(player.has((flecs::IsA, ecs.base_player)));

        // Test custom WorkingOn relationship
        let job_e = ecs.world().entity().set(JobTask {
            pos: WorldPos::new(2, 2, 1),
            job_type: JobType::Mine,
            build_block: BlockId::AIR,
            job_idx: 0,
        });

        colonist.add((WorkingOn, job_e));
        assert!(colonist.has((WorkingOn, job_e)));

        // When job entity is destructed, OnDeleteTarget should clean up the relationship
        job_e.destruct();
        assert!(!colonist.has((WorkingOn, job_e)));
    }
}
