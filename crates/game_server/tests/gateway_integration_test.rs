//! End-to-end integration tests for `game_server`.
//!
//! Tests:
//! 1. HTTP static file serving with COOP and COEP security headers.
//! 2. WebSocket JSON mode negotiation, Auth, Ping/Pong, Input latching, FIFO commands, and broadcast.
//! 3. WebSocket Binary mode negotiation with Postcard encoding.
//! 4. Seamless session reconnection within the Grace Period.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use game_net::{ClientEnvelope, DualCodec, ServerEnvelope};
use game_server::{start_server, ServerConfig};
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

#[tokio::test]
async fn test_http_static_serving_and_coop_coep_headers() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let static_dir = manifest_dir.parent().unwrap().parent().unwrap().join("assets/web");

    let config = ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
        .with_static_dir(static_dir);

    let server_handle = start_server::<TestInput, TestCmd, TestSnapshot, TestEvent>(config)
        .await
        .expect("Server must start");

    let addr = server_handle.local_addr();

    // Perform raw HTTP request to verify headers and HTML content
    let mut stream = TcpStream::connect(addr).await.expect("Failed to connect via TCP");
    let req = format!("GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", addr);
    stream.write_all(req.as_bytes()).await.unwrap();

    let mut response_bytes = Vec::new();
    stream.read_to_end(&mut response_bytes).await.unwrap();
    let response_str = String::from_utf8_lossy(&response_bytes);

    // Verify HTTP 200
    assert!(response_str.contains("HTTP/1.1 200 OK"));
    // Verify COOP header
    assert!(
        response_str.contains("cross-origin-opener-policy: same-origin")
            || response_str.contains("Cross-Origin-Opener-Policy: same-origin")
    );
    // Verify COEP header
    assert!(
        response_str.contains("cross-origin-embedder-policy: require-corp")
            || response_str.contains("Cross-Origin-Embedder-Policy: require-corp")
    );
    // Verify static HTML content
    assert!(response_str.contains("Voxel Engine Web Client"));

    server_handle.shutdown();
}

