//! Session management, authentication, heartbeat watchdog, and grace period reconnection.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use game_net::PlayerId;
use serde::{Deserialize, Serialize};

/// Session connection status state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    /// Initial connection established, awaiting authentication.
    Connecting,
    /// Authenticated successfully, awaiting game entry.
    Authenticated,
    /// Actively participating in the game simulation.
    InGame,
    /// Connection lost; within the grace period awaiting reconnection.
    Disconnected,
    /// Grace period expired or explicitly terminated.
    Terminated,
}

/// Metadata and state for an individual player session.
#[derive(Debug, Clone)]
pub struct SessionState {
    /// Authentication token identifying the client.
    pub token: String,
    /// Assigned authoritative player ID.
    pub player_id: PlayerId,
    /// Client version string sent during auth handshake.
    pub client_version: String,
    /// Current state machine status.
    pub status: SessionStatus,
    /// Time when the session was created.
    pub created_at: Instant,
    /// Time of the most recent heartbeat/ping or message from the client.
    pub last_heartbeat: Instant,
    /// Time when disconnection was detected (for grace period tracking).
    pub disconnected_at: Option<Instant>,
}

impl SessionState {
    /// Returns true if the session is currently considered actively connected.
    pub fn is_connected(&self) -> bool {
        matches!(self.status, SessionStatus::Authenticated | SessionStatus::InGame)
    }

    /// Returns true if the session is in the disconnected grace period.
    pub fn is_in_grace_period(&self) -> bool {
        matches!(self.status, SessionStatus::Disconnected)
    }
}

/// Timeout events produced by the watchdog checker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionTimeoutEvent {
    /// Client missed heartbeat deadlines and entered the Disconnected grace period.
    HeartbeatTimedOut {
        player_id: PlayerId,
        token: String,
    },
    /// Disconnected client did not reconnect before grace period expiration.
    GracePeriodExpired {
        player_id: PlayerId,
        token: String,
    },
}

/// Thread-safe session manager supporting token lookup, heartbeat tracking,
/// and seamless grace-period reconnection.
#[derive(Debug, Clone)]
pub struct SessionManager {
    sessions: Arc<DashMap<String, SessionState>>,
    token_by_player: Arc<DashMap<PlayerId, String>>,
    heartbeat_timeout: Duration,
    grace_period: Duration,
    next_player_id: Arc<AtomicU64>,
}

impl SessionManager {
    /// Creates a new `SessionManager` with the specified heartbeat timeout and grace period.
    pub fn new(heartbeat_timeout: Duration, grace_period: Duration) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            token_by_player: Arc::new(DashMap::new()),
            heartbeat_timeout,
            grace_period,
            next_player_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Authenticates a client with a token and client version.
    ///
    /// If the token corresponds to an existing active or grace-period session:
    /// - Reconnects the session.
    /// - Restores status to `Authenticated` or `InGame`.
    /// - Refreshes heartbeat and resets `disconnected_at`.
    /// - Returns `(player_id, true)`.
    ///
    /// If the token is new or terminated:
    /// - Allocates a new `PlayerId`.
    /// - Registers the session in `Authenticated` status.
    /// - Returns `(player_id, false)`.
    pub fn authenticate(&self, token: &str, client_version: &str) -> (PlayerId, bool) {
        if let Some(mut entry) = self.sessions.get_mut(token) {
            let session = entry.value_mut();
            if session.status != SessionStatus::Terminated {
                // Seamless reconnection
                let player_id = session.player_id;
                session.status = SessionStatus::Authenticated;
                session.client_version = client_version.to_string();
                session.last_heartbeat = Instant::now();
                session.disconnected_at = None;
                return (player_id, true);
            }
        }

        // New session (or previously terminated token reactivation)
        let player_id = self.next_player_id.fetch_add(1, Ordering::SeqCst);
        let now = Instant::now();
        let state = SessionState {
            token: token.to_string(),
            player_id,
            client_version: client_version.to_string(),
            status: SessionStatus::Authenticated,
            created_at: now,
            last_heartbeat: now,
            disconnected_at: None,
        };

        self.sessions.insert(token.to_string(), state);
        self.token_by_player.insert(player_id, token.to_string());
        (player_id, false)
    }

