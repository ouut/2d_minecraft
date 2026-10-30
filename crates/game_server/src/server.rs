//! Generic game server hosting HTTP static assets with COOP/COEP headers
//! and dual-mode JSON/binary WebSocket communication.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use game_net::{ClientEnvelope, DualCodec, PlayerId, ServerEnvelope};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tower_http::services::ServeDir;

use crate::pipeline::IngestionPipeline;
use crate::session::{SessionEvent, SessionManager, SessionTimeoutEvent};

/// Source of static assets for web client hosting.
#[derive(Clone, Debug)]
pub enum StaticAssetSource {
    /// Host files from a filesystem directory.
    FileSystem(PathBuf),
    /// Host embedded in-memory static files `(path, bytes, mime)`.
    Embedded(&'static [(&'static str, &'static [u8], &'static str)]),
}

/// Configuration for the generic game server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Socket address to bind to (e.g., `0.0.0.0:8083` or `0.0.0.0:0` for ephemeral port).
    pub bind_addr: SocketAddr,
    /// Heartbeat timeout in seconds before an active session enters the grace period.
    pub heartbeat_timeout_secs: u64,
    /// Grace period in seconds for disconnected sessions to reconnect before termination.
    pub grace_period_secs: u64,
    /// Static asset source (FileSystem or Embedded in-memory).
    pub static_assets: Option<StaticAssetSource>,
    /// Maximum allowed WebSocket frame size in bytes (default: 64KB).
    pub max_frame_size: usize,
    /// Maximum commands accepted from a single player within a single simulation tick (default: 50).
    pub max_commands_per_tick: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([0, 0, 0, 0], 8083)),
            heartbeat_timeout_secs: 10,
            grace_period_secs: 60,
            static_assets: None,
            max_frame_size: 64 * 1024,
            max_commands_per_tick: 50,
        }
    }
}

impl ServerConfig {
    /// Creates a configuration with the given bind address and default timeouts.
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self {
            bind_addr,
            ..Default::default()
        }
    }

    /// Sets the bind address.
    pub fn with_bind_addr(mut self, addr: SocketAddr) -> Self {
        self.bind_addr = addr;
        self
    }

    /// Sets the heartbeat timeout in seconds.
    pub fn with_heartbeat_timeout_secs(mut self, secs: u64) -> Self {
        self.heartbeat_timeout_secs = secs;
        self
    }

    /// Sets the grace period timeout in seconds.
    pub fn with_grace_period_secs(mut self, secs: u64) -> Self {
        self.grace_period_secs = secs;
        self
    }

    /// Sets the static directory for web client hosting.
    pub fn with_static_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.static_assets = Some(StaticAssetSource::FileSystem(dir.into()));
        self
    }

    /// Sets the static asset source.
    pub fn with_static_assets(mut self, assets: StaticAssetSource) -> Self {
        self.static_assets = Some(assets);
        self
    }

    /// Sets the embedded static assets.
    pub fn with_embedded_assets(
        mut self,
        assets: &'static [(&'static str, &'static [u8], &'static str)],
    ) -> Self {
        self.static_assets = Some(StaticAssetSource::Embedded(assets));
        self
    }

    /// Sets the maximum frame size in bytes.
    pub fn with_max_frame_size(mut self, size: usize) -> Self {
        self.max_frame_size = size;
        self
    }

    /// Sets the maximum commands allowed per player per tick.
    pub fn with_max_commands_per_tick(mut self, limit: usize) -> Self {
        self.max_commands_per_tick = limit;
        self
    }
}

/// Player network output channel supporting backpressure protection:
/// - Discrete events/control envelopes (`AuthSuccess`, `Pong`, `Event`, `Kick`) are strictly delivered via FIFO channel.
/// - Authoritative snapshots (`Snapshot`) are delivered with a latest-only "Drop Oldest Snapshot" policy
///   to prevent memory accumulation and OOM on slow or blocked client connections.
#[derive(Clone)]
pub struct PlayerSender<TSnapshot, TEvent> {
    event_tx: mpsc::UnboundedSender<ServerEnvelope<TSnapshot, TEvent>>,
    snapshot_tx: Arc<watch::Sender<Option<TSnapshot>>>,
}