#[tokio::test]
async fn test_websocket_json_text_flow_and_pipeline() {
    let config = ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
        .with_heartbeat_timeout_secs(5)
        .with_grace_period_secs(10);

    let server = start_server::<TestInput, TestCmd, TestSnapshot, TestEvent>(config)
        .await
        .unwrap();

    let ws_url = format!("ws://{}/ws", server.local_addr());
    let (mut client_ws, _) = connect_async(&ws_url).await.expect("WS connect failed");

    // 1. Send Auth message in JSON text format
    let auth_msg = ClientEnvelope::<TestInput, TestCmd>::Auth {
        token: "session_token_json_123".into(),
        client_version: "1.0.0".into(),
    };
    let json_text = DualCodec::encode_text(&auth_msg).unwrap();
    client_ws.send(Message::Text(json_text)).await.unwrap();

    // 2. Expect ServerEnvelope::AuthSuccess
    let reply = client_ws.next().await.unwrap().unwrap();
    let player_id = match reply {
        Message::Text(text) => {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
            match env {
                ServerEnvelope::AuthSuccess { player_id, server_tick } => {
                    assert_eq!(server_tick, 0);
                    player_id
                }
                other => panic!("Expected AuthSuccess, got {:?}", other),
            }
        }
        Message::Binary(_) => panic!("Expected text frame, got binary"),
        _ => panic!("Unexpected frame"),
    };

    // 3. Send Ping
    let ping_msg = ClientEnvelope::<TestInput, TestCmd>::Ping {
        client_time_ms: 54321,
    };
    client_ws
        .send(Message::Text(DualCodec::encode_text(&ping_msg).unwrap()))
        .await
        .unwrap();

    // 4. Expect Pong
    let pong_reply = client_ws.next().await.unwrap().unwrap();
    match pong_reply {
        Message::Text(text) => {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
            match env {
                ServerEnvelope::Pong { client_time_ms, server_time_ms } => {
                    assert_eq!(client_time_ms, 54321);
                    assert!(server_time_ms > 0);
                }
                other => panic!("Expected Pong, got {:?}", other),
            }
        }
        _ => panic!("Expected text frame"),
    }

    // 5. High-frequency Input Latching: send 3 inputs rapidly
    for seq in 1..=3 {
        let input_msg = ClientEnvelope::<TestInput, TestCmd>::Input(TestInput {
            move_x: seq as f32,
            move_y: 0.0,
            seq,
        });
        client_ws
            .send(Message::Text(DualCodec::encode_text(&input_msg).unwrap()))
            .await
            .unwrap();
    }

    // 6. Discrete FIFO Commands: send 2 distinct commands
    let cmd1 = ClientEnvelope::<TestInput, TestCmd>::Command(TestCmd::PlaceBlock {
        x: 10,
        y: 20,
        block_id: 5,
    });
    let cmd2 = ClientEnvelope::<TestInput, TestCmd>::Command(TestCmd::Chat("Hello World".into()));
    client_ws
        .send(Message::Text(DualCodec::encode_text(&cmd1).unwrap()))
        .await
        .unwrap();
    client_ws
        .send(Message::Text(DualCodec::encode_text(&cmd2).unwrap()))
        .await
        .unwrap();

    // Allow background network processing
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify Ingestion Pipeline draining:
    // Latch: Only the latest input frame (seq = 3) should be retained
    let inputs = server.drain_inputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].0, player_id);
    assert_eq!(inputs[0].1.seq, 3);
    assert_eq!(inputs[0].1.move_x, 3.0);

    // FIFO Commands: Both commands should be drained in strict FIFO order
    let commands = server.drain_commands();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0, player_id);
    assert_eq!(
        commands[0].1,
        vec![
            TestCmd::PlaceBlock { x: 10, y: 20, block_id: 5 },
            TestCmd::Chat("Hello World".into()),
        ]
    );

    // 7. Server broadcast
    let broadcast_event = ServerEnvelope::Event(TestEvent::ServerAnnouncement("System Restart".into()));
    let sent_count = server.broadcast(broadcast_event);
    assert_eq!(sent_count, 1);

    // Client receives broadcast in JSON text mode
    let broadcast_msg = client_ws.next().await.unwrap().unwrap();
    match broadcast_msg {
        Message::Text(text) => {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
            assert_eq!(
                env,
                ServerEnvelope::Event(TestEvent::ServerAnnouncement("System Restart".into()))
            );
        }
        _ => panic!("Expected text broadcast"),
    }

    server.shutdown();
}