    /// Updates the heartbeat timestamp for a token.
    ///
    /// If the session was previously `Disconnected`, this restores it to `Authenticated`.
    pub fn update_heartbeat(&self, token: &str) -> bool {
        if let Some(mut entry) = self.sessions.get_mut(token) {
            let session = entry.value_mut();
            session.last_heartbeat = Instant::now();
            if session.status == SessionStatus::Disconnected {
                session.status = SessionStatus::Authenticated;
                session.disconnected_at = None;
            }
            true
        } else {
            false
        }
    }

    /// Updates the heartbeat timestamp for a given `PlayerId`.
    pub fn update_heartbeat_by_player(&self, player_id: PlayerId) -> bool {
        if let Some(token) = self.token_by_player.get(&player_id) {
            self.update_heartbeat(token.value())
        } else {
            false
        }
    }

    /// Transitions session to `InGame` status.
    pub fn set_in_game(&self, token: &str) -> bool {
        if let Some(mut entry) = self.sessions.get_mut(token) {
            let session = entry.value_mut();
            if session.status == SessionStatus::Authenticated {
                session.status = SessionStatus::InGame;
                return true;
            }
        }
        false
    }

    /// Transitions session to `Disconnected` status, starting its grace period timer.
    pub fn disconnect(&self, token: &str) -> bool {
        if let Some(mut entry) = self.sessions.get_mut(token) {
            let session = entry.value_mut();
            if session.status != SessionStatus::Terminated {
                session.status = SessionStatus::Disconnected;
                session.disconnected_at = Some(Instant::now());
                return true;
            }
        }
        false
    }

    /// Transitions a player's session to `Disconnected` by `PlayerId`.
    pub fn disconnect_player(&self, player_id: PlayerId) -> bool {
        if let Some(token) = self.token_by_player.get(&player_id) {
            self.disconnect(token.value())
        } else {
            false
        }
    }

    /// Explicitly terminates a session.
    pub fn terminate(&self, token: &str) -> bool {
        if let Some(mut entry) = self.sessions.get_mut(token) {
            let session = entry.value_mut();
            session.status = SessionStatus::Terminated;
            return true;
        }
        false
    }

    /// Watchdog scan: checks for heartbeat timeouts and grace period expirations.
    ///
    /// - Active sessions with `now - last_heartbeat > heartbeat_timeout` transition to `Disconnected`.
    /// - Disconnected sessions with `now - disconnected_at > grace_period` transition to `Terminated`.
    pub fn check_timeouts(&self, now: Instant) -> Vec<SessionTimeoutEvent> {
        let mut events = Vec::new();

        for mut entry in self.sessions.iter_mut() {
            let session = entry.value_mut();
            match session.status {
                SessionStatus::Authenticated | SessionStatus::InGame => {
                    if now.duration_since(session.last_heartbeat) > self.heartbeat_timeout {
                        session.status = SessionStatus::Disconnected;
                        session.disconnected_at = Some(now);
                        events.push(SessionTimeoutEvent::HeartbeatTimedOut {
                            player_id: session.player_id,
                            token: session.token.clone(),
                        });
                    }
                }
                SessionStatus::Disconnected => {
                    if let Some(disconnected_at) = session.disconnected_at {
                        if now.duration_since(disconnected_at) > self.grace_period {
                            session.status = SessionStatus::Terminated;
                            events.push(SessionTimeoutEvent::GracePeriodExpired {
                                player_id: session.player_id,
                                token: session.token.clone(),
                            });
                        }
                    }
                }
                SessionStatus::Connecting | SessionStatus::Terminated => {}
            }
        }

        events
    }

    /// Gets a cloned snapshot of session state by token.
    pub fn get_session(&self, token: &str) -> Option<SessionState> {
        self.sessions.get(token).map(|s| s.clone())
    }

