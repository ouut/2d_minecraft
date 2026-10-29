//! Industrial-grade hardening integration tests for `game_server` v0.2.0.
//!
//! Covers:
//! 1. Embedded static WebAssembly asset serving with COOP/COEP headers and MIME resolution.
//! 2. SessionEvent lifecycle feed (join, disconnect, reconnect, terminate).
//! 3. Slow client backpressure control (Drop Oldest Snapshot conflation without event loss).
//! 4. Directed multicast and spatial filtered broadcast.
//! 5. DoS protections: max_frame_size truncation and max_commands_per_tick frequency limits.

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use game_net::{ClientEnvelope, DualCodec, ServerEnvelope};
use game_server::{
    start_server, start_server_with_embedded, ServerConfig, SessionEvent,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TestInput {
    pub move_x: f32,
    pub move_y: f32,
    pub seq: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum TestCmd {
    PlaceBlock { x: i32, y: i32, block_id: u8 },
    Chat(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TestSnapshot {
    pub tick: u64,
    pub entities_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum TestEvent {
    PlayerJoined { id: u64 },
    ServerAnnouncement(String),
}

static EMBEDDED_FILES: &[(&str, &[u8], &str)] = &[
    (
        "/index.html",
        b"<!DOCTYPE html><html><body><h1>Embedded 2D Minecraft WASM</h1></body></html>",
        "text/html",
    ),
    (
        "/pkg/minecraft_client.wasm",
        b"\0asm\x01\0\0\0embedded_wasm_payload",
        "application/wasm",
    ),
    (
        "/pkg/minecraft_client.js",
        b"export function init() { console.log('WASM init'); }",
        "application/javascript",
    ),
];

#[tokio::test]
async fn test_embedded_static_assets_serving_and_coop_coep() {
    let config = ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
        .with_embedded_assets(EMBEDDED_FILES);

    let server = start_server_with_embedded::<TestInput, TestCmd, TestSnapshot, TestEvent>(
        config,
        EMBEDDED_FILES,
    )
    .await
    .expect("Failed to start embedded server");

    let addr = server.local_addr();

    // 1. GET / -> resolves to /index.html with COOP/COEP and Content-Type: text/html
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!("GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", addr);
    stream.write_all(req.as_bytes()).await.unwrap();

    let mut response_bytes = Vec::new();
    stream.read_to_end(&mut response_bytes).await.unwrap();
    let response_str = String::from_utf8_lossy(&response_bytes);

    assert!(response_str.contains("HTTP/1.1 200 OK"));
    assert!(response_str.to_lowercase().contains("cross-origin-opener-policy: same-origin"));
    assert!(response_str.to_lowercase().contains("cross-origin-embedder-policy: require-corp"));
    assert!(response_str.to_lowercase().contains("content-type: text/html"));
    assert!(response_str.contains("Embedded 2D Minecraft WASM"));

    // 2. GET /pkg/minecraft_client.wasm -> Content-Type: application/wasm
    let mut stream2 = TcpStream::connect(addr).await.unwrap();
    let req2 = format!(
        "GET /pkg/minecraft_client.wasm HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        addr
    );
    stream2.write_all(req2.as_bytes()).await.unwrap();

    let mut response_bytes2 = Vec::new();
    stream2.read_to_end(&mut response_bytes2).await.unwrap();
    let response_str2 = String::from_utf8_lossy(&response_bytes2);

    assert!(response_str2.contains("HTTP/1.1 200 OK"));
    assert!(response_str2.to_lowercase().contains("content-type: application/wasm"));
    assert!(response_str2.to_lowercase().contains("cross-origin-opener-policy: same-origin"));
    assert!(response_bytes2.windows(b"embedded_wasm_payload".len()).any(|w| w == b"embedded_wasm_payload"));

    // 3. GET /missing.html -> 404 Not Found
    let mut stream3 = TcpStream::connect(addr).await.unwrap();
    let req3 = format!(
        "GET /missing.html HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        addr
    );
    stream3.write_all(req3.as_bytes()).await.unwrap();

    let mut response_bytes3 = Vec::new();
    stream3.read_to_end(&mut response_bytes3).await.unwrap();
    let response_str3 = String::from_utf8_lossy(&response_bytes3);
    assert!(response_str3.contains("HTTP/1.1 404 Not Found"));

    server.shutdown();
}

#[tokio::test]
async fn test_session_event_feed_lifecycle() {
    let config = ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
        .with_heartbeat_timeout_secs(5)
        .with_grace_period_secs(60);

    let server = start_server::<TestInput, TestCmd, TestSnapshot, TestEvent>(config)
        .await
        .unwrap();

    let ws_url = format!("ws://{}/ws", server.local_addr());
    let token = "test_player_feed_token";

    // 1. Client connects and authenticates -> SessionEvent::PlayerJoined
    let (mut ws, _) = connect_async(&ws_url).await.unwrap();
    let auth = ClientEnvelope::<TestInput, TestCmd>::Auth {
        token: token.into(),
        client_version: "1.0.0".into(),
    };
    ws.send(Message::Text(DualCodec::encode_text(&auth).unwrap()))
        .await
        .unwrap();

    let reply = ws.next().await.unwrap().unwrap();
    let player_id = match reply {
        Message::Text(text) => {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
            match env {
                ServerEnvelope::AuthSuccess { player_id, .. } => player_id,
                _ => panic!("Expected AuthSuccess"),
            }
        }
        _ => panic!("Expected text frame"),
    };

    // Drain session events: should contain PlayerJoined
    let events = server.drain_session_events();
    assert_eq!(
        events,
        vec![SessionEvent::PlayerJoined {
            player_id,
            token: token.into(),
        }]
    );

    // 2. Client disconnects -> SessionEvent::PlayerDisconnected
    drop(ws);
    tokio::time::sleep(Duration::from_millis(50)).await;

    let events = server.drain_session_events();
    assert_eq!(
        events,
        vec![SessionEvent::PlayerDisconnected {
            player_id,
            grace_period_secs: 60,
        }]
    );

    // 3. Client reconnects with same token -> SessionEvent::PlayerReconnected
    let (mut ws2, _) = connect_async(&ws_url).await.unwrap();
    ws2.send(Message::Text(DualCodec::encode_text(&auth).unwrap()))
        .await
        .unwrap();

    let _reply2 = ws2.next().await.unwrap().unwrap();
    let events = server.drain_session_events();
    assert_eq!(
        events,
        vec![SessionEvent::PlayerReconnected { player_id }]
    );

    server.shutdown();
}

#[tokio::test]
async fn test_multicast_and_spatial_filtered_broadcast() {
    let config = ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)));
    let server = start_server::<TestInput, TestCmd, TestSnapshot, TestEvent>(config)
        .await
        .unwrap();

    let ws_url = format!("ws://{}/ws", server.local_addr());

    // Helper to connect a player
    async fn connect_player(
        ws_url: &str,
        token: &str,
    ) -> (tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>, u64) {
        let (mut ws, _) = connect_async(ws_url).await.unwrap();
        let auth = ClientEnvelope::<TestInput, TestCmd>::Auth {
            token: token.into(),
            client_version: "1.0".into(),
        };
        ws.send(Message::Text(DualCodec::encode_text(&auth).unwrap()))
            .await
            .unwrap();
        let reply = ws.next().await.unwrap().unwrap();
        let pid = match reply {
            Message::Text(text) => {
                let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
                match env {
                    ServerEnvelope::AuthSuccess { player_id, .. } => player_id,
                    _ => panic!("Expected AuthSuccess"),
                }
            }
            _ => panic!("Expected text frame"),
        };
        (ws, pid)
    }

    let (mut ws1, pid1) = connect_player(&ws_url, "p1").await;
    let (mut ws2, pid2) = connect_player(&ws_url, "p2").await;
    let (mut ws3, pid3) = connect_player(&ws_url, "p3").await;

    // Multicast to pid1 and pid3
    let multicast_msg = ServerEnvelope::Event(TestEvent::ServerAnnouncement("P1 and P3 Only".into()));
    let sent_count = server.multicast(&[pid1, pid3], multicast_msg.clone());
    assert_eq!(sent_count, 2);

    // Verify p1 receives
    let msg1 = ws1.next().await.unwrap().unwrap();
    if let Message::Text(text) = msg1 {
        let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
        assert_eq!(env, multicast_msg);
    } else {
        panic!("Expected text message");
    }

    // Verify p3 receives
    let msg3 = ws3.next().await.unwrap().unwrap();
    if let Message::Text(text) = msg3 {
        let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
        assert_eq!(env, multicast_msg);
    } else {
        panic!("Expected text message");
    }

    // Spatial / Filtered broadcast: only target pid2
    let filter_msg = ServerEnvelope::Event(TestEvent::ServerAnnouncement("Filtered P2".into()));
    let filtered_count = server.broadcast_filter(filter_msg.clone(), |pid| pid == pid2);
    assert_eq!(filtered_count, 1);

    let msg2 = ws2.next().await.unwrap().unwrap();
    if let Message::Text(text) = msg2 {
        let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
        assert_eq!(env, filter_msg);
    } else {
        panic!("Expected text message");
    }

    server.shutdown();
}

#[tokio::test]
async fn test_slow_client_backpressure_and_snapshot_conflation() {
    let config = ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)));
    let server = start_server::<TestInput, TestCmd, TestSnapshot, TestEvent>(config)
        .await
        .unwrap();

    let ws_url = format!("ws://{}/ws", server.local_addr());
    let (mut ws, _) = connect_async(&ws_url).await.unwrap();

    // Authenticate
    let auth = ClientEnvelope::<TestInput, TestCmd>::Auth {
        token: "slow_client_tok".into(),
        client_version: "1.0".into(),
    };
    ws.send(Message::Text(DualCodec::encode_text(&auth).unwrap()))
        .await
        .unwrap();

    let reply = ws.next().await.unwrap().unwrap();
    let player_id = match reply {
        Message::Text(text) => {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
            match env {
                ServerEnvelope::AuthSuccess { player_id, .. } => player_id,
                _ => panic!("Expected AuthSuccess"),
            }
        }
        _ => panic!("Expected text frame"),
    };

    // Allow writer task to become idle
    tokio::time::sleep(Duration::from_millis(30)).await;

    // Send rapid burst of snapshots followed by an event and more snapshots
    for tick in 1..=10 {
        server.send_to(
            player_id,
            ServerEnvelope::Snapshot(TestSnapshot {
                tick,
                entities_count: tick as u32 * 5,
            }),
        );
    }

    // Send a discrete event: MUST NOT BE DROPPED
    server.send_to(
        player_id,
        ServerEnvelope::Event(TestEvent::ServerAnnouncement("Guaranteed Event".into())),
    );

    // Send more snapshots overwriting older ones
    for tick in 11..=20 {
        server.send_to(
            player_id,
            ServerEnvelope::Snapshot(TestSnapshot {
                tick,
                entities_count: tick as u32 * 5,
            }),
        );
    }

    // Give writer task a brief moment to process
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Now collect all messages that client receives
    let mut received_envelopes = Vec::new();
    while let Ok(Some(Ok(msg))) = tokio::time::timeout(Duration::from_millis(100), ws.next()).await {
        if let Message::Text(text) = msg {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
            received_envelopes.push(env);
        }
    }

    // 1. Critical Discrete Event must ALWAYS be received!
    let has_event = received_envelopes.iter().any(|env| {
        matches!(env, ServerEnvelope::Event(TestEvent::ServerAnnouncement(msg)) if msg == "Guaranteed Event")
    });
    assert!(has_event, "Discrete event must NOT be dropped under backpressure!");

    // 2. Snapshot Conflation: Total messages received should be significantly fewer than 20 snapshots
    // because intermediate snapshots are overwritten in-place!
    let snapshot_count = received_envelopes
        .iter()
        .filter(|env| matches!(env, ServerEnvelope::Snapshot(_)))
        .count();
    assert!(
        snapshot_count < 20,
        "Snapshot count should be conflated (received {}, expected < 20)",
        snapshot_count
    );

    // 3. The latest snapshot received should reflect the most recent state (tick 20)
    let last_snapshot = received_envelopes
        .iter()
        .filter_map(|env| match env {
            ServerEnvelope::Snapshot(s) => Some(s),
            _ => None,
        })
        .next_back();
    assert!(last_snapshot.is_some());
    assert_eq!(last_snapshot.unwrap().tick, 20);

    server.shutdown();
}

