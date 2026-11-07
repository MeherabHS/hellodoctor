//! Email delivery orchestration (Phase 0).
//!
//! OTP delivery — Patient (email+password login/registration), Doctor and
//! Admin second-factor codes, and Phase 2 password-reset codes — all send
//! through this module's must-succeed-or-fail path. Login OTP content is
//! the branded `services::email_templates` templates (see that module's
//! "Branded login-OTP templates" section for the cutover); `otp_email_content`
//! below is kept only as a fallback/reference for the pre-cutover plain-text
//! wording and by its own unit test.
//!
//! Durable tracking lives in `repository::email_outbox_repo` — one row per
//! send attempt, status/attempt-count/sanitized-failure-reason only, never
//! the OTP code itself (the code is never persisted anywhere outside the
//! existing, already-hashed challenge tables this module's callers manage).
//!
//! Retry for a `Retryable` provider outcome happens **inline**, within the
//! same request, bounded to a small, fixed number of extra attempts with a
//! short backoff — never deferred to the 60-second notification scheduler.
//! Deferring it would require persisting the plaintext code somewhere for a
//! later retry to re-send (a secret this codebase otherwise never writes to
//! disk), and would make a user waiting on a login screen wait up to a
//! minute for nothing. The scheduler's only involvement here is
//! `run_email_outbox_expiry_sweep`, a periodic reconciliation pass that
//! closes out rows an interrupted process never finished — it never
//! resends.

use crate::error::AppError;
use crate::repository::{email_outbox_repo, AppState};
use crate::services::email_transport::{EmailMessage, EmailOutcome};
use chrono::{DateTime, Utc};
use std::time::Duration;
use uuid::Uuid;

/// Which login/registration realm an OTP email belongs to — purely a label
/// for `email_outbox.email_type` and tracing; never a behavior branch (the
/// message content and delivery logic are identical across realms).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtpEmailKind {
    Patient,
    Doctor,
    Admin,
    /// Phase 2: Secure Email OTP Password Recovery. Shares this module's
    /// must-succeed-or-fail delivery semantics with the three login kinds
    /// (never the fire-and-forget `TransactionalEmailKind` path) because a
    /// reset code the user never receives must fail the request, not be
    /// silently swallowed.
    PasswordReset,
}

impl OtpEmailKind {
    fn as_db_value(self) -> &'static str {
        match self {
            Self::Patient => "PATIENT_OTP",
            Self::Doctor => "DOCTOR_OTP",
            Self::Admin => "ADMIN_OTP",
            Self::PasswordReset => "PASSWORD_RESET_OTP",
        }
    }

    fn as_dedup_label(self) -> &'static str {
        match self {
            Self::Patient => "patient",
            Self::Doctor => "doctor",
            Self::Admin => "admin",
            Self::PasswordReset => "password-reset",
        }
    }
}

/// Up to this many total attempts (the first attempt plus this many
/// retries) for a provider outcome classified `Retryable`. Deliberately
/// small: this runs inline inside a login/registration HTTP request with a
/// user actively waiting on the result — see the module doc for why this is
/// never deferred to the background scheduler instead.
const MAX_INLINE_ATTEMPTS: u32 = 2;
const INLINE_RETRY_BACKOFF: Duration = Duration::from_millis(500);

/// Builds the exact OTP email content — unchanged from the pre-Phase-0
/// implementation (plain text only, no HTML; generic wording with no
/// PHI/account detail beyond the code itself). Kept as its own pure function
/// so it can be unit-tested without any transport/config/network
/// involvement, mirroring `notification_transport::build_fcm_message`'s
/// rationale.
pub fn otp_email_content(code: &str) -> (&'static str, String) {
    (
        "AfyaMkononi verification code",
        format!(
            "Your AfyaMkononi verification code is {code}. It expires in 5 minutes. If you did not request this, ignore this email."
        ),
    )
}

/// Sends an OTP email and durably tracks the attempt. Preserves the exact
/// pre-Phase-0 behavior callers already depend on:
/// - The development bypass (`ENVIRONMENT=development` and
///   `DEV_AUTH_OTP_ENABLED=true`) still short-circuits before any outbox
///   row, transport call, or network attempt — identical to the original
///   `send_email_otp`'s first check.
/// - The two error messages already surfaced to callers
///   (`"Azure email OTP delivery is not configured."` /
///   `"Azure email delivery failed."`) are unchanged in wording, so existing
///   call-site error-handling in `auth_service.rs` (delete the just-inserted
///   challenge, propagate the error) is unaffected. The *status code* behind
///   `"Azure email delivery failed."` was corrected from 500 (`Internal`) to
///   503 (`ServiceUnavailable`) — see the call sites below — since this is a
///   dependency failing, not an unhandled bug; no caller branched on the old
///   status code.
pub async fn send_otp_email_tracked(
    state: &AppState,
    recipient: &str,
    code: &str,
    kind: OtpEmailKind,
    correlation_token: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), AppError> {
    let (subject, body) = otp_email_content(code);
    send_otp_email_tracked_inner(
        state,
        recipient,
        kind,
        correlation_token,
        expires_at,
        subject,
        &body,
        None,
    )
    .await
}

