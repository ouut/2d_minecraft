use voxel_core::block::BlockId;
use voxel_core::coords::WorldPos;
use voxel_core::traits::{JobDesignation, JobRenderView, JobType};

#[derive(Clone, Debug, Default)]
pub struct JobManager {
    designations: Vec<JobDesignation>,
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            designations: Vec::new(),
        }
    }

    pub fn designate_box(
        &mut self,
        min: WorldPos,
        max: WorldPos,
        job_type: JobType,
        build_block: BlockId,
    ) {
        let x_min = min.x.min(max.x);
        let x_max = min.x.max(max.x);
        let y_min = min.y.min(max.y);
        let y_max = min.y.max(max.y);
        let z_min = min.z.min(max.z);
        let z_max = min.z.max(max.z);

        for x in x_min..=x_max {
            for y in y_min..=y_max {
                for z in z_min..=z_max {
                    let pos = WorldPos::new(x, y, z);
                    if !self.has_job_at(pos) {
                        self.designations.push(JobDesignation {
                            pos,
                            job_type,
                            build_block,
                        });
                    }
                }
            }
        }
    }

    pub fn has_job_at(&self, pos: WorldPos) -> bool {
        self.designations.iter().any(|j| j.pos == pos)
    }

    pub fn remove_job_at(&mut self, pos: WorldPos) {
        self.designations.retain(|j| j.pos != pos);
    }

    pub fn designations(&self) -> &[JobDesignation] {
        &self.designations
    }

    pub fn find_closest_job(&self, pos: WorldPos) -> Option<(usize, JobDesignation)> {
        let mut best: Option<(usize, i32)> = None;

        for (idx, job) in self.designations.iter().enumerate() {
            let dist = pos.manhattan_dist(job.pos);
            if let Some((_, best_dist)) = best {
                if dist < best_dist {
                    best = Some((idx, dist));
                }
            } else {
                best = Some((idx, dist));
            }
        }

        best.map(|(idx, _)| (idx, self.designations[idx].clone()))
    }

    pub fn complete_job(&mut self, idx: usize) -> Option<JobDesignation> {
        if idx < self.designations.len() {
            Some(self.designations.swap_remove(idx))
        } else {
            None
        }
    }

    pub fn get_job_render_views(&self) -> Vec<JobRenderView> {
        self.designations
            .iter()
            .map(|j| JobRenderView {
                pos: j.pos,
                job_type: j.job_type,
                build_block: j.build_block,
            })
            .collect()
    }
}
