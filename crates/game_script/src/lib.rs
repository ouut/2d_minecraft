//! # Game Script - Universal Two-Tier Scheme Scripting Engine & Plugin System
//!
//! Universal Layer 0/1 scripting framework across 2D Minecraft, LispCraft, and NoCraft.
//! Zero game-specific voxel logic, providing:
//! - Dual-Paradigm support:
//!   - Paradigm A: ECS-driven transactional [`CommandSink`]
//!   - Paradigm B: Scheme-Authoritative state tick & snapshot extraction
//! - Precompiled native function calling with [`ScriptApp::call_function`] & [`ScriptApp::safe_call_function`]
//! - Single persistent daemon watchdog [`PersistentWatchdog`] eliminating runtime thread creation
//! - Bevy-style modular [`ScriptPlugin`] architecture
//! - Bidirectional lossless [`SteelVal`] <-> JSON serialization and file save/load
//! - Load search directory stack [`ScriptApp::with_load_path`] for multi-file `require`/`load`
//! - Recursive `.scm` file modification watcher [`ScriptWatcher`]

pub mod app;
pub mod command;
pub mod interop;
pub mod plugin;
pub mod watchdog;
pub mod watcher;

// Re-export core types and traits
pub use app::{ScriptApp, ScriptError};
pub use command::{AnyCommand, CommandSink};
pub use interop::{json_to_steel_val, steel_val_to_json, InteropError};
pub use plugin::{PluginContext, PluginError, ScriptPlugin};
pub use watchdog::PersistentWatchdog;
pub use watcher::ScriptWatcher;

// Convenient re-exports of Steel Scheme VM types
pub use steel::rvals::SteelVal;
pub use steel::steel_vm::engine::Engine;
pub use steel::steel_vm::register_fn::RegisterFn;
