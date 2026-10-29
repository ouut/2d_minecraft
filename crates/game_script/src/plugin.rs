use std::fmt;
use steel::steel_vm::engine::Engine;
use crate::command::CommandSink;

/// Error returned when a plugin fails to initialize or build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginError(pub String);

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PluginError: {}", self.0)
    }
}

impl std::error::Error for PluginError {}

impl From<&str> for PluginError {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for PluginError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Context passed to plugins during initialization.
///
/// Holds references to the Steel Scheme VM [`Engine`] and the transactional [`CommandSink`].
pub struct PluginContext<'a> {
    pub engine: &'a mut Engine,
    pub sink: &'a CommandSink,
}

impl<'a> PluginContext<'a> {
    /// Create a new plugin context.
    pub fn new(engine: &'a mut Engine, sink: &'a CommandSink) -> Self {
        Self { engine, sink }
    }

    /// Mutable reference to the Steel Scheme VM engine.
    pub fn engine(&mut self) -> &mut Engine {
        self.engine
    }

    /// Reference to the command sink bus.
    pub fn sink(&self) -> &CommandSink {
        self.sink
    }
}

/// Trait defining a game script plugin (Bevy-style builder pattern).
///
/// Implementations register native Rust functions, types, and hooks into the VM
/// and configure domain-specific command handlers.
pub trait ScriptPlugin: Send + Sync + 'static {
    /// Unique name of the plugin for diagnostics and dependency tracking.
    fn name(&self) -> &'static str;

    /// Mount and configure native capabilities into the VM and command sink.
    fn build(&self, ctx: &mut PluginContext) -> Result<(), PluginError>;
}

impl<F> ScriptPlugin for (&'static str, F)
where
    F: Fn(&mut PluginContext) -> Result<(), PluginError> + Send + Sync + 'static,
{
    fn name(&self) -> &'static str {
        self.0
    }

    fn build(&self, ctx: &mut PluginContext) -> Result<(), PluginError> {
        (self.1)(ctx)
    }
}
