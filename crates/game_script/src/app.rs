use std::fmt;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use steel::rvals::SteelVal;
use steel::steel_vm::engine::Engine;

use crate::command::{AnyCommand, CommandSink};
use crate::plugin::{PluginContext, PluginError, ScriptPlugin};
use crate::watcher::ScriptWatcher;

/// Errors produced during script execution, evaluation, or plugin management.
#[derive(Debug)]
pub enum ScriptError {
    /// Evaluation or runtime error from the Scheme VM engine.
    Engine(String),
    /// Execution was terminated by the watchdog timer due to exceeding allowed execution time.
    Timeout(u64),
    /// File system or IO error when loading script files.
    Io(std::io::Error),
    /// Plugin initialization or binding error.
    Plugin(PluginError),
    /// General script error.
    Other(String),
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(e) => write!(f, "Script engine error: {}", e),
            Self::Timeout(ms) => write!(f, "Script execution timed out after {} ms", ms),
            Self::Io(e) => write!(f, "Script IO error: {}", e),
            Self::Plugin(e) => write!(f, "Script plugin error: {}", e),
            Self::Other(e) => write!(f, "Script error: {}", e),
        }
    }
}

impl std::error::Error for ScriptError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Plugin(e) => Some(e),
            _ => None,
        }
    }
}

impl From<steel::rerrs::SteelErr> for ScriptError {
    fn from(e: steel::rerrs::SteelErr) -> Self {
        Self::Engine(e.to_string())
    }
}

impl From<PluginError> for ScriptError {
    fn from(e: PluginError) -> Self {
        Self::Plugin(e)
    }
}

impl From<std::io::Error> for ScriptError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Core host container for the universal Scheme scripting engine.
///
/// Designed with a Bevy-style Builder pattern for modular plugin mounting,
/// thread-safe transactional command dispatching, REPL evaluation, and watchdog-protected execution.
pub struct ScriptApp {
    pub engine: Engine,
    pub sink: CommandSink,
    pub watcher: Option<ScriptWatcher>,
    pub plugins: Vec<&'static str>,
}

impl Default for ScriptApp {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptApp {
    /// Create a new `ScriptApp` with an empty VM engine and command sink.
    pub fn new() -> Self {
        Self {
            engine: Engine::new(),
            sink: CommandSink::new(),
            watcher: None,
            plugins: Vec::new(),
        }
    }

    /// Add and build a script plugin (Bevy-style builder pattern).
    ///
    /// Allows method chaining: `app.add_plugin(P1)?.add_plugin(P2)?`.
    pub fn add_plugin(&mut self, plugin: impl ScriptPlugin) -> Result<&mut Self, PluginError> {
        let mut ctx = PluginContext::new(&mut self.engine, &self.sink);
        plugin.build(&mut ctx)?;
        self.plugins.push(plugin.name());
        Ok(self)
    }

    /// Set or configure the script directory watcher.
    pub fn watch_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.watcher = Some(ScriptWatcher::new(dir));
        self
    }

    /// Attach a pre-configured [`ScriptWatcher`].
    pub fn set_watcher(&mut self, watcher: ScriptWatcher) -> &mut Self {
        self.watcher = Some(watcher);
        self
    }

    /// Run a Scheme script string and return the resulting values.
    pub fn run(&mut self, script: &str) -> Result<Vec<SteelVal>, ScriptError> {
        self.engine
            .run(script.to_string())
            .map_err(|e| ScriptError::Engine(e.to_string()))
    }

    /// Evaluate an expression in a REPL style, returning formatted output string.
    pub fn eval_repl(&mut self, expr: &str) -> Result<String, ScriptError> {
        let vals = self
            .engine
            .run(expr.to_string())
            .map_err(|e| ScriptError::Engine(e.to_string()))?;
        if vals.is_empty() {
            Ok("=> ()".to_string())
        } else {
            let out = vals
                .iter()
                .map(|v| format!("{}", v))
                .collect::<Vec<_>>()
                .join(" ");
            Ok(format!("=> {}", out))
        }
    }

    /// Safely evaluate an expression with a timeout watchdog to prevent infinite loops.
    ///
    /// Spawns a background timer watchdog using Steel's [`steel::steel_vm::ThreadStateController`].
    /// If evaluation exceeds `timeout_ms`, the VM execution is interrupted and [`ScriptError::Timeout`] is returned.
    pub fn safe_eval(&mut self, expr: &str, timeout_ms: u64) -> Result<String, ScriptError> {
        let controller = self.engine.get_thread_state_controller();
        let (tx, rx) = mpsc::channel();
        let controller_clone = controller.clone();

        let watchdog = std::thread::spawn(move || {
            if let Err(mpsc::RecvTimeoutError::Timeout) =
                rx.recv_timeout(Duration::from_millis(timeout_ms))
            {
                controller_clone.interrupt();
            }
        });

        let eval_res = self.eval_repl(expr);
        let _ = tx.send(());
        let _ = watchdog.join();
        controller.resume();

        match eval_res {
            Ok(out) => Ok(out),
            Err(err) => {
                let err_str = err.to_string();
                if err_str.contains("Interrupted") {
                    Err(ScriptError::Timeout(timeout_ms))
                } else {
                    Err(err)
                }
            }
        }
    }

    /// Drain all transactional commands currently waiting in the command sink.
    pub fn drain_commands(&self) -> Vec<Box<dyn AnyCommand>> {
        self.sink.drain()
    }

    /// Check if watched script files have been modified.
    /// If modified, automatically reloads and re-executes all tracked `.scm` files in deterministic order.
    ///
    /// Returns `Ok(true)` if scripts were reloaded, `Ok(false)` if no changes were detected.
    pub fn check_reload(&mut self) -> Result<bool, ScriptError> {
        if let Some(ref mut watcher) = self.watcher {
            if watcher.check_modified() {
                let files = watcher.tracked_files();
                for file in files {
                    let code = std::fs::read_to_string(&file).map_err(ScriptError::Io)?;
                    self.run(&code)?;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Access the command sink reference.
    pub fn sink(&self) -> &CommandSink {
        &self.sink
    }

    /// Access the mutable VM engine reference.
    pub fn engine(&mut self) -> &mut Engine {
        &mut self.engine
    }

    /// List registered plugin names.
    pub fn plugins(&self) -> &[&'static str] {
        &self.plugins
    }
}
