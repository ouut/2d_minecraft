//! Concurrent ingestion pipeline providing a Latest-Only Latch for high-frequency input
//! and a strictly ordered FIFO Command Queue for discrete gameplay actions.

use std::collections::VecDeque;
use std::sync::Arc;

use dashmap::DashMap;
use game_net::PlayerId;

/// Generic ingestion pipeline decoupled from specific game definitions.
///
/// Handles:
/// 1. `Latest-Only Latch`: High-frequency player movement/directional input.
///    Pushes overwrite previous unconsumed inputs for the same player, preventing input lag/queuing.
/// 2. `FIFO Command Queue`: Discrete, reliable player actions (e.g., block placements, chat, combat).
///    Pushes are queued in strict arrival order and drained together per tick.
#[derive(Debug)]
pub struct IngestionPipeline<TInput, TCmd> {
    inputs: Arc<DashMap<PlayerId, TInput>>,
    commands: Arc<DashMap<PlayerId, VecDeque<TCmd>>>,
}

impl<TInput, TCmd> Clone for IngestionPipeline<TInput, TCmd> {
    fn clone(&self) -> Self {
        Self {
            inputs: Arc::clone(&self.inputs),
            commands: Arc::clone(&self.commands),
        }
    }
}

impl<TInput, TCmd> Default for IngestionPipeline<TInput, TCmd> {
    fn default() -> Self {
        Self::new()
    }
}

impl<TInput, TCmd> IngestionPipeline<TInput, TCmd> {
    /// Creates an empty `IngestionPipeline`.
    pub fn new() -> Self {
        Self {
            inputs: Arc::new(DashMap::new()),
            commands: Arc::new(DashMap::new()),
        }
    }

    /// Latches the latest input for a player, overwriting any previous unconsumed frame.
    pub fn push_input(&self, player_id: PlayerId, input: TInput) {
        self.inputs.insert(player_id, input);
    }

    /// Pushes a discrete command to the end of the player's FIFO queue.
    pub fn push_command(&self, player_id: PlayerId, cmd: TCmd) {
        let mut entry = self.commands.entry(player_id).or_default();
        entry.push_back(cmd);
    }

    /// Pushes multiple commands into the player's FIFO queue in iteration order.
    pub fn push_commands<I>(&self, player_id: PlayerId, cmds: I)
    where
        I: IntoIterator<Item = TCmd>,
    {
        let mut entry = self.commands.entry(player_id).or_default();
        for cmd in cmds {
            entry.push_back(cmd);
        }
    }

    /// Atomically drains all latest inputs for all players, clearing the latch.
    ///
    /// Intended to be called once per server tick by the game loop.
    pub fn drain_inputs(&self) -> Vec<(PlayerId, TInput)> {
        let keys: Vec<PlayerId> = self.inputs.iter().map(|entry| *entry.key()).collect();
        let mut result = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some((pid, val)) = self.inputs.remove(&key) {
                result.push((pid, val));
            }
        }
        result
    }

    /// Atomically drains all queued commands for all players, preserving FIFO order.
    ///
    /// Intended to be called once per server tick by the game loop.
    pub fn drain_commands(&self) -> Vec<(PlayerId, Vec<TCmd>)> {
        let keys: Vec<PlayerId> = self.commands.iter().map(|entry| *entry.key()).collect();
        let mut result = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some((pid, mut queue)) = self.commands.remove(&key) {
                if !queue.is_empty() {
                    result.push((pid, queue.drain(..).collect()));
                }
            }
        }
        result
    }

    /// Returns the number of players with pending inputs.
    pub fn pending_input_count(&self) -> usize {
        self.inputs.len()
    }

    /// Returns the number of commands currently queued for a given player.
    pub fn command_count_for_player(&self, player_id: PlayerId) -> usize {
        self.commands
            .get(&player_id)
            .map(|q| q.len())
            .unwrap_or(0)
    }

    /// Removes all pending inputs and commands for a given player.
    pub fn remove_player(&self, player_id: PlayerId) {
        self.inputs.remove(&player_id);
        self.commands.remove(&player_id);
    }

    /// Clears all pending inputs and commands across all players.
    pub fn clear(&self) {
        self.inputs.clear();
        self.commands.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestInput {
        dx: f32,
        dy: f32,
        seq: u32,
    }

    #[derive(Debug, Clone, PartialEq)]
    enum TestCmd {
        Jump,
        Mine { x: i32, y: i32 },
        Chat(String),
    }

    #[test]
    fn test_latest_only_latch() {
        let pipeline: IngestionPipeline<TestInput, TestCmd> = IngestionPipeline::new();

        // Player 1 sends 3 high frequency movement inputs in a single tick window
        pipeline.push_input(1, TestInput { dx: 1.0, dy: 0.0, seq: 1 });
        pipeline.push_input(1, TestInput { dx: 2.0, dy: 0.0, seq: 2 });
        pipeline.push_input(1, TestInput { dx: 3.0, dy: 0.5, seq: 3 });

        // Player 2 sends 1 input
        pipeline.push_input(2, TestInput { dx: -1.0, dy: -1.0, seq: 10 });

        let drained = pipeline.drain_inputs();
        assert_eq!(drained.len(), 2);

        let p1_input = drained.iter().find(|(pid, _)| *pid == 1).map(|(_, inp)| inp).unwrap();
        // Crucial: Only the latest seq 3 must be retained, seq 1 and 2 discarded!
        assert_eq!(p1_input.seq, 3);
        assert_eq!(p1_input.dx, 3.0);

        let p2_input = drained.iter().find(|(pid, _)| *pid == 2).map(|(_, inp)| inp).unwrap();
        assert_eq!(p2_input.seq, 10);

        // After drain, pipeline latch should be completely empty
        assert!(pipeline.drain_inputs().is_empty());
    }

    #[test]
    fn test_fifo_command_queue() {
        let pipeline: IngestionPipeline<TestInput, TestCmd> = IngestionPipeline::new();

        // Player 1 performs 3 distinct discrete commands
        pipeline.push_command(1, TestCmd::Jump);
        pipeline.push_command(1, TestCmd::Mine { x: 10, y: 20 });
        pipeline.push_command(1, TestCmd::Chat("Hello".into()));

        // Drain commands
        let drained = pipeline.drain_commands();
        assert_eq!(drained.len(), 1);

        let (pid, cmds) = &drained[0];
        assert_eq!(*pid, 1);
        assert_eq!(
            cmds,
            &vec![
                TestCmd::Jump,
                TestCmd::Mine { x: 10, y: 20 },
                TestCmd::Chat("Hello".into()),
            ]
        );

        // Subsequent drain is empty
        assert!(pipeline.drain_commands().is_empty());
    }
}