    /// Gets a cloned snapshot of session state by player ID.
    pub fn get_session_by_player(&self, player_id: PlayerId) -> Option<SessionState> {
        self.token_by_player
            .get(&player_id)
            .and_then(|tok| self.get_session(tok.value()))
    }

    /// Returns true if the player is currently actively connected.
    pub fn is_connected(&self, player_id: PlayerId) -> bool {
        self.get_session_by_player(player_id)
            .map(|s| s.is_connected())
            .unwrap_or(false)
    }

    /// Returns the total count of currently connected (active) sessions.
    pub fn active_session_count(&self) -> usize {
        self.sessions
            .iter()
            .filter(|entry| entry.value().is_connected())
            .count()
    }

    /// Returns a list of player IDs for all currently connected sessions.
    pub fn all_active_player_ids(&self) -> Vec<PlayerId> {
        self.sessions
            .iter()
            .filter(|entry| entry.value().is_connected())
            .map(|entry| entry.value().player_id)
            .collect()
    }

    /// Completely removes a session and its reverse mapping.
    pub fn remove_session(&self, token: &str) -> Option<SessionState> {
        if let Some((_, session)) = self.sessions.remove(token) {
            self.token_by_player.remove(&session.player_id);
            Some(session)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_lifecycle_and_reconnection() {
        let mgr = SessionManager::new(Duration::from_secs(5), Duration::from_secs(60));

        // 1. Initial authentication
        let (p1, reconnected) = mgr.authenticate("token-user-1", "1.0.0");
        assert_eq!(p1, 1);
        assert!(!reconnected);
        assert!(mgr.is_connected(p1));

        let sess = mgr.get_session("token-user-1").unwrap();
        assert_eq!(sess.status, SessionStatus::Authenticated);

        // 2. Set to InGame
        assert!(mgr.set_in_game("token-user-1"));
        let sess = mgr.get_session("token-user-1").unwrap();
        assert_eq!(sess.status, SessionStatus::InGame);

        // 3. Disconnect into Grace Period
        assert!(mgr.disconnect("token-user-1"));
        assert!(!mgr.is_connected(p1));
        let sess = mgr.get_session("token-user-1").unwrap();
        assert_eq!(sess.status, SessionStatus::Disconnected);
        assert!(sess.disconnected_at.is_some());

        // 4. Reconnect within Grace Period with same token
        let (p1_reconnect, reconnected) = mgr.authenticate("token-user-1", "1.0.1");
        assert_eq!(p1_reconnect, p1);
        assert!(reconnected);
        assert!(mgr.is_connected(p1));
        let sess = mgr.get_session("token-user-1").unwrap();
        assert_eq!(sess.status, SessionStatus::Authenticated);
        assert_eq!(sess.client_version, "1.0.1");
        assert!(sess.disconnected_at.is_none());
    }

    #[test]
    fn test_session_timeout_watchdog() {
        let mgr = SessionManager::new(Duration::from_millis(100), Duration::from_millis(200));

        let (p1, _) = mgr.authenticate("tok1", "1.0.0");
        let start = Instant::now();

        // No timeout initially
        let events = mgr.check_timeouts(start);
        assert!(events.is_empty());

        // Heartbeat timeout reached
        let t1 = start + Duration::from_millis(150);
        let events = mgr.check_timeouts(t1);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            SessionTimeoutEvent::HeartbeatTimedOut {
                player_id: p1,
                token: "tok1".into()
            }
        );

        let sess = mgr.get_session("tok1").unwrap();
        assert_eq!(sess.status, SessionStatus::Disconnected);

        // Grace period expired
        let t2 = t1 + Duration::from_millis(250);
        let events = mgr.check_timeouts(t2);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            SessionTimeoutEvent::GracePeriodExpired {
                player_id: p1,
                token: "tok1".into()
            }
        );

        let sess = mgr.get_session("tok1").unwrap();
        assert_eq!(sess.status, SessionStatus::Terminated);
    }
}
