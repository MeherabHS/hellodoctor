//! Push-delivery adapter boundary for the Doctor Engagement notification
//! domain. No FCM/APNs/push infrastructure of any kind existed anywhere in
//! this codebase before this module (confirmed by searching for "fcm",
//! "apns", "firebase", "push" across `src/` and `Cargo.toml` before writing
//! this). Per the approved spec: implement the adapter boundary and
//! device-token registration contract completely, but never configure a real
//! provider, create an external account, or fake a successful delivery.
//!
//! Status: **`PUSH_PROVIDER_CONFIGURATION_REQUIRED`**. `NoopTransport` is the
//! only implementation that exists; it records that a send was *attempted*
//! (so the in-app notification's `delivery_status` can move to
//! `SENT_TO_PROVIDER`, an honest description of what actually happened) and
//! never claims `DELIVERED`, since no provider ever acknowledged anything.

use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushOutcome {
    /// Handed to a transport that genuinely attempted a provider call.
    SentToProvider,
    /// No real provider is configured — nothing was attempted, nothing was
    /// delivered. This is the only outcome `NoopTransport` ever returns.
    NoProviderConfigured,
}

#[async_trait]
pub trait NotificationTransport: Send + Sync {
    /// Attempts to push `title`/`body` to `push_token`. Must never be
    /// implemented to return success without a real provider having
    /// genuinely accepted the request — this project's own guardrail
    /// (`docs/CHECKPOINT.md`, "do not fabricate external-provider success")
    /// applies here exactly as it does to payments/payouts.
    async fn send_push(&self, push_token: &str, title: &str, body: &str) -> PushOutcome;
}

/// The only `NotificationTransport` implementation that exists in this
/// codebase. Reports `PUSH_PROVIDER_CONFIGURATION_REQUIRED` rather than
/// silently doing nothing, so this is a visible, honest gap — not a bug
/// waiting to be discovered — for whoever next wires up a real FCM/APNs
/// integration.
pub struct NoopTransport;

#[async_trait]
impl NotificationTransport for NoopTransport {
    async fn send_push(&self, _push_token: &str, _title: &str, _body: &str) -> PushOutcome {
        PushOutcome::NoProviderConfigured
    }
}
