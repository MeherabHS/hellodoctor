//! Live chat nudge (product owner, 2026-11-21).
//!
//! `GET /api/v1/chat/live` upgrades to a WebSocket that tells a signed-in app
//! "conversation X has something new". The app then fetches the conversation
//! over the normal HTTP endpoints, so message content, authorization, row-level
//! security and rate limits all stay on the existing HTTP path. Polling remains
//! as the fallback whenever the socket is not connected.
//!
//! Source of the signal: the `notifications_chat_live` trigger NOTIFYs channel
//! `chat_live` after each committed NEW_CHAT_MESSAGE notification. One
//! listener per backend process forwards those into an in-process broadcast;
//! each socket only ever receives nudges addressed to its own user.
//!
//! What a socket can see: identifiers only (`{"type":"chat",
//! "conversation_id":...,"kind":"CHAT"|"PEER_CHAT"}`), never text. The
//! upgrade request passes the ordinary auth middleware (bearer token, device
//! proof, live session check). Sockets are closed after [`SOCKET_LIFETIME`] so
//! a revoked or expired session stops receiving nudges within that window, and
//! the app reconnects with a fresh token. Client frames are ignored.

use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::domain::ids::UserId;
use crate::error::AppError;
use crate::repository::AppState;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Postgres NOTIFY channel written by the `notify_chat_live()` trigger.
pub const CHANNEL: &str = "chat_live";
/// A socket is closed (code 4000) after this long; the app reconnects.
pub const SOCKET_LIFETIME: Duration = Duration::from_secs(5 * 60);
/// Keep-alive so mobile networks and proxies do not drop an idle socket.
const PING_EVERY: Duration = Duration::from_secs(25);
/// One phone and a tablet with both apps open is four; more is not needed.
pub const MAX_SOCKETS_PER_USER: usize = 4;
/// Process-wide ceiling; beyond it apps simply keep polling.
pub const MAX_SOCKETS: usize = 10_000;
const BROADCAST_CAPACITY: usize = 4_096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    /// Something new in `conversation_id` for `user_id`.
    Chat {
        user_id: Uuid,
        conversation_id: Uuid,
        kind: String,
    },
    /// Nudges may have been missed (listener reconnected): refresh everything.
    Resync,
}

pub struct ChatLiveHub {
    tx: broadcast::Sender<Signal>,
    per_user: Mutex<HashMap<Uuid, usize>>,
    total: AtomicUsize,
}

/// Holds one socket's place in the connection caps until dropped.
pub struct SocketSlot {
    user_id: Uuid,
}

impl Drop for SocketSlot {
    fn drop(&mut self) {
        let hub = ChatLiveHub::global();
        hub.total.fetch_sub(1, Ordering::SeqCst);
        let mut users = hub.per_user.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(count) = users.get_mut(&self.user_id) {
            *count -= 1;
            if *count == 0 {
                users.remove(&self.user_id);
            }
        }
    }
}

impl ChatLiveHub {
    pub fn global() -> &'static ChatLiveHub {
        static HUB: OnceLock<ChatLiveHub> = OnceLock::new();
        HUB.get_or_init(|| ChatLiveHub {
            tx: broadcast::channel(BROADCAST_CAPACITY).0,
            per_user: Mutex::new(HashMap::new()),
            total: AtomicUsize::new(0),
        })
    }

    pub fn publish(&self, signal: Signal) {
        // No receivers simply means nobody is connected right now.
        let _ = self.tx.send(signal);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Signal> {
        self.tx.subscribe()
    }

    /// Sockets currently open for `user_id` (tests and diagnostics).
    pub fn sockets_for(&self, user_id: Uuid) -> usize {
        let users = self.per_user.lock().unwrap_or_else(|e| e.into_inner());
        users.get(&user_id).copied().unwrap_or(0)
    }

    pub fn try_reserve(&self, user_id: Uuid) -> Result<SocketSlot, AppError> {
        let mut users = self.per_user.lock().unwrap_or_else(|e| e.into_inner());
        let count = users.entry(user_id).or_insert(0);
        if *count >= MAX_SOCKETS_PER_USER {
            return Err(AppError::RateLimitExceeded(
                "Too many live chat connections for this account.".into(),
            ));
        }
        if self.total.fetch_add(1, Ordering::SeqCst) >= MAX_SOCKETS {
            self.total.fetch_sub(1, Ordering::SeqCst);
            return Err(AppError::service_unavailable(
                "Live chat is at capacity; the app keeps polling.".to_string(),
            ));
        }
        *count += 1;
        Ok(SocketSlot { user_id })
    }
}

#[derive(Deserialize)]
struct Payload {
    u: Uuid,
    c: Uuid,
    #[serde(default)]
    k: String,
}