#[tokio::test]
async fn test_websocket_binary_postcard_flow() {
    let config = ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)));
    let server = start_server::<TestInput, TestCmd, TestSnapshot, TestEvent>(config)
        .await
        .unwrap();

    let ws_url = format!("ws://{}/ws", server.local_addr());
    let (mut client_ws, _) = connect_async(&ws_url).await.expect("WS connect failed");

    // 1. Send Auth in Binary Postcard format
    let auth_msg = ClientEnvelope::<TestInput, TestCmd>::Auth {
        token: "session_token_bin_999".into(),
        client_version: "2.0.0".into(),
    };
    let bin_bytes = DualCodec::encode_binary(&auth_msg).unwrap();
    client_ws.send(Message::Binary(bin_bytes)).await.unwrap();

    // 2. Expect ServerEnvelope::AuthSuccess in Binary format
    let reply = client_ws.next().await.unwrap().unwrap();
    let player_id = match reply {
        Message::Binary(bytes) => {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_binary(&bytes).unwrap();
            match env {
                ServerEnvelope::AuthSuccess { player_id, .. } => player_id,
                other => panic!("Expected AuthSuccess, got {:?}", other),
            }
        }
        Message::Text(_) => panic!("Expected binary frame, got text!"),
        _ => panic!("Unexpected frame"),
    };

    // 3. Send Ping in Binary format
    let ping_msg = ClientEnvelope::<TestInput, TestCmd>::Ping { client_time_ms: 77777 };
    client_ws
        .send(Message::Binary(DualCodec::encode_binary(&ping_msg).unwrap()))
        .await
        .unwrap();

    // 4. Expect Pong in Binary format
    let pong_reply = client_ws.next().await.unwrap().unwrap();
    match pong_reply {
        Message::Binary(bytes) => {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_binary(&bytes).unwrap();
            match env {
                ServerEnvelope::Pong { client_time_ms, .. } => {
                    assert_eq!(client_time_ms, 77777);
                }
                other => panic!("Expected Pong, got {:?}", other),
            }
        }
        _ => panic!("Expected binary frame"),
    }

    // 5. Send Input & Command in Binary format
    let input_msg = ClientEnvelope::<TestInput, TestCmd>::Input(TestInput {
        move_x: -1.5,
        move_y: 2.5,
        seq: 100,
    });
    client_ws
        .send(Message::Binary(DualCodec::encode_binary(&input_msg).unwrap()))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    let inputs = server.drain_inputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].0, player_id);
    assert_eq!(inputs[0].1.seq, 100);

    // 6. Server direct send to player in Binary mode
    let snapshot = ServerEnvelope::Snapshot(TestSnapshot {
        tick: 42,
        entities_count: 10,
    });
    assert!(server.send_to(player_id, snapshot));

    let snap_msg = client_ws.next().await.unwrap().unwrap();
    match snap_msg {
        Message::Binary(bytes) => {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_binary(&bytes).unwrap();
            assert_eq!(
                env,
                ServerEnvelope::Snapshot(TestSnapshot { tick: 42, entities_count: 10 })
            );
        }
        _ => panic!("Expected binary frame for snapshot"),
    }

    server.shutdown();
}

#[tokio::test]
async fn test_grace_period_seamless_reconnection() {
    let config = ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
        .with_heartbeat_timeout_secs(2)
        .with_grace_period_secs(10);

    let server = start_server::<TestInput, TestCmd, TestSnapshot, TestEvent>(config)
        .await
        .unwrap();

    let ws_url = format!("ws://{}/ws", server.local_addr());
    let token = "player_reconnect_token_xyz";

    // First connection
    let (mut ws1, _) = connect_async(&ws_url).await.unwrap();
    let auth_msg = ClientEnvelope::<TestInput, TestCmd>::Auth {
        token: token.into(),
        client_version: "1.0".into(),
    };
    ws1.send(Message::Text(DualCodec::encode_text(&auth_msg).unwrap()))
        .await
        .unwrap();

    let reply1 = ws1.next().await.unwrap().unwrap();
    let player_id1 = match reply1 {
        Message::Text(text) => {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
            match env {
                ServerEnvelope::AuthSuccess { player_id, .. } => player_id,
                _ => panic!("Expected AuthSuccess"),
            }
        }
        _ => panic!("Expected text frame"),
    };

    // Client 1 closes connection
    drop(ws1);
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify session enters Disconnected (Grace Period)
    let sess = server.session_manager().get_session(token).unwrap();
    assert!(sess.is_in_grace_period());

    // Second connection with same token: Reconnect!
    let (mut ws2, _) = connect_async(&ws_url).await.unwrap();
    ws2.send(Message::Text(DualCodec::encode_text(&auth_msg).unwrap()))
        .await
        .unwrap();

    let reply2 = ws2.next().await.unwrap().unwrap();
    let player_id2 = match reply2 {
        Message::Text(text) => {
            let env: ServerEnvelope<TestSnapshot, TestEvent> = DualCodec::decode_text(&text).unwrap();
            match env {
                ServerEnvelope::AuthSuccess { player_id, .. } => player_id,
                _ => panic!("Expected AuthSuccess"),
            }
        }
        _ => panic!("Expected text frame"),
    };

    // Reconnected player must retain the exact same PlayerId!
    assert_eq!(player_id1, player_id2);

    let sess_after = server.session_manager().get_session(token).unwrap();
    assert!(sess_after.is_connected());

    server.shutdown();
}
