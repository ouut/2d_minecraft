use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use steel::rvals::SteelVal;
use steel::steel_vm::engine::Engine;

use crate::command::{AnyCommand, CommandSink};
use crate::interop::{json_to_steel_val, steel_val_to_json, InteropError};
use crate::plugin::{PluginContext, PluginError, ScriptPlugin};
use crate::watchdog::PersistentWatchdog;
use crate::watcher::ScriptWatcher;

/// Errors produced during script execution, evaluation, interop, or plugin management.
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
    /// Interop data conversion error between SteelVal and JSON.
    Interop(InteropError),
    /// JSON serialization or deserialization error.
    Serialization(String),
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
            Self::Interop(e) => write!(f, "Script interop error: {}", e),
            Self::Serialization(e) => write!(f, "Script serialization error: {}", e),
            Self::Other(e) => write!(f, "Script error: {}", e),
        }
    }
}

impl std::error::Error for ScriptError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Plugin(e) => Some(e),
            Self::Interop(e) => Some(e),
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

impl From<InteropError> for ScriptError {
    fn from(e: InteropError) -> Self {
        Self::Interop(e)
    }
}

impl From<serde_json::Error> for ScriptError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e.to_string())
    }
}

/// Core host container for the universal Scheme scripting engine.
///
/// Designed with:
/// - **Dual-Paradigm Support**:
///   - *Paradigm A (ECS-driven)*: Bevy-style modular plugins emitting to lock-free [`CommandSink`].
///   - *Paradigm B (Scheme-Authoritative)*: Scheme controls world state; Rust main loop drives
///     ticks via [`call_function`](Self::call_function) and extracts snapshots.
/// - **Precompiled Function Calling**: Direct function pointer invocation without string parsing or compilation.
/// - **Single Daemon Watchdog**: [`PersistentWatchdog`] preventing infinite loops with zero OS thread recreation.
/// - **Full World State Serialization**: JSON-based dump & restore via [`dump_state_json`](Self::dump_state_json)
///   and [`load_state_json`](Self::load_state_json).
/// - **Module Load Paths**: Search directory stack for multi-file `require` and `load`.
pub struct ScriptApp {
    pub engine: Engine,
    pub sink: CommandSink,
    pub watcher: Option<ScriptWatcher>,
    pub plugins: Vec<&'static str>,
    watchdog: PersistentWatchdog,
    load_paths: Vec<PathBuf>,
}

