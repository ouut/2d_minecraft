use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use voxel_core::block::{BlockId, BlockRegistry};
use voxel_core::coords::WorldPos;
use voxel_core::traits::WorldAccess;

#[derive(Copy, Clone, Eq, PartialEq)]
struct Node {
    pos: WorldPos,
    f_cost: i32,
    g_cost: i32,
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap
        other.f_cost.cmp(&self.f_cost)
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn is_passable(world: &dyn WorldAccess, registry: &BlockRegistry, pos: WorldPos) -> bool {
    if !world.in_bounds(pos.z) {
        return false;
    }
    let v = world.get_voxel(pos);
    if v.id.is_air()
        || v.id == BlockId::TORCH
        || v.id == BlockId::LADDER
        || v.id == BlockId::RAMP
        || v.id == BlockId::STAIR
    {
        true
    } else if registry.is_liquid(v.id) {
        // Shallow liquid is passable
        v.meta < 4
    } else {
        !registry.is_solid(v.id)
    }
}

pub fn is_standable(world: &dyn WorldAccess, registry: &BlockRegistry, pos: WorldPos) -> bool {
    if !is_passable(world, registry, pos) {
        return false;
    }
    let below = pos.offset(0, 0, -1);
    if !world.in_bounds(below.z) {
        return false;
    }
    let below_v = world.get_voxel(below);
    registry.is_solid(below_v.id)
        || below_v.id == BlockId::RAMP
        || below_v.id == BlockId::STAIR
        || below_v.id == BlockId::LADDER
}

pub fn find_path_3d(
    world: &dyn WorldAccess,
    registry: &BlockRegistry,
    start: WorldPos,
    goal: WorldPos,
    max_nodes: usize,
) -> Option<Vec<WorldPos>> {
    if start == goal {
        return Some(vec![start]);
    }

    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<WorldPos, WorldPos> = HashMap::new();
    let mut g_score: HashMap<WorldPos, i32> = HashMap::new();

    let heuristic = |p: WorldPos| -> i32 {
        (p.x - goal.x).abs() * 10 + (p.y - goal.y).abs() * 10 + (p.z - goal.z).abs() * 14
    };

    g_score.insert(start, 0);
    open_set.push(Node {
        pos: start,
        f_cost: heuristic(start),
        g_cost: 0,
    });

    let mut nodes_evaluated = 0;

    while let Some(current) = open_set.pop() {
        nodes_evaluated += 1;
        if nodes_evaluated > max_nodes {
            break;
        }

        if current.pos == goal || current.pos.manhattan_dist(goal) <= 1 {
            // Reconstruct path
            let mut path = Vec::new();
            let mut curr = current.pos;
            path.push(curr);
            while let Some(&prev) = came_from.get(&curr) {
                path.push(prev);
                curr = prev;
            }
            path.reverse();
            return Some(path);
        }

        let curr_g = match g_score.get(&current.pos) {
            Some(&g) => g,
            None => continue,
        };
        if current.g_cost > curr_g {
            continue;
        }

        // Neighbors exploration
        // 1. Same Z (cardinal 4 directions + diagonals)
        let same_z_moves = [
            (1, 0, 10),
            (-1, 0, 10),
            (0, 1, 10),
            (0, -1, 10),
            (1, 1, 14),
            (-1, 1, 14),
            (1, -1, 14),
            (-1, -1, 14),
        ];

        for (dx, dy, cost) in same_z_moves {
            let next_p = current.pos.offset(dx, dy, 0);
            if dx != 0 && dy != 0 {
                // Diagonal corner-cutting check
                if !is_passable(world, registry, current.pos.offset(dx, 0, 0))
                    || !is_passable(world, registry, current.pos.offset(0, dy, 0))
                {
                    continue;
                }
            }

            if is_standable(world, registry, next_p) {
                let tentative_g = curr_g + cost;
                if tentative_g < *g_score.get(&next_p).unwrap_or(&i32::MAX) {
                    g_score.insert(next_p, tentative_g);
                    came_from.insert(next_p, current.pos);
                    open_set.push(Node {
                        pos: next_p,
                        f_cost: tentative_g + heuristic(next_p),
                        g_cost: tentative_g,
                    });
                }
            }
        }

        // 2. Up Z (z -> z+1) via Ramp or Stair/Ladder
        let up_moves = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
        ];

        let curr_v = world.get_voxel(current.pos);
        for (dx, dy) in up_moves {
            let next_up = current.pos.offset(dx, dy, 1);
            let target_stand = current.pos.offset(dx, dy, 0);
            let target_stand_v = world.get_voxel(target_stand);

            let can_climb = target_stand_v.id == BlockId::RAMP
                || curr_v.id == BlockId::STAIR
                || curr_v.id == BlockId::LADDER
                || target_stand_v.id == BlockId::STAIR;

            if can_climb && is_standable(world, registry, next_up) {
                let tentative_g = curr_g + 18;
                if tentative_g < *g_score.get(&next_up).unwrap_or(&i32::MAX) {
                    g_score.insert(next_up, tentative_g);
                    came_from.insert(next_up, current.pos);
                    open_set.push(Node {
                        pos: next_up,
                        f_cost: tentative_g + heuristic(next_up),
                        g_cost: tentative_g,
                    });
                }
            }
        }

        // 3. Down Z (z -> z-1) via Ramp/Stair or safe drop 1 tile
        for (dx, dy) in up_moves {
            let next_down = current.pos.offset(dx, dy, -1);
            if is_standable(world, registry, next_down) {
                // Must have head clearance at target level z
                let head_clear = is_passable(world, registry, current.pos.offset(dx, dy, 0));
                if head_clear {
                    let tentative_g = curr_g + 12;
                    if tentative_g < *g_score.get(&next_down).unwrap_or(&i32::MAX) {
                        g_score.insert(next_down, tentative_g);
                        came_from.insert(next_down, current.pos);
                        open_set.push(Node {
                            pos: next_down,
                            f_cost: tentative_g + heuristic(next_down),
                            g_cost: tentative_g,
                        });
                    }
                }
            }
        }
    }

    None
}