#[tokio::test]
async fn test_dos_protection_max_frame_size() {
    let config = ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
        .with_max_frame_size(256);

    let server = start_server::<TestInput, TestCmd, TestSnapshot, TestEvent>(config)
        .await
        .unwrap();

    let ws_url = format!("ws://{}/ws", server.local_addr());
    let (mut ws, _) = connect_async(&ws_url).await.unwrap();

    // Send valid auth frame (< 256 bytes)
    let auth = ClientEnvelope::<TestInput, TestCmd>::Auth {
        token: "tok_dos_size".into(),
        client_version: "1.0".into(),
    };
    ws.send(Message::Text(DualCodec::encode_text(&auth).unwrap()))
        .await
        .unwrap();

    let reply = ws.next().await.unwrap().unwrap();
    match reply {
        Message::Text(text) => {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
            assert!(matches!(env, ServerEnvelope::AuthSuccess { .. }));
        }
        _ => panic!("Expected AuthSuccess"),
    }

    // Now send an oversized text frame (> 256 bytes, e.g. 1KB)
    let huge_text = "A".repeat(1024);
    ws.send(Message::Text(huge_text)).await.unwrap();

    // Expect ServerEnvelope::Kick frame
    let kick_reply = ws.next().await.unwrap().unwrap();
    match kick_reply {
        Message::Text(text) => {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
            match env {
                ServerEnvelope::Kick { reason } => {
                    assert!(reason.contains("Frame size") || reason.contains("exceeds"));
                }
                other => panic!("Expected Kick, got {:?}", other),
            }
        }
        _ => panic!("Expected text kick frame"),
    }

    // Socket should now be closed / disconnected
    let next = ws.next().await;
    assert!(next.is_none() || matches!(next, Some(Ok(Message::Close(_)))));

    server.shutdown();
}

