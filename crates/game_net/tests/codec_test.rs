use game_net::{ClientEnvelope, CodecError, CodecFormat, DualCodec, ServerEnvelope};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MockInput {
    movement_x: f32,
    movement_y: f32,
    jump: bool,
    action_button: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum MockCommand {
    PlaceBlock { x: i32, y: i32, z: i32, block_type: u16 },
    BreakBlock { x: i32, y: i32, z: i32 },
    ChatMessage(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MockSnapshot {
    tick: u64,
    entities: Vec<(u64, f32, f32, f32)>,
    world_version: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum MockEvent {
    PlayerJoined { player_id: u64, name: String },
    PlayerLeft { player_id: u64 },
    Explosion { x: f32, y: f32, z: f32, power: f32 },
}

type TestClientEnvelope = ClientEnvelope<MockInput, MockCommand>;
type TestServerEnvelope = ServerEnvelope<MockSnapshot, MockEvent>;

#[test]
fn test_client_envelope_json_roundtrip() {
    let envelopes: Vec<TestClientEnvelope> = vec![
        ClientEnvelope::Auth {
            token: "secret_token_12345".to_string(),
            client_version: "0.1.0".to_string(),
        },
        ClientEnvelope::Ping {
            client_time_ms: 1_700_000_000_123,
        },
        ClientEnvelope::Input(MockInput {
            movement_x: 0.75,
            movement_y: -0.25,
            jump: true,
            action_button: 1,
        }),
        ClientEnvelope::Command(MockCommand::PlaceBlock {
            x: 10,
            y: -5,
            z: 24,
            block_type: 7,
        }),
        ClientEnvelope::Command(MockCommand::ChatMessage("Hello multiplayer!".to_string())),
    ];

    for env in envelopes {
        let json_str = DualCodec::encode_text(&env).expect("JSON encoding should succeed");
        let decoded: TestClientEnvelope =
            DualCodec::decode_text(&json_str).expect("JSON decoding should succeed");
        assert_eq!(env, decoded);
    }
}

#[test]
fn test_client_envelope_binary_roundtrip() {
    let envelopes: Vec<TestClientEnvelope> = vec![
        ClientEnvelope::Auth {
            token: "secret_token_binary_999".to_string(),
            client_version: "2.5.0".to_string(),
        },
        ClientEnvelope::Ping {
            client_time_ms: 9_999_888_777,
        },
        ClientEnvelope::Input(MockInput {
            movement_x: -1.0,
            movement_y: 1.0,
            jump: false,
            action_button: 0,
        }),
        ClientEnvelope::Command(MockCommand::BreakBlock { x: 42, y: 0, z: 12 }),
    ];

    for env in envelopes {
        let bin_bytes = DualCodec::encode_binary(&env).expect("Binary encoding should succeed");
        let decoded: TestClientEnvelope =
            DualCodec::decode_binary(&bin_bytes).expect("Binary decoding should succeed");
        assert_eq!(env, decoded);
    }
}

#[test]
fn test_server_envelope_json_roundtrip() {
    let envelopes: Vec<TestServerEnvelope> = vec![
        ServerEnvelope::AuthSuccess {
            player_id: 42,
            server_tick: 1050,
        },
        ServerEnvelope::AuthFailed {
            reason: "Token expired".to_string(),
        },
        ServerEnvelope::Pong {
            client_time_ms: 1_700_000_000_100,
            server_time_ms: 1_700_000_000_120,
        },
        ServerEnvelope::Snapshot(MockSnapshot {
            tick: 1050,
            entities: vec![
                (1, 0.0, 10.0, 5.0),
                (2, 100.5, 200.2, 12.0),
                (42, 15.0, 32.0, 64.0),
            ],
            world_version: 3,
        }),
        ServerEnvelope::Event(MockEvent::PlayerJoined {
            player_id: 100,
            name: "Steve".to_string(),
        }),
        ServerEnvelope::Event(MockEvent::Explosion {
            x: 10.0,
            y: 20.0,
            z: 30.0,
            power: 4.5,
        }),
        ServerEnvelope::Kick {
            reason: "AFK timeout".to_string(),
        },
    ];

    for env in envelopes {
        let json_str = DualCodec::encode_text(&env).expect("JSON encoding should succeed");
        let decoded: TestServerEnvelope =
            DualCodec::decode_text(&json_str).expect("JSON decoding should succeed");
        assert_eq!(env, decoded);
    }
}

#[test]
fn test_server_envelope_binary_roundtrip() {
    let envelopes: Vec<TestServerEnvelope> = vec![
        ServerEnvelope::AuthSuccess {
            player_id: 888,
            server_tick: 50000,
        },
        ServerEnvelope::AuthFailed {
            reason: "Version mismatch".to_string(),
        },
        ServerEnvelope::Pong {
            client_time_ms: 1000,
            server_time_ms: 1025,
        },
        ServerEnvelope::Snapshot(MockSnapshot {
            tick: 50000,
            entities: vec![(888, 12.0, 34.0, 56.0)],
            world_version: 1,
        }),
        ServerEnvelope::Event(MockEvent::PlayerLeft { player_id: 888 }),
        ServerEnvelope::Kick {
            reason: "Server maintenance".to_string(),
        },
    ];

    for env in envelopes {
        let bin_bytes = DualCodec::encode_binary(&env).expect("Binary encoding should succeed");
        let decoded: TestServerEnvelope =
            DualCodec::decode_binary(&bin_bytes).expect("Binary decoding should succeed");
        assert_eq!(env, decoded);
    }
}

#[test]
fn test_dual_codec_explicit_format() {
    let msg: TestClientEnvelope = ClientEnvelope::Ping {
        client_time_ms: 424242,
    };

    // JSON format
    let json_bytes = DualCodec::encode(CodecFormat::Json, &msg).unwrap();
    let decoded_json: TestClientEnvelope =
        DualCodec::decode(CodecFormat::Json, &json_bytes).unwrap();
    assert_eq!(msg, decoded_json);

    // Binary format
    let bin_bytes = DualCodec::encode(CodecFormat::Binary, &msg).unwrap();
    let decoded_bin: TestClientEnvelope =
        DualCodec::decode(CodecFormat::Binary, &bin_bytes).unwrap();
    assert_eq!(msg, decoded_bin);
}

#[test]
fn test_dual_codec_adaptive_decoding() {
    let client_msg: TestClientEnvelope = ClientEnvelope::Auth {
        token: "adaptive_jwt_token".to_string(),
        client_version: "1.0.0".to_string(),
    };

    // 1. JSON bytes adaptively decoded
    let json_bytes = DualCodec::encode(CodecFormat::Json, &client_msg).unwrap();
    let decoded_from_json: TestClientEnvelope = DualCodec::decode_adaptive(&json_bytes).unwrap();
    assert_eq!(client_msg, decoded_from_json);

    // 2. Binary bytes adaptively decoded
    let bin_bytes = DualCodec::encode(CodecFormat::Binary, &client_msg).unwrap();
    let decoded_from_bin: TestClientEnvelope = DualCodec::decode_adaptive(&bin_bytes).unwrap();
    assert_eq!(client_msg, decoded_from_bin);

    // 3. Server envelope adaptive decoding
    let server_msg: TestServerEnvelope = ServerEnvelope::AuthSuccess {
        player_id: 777,
        server_tick: 9999,
    };
    let s_json = DualCodec::encode(CodecFormat::Json, &server_msg).unwrap();
    let s_bin = DualCodec::encode(CodecFormat::Binary, &server_msg).unwrap();
    assert_eq!(
        DualCodec::decode_adaptive::<TestServerEnvelope>(&s_json).unwrap(),
        server_msg
    );
    assert_eq!(
        DualCodec::decode_adaptive::<TestServerEnvelope>(&s_bin).unwrap(),
        server_msg
    );
}

#[test]
fn test_postcard_binary_compactness() {
    let snapshot = ServerEnvelope::<MockSnapshot, MockEvent>::Snapshot(MockSnapshot {
        tick: 123456,
        entities: (0..50)
            .map(|i| (i, i as f32 * 1.5, i as f32 * 2.5, i as f32 * 0.5))
            .collect(),
        world_version: 42,
    });

    let json_bytes = DualCodec::encode(CodecFormat::Json, &snapshot).unwrap();
    let binary_bytes = DualCodec::encode(CodecFormat::Binary, &snapshot).unwrap();

    println!(
        "50-entity Snapshot size -> JSON: {} bytes, Postcard Binary: {} bytes (ratio: {:.1}%)",
        json_bytes.len(),
        binary_bytes.len(),
        (binary_bytes.len() as f64 / json_bytes.len() as f64) * 100.0
    );

    // Binary format is more compact than JSON text
    assert!(binary_bytes.len() < json_bytes.len());
}

#[test]
fn test_codec_error_handling() {
    let invalid_json = "{ invalid_json: true ";
    let err_json = DualCodec::decode_text::<TestClientEnvelope>(invalid_json);
    assert!(matches!(err_json, Err(CodecError::Json(_))));

    let invalid_postcard = &[0xFF, 0xFF, 0x00];
    let err_bin = DualCodec::decode_binary::<TestClientEnvelope>(invalid_postcard);
    assert!(matches!(err_bin, Err(CodecError::Postcard(_))));

    // Test error formatting and trait implementation
    if let Err(e) = err_json {
        let msg = format!("{}", e);
        assert!(msg.starts_with("JSON codec error:"));
        use std::error::Error;
        assert!(e.source().is_some());
    }
}
