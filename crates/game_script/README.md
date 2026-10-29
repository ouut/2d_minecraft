# game_script

A universal, cross-game two-tier plugin-based Scheme scripting engine in Rust.

## Features
- **Bevy-Style Host Plugin Architecture**: `ScriptPlugin` trait and `PluginContext`.
- **Transactional Command Bus**: `CommandSink` avoiding borrow conflicts.
- **Sandboxed Execution**: Watchdog timeout interruption preventing infinite loops.
- **Bi-directional Data Marshalling**: Lossless conversion between `SteelVal` and `serde_json::Value`.
- **Hot-Reloading**: File mtime watcher.

## License
MIT OR Apache-2.0