impl<TSnapshot: Clone, TEvent> PlayerSender<TSnapshot, TEvent> {
    pub fn new(
        event_tx: mpsc::UnboundedSender<ServerEnvelope<TSnapshot, TEvent>>,
        snapshot_tx: Arc<watch::Sender<Option<TSnapshot>>>,
    ) -> Self {
        Self {
            event_tx,
            snapshot_tx,
        }
    }

    /// Sends a server envelope to the client.
    ///
    /// For discrete control messages and simulation events (`AuthSuccess`, `Pong`, `Event`, `Kick`),
    /// messages are queued into the FIFO event channel without loss.
    ///
    /// For high-frequency world snapshots (`Snapshot`), the latest snapshot replaces any pending
    /// unconsumed snapshot ("Drop Oldest Snapshot"), preventing memory accumulation and OOM
    /// under slow-client backpressure.
    ///
    /// Returns `true` if dispatched, or `false` if the connection has been terminated.
    pub fn send(&self, envelope: ServerEnvelope<TSnapshot, TEvent>) -> bool {
        match envelope {
            ServerEnvelope::Snapshot(snap) => {
                if self.snapshot_tx.receiver_count() == 0 {
                    false
                } else {
                    self.snapshot_tx.send_replace(Some(snap));
                    true
                }
            }
            other => self.event_tx.send(other).is_ok(),
        }
    }
}

/// Shared internal server state across WebSocket sessions and the HTTP gateway.
pub struct ServerContext<TInput, TCmd, TSnapshot, TEvent> {
    pub session_manager: SessionManager,
    pub pipeline: IngestionPipeline<TInput, TCmd>,
    pub connections: DashMap<PlayerId, PlayerSender<TSnapshot, TEvent>>,
    pub current_tick: AtomicU64,
    pub config: ServerConfig,
}

impl<TInput, TCmd, TSnapshot, TEvent> ServerContext<TInput, TCmd, TSnapshot, TEvent> {
    pub fn new(config: ServerConfig) -> Self {
        let heartbeat_timeout = Duration::from_secs(config.heartbeat_timeout_secs);
        let grace_period = Duration::from_secs(config.grace_period_secs);
        Self {
            session_manager: SessionManager::new(heartbeat_timeout, grace_period),
            pipeline: IngestionPipeline::new(),
            connections: DashMap::new(),
            current_tick: AtomicU64::new(0),
            config,
        }
    }
}

/// Handle to a running game server, providing operational controls, pipeline draining,
/// message sending/broadcasting, and clean shutdown.
pub struct ServerHandle<TInput, TCmd, TSnapshot, TEvent> {
    local_addr: SocketAddr,
    context: Arc<ServerContext<TInput, TCmd, TSnapshot, TEvent>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    server_task: JoinHandle<Result<(), std::io::Error>>,
    watchdog_task: JoinHandle<()>,
}

impl<TInput, TCmd, TSnapshot: Clone, TEvent> ServerHandle<TInput, TCmd, TSnapshot, TEvent> {
    /// Returns the local socket address the server bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Returns a reference to the ingestion pipeline.
    pub fn pipeline(&self) -> &IngestionPipeline<TInput, TCmd> {
        &self.context.pipeline
    }

    /// Returns a reference to the session manager.
    pub fn session_manager(&self) -> &SessionManager {
        &self.context.session_manager
    }

    /// Drains all high-frequency player inputs (Latest-Only Latch).
    pub fn drain_inputs(&self) -> Vec<(PlayerId, TInput)> {
        self.context.pipeline.drain_inputs()
    }

    /// Drains all discrete player commands (FIFO Queue).
    pub fn drain_commands(&self) -> Vec<(PlayerId, Vec<TCmd>)> {
        self.context.pipeline.drain_commands()
    }

    /// Drains all player lifecycle events (PlayerJoined, PlayerDisconnected, PlayerReconnected, PlayerTerminated).
    pub fn drain_session_events(&self) -> Vec<SessionEvent> {
        self.context.session_manager.drain_session_events()
    }

