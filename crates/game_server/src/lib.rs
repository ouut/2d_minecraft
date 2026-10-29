//! Generic game server gateway and concurrent ingestion pipeline.
//!
//! Provides:
//! - `SessionManager`: Token-based authentication, heartbeat watchdog, and grace period reconnection.
//! - `IngestionPipeline`: High-frequency input latch (latest-only) and strict FIFO command queuing.
//! - `GenericGameServer`: Axum HTTP static file serving with COOP/COEP headers and dual-mode
//!   JSON/Binary WebSocket server.

pub mod gateway;
pub mod pipeline;
pub mod server;
pub mod session;

// Direct re-exports of core server types
pub use pipeline::IngestionPipeline;
pub use server::{
    start_server, start_server_with_embedded, start_server_with_static, GenericGameServer,
    PlayerSender, ServerConfig, ServerContext, ServerHandle, StaticAssetSource,
};
pub use session::{
    SessionEvent, SessionManager, SessionState, SessionStatus, SessionTimeoutEvent,
};

// Re-export Layer 0 protocol types for downstream convenience
pub use game_net::{ClientEnvelope, CodecError, CodecFormat, DualCodec, PlayerId, ServerEnvelope};