#[tokio::test]
async fn test_dos_protection_max_commands_per_tick() {
    let config = ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
        .with_max_commands_per_tick(5);

    let server = start_server::<TestInput, TestCmd, TestSnapshot, TestEvent>(config)
        .await
        .unwrap();

    let ws_url = format!("ws://{}/ws", server.local_addr());
    let (mut ws, _) = connect_async(&ws_url).await.unwrap();

    // Send valid auth
    let auth = ClientEnvelope::<TestInput, TestCmd>::Auth {
        token: "tok_dos_cmd_rate".into(),
        client_version: "1.0".into(),
    };
    ws.send(Message::Text(DualCodec::encode_text(&auth).unwrap()))
        .await
        .unwrap();

    let reply = ws.next().await.unwrap().unwrap();
    assert!(matches!(reply, Message::Text(_)));

    // Send 5 commands within tick 0 (within limit)
    for i in 1..=5 {
        let cmd = ClientEnvelope::<TestInput, TestCmd>::Command(TestCmd::Chat(format!("Msg {}", i)));
        ws.send(Message::Text(DualCodec::encode_text(&cmd).unwrap()))
            .await
            .unwrap();
    }

    // Wait for all 5 commands to arrive and drain
    let mut total_cmds = Vec::new();
    for _ in 0..30 {
        let drained = server.drain_commands();
        if let Some((_, list)) = drained.into_iter().next() {
            total_cmds.extend(list);
        }
        if total_cmds.len() >= 5 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(total_cmds.len(), 5);

    // Now send the 6th command in the same tick -> exceeds max_commands_per_tick (5)
    let flood_cmd = ClientEnvelope::<TestInput, TestCmd>::Command(TestCmd::Chat("Flood Msg 6".into()));
    ws.send(Message::Text(DualCodec::encode_text(&flood_cmd).unwrap()))
        .await
        .unwrap();

    // Expect Kick envelope
    let kick_reply = ws.next().await.unwrap().unwrap();
    match kick_reply {
        Message::Text(text) => {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
            match env {
                ServerEnvelope::Kick { reason } => {
                    assert!(reason.contains("Command rate limit exceeded"));
                }
                other => panic!("Expected Kick, got {:?}", other),
            }
        }
        _ => panic!("Expected text kick frame"),
    }

    server.shutdown();
}