/// Phase 2: identical delivery contract to `send_otp_email_tracked` (same
/// outbox tracking, dev bypass, must-propagate-failure semantics, inline
/// retry budget), but sourcing its content from a branded
/// `email_templates::RenderedEmail` instead of the plain-text
/// `otp_email_content` — used for the new password-reset OTP, never for the
/// three existing login kinds (their live content is deliberately
/// unchanged; see `services::email_templates`'s module doc for the dormant
/// branded login-OTP templates awaiting a separate cutover decision).
pub async fn send_rendered_otp_email_tracked(
    state: &AppState,
    recipient: &str,
    rendered: &crate::services::email_templates::RenderedEmail,
    kind: OtpEmailKind,
    correlation_token: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), AppError> {
    send_otp_email_tracked_inner(
        state,
        recipient,
        kind,
        correlation_token,
        expires_at,
        &rendered.subject,
        &rendered.text_body,
        Some(&rendered.html_body),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn send_otp_email_tracked_inner(
    state: &AppState,
    recipient: &str,
    kind: OtpEmailKind,
    correlation_token: &str,
    expires_at: DateTime<Utc>,
    subject: &str,
    body: &str,
    html_body: Option<&str>,
) -> Result<(), AppError> {
    if state.config.environment == "development" && state.config.dev_auth_otp_enabled {
        return Ok(());
    }
    if !state.email_transport.is_configured() {
        return Err(AppError::Conflict(
            "Azure email OTP delivery is not configured.".into(),
        ));
    }

    let dedup_key = format!("otp-email:{}:{correlation_token}", kind.as_dedup_label());
    let outbox_id = Uuid::new_v4();
    email_outbox_repo::insert_if_new(
        &state.auth_db,
        outbox_id,
        kind.as_db_value(),
        recipient,
        &dedup_key,
        expires_at,
    )
    .await?;

    let message = EmailMessage {
        recipient,
        subject,
        plain_text_body: body,
        html_body,
    };

    let mut attempts: u32 = 0;
    loop {
        attempts += 1;
        match state.email_transport.send_email(&message).await {
            EmailOutcome::SentToProvider {
                provider_message_id,
            } => {
                email_outbox_repo::mark_sent(
                    &state.auth_db,
                    outbox_id,
                    provider_message_id.as_deref(),
                )
                .await?;
                return Ok(());
            }
            EmailOutcome::NoProviderConfigured => {
                email_outbox_repo::mark_failed(
                    &state.auth_db,
                    outbox_id,
                    "No email provider configured",
                )
                .await?;
                return Err(AppError::Conflict(
                    "Azure email OTP delivery is not configured.".into(),
                ));
            }
            EmailOutcome::Permanent { reason } => {
                tracing::error!(
                    status_class = "permanent",
                    attempts,
                    "OTP email delivery rejected"
                );
                email_outbox_repo::record_attempt(
                    &state.auth_db,
                    outbox_id,
                    attempts as i32,
                    Some(&reason),
                )
                .await?;
                email_outbox_repo::mark_failed(&state.auth_db, outbox_id, &reason).await?;
                // `ServiceUnavailable` (503), not `internal` (500): the OTP
                // delivery dependency itself rejected/could not complete the
                // request — a caller must retry later, this is not "the
                // server has a genuine unhandled bug" (see `AppError::ServiceUnavailable`'s
                // own doc comment, B-05 in docs/audit/DOCTOR_SECURITY_BUG_AUDIT.md).
                // Login must still fail closed either way — this only fixes
                // the status code a caller observes, never whether the
                // request succeeds.
                return Err(AppError::service_unavailable(
                    "Azure email delivery failed.".into(),
                ));
            }
            EmailOutcome::Retryable { reason } => {
                email_outbox_repo::record_attempt(
                    &state.auth_db,
                    outbox_id,
                    attempts as i32,
                    Some(&reason),
                )
                .await?;
                if attempts >= MAX_INLINE_ATTEMPTS {
                    tracing::error!(
                        status_class = "retryable_exhausted",
                        attempts,
                        "OTP email delivery exhausted its inline retry budget"
                    );
                    email_outbox_repo::mark_failed(&state.auth_db, outbox_id, &reason).await?;
                    return Err(AppError::service_unavailable(
                        "Azure email delivery failed.".into(),
                    ));
                }
                tokio::time::sleep(INLINE_RETRY_BACKOFF * attempts).await;
            }
        }
    }
}

/// Periodic reconciliation only — see `email_outbox_repo::expire_stale`'s
/// doc comment. Called from `notification_scheduler`'s existing 60-second
/// tick loop so this doesn't need its own scheduler/task.
pub async fn run_email_outbox_expiry_sweep(state: &AppState) -> Result<u64, sqlx::Error> {
    email_outbox_repo::expire_stale(&state.system_db).await
}

/// Phase 2: which business event a transactional email belongs to — a label
/// for `email_outbox.email_type`. Unlike OTP, these all run on the
/// `hellodoctor_system_worker` pool (see migrations/20261206000001), never
/// `hellodoctor_auth_worker` — most of these triggers (doctor review,
/// booking, payment, refund) have nothing to do with the auth worker's
/// capability scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionalEmailKind {
    PatientWelcome,
    DoctorApplicationReceived,
    DoctorApproved,
    DoctorRejected,
    ConsultationConfirmed,
    ConsultationRescheduled,
    PaymentConfirmed,
    RefundProcessed,
    PasswordChanged,
}

impl TransactionalEmailKind {
    fn as_db_value(self) -> &'static str {
        match self {
            Self::PatientWelcome => "PATIENT_WELCOME",
            Self::DoctorApplicationReceived => "DOCTOR_APPLICATION_RECEIVED",
            Self::DoctorApproved => "DOCTOR_APPROVED",
            Self::DoctorRejected => "DOCTOR_REJECTED",
            Self::ConsultationConfirmed => "CONSULTATION_CONFIRMED",
            Self::ConsultationRescheduled => "CONSULTATION_RESCHEDULED",
            Self::PaymentConfirmed => "PAYMENT_CONFIRMED",
            Self::RefundProcessed => "REFUND_PROCESSED",
            Self::PasswordChanged => "PASSWORD_CHANGED",
        }
    }
}