impl Default for ScriptApp {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptApp {
    /// Create a new `ScriptApp` with an empty VM engine, command sink, and persistent watchdog.
    pub fn new() -> Self {
        Self {
            engine: Engine::new(),
            sink: CommandSink::new(),
            watcher: None,
            plugins: Vec::new(),
            watchdog: PersistentWatchdog::new(),
            load_paths: Vec::new(),
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

    /// Append a directory to the module search path stack for relative `require` and `load`.
    pub fn with_load_path(&mut self, path: impl AsRef<Path>) -> &mut Self {
        let p = path.as_ref().to_path_buf();
        self.engine.add_search_directory(p.clone());
        self.load_paths.push(p);
        self
    }

    /// Get the configured module search path stack.
    pub fn load_paths(&self) -> &[PathBuf] {
        &self.load_paths
    }

    /// Resolve a script file path against the configured search paths.
    pub fn resolve_path(&self, path: impl AsRef<Path>) -> Option<PathBuf> {
        let p = path.as_ref();
        if p.is_absolute() && p.exists() {
            return Some(p.to_path_buf());
        }
        if p.exists() {
            return Some(p.to_path_buf());
        }
        for base in &self.load_paths {
            let candidate = base.join(p);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        None
    }

    /// Run a Scheme script file, resolving its path against load paths and compiling with file context.
    pub fn run_file(&mut self, path: impl AsRef<Path>) -> Result<Vec<SteelVal>, ScriptError> {
        let p = path.as_ref();
        let resolved = self.resolve_path(p).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Script file not found: {}", p.display()),
            )
        })?;
        let code = std::fs::read_to_string(&resolved)?;
        self.engine
            .compile_and_run_raw_program_with_path(code, resolved)
            .map_err(|e| ScriptError::Engine(e.to_string()))
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

    /// Call a precompiled Scheme function by name with native [`SteelVal`] arguments.
    ///
    /// Executes directly against the VM internal function pointer, completely bypassing
    /// the AST compilation, string formatting, macro expansion, and lexer/parser stages.
    pub fn call_function(
        &mut self,
        func_name: &str,
        args: Vec<SteelVal>,
    ) -> Result<SteelVal, ScriptError> {
        self.engine
            .call_function_by_name_with_args(func_name, args)
            .map_err(|e| ScriptError::Engine(e.to_string()))
    }

    /// Safely call a precompiled Scheme function by name with a timeout watchdog.
    ///
    /// Guarded by the persistent single-daemon-thread watchdog.
    /// Returns [`ScriptError::Timeout`] if execution exceeds `timeout_ms`.
    pub fn safe_call_function(
        &mut self,
        func_name: &str,
        args: Vec<SteelVal>,
        timeout_ms: u64,
    ) -> Result<SteelVal, ScriptError> {
        let controller = self.engine.get_thread_state_controller();
        let engine = &mut self.engine;
        let res = self.watchdog.watch(
            &controller,
            Duration::from_millis(timeout_ms),
            || {
                engine
                    .call_function_by_name_with_args(func_name, args)
                    .map_err(|e| ScriptError::Engine(e.to_string()))
            },
        );

        match res {
            Ok(val) => Ok(val),
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

    /// Safely evaluate an expression with a timeout watchdog to prevent infinite loops.
    ///
    /// Monitored by the persistent single-daemon-thread watchdog.
    /// If evaluation exceeds `timeout_ms`, the VM execution is interrupted and [`ScriptError::Timeout`] is returned.
    pub fn safe_eval(&mut self, expr: &str, timeout_ms: u64) -> Result<String, ScriptError> {
        let controller = self.engine.get_thread_state_controller();
        let engine = &mut self.engine;
        let eval_res: Result<String, ScriptError> = self.watchdog.watch(
            &controller,
            Duration::from_millis(timeout_ms),
            || {
                let vals = engine
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
            },
        );

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

    /// Extract an identifier's bound value directly from the global VM environment.
    pub fn extract_value(&self, name: &str) -> Result<SteelVal, ScriptError> {
        self.engine
            .extract_value(name)
            .map_err(|e| ScriptError::Engine(e.to_string()))
    }

    /// Drive a Scheme-Authoritative game tick.
    ///
    /// Calls the Scheme function `func_name` (e.g. `"tick"` or `"update!"`) with `dt` (seconds).
    /// Returns the resulting [`SteelVal`].
    pub fn tick(&mut self, func_name: &str, dt: f64) -> Result<SteelVal, ScriptError> {
        self.call_function(func_name, vec![SteelVal::NumV(dt)])
    }

    /// Extract a state snapshot from Scheme as a JSON value.
    ///
    /// Can evaluate a variable name (e.g. `"game-state"`) or an expression (e.g. `"(get-state)"`).
    pub fn extract_snapshot(&mut self, expr: &str) -> Result<serde_json::Value, ScriptError> {
        self.dump_state_json(expr)
    }

    /// Dump state evaluated by `expr` to a [`serde_json::Value`].
    ///
    /// Evaluates `expr` in the VM and losslessly converts the resulting [`SteelVal`] to JSON.
    pub fn dump_state_json(&mut self, expr: &str) -> Result<serde_json::Value, ScriptError> {
        let vals = self
            .engine
            .run(expr.to_string())
            .map_err(|e| ScriptError::Engine(e.to_string()))?;
        let val = vals.into_iter().last().unwrap_or(SteelVal::Void);
        steel_val_to_json(&val).map_err(ScriptError::from)
    }

    /// Load state into a Scheme global variable `var_name` from a [`serde_json::Value`].
    ///
    /// Converts JSON into the corresponding Scheme [`SteelVal`] hierarchy and registers/updates
    /// the global binding in the VM environment.
    pub fn load_state_json(
        &mut self,
        var_name: &str,
        json: &serde_json::Value,
    ) -> Result<(), ScriptError> {
        let val = json_to_steel_val(json);
        self.engine.register_value(var_name, val);
        Ok(())
    }

    /// Dump state evaluated by `expr` directly to a JSON file on disk.
    pub fn dump_state_to_file(
        &mut self,
        expr: &str,
        path: impl AsRef<Path>,
    ) -> Result<(), ScriptError> {
        let json = self.dump_state_json(expr)?;
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let file = std::fs::File::create(p)?;
        serde_json::to_writer_pretty(file, &json)?;
        Ok(())
    }

    /// Load state from a JSON file on disk into a Scheme global variable `var_name`.
    pub fn load_state_from_file(
        &mut self,
        var_name: &str,
        path: impl AsRef<Path>,
    ) -> Result<(), ScriptError> {
        let p = path.as_ref();
        let content = std::fs::read_to_string(p)?;
        let json: serde_json::Value = serde_json::from_str(&content)?;
        self.load_state_json(var_name, &json)
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
                    self.engine
                        .compile_and_run_raw_program_with_path(code, file)
                        .map_err(|e| ScriptError::Engine(e.to_string()))?;
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

    /// Access the persistent watchdog reference.
    pub fn watchdog(&self) -> &PersistentWatchdog {
        &self.watchdog
    }

    /// List registered plugin names.
    pub fn plugins(&self) -> &[&'static str] {
        &self.plugins
    }
}
