use std::any::Any;
use std::sync::Arc;
use crossbeam_queue::SegQueue;

/// Contract for arbitrary transactional commands produced by scripts and plugins.
pub trait AnyCommand: Any + Send + Sync + 'static {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any + Send + Sync + 'static> AnyCommand for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl dyn AnyCommand {
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}

/// A non-blocking, lock-free transactional command bus.
///
/// Scripts and native plugins emit domain commands implementing [`AnyCommand`]
/// into the sink without locking or borrowing conflicts.
/// The host game loop or engine drains and dispatches them each frame/tick.
#[derive(Clone, Default)]
pub struct CommandSink {
    queue: Arc<SegQueue<Box<dyn AnyCommand>>>,
}

impl CommandSink {
    /// Create a new empty command sink.
    pub fn new() -> Self {
        Self {
            queue: Arc::new(SegQueue::new()),
        }
    }

    /// Emit a command into the sink.
    /// This operation is non-blocking and lock-free.
    pub fn emit<C: AnyCommand>(&self, cmd: C) {
        self.queue.push(Box::new(cmd));
    }

    /// Drain and return all pending commands from the sink.
    /// This operation is non-blocking and removes all commands currently in the queue.
    pub fn drain(&self) -> Vec<Box<dyn AnyCommand>> {
        let mut cmds = Vec::new();
        while let Some(cmd) = self.queue.pop() {
            cmds.push(cmd);
        }
        cmds
    }

    /// Returns `true` if the sink currently contains no commands.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Returns the number of commands currently queued.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Clear all pending commands without processing them.
    pub fn clear(&self) {
        while self.queue.pop().is_some() {}
    }
}

impl std::fmt::Debug for CommandSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandSink")
            .field("len", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct TestSpawnCommand {
        entity_id: u32,
        name: String,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct TestLogCommand(String);

    #[test]
    fn test_command_sink_emit_and_drain() {
        let sink = CommandSink::new();
        assert!(sink.is_empty());
        assert_eq!(sink.len(), 0);

        sink.emit(TestSpawnCommand {
            entity_id: 101,
            name: "Goblin".into(),
        });
        sink.emit(TestLogCommand("Spawned goblin".into()));

        assert_eq!(sink.len(), 2);
        assert!(!sink.is_empty());

        let cmds = sink.drain();
        assert_eq!(cmds.len(), 2);
        assert!(sink.is_empty());

        // Test downcasting
        let cmd0 = cmds[0].downcast_ref::<TestSpawnCommand>().unwrap();
        assert_eq!(cmd0.entity_id, 101);
        assert_eq!(cmd0.name, "Goblin");

        let cmd1 = cmds[1].as_ref().as_any().downcast_ref::<TestLogCommand>().unwrap();
        assert_eq!(cmd1.0, "Spawned goblin");
    }

    #[test]
    fn test_command_sink_multithreaded() {
        let sink = CommandSink::new();
        let mut handles = Vec::new();

        for i in 0..10 {
            let s = sink.clone();
            handles.push(std::thread::spawn(move || {
                for j in 0..100 {
                    s.emit(TestSpawnCommand {
                        entity_id: i * 1000 + j,
                        name: format!("Entity_{}_{}", i, j),
                    });
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let drained = sink.drain();
        assert_eq!(drained.len(), 1000);
        assert!(sink.is_empty());
    }
}
