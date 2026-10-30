//! Standalone Web Gateway serving WebGL 3D Minecraft client and WebSocket.

use std::net::SocketAddr;
use std::path::PathBuf;
use game_server::{start_server, ServerConfig, StaticAssetSource};
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8083);

    let bind_addr = SocketAddr::from(([0, 0, 0, 0], port));
    let web_dir = PathBuf::from("assets/web");

    println!("==================================================");
    println!("🎮 2D Minecraft & 3D WebGL Gateway");
    println!("🚀 HTTP Web Client: http://localhost:{}/", port);
    println!("🔌 WebSocket Route: ws://localhost:{}/ws", port);
    println!("📁 Static Directory: {}", web_dir.display());
    println!("==================================================");

    let config = ServerConfig::new(bind_addr)
        .with_static_assets(StaticAssetSource::FileSystem(web_dir))
        .with_heartbeat_timeout_secs(15)
        .with_grace_period_secs(30);

    let handle = start_server::<Value, Value, Value, Value>(config).await?;

    tokio::signal::ctrl_c().await?;
    println!("\nShutting down server...");
    handle.shutdown();
    Ok(())
}