/// No natural expiry exists for a transactional email the way an OTP has a
/// challenge lifetime — this is purely outbox hygiene (so an interrupted
/// send doesn't sit as `CREATED` forever), never a validity window on the
/// content itself (there is no deferred retry of stale content: delivery is
/// always attempted exactly once, inline, at call time).
const TRANSACTIONAL_EMAIL_TTL_HOURS: i64 = 24;

/// Generous, single shared budget across every transactional-email category
/// for one recipient — defense-in-depth against a bug or malicious actor
/// generating many *distinct* legitimate-looking events (each with its own
/// `dedup_key`, so the outbox's own per-event uniqueness can't catch this)
/// to spam one mailbox or burn Azure send quota. Not a per-category budget:
/// the invariant is "this mailbox doesn't get flooded by us," regardless of
/// which of the 9 categories is doing the flooding.
const TRANSACTIONAL_EMAIL_RECIPIENT_LIMIT: u32 = 20;
const TRANSACTIONAL_EMAIL_RECIPIENT_WINDOW_SECS: u64 = 86_400;

/// Sends a rendered Phase 1A/2 template through the same durable outbox +
/// bounded-inline-retry pipeline `send_otp_email_tracked` uses, generalized
/// for arbitrary HTML/text content.
///
/// Unlike OTP, this **never propagates a delivery failure (or a rate-limit
/// hit) to the caller** — every call site is a best-effort notification
/// about a business event that has *already succeeded* (doctor approved,
/// payment confirmed, etc.). That event's own success must never depend on
/// whether its informational email went out; every failure mode here is
/// logged, not returned. For the same reason the abuse limiter fails
/// *open* on a Redis outage (skip the check, still attempt the send) rather
/// than fail-closed the way the security-critical OTP delivery limiter
/// does — missing one transactional email during a Redis blip is a far
/// smaller risk than silently discarding every transactional email the
/// platform ever tries to send during that blip.
///
/// Call this **after** the triggering transaction has committed, never from
/// inside it — same principle `send_otp_email_tracked` already follows: a
/// network call must never hold a database transaction open.
pub async fn send_templated_email(
    state: &AppState,
    recipient: &str,
    rendered: &crate::services::email_templates::RenderedEmail,
    kind: TransactionalEmailKind,
    dedup_key: &str,
) {
    let recipient = recipient.trim();
    if recipient.is_empty() {
        tracing::warn!(
            email_type = kind.as_db_value(),
            "Skipping transactional email: recipient has no email on file"
        );
        return;
    }
    if !state.email_transport.is_configured() {
        tracing::info!(
            email_type = kind.as_db_value(),
            "Transactional email not sent: Azure email is not configured"
        );
        return;
    }

    let recipient_key = format!(
        "ratelimit:transactional-email:{}",
        crate::crypto::hash_code(&recipient.to_lowercase())
    );
    if let Err(error) = crate::rate_limit::check_and_increment(
        &state.redis,
        &recipient_key,
        TRANSACTIONAL_EMAIL_RECIPIENT_LIMIT,
        TRANSACTIONAL_EMAIL_RECIPIENT_WINDOW_SECS,
    )
    .await
    {
        tracing::warn!(
            email_type = kind.as_db_value(),
            error = ?error,
            "Transactional email skipped (rate limit reached or limiter unavailable)"
        );
        return;
    }

    let outbox_id = Uuid::new_v4();
    let expires_at = Utc::now() + chrono::Duration::hours(TRANSACTIONAL_EMAIL_TTL_HOURS);
    match email_outbox_repo::insert_if_new(
        &state.system_db,
        outbox_id,
        kind.as_db_value(),
        recipient,
        dedup_key,
        expires_at,
    )
    .await
    {
        Ok(true) => {}
        // Unlike OTP's dedup_key (defensive-only by design — each OTP call
        // already carries a freshly generated correlation token, so a
        // collision can't happen in practice), a transactional event's
        // dedup_key is meant to provide real idempotency: the same
        // `dedup_key` means "this exact business event already has an
        // outbox row," so a second call (e.g. a retried request reaching
        // this far despite the triggering transaction's own conflict
        // guards) must not send a second email.
        Ok(false) => {
            tracing::info!(
                email_type = kind.as_db_value(),
                "Transactional email skipped: already recorded for this event (dedup_key)"
            );
            return;
        }
        Err(error) => {
            tracing::error!(
                email_type = kind.as_db_value(),
                error = ?error,
                "Failed to record transactional email outbox row"
            );
            return;
        }
    }

    let message = EmailMessage {
        recipient,
        subject: &rendered.subject,
        plain_text_body: &rendered.text_body,
        html_body: Some(&rendered.html_body),
    };

    let mut attempts: u32 = 0;
    loop {
        attempts += 1;
        match state.email_transport.send_email(&message).await {
            EmailOutcome::SentToProvider {
                provider_message_id,
            } => {
                let _ = email_outbox_repo::mark_sent(
                    &state.system_db,
                    outbox_id,
                    provider_message_id.as_deref(),
                )
                .await;
                return;
            }
            EmailOutcome::NoProviderConfigured => {
                let _ = email_outbox_repo::mark_failed(
                    &state.system_db,
                    outbox_id,
                    "No email provider configured",
                )
                .await;
                return;
            }
            EmailOutcome::Permanent { reason } => {
                tracing::error!(
                    email_type = kind.as_db_value(),
                    status_class = "permanent",
                    attempts,
                    "Transactional email delivery rejected"
                );
                let _ = email_outbox_repo::record_attempt(
                    &state.system_db,
                    outbox_id,
                    attempts as i32,
                    Some(&reason),
                )
                .await;
                let _ = email_outbox_repo::mark_failed(&state.system_db, outbox_id, &reason).await;
                return;
            }
            EmailOutcome::Retryable { reason } => {
                let _ = email_outbox_repo::record_attempt(
                    &state.system_db,
                    outbox_id,
                    attempts as i32,
                    Some(&reason),
                )
                .await;
                if attempts >= MAX_INLINE_ATTEMPTS {
                    tracing::error!(
                        email_type = kind.as_db_value(),
                        status_class = "retryable_exhausted",
                        attempts,
                        "Transactional email exhausted its inline retry budget"
                    );
                    let _ =
                        email_outbox_repo::mark_failed(&state.system_db, outbox_id, &reason).await;
                    return;
                }
                tokio::time::sleep(INLINE_RETRY_BACKOFF * attempts).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn otp_email_content_preserves_the_pre_phase_0_wording_and_embeds_the_code() {
        let (subject, body) = otp_email_content("123456");
        assert_eq!(subject, "AfyaMkononi verification code");
        assert!(body.contains("123456"));
        assert!(body.contains("expires in 5 minutes"));
        assert!(body.contains("If you did not request this, ignore this email."));
    }

    #[test]
    fn otp_email_kind_labels_are_stable_and_distinct() {
        let kinds = [
            OtpEmailKind::Patient,
            OtpEmailKind::Doctor,
            OtpEmailKind::Admin,
            OtpEmailKind::PasswordReset,
        ];
        let db_values: Vec<_> = kinds.iter().map(|k| k.as_db_value()).collect();
        assert_eq!(
            db_values,
            [
                "PATIENT_OTP",
                "DOCTOR_OTP",
                "ADMIN_OTP",
                "PASSWORD_RESET_OTP"
            ]
        );
        let labels: Vec<_> = kinds.iter().map(|k| k.as_dedup_label()).collect();
        assert_eq!(labels, ["patient", "doctor", "admin", "password-reset"]);
    }
}
