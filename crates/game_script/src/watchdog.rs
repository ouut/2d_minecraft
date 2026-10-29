use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use steel::steel_vm::ThreadStateController;

/// Internal task description passed to the persistent watchdog daemon thread.
struct WatchdogTask {
    controller: ThreadStateController,
    deadline: Instant,
    completed: Arc<AtomicBool>,
}

enum WatchdogState {
    Idle,
    Running(WatchdogTask),
    Shutdown,
}

struct WatchdogShared {
    state: Mutex<WatchdogState>,
    cvar: Condvar,
}

/// A persistent, single-daemon-thread watchdog for script evaluation and function calls.
///
/// Completely eliminates the overhead and kernel context switching jitter of spawning a
/// new OS thread on every execution. Instead, a single background daemon thread is kept alive
/// and sleeps on a condition variable until a task deadline is registered.
///
/// If the main thread completes the task before the deadline, it marks completion via an
/// atomic flag and signals the watchdog to return to idle. If the deadline expires without
/// completion, the watchdog interrupts the VM via [`ThreadStateController::interrupt`].
pub struct PersistentWatchdog {
    shared: Arc<WatchdogShared>,
    thread: Option<JoinHandle<()>>,
}

impl Default for PersistentWatchdog {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistentWatchdog {
    /// Create and start a new `PersistentWatchdog` daemon thread.
    pub fn new() -> Self {
        let shared = Arc::new(WatchdogShared {
            state: Mutex::new(WatchdogState::Idle),
            cvar: Condvar::new(),
        });
        let thread_shared = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("game_script-watchdog".into())
            .spawn(move || Self::worker_loop(thread_shared))
            .expect("Failed to spawn persistent watchdog daemon thread");

        Self {
            shared,
            thread: Some(thread),
        }
    }

    /// Execute a closure `f` guarded by this persistent watchdog.
    ///
    /// If execution exceeds `timeout`, the watchdog interrupts the VM controller.
    /// Automatically resumes the controller upon return or unwind (panic-safe).
    pub fn watch<T, F: FnOnce() -> T>(
        &self,
        controller: &ThreadStateController,
        timeout: Duration,
        f: F,
    ) -> T {
        let completed = Arc::new(AtomicBool::new(false));
        let deadline = Instant::now() + timeout;

        // Submit the task to the persistent daemon
        {
            let mut state = self.shared.state.lock().unwrap();
            while matches!(*state, WatchdogState::Running(_)) {
                state = self.shared.cvar.wait(state).unwrap();
            }
            if matches!(*state, WatchdogState::Shutdown) {
                drop(state);
                return f();
            }
            *state = WatchdogState::Running(WatchdogTask {
                controller: controller.clone(),
                deadline,
                completed: Arc::clone(&completed),
            });
            self.shared.cvar.notify_one();
        }

        // RAII guard handles marking completion, waking up watchdog, waiting for Idle reset, and resuming controller
        let _guard = WatchGuard {
            shared: &self.shared,
            completed: Arc::clone(&completed),
            controller,
        };

        f()
    }

    /// Background worker loop for the persistent watchdog thread.
    fn worker_loop(shared: Arc<WatchdogShared>) {
        let mut state = shared.state.lock().unwrap();
        loop {
            match &*state {
                WatchdogState::Shutdown => break,
                WatchdogState::Idle => {
                    state = shared.cvar.wait(state).unwrap();
                }
                WatchdogState::Running(task) => {
                    let controller = task.controller.clone();
                    let deadline = task.deadline;
                    let completed = Arc::clone(&task.completed);

                    // Wait until task is completed or deadline is reached
                    while !completed.load(Ordering::SeqCst) {
                        let now = Instant::now();
                        if now >= deadline {
                            break;
                        }
                        let remaining = deadline - now;
                        let (next_state, wait_res) =
                            shared.cvar.wait_timeout(state, remaining).unwrap();
                        state = next_state;
                        if wait_res.timed_out() {
                            break;
                        }
                        if matches!(*state, WatchdogState::Shutdown) {
                            return;
                        }
                    }

                    // If deadline exceeded and task is still running, interrupt VM!
                    if !completed.load(Ordering::SeqCst) {
                        controller.interrupt();
                        // Wait for main thread to finish evaluation and mark completed
                        while !completed.load(Ordering::SeqCst) {
                            if matches!(*state, WatchdogState::Shutdown) {
                                return;
                            }
                            state = shared.cvar.wait(state).unwrap();
                        }
                    }

                    // Reset to Idle and notify main thread
                    *state = WatchdogState::Idle;
                    shared.cvar.notify_all();
                }
            }
        }
    }
}

struct WatchGuard<'a> {
    shared: &'a WatchdogShared,
    completed: Arc<AtomicBool>,
    controller: &'a ThreadStateController,
}

impl<'a> Drop for WatchGuard<'a> {
    fn drop(&mut self) {
        self.completed.store(true, Ordering::SeqCst);
        let mut state = self.shared.state.lock().unwrap();
        self.shared.cvar.notify_all();
        while matches!(*state, WatchdogState::Running(_)) {
            state = self.shared.cvar.wait(state).unwrap();
        }
        self.controller.resume();
    }
}

impl Drop for PersistentWatchdog {
    fn drop(&mut self) {
        {
            let mut state = self.shared.state.lock().unwrap();
            *state = WatchdogState::Shutdown;
            self.shared.cvar.notify_all();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use steel::steel_vm::engine::Engine;

    #[test]
    fn test_persistent_watchdog_reuse() {
        let watchdog = PersistentWatchdog::new();
        let mut engine = Engine::new();
        let controller = engine.get_thread_state_controller();

        // Run 50 sequential tasks rapidly through the same persistent watchdog
        for i in 0..50 {
            let res = watchdog.watch(&controller, Duration::from_millis(200), || {
                engine.run(format!("(+ {} 1)", i))
            });
            assert!(res.is_ok());
        }
    }

    #[test]
    fn test_persistent_watchdog_timeout_and_recover() {
        let watchdog = PersistentWatchdog::new();
        let mut engine = Engine::new();
        let controller = engine.get_thread_state_controller();

        // 1. Timeout on infinite loop
        let start = Instant::now();
        let res = watchdog.watch(&controller, Duration::from_millis(50), || {
            engine.run("(let loop () (loop))")
        });
        assert!(res.is_err());
        assert!(start.elapsed() < Duration::from_millis(500));

        // 2. Immediately reuse the same watchdog and engine
        let res_after = watchdog.watch(&controller, Duration::from_millis(100), || {
            engine.run("(+ 10 20)")
        });
        assert!(res_after.is_ok());
    }
}