    /// Sends a server envelope to a specific player.
    ///
    /// Returns `true` if the player connection was found and the message queued.
    pub fn send_to(&self, player_id: PlayerId, envelope: ServerEnvelope<TSnapshot, TEvent>) -> bool {
        if let Some(sender) = self.context.connections.get(&player_id) {
            sender.send(envelope)
        } else {
            false
        }
    }

    /// Broadcasts a server envelope to all currently connected players.
    ///
    /// Returns the number of recipients the message was successfully dispatched to.
    pub fn broadcast(&self, envelope: ServerEnvelope<TSnapshot, TEvent>) -> usize
    where
        TSnapshot: Clone,
        TEvent: Clone,
    {
        let mut count = 0;
        for entry in self.context.connections.iter() {
            if entry.value().send(envelope.clone()) {
                count += 1;
            }
        }
        count
    }

    /// Multicasts a server envelope to a targeted list of players.
    ///
    /// Avoids $O(N^2)$ bandwidth explosion by restricting delivery to specific recipients.
    pub fn multicast(&self, recipients: &[PlayerId], envelope: ServerEnvelope<TSnapshot, TEvent>) -> usize
    where
        TSnapshot: Clone,
        TEvent: Clone,
    {
        let mut count = 0;
        for &pid in recipients {
            if let Some(sender) = self.context.connections.get(&pid) {
                if sender.send(envelope.clone()) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Broadcasts a server envelope to connected players that satisfy a spatial or logical filter predicate.
    ///
    /// Solves $O(N^2)$ bandwidth scaling in large multiplayer worlds with multiple viewports.
    pub fn broadcast_filter<F>(&self, envelope: ServerEnvelope<TSnapshot, TEvent>, filter: F) -> usize
    where
        TSnapshot: Clone,
        TEvent: Clone,
        F: Fn(PlayerId) -> bool,
    {
        let mut count = 0;
        for entry in self.context.connections.iter() {
            let pid = *entry.key();
            if filter(pid) && entry.value().send(envelope.clone()) {
                count += 1;
            }
        }
        count
    }

    /// Forcibly kicks a connected player with a given reason.
    pub fn kick(&self, player_id: PlayerId, reason: impl Into<String>) -> bool {
        let reason = reason.into();
        if let Some((_, sender)) = self.context.connections.remove(&player_id) {
            let _ = sender.send(ServerEnvelope::Kick { reason });
            self.context.session_manager.disconnect_player(player_id);
            true
        } else {
            false
        }
    }

    /// Returns a list of player IDs for all currently active WebSocket connections.
    pub fn connected_players(&self) -> Vec<PlayerId> {
        self.context
            .connections
            .iter()
            .map(|entry| *entry.key())
            .collect()
    }

    /// Gets the current authoritative server simulation tick.
    pub fn server_tick(&self) -> u64 {
        self.context.current_tick.load(Ordering::Relaxed)
    }

    /// Increments the server simulation tick counter and returns the new value.
    pub fn increment_server_tick(&self) -> u64 {
        self.context.current_tick.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Sets the server simulation tick counter.
    pub fn set_server_tick(&self, tick: u64) {
        self.context.current_tick.store(tick, Ordering::SeqCst);
    }

    /// Gracefully triggers server shutdown and terminates background tasks.
    pub fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.watchdog_task.abort();
    }

    /// Aborts the server immediately without waiting for graceful shutdown.
    pub fn abort(&self) {
        self.server_task.abort();
        self.watchdog_task.abort();
    }

    /// Waits for the underlying HTTP/WebSocket server task to complete.
    pub async fn wait(self) -> Result<Result<(), std::io::Error>, tokio::task::JoinError> {
        self.server_task.await
    }
}

/// Generic game server instance decoupled from specific game state definitions via generics.
///
/// - `TInput`: High-frequency player input frame (latched).
/// - `TCmd`: Discrete game command (FIFO queued).
/// - `TSnapshot`: Authoritative world state snapshot.
/// - `TEvent`: Discrete game simulation event.
pub struct GenericGameServer<TInput, TCmd, TSnapshot, TEvent> {
    config: ServerConfig,
    context: Arc<ServerContext<TInput, TCmd, TSnapshot, TEvent>>,
}

impl<TInput, TCmd, TSnapshot, TEvent> GenericGameServer<TInput, TCmd, TSnapshot, TEvent>
where
    TInput: DeserializeOwned + Send + Sync + 'static,
    TCmd: DeserializeOwned + Send + Sync + 'static,
    TSnapshot: Serialize + Clone + Send + Sync + 'static,
    TEvent: Serialize + Clone + Send + Sync + 'static,
{
    /// Creates a new `GenericGameServer` instance from configuration.
    pub fn new(config: ServerConfig) -> Self {
        let context = Arc::new(ServerContext::new(config.clone()));
        Self { config, context }
    }

    /// Returns a reference to the server configuration.
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Returns a clone of the shared server context.
    pub fn context(&self) -> Arc<ServerContext<TInput, TCmd, TSnapshot, TEvent>> {
        Arc::clone(&self.context)
    }

    /// Returns a reference to the ingestion pipeline.
    pub fn pipeline(&self) -> &IngestionPipeline<TInput, TCmd> {
        &self.context.pipeline
    }

    /// Returns a reference to the session manager.
    pub fn session_manager(&self) -> &SessionManager {
        &self.context.session_manager
    }

    /// Drains all high-frequency player inputs (Latest-Only Latch).
    pub fn drain_inputs(&self) -> Vec<(PlayerId, TInput)> {
        self.context.pipeline.drain_inputs()
    }

    /// Drains all discrete player commands (FIFO Queue).
    pub fn drain_commands(&self) -> Vec<(PlayerId, Vec<TCmd>)> {
        self.context.pipeline.drain_commands()
    }

    /// Drains all player lifecycle events (PlayerJoined, PlayerDisconnected, PlayerReconnected, PlayerTerminated).
    pub fn drain_session_events(&self) -> Vec<SessionEvent> {
        self.context.session_manager.drain_session_events()
    }

    /// Sends a server envelope to a specific player.
    pub fn send_to(&self, player_id: PlayerId, envelope: ServerEnvelope<TSnapshot, TEvent>) -> bool {
        if let Some(sender) = self.context.connections.get(&player_id) {
            sender.send(envelope)
        } else {
            false
        }
    }

    /// Broadcasts a server envelope to all connected players.
    pub fn broadcast(&self, envelope: ServerEnvelope<TSnapshot, TEvent>) -> usize {
        let mut count = 0;
        for entry in self.context.connections.iter() {
            if entry.value().send(envelope.clone()) {
                count += 1;
            }
        }
        count
    }

    /// Multicasts a server envelope to a targeted list of players.
    pub fn multicast(&self, recipients: &[PlayerId], envelope: ServerEnvelope<TSnapshot, TEvent>) -> usize {
        let mut count = 0;
        for &pid in recipients {
            if let Some(sender) = self.context.connections.get(&pid) {
                if sender.send(envelope.clone()) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Broadcasts a server envelope to connected players that satisfy a filter predicate.
    pub fn broadcast_filter<F>(&self, envelope: ServerEnvelope<TSnapshot, TEvent>, filter: F) -> usize
    where
        F: Fn(PlayerId) -> bool,
    {
        let mut count = 0;
        for entry in self.context.connections.iter() {
            let pid = *entry.key();
            if filter(pid) && entry.value().send(envelope.clone()) {
                count += 1;
            }
        }
        count
    }

    /// Starts the server, binding the TCP socket and running HTTP + WebSocket services.
    pub async fn start(self) -> Result<ServerHandle<TInput, TCmd, TSnapshot, TEvent>, std::io::Error> {
        let listener = tokio::net::TcpListener::bind(self.config.bind_addr).await?;
        let local_addr = listener.local_addr()?;

        let context = self.context;

        // Build Axum router
        let mut router = Router::new()
            .route("/ws", get(ws_handler))
            .with_state(Arc::clone(&context));

        // Attach static asset serving (FileSystem or Embedded WASM/HTML)
        if let Some(ref assets) = self.config.static_assets {
            match assets {
                StaticAssetSource::FileSystem(static_path) => {
                    let serve_dir =
                        ServeDir::new(static_path).append_index_html_on_directories(true);
                    router = router.fallback_service(serve_dir);
                }
                StaticAssetSource::Embedded(embedded_entries) => {
                    let entries = *embedded_entries;
                    router = router.fallback(move |uri: axum::http::Uri| async move {
                        let req_path = uri.path();
                        let clean_req = req_path.trim_start_matches('/');
                        for &(path, bytes, mime) in entries {
                            let clean_item = path.trim_start_matches('/');
                            if clean_item == clean_req
                                || (clean_req.is_empty()
                                    && (clean_item == "index.html" || clean_item.is_empty()))
                            {
                                let mut resp = Response::new(axum::body::Body::from(bytes));
                                if let Ok(val) = HeaderValue::from_str(mime) {
                                    resp.headers_mut().insert(axum::http::header::CONTENT_TYPE, val);
                                }
                                return resp;
                            }
                        }
                        Response::builder()
                            .status(axum::http::StatusCode::NOT_FOUND)
                            .body(axum::body::Body::from("404 Not Found"))
                            .unwrap()
                    });
                }
            }
        }

        // Apply COOP and COEP security headers layer
        router = router.layer(middleware::from_fn(coop_coep_middleware));

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Spawn Axum server
        let server_future = axum::serve(listener, router).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        let server_task = tokio::spawn(async move {
            server_future.await
        });

        // Spawn watchdog task
        let ctx_watchdog = Arc::clone(&context);
        let watchdog_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(250));
            loop {
                interval.tick().await;
                let now = Instant::now();
                let events = ctx_watchdog.session_manager.check_timeouts(now);
                for event in events {
                    match event {
                        SessionTimeoutEvent::HeartbeatTimedOut { player_id, .. } => {
                            if let Some((_, sender)) = ctx_watchdog.connections.remove(&player_id) {
                                let _ = sender.send(ServerEnvelope::Kick {
                                    reason: "Heartbeat timeout".to_string(),
                                });
                            }
                        }
                        SessionTimeoutEvent::GracePeriodExpired { player_id, .. } => {
                            ctx_watchdog.pipeline.remove_player(player_id);
                        }
                    }
                }
            }
        });

        Ok(ServerHandle {
            local_addr,
            context,
            shutdown_tx: Some(shutdown_tx),
            server_task,
            watchdog_task,
        })
    }
}

/// Middleware that injects Cross-Origin-Opener-Policy (COOP) and Cross-Origin-Embedder-Policy (COEP)
/// response headers required for SharedArrayBuffer and WASM multithreading.
async fn coop_coep_middleware(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        HeaderName::from_static("cross-origin-embedder-policy"),
        HeaderValue::from_static("require-corp"),
    );
    response
}

/// WebSocket upgrade handler.
async fn ws_handler<TInput, TCmd, TSnapshot, TEvent>(
    ws: WebSocketUpgrade,
    State(ctx): State<Arc<ServerContext<TInput, TCmd, TSnapshot, TEvent>>>,
) -> impl IntoResponse
where
    TInput: DeserializeOwned + Send + Sync + 'static,
    TCmd: DeserializeOwned + Send + Sync + 'static,
    TSnapshot: Serialize + Clone + Send + Sync + 'static,
    TEvent: Serialize + Clone + Send + Sync + 'static,
{
    ws.on_upgrade(move |socket| handle_socket(socket, ctx))
}

/// Manages an individual WebSocket connection with dual-mode automatic negotiation,
/// backpressure protection (Drop Oldest Snapshot), and DoS protection.
async fn handle_socket<TInput, TCmd, TSnapshot, TEvent>(
    socket: WebSocket,
    ctx: Arc<ServerContext<TInput, TCmd, TSnapshot, TEvent>>,
) where
    TInput: DeserializeOwned + Send + Sync + 'static,
    TCmd: DeserializeOwned + Send + Sync + 'static,
    TSnapshot: Serialize + Clone + Send + Sync + 'static,
    TEvent: Serialize + Clone + Send + Sync + 'static,
{
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<ServerEnvelope<TSnapshot, TEvent>>();
    let (snapshot_tx, mut snapshot_rx) = watch::channel::<Option<TSnapshot>>(None);
    let snapshot_tx = Arc::new(snapshot_tx);
    let player_sender = PlayerSender::new(event_tx, Arc::clone(&snapshot_tx));

    // Codec negotiation flag: false = Text (JSON), true = Binary (Postcard)
    let is_binary_mode = Arc::new(AtomicBool::new(false));

    // Writer task: sends discrete events & latest snapshots according to negotiated mode
    let is_binary_writer = Arc::clone(&is_binary_mode);
    let writer_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;

                maybe_event = event_rx.recv() => {
                    match maybe_event {
                        Some(envelope) => {
                            let is_kick = matches!(envelope, ServerEnvelope::Kick { .. });
                            let msg = if is_binary_writer.load(Ordering::Acquire) {
                                match DualCodec::encode_binary(&envelope) {
                                    Ok(bytes) => Message::Binary(bytes),
                                    Err(_) => continue,
                                }
                            } else {
                                match DualCodec::encode_text(&envelope) {
                                    Ok(text) => Message::Text(text),
                                    Err(_) => continue,
                                }
                            };
                            if ws_sender.send(msg).await.is_err() {
                                break;
                            }
                            if is_kick {
                                let _ = ws_sender.close().await;
                                break;
                            }
                        }
                        None => break,
                    }
                }
                res = snapshot_rx.changed() => {
                    if res.is_err() {
                        break;
                    }
                    let maybe_snap = {
                        let borrow = snapshot_rx.borrow_and_update();
                        borrow.clone()
                    };
                    if let Some(snap) = maybe_snap {
                        let envelope: ServerEnvelope<TSnapshot, TEvent> =
                        ServerEnvelope::Snapshot(snap);
                        let msg = if is_binary_writer.load(Ordering::Acquire) {
                            match DualCodec::encode_binary(&envelope) {
                                Ok(bytes) => Message::Binary(bytes),
                                Err(_) => continue,
                            }
                        } else {
                            match DualCodec::encode_text(&envelope) {
                                Ok(text) => Message::Text(text),
                                Err(_) => continue,
                            }
                        };
                        if ws_sender.send(msg).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    let mut player_id: Option<PlayerId> = None;
    let mut session_token: Option<String> = None;
    let mut last_command_tick: u64 = ctx.current_tick.load(Ordering::Relaxed);
    let mut commands_in_tick: usize = 0;

    // Reader loop: reads and dispatches ClientEnvelopes with DoS frame & command rate limits
    while let Some(msg_res) = ws_receiver.next().await {
        let msg = match msg_res {
            Ok(m) => m,
            Err(_) => break,
        };

        let envelope: ClientEnvelope<TInput, TCmd> = match msg {
            Message::Text(text) => {
                if text.len() > ctx.config.max_frame_size {
                    let reason = format!(
                        "Frame size {} bytes exceeds maximum allowed limit of {} bytes",
                        text.len(),
                        ctx.config.max_frame_size
                    );
                    let _ = player_sender.send(ServerEnvelope::Kick { reason });
                    break;
                }
                is_binary_mode.store(false, Ordering::Release);
                match DualCodec::decode_text(&text) {
                    Ok(env) => env,
                    Err(_) => continue,
                }
            }
            Message::Binary(bytes) => {
                if bytes.len() > ctx.config.max_frame_size {
                    let reason = format!(
                        "Frame size {} bytes exceeds maximum allowed limit of {} bytes",
                        bytes.len(),
                        ctx.config.max_frame_size
                    );
                    let _ = player_sender.send(ServerEnvelope::Kick { reason });
                    break;
                }
                is_binary_mode.store(true, Ordering::Release);
                match DualCodec::decode_binary(&bytes) {
                    Ok(env) => env,
                    Err(_) => continue,
                }
            }
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Close(_) => break,
        };

        match envelope {
            ClientEnvelope::Auth {
                token,
                client_version,
            } => {
                let (pid, _reconnected) =
                    ctx.session_manager.authenticate(&token, &client_version);
                player_id = Some(pid);
                session_token = Some(token);
                ctx.connections.insert(pid, player_sender.clone());

                let tick = ctx.current_tick.load(Ordering::Relaxed);
                let _ = player_sender.send(ServerEnvelope::AuthSuccess {
                    player_id: pid,
                    server_tick: tick,
                });
            }
            ClientEnvelope::Ping { client_time_ms } => {
                if let Some(pid) = player_id {
                    ctx.session_manager.update_heartbeat_by_player(pid);
                }
                let server_time_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let _ = player_sender.send(ServerEnvelope::Pong {
                    client_time_ms,
                    server_time_ms,
                });
            }
            ClientEnvelope::Input(input) => {
                if let Some(pid) = player_id {
                    ctx.session_manager.update_heartbeat_by_player(pid);
                    ctx.pipeline.push_input(pid, input);
                }
            }
            ClientEnvelope::Command(cmd) => {
                let cur_tick = ctx.current_tick.load(Ordering::Relaxed);
                if cur_tick != last_command_tick {
                    last_command_tick = cur_tick;
                    commands_in_tick = 0;
                }
                commands_in_tick += 1;
                if commands_in_tick > ctx.config.max_commands_per_tick {
                    let reason = format!(
                        "Command rate limit exceeded: {} commands in tick {} (max: {})",
                        commands_in_tick, cur_tick, ctx.config.max_commands_per_tick
                    );
                    let _ = player_sender.send(ServerEnvelope::Kick { reason });
                    break;
                }

                if let Some(pid) = player_id {
                    ctx.session_manager.update_heartbeat_by_player(pid);
                    ctx.pipeline.push_command(pid, cmd);
                }
            }
        }
    }

    // Clean up when connection terminates
    if let Some(pid) = player_id {
        ctx.connections.remove(&pid);
        if let Some(ref tok) = session_token {
            ctx.session_manager.disconnect(tok);
        }
    }
    let _ = tokio::time::timeout(Duration::from_millis(50), writer_task).await;
}

/// Convenience function to create and start a generic game server.
pub async fn start_server<TInput, TCmd, TSnapshot, TEvent>(
    config: ServerConfig,
) -> Result<ServerHandle<TInput, TCmd, TSnapshot, TEvent>, std::io::Error>
where
    TInput: DeserializeOwned + Send + Sync + 'static,
    TCmd: DeserializeOwned + Send + Sync + 'static,
    TSnapshot: Serialize + Clone + Send + Sync + 'static,
    TEvent: Serialize + Clone + Send + Sync + 'static,
{
    GenericGameServer::new(config).start().await
}

/// Convenience function to create and start a server with an explicit static file directory.
pub async fn start_server_with_static<TInput, TCmd, TSnapshot, TEvent>(
    config: ServerConfig,
    static_dir: impl Into<PathBuf>,
) -> Result<ServerHandle<TInput, TCmd, TSnapshot, TEvent>, std::io::Error>
where
    TInput: DeserializeOwned + Send + Sync + 'static,
    TCmd: DeserializeOwned + Send + Sync + 'static,
    TSnapshot: Serialize + Clone + Send + Sync + 'static,
    TEvent: Serialize + Clone + Send + Sync + 'static,
{
    let config = config.with_static_dir(static_dir);
    GenericGameServer::new(config).start().await
}

/// Convenience function to create and start a server with embedded static assets.
pub async fn start_server_with_embedded<TInput, TCmd, TSnapshot, TEvent>(
    config: ServerConfig,
    embedded_assets: &'static [(&'static str, &'static [u8], &'static str)],
) -> Result<ServerHandle<TInput, TCmd, TSnapshot, TEvent>, std::io::Error>
where
    TInput: DeserializeOwned + Send + Sync + 'static,
    TCmd: DeserializeOwned + Send + Sync + 'static,
    TSnapshot: Serialize + Clone + Send + Sync + 'static,
    TEvent: Serialize + Clone + Send + Sync + 'static,
{
    let config = config.with_embedded_assets(embedded_assets);
    GenericGameServer::new(config).start().await
}