/// Parses one `chat_live` NOTIFY payload written by the trigger.
pub fn parse_payload(payload: &str) -> Option<Signal> {
    let p: Payload = serde_json::from_str(payload).ok()?;
    Some(Signal::Chat {
        user_id: p.u,
        conversation_id: p.c,
        kind: p.k,
    })
}

/// Starts the per-process listener that forwards `chat_live` NOTIFYs into the
/// hub. It reconnects on its own and tells sockets to resync after a gap.
pub fn start(state: AppState) {
    tokio::spawn(async move {
        let mut first = true;
        loop {
            match sqlx::postgres::PgListener::connect_with(&state.db).await {
                Ok(mut listener) => {
                    if let Err(error) = listener.listen(CHANNEL).await {
                        tracing::warn!(%error, "chat live: LISTEN failed");
                    } else {
                        if !first {
                            ChatLiveHub::global().publish(Signal::Resync);
                        }
                        first = false;
                        loop {
                            match listener.recv().await {
                                Ok(notification) => {
                                    if let Some(signal) = parse_payload(notification.payload()) {
                                        ChatLiveHub::global().publish(signal);
                                    }
                                }
                                Err(error) => {
                                    tracing::warn!(%error, "chat live: listener lost");
                                    break;
                                }
                            }
                        }
                    }
                }
                Err(error) => tracing::warn!(%error, "chat live: listener connect failed"),
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

/// `GET /chat/live` — WebSocket upgrade for the signed-in user's own nudges.
pub async fn live(
    State(_state): State<AppState>,
    ctx: AuthContext,
    ws: WebSocketUpgrade,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let slot = ChatLiveHub::global().try_reserve(ctx.user_id.into_inner())?;
    let rx = ChatLiveHub::global().subscribe();
    let user_id = ctx.user_id;
    Ok(ws
        .max_message_size(1024)
        .max_frame_size(1024)
        .on_upgrade(move |socket| serve(socket, rx, user_id, slot)))
}

/// The JSON frame sent for one signal, or `None` when it is someone else's.
pub fn frame_for(signal: &Signal, me: Uuid) -> Option<String> {
    match signal {
        Signal::Chat {
            user_id,
            conversation_id,
            kind,
        } if *user_id == me => Some(
            serde_json::json!({"type": "chat", "conversation_id": conversation_id, "kind": kind})
                .to_string(),
        ),
        Signal::Chat { .. } => None,
        Signal::Resync => Some(r#"{"type":"resync"}"#.to_string()),
    }
}

async fn serve(
    mut socket: WebSocket,
    mut rx: broadcast::Receiver<Signal>,
    user_id: UserId,
    _slot: SocketSlot,
) {
    let me = user_id.into_inner();
    let _ = socket
        .send(Message::Text(r#"{"type":"ready"}"#.to_string()))
        .await;
    let deadline = tokio::time::sleep(SOCKET_LIFETIME);
    tokio::pin!(deadline);
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;
    loop {
        tokio::select! {
            _ = &mut deadline => {
                let _ = socket.send(Message::Close(Some(CloseFrame {
                    code: 4000,
                    reason: "reconnect".into(),
                }))).await;
                break;
            }
            _ = ping.tick() => {
                if socket.send(Message::Ping(Vec::new())).await.is_err() {
                    break;
                }
            }
            signal = rx.recv() => {
                let frame = match signal {
                    Ok(signal) => frame_for(&signal, me),
                    // Fell behind: tell the app to refresh instead.
                    Err(broadcast::error::RecvError::Lagged(_)) => frame_for(&Signal::Resync, me),
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if let Some(frame) = frame {
                    if socket.send(Message::Text(frame)).await.is_err() {
                        break;
                    }
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    // Client frames carry no meaning; pongs and the like are ignored.
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_parses_and_frames_only_reach_their_recipient() {
        let u = Uuid::new_v4();
        let c = Uuid::new_v4();
        let signal = parse_payload(&format!(r#"{{"u":"{u}","c":"{c}","k":"CHAT"}}"#)).unwrap();
        let frame = frame_for(&signal, u).unwrap();
        assert!(frame.contains(&c.to_string()));
        assert!(frame.contains(r#""type":"chat""#));
        assert_eq!(frame_for(&signal, Uuid::new_v4()), None);
        assert!(frame_for(&Signal::Resync, u).unwrap().contains("resync"));
        assert_eq!(parse_payload("not json"), None);
    }

    #[test]
    fn per_user_cap_is_enforced_and_released() {
        let hub = ChatLiveHub::global();
        let user = Uuid::new_v4();
        let slots: Vec<_> = (0..MAX_SOCKETS_PER_USER)
            .map(|_| hub.try_reserve(user).unwrap())
            .collect();
        assert!(hub.try_reserve(user).is_err());
        assert_eq!(hub.sockets_for(user), MAX_SOCKETS_PER_USER);
        drop(slots);
        assert_eq!(hub.sockets_for(user), 0);
        assert!(hub.try_reserve(user).is_ok());
    }
}
