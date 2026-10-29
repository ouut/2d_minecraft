# game_server

A generic, cross-game authoritative server framework with Axum HTTP WASM static serving and WebSocket gateway.

## Features
- **Integrated Web Gateway**: Single-port serving for static WASM/HTML/assets (with COOP/COEP headers) and WebSocket (`/ws`).
- **Session & Token Management**: Heartbeat watchdog and 60-second grace period reconnection.
- **Dual Ingestion Pipeline**:
  - Latest-Only Latch for high-frequency player movement (no backlog).
  - FIFO Command Queue for strictly ordered player actions and script commands.

## License
MIT OR Apache-2.0
