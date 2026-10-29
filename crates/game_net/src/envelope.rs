use serde::{Deserialize, Serialize};

/// Generic client player identifier.
pub type PlayerId = u64;

/// Generic client message envelope across games.
///
/// Contains client-to-server messages including authentication, heartbeat ping,
/// high-frequency player inputs (`TInput`), and game commands (`TCmd`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientEnvelope<TInput, TCmd> {
    /// Authentication handshake request.
    Auth {
        token: String,
        client_version: String,
    },
    /// Latency measurement ping.
    Ping {
        client_time_ms: u64,
    },
    /// High-frequency input state frame.
    Input(TInput),
    /// Explicit player action / command.
    Command(TCmd),
}

/// Generic server message envelope across games.
///
/// Contains server-to-client messages including authentication responses,
/// heartbeat pong, authoritative world snapshots (`TSnapshot`), simulation events (`TEvent`),
/// and disconnection notices (`Kick`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerEnvelope<TSnapshot, TEvent> {
    /// Authentication succeeded.
    AuthSuccess {
        player_id: u64,
        server_tick: u64,
    },
    /// Authentication failed.
    AuthFailed {
        reason: String,
    },
    /// Latency measurement pong.
    Pong {
        client_time_ms: u64,
        server_time_ms: u64,
    },
    /// Authoritative state snapshot broadcast.
    Snapshot(TSnapshot),
    /// Simulation / notification event.
    Event(TEvent),
    /// Client forcibly disconnected by server.
    Kick {
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_construction_and_equality() {
        let auth_client: ClientEnvelope<(), ()> = ClientEnvelope::Auth {
            token: "xyz".to_string(),
            client_version: "1.0.0".to_string(),
        };
        let auth_client_clone = auth_client.clone();
        assert_eq!(auth_client, auth_client_clone);

        let ping_client: ClientEnvelope<(), ()> = ClientEnvelope::Ping { client_time_ms: 100 };
        assert_ne!(auth_client, ping_client);

        let kick_server: ServerEnvelope<(), ()> = ServerEnvelope::Kick {
            reason: "Cheating".to_string(),
        };
        assert_eq!(kick_server.clone(), kick_server);
    }
}
