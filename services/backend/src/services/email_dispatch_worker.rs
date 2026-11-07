//! Phase 1B: background dispatch for transactional (non-OTP) email.
//!
//! A business handler enqueues a *thin* `email_outbox` row (see
//! `repository::email_outbox_repo::enqueue_transactional`'s doc comment) on
//! its own transaction, atomically with the business event. This module
//! resolves fresh display content for that row at send time and actually
//! delivers it, via two entry points sharing one core (`process_claim`):
//!
//! - `dispatch_now`: a best-effort, fire-and-forget attempt at exactly one
//!   row, called right after its creating transaction commits — gives low
//!   latency in the common case without making the triggering HTTP request
//!   wait on it (see each call site's own `tokio::spawn`).
//! - `run_transactional_email_dispatch_pass`: the durable fallback, called
//!   from the existing 60-second scheduler tick
//!   (`notification_scheduler::start`). Claims a bounded batch of due rows
//!   — anything `dispatch_now` missed (process restart before it ran, a
//!   transient failure now past its backoff, a stale lease recovered from a
//!   crashed attempt) — and processes each the same way.
//!
//! Retry/backoff mirrors `notification_scheduler`'s own push-delivery
//! formula (`30 * 2^attempts`, capped at 3600s) for consistency, not because
//! the two providers share any code.

use crate::domain::ids::{DoctorProfileId, UserId};
use crate::error::AppError;
use crate::repository::{email_outbox_repo, AppState};
use crate::services::email_templates::{self as tmpl, RenderedEmail};
use crate::services::email_transport::{EmailMessage, EmailOutcome};
use chrono::Utc;
use uuid::Uuid;

/// How long a claim is held before it's considered abandoned and eligible
/// for another worker (or this same worker, next tick) to reclaim — see
/// `email_outbox_repo::claim_due_transactional`'s stale-lease-recovery doc.
/// Generous relative to how long an actual send ever takes (bounded by
/// `email_transport::ACS_REQUEST_TIMEOUT`, ~8s): a crashed worker's claim
/// becomes reclaimable within two minutes, not stuck forever.
const LEASE_SECONDS: i64 = 120;
const BATCH_LIMIT: i64 = 50;
/// Separate, larger budget than OTP's `MAX_INLINE_ATTEMPTS` (2) — these
/// retries are spread across ticks with real backoff, not inline sleeps
/// inside a request a user is waiting on, so a more generous budget before
/// giving up is the right tradeoff here.
const MAX_ATTEMPTS: i32 = 8;

/// Best-effort immediate dispatch for one specific row. Never propagates an
/// error to its caller — see each call site, which always spawns this
/// detached rather than awaiting it inline, so a slow/degraded provider can
/// never delay the booking/approval/payment response that already committed
/// before this runs.
pub async fn dispatch_now(state: &AppState, outbox_id: Uuid) {
    // Matches `run_transactional_email_dispatch_pass`'s own guard: an
    // unconfigured transport must never even claim the row. Claiming and
    // then immediately marking it permanently `FAILED` (via
    // `EmailOutcome::NoProviderConfigured`) would make a row created before
    // a provider is configured un-recoverable forever — including, in
    // development/test, across the entire lifetime of a process that never
    // configures one at all — rather than staying `CREATED` for the next
    // attempt (by this same path, or the periodic pass) once one exists.
    if !state.email_transport.is_configured() {
        return;
    }
    match email_outbox_repo::claim_specific_transactional(
        &state.system_db,
        outbox_id,
        LEASE_SECONDS,
    )
    .await
    {
        Ok(Some(claim)) => {
            if let Err(error) = process_claim(state, claim).await {
                tracing::warn!(
                    error = ?error,
                    outbox_id = %outbox_id,
                    "immediate transactional email dispatch failed; left for the periodic worker"
                );
            }
        }
        // Already claimed by the periodic worker, already sent, or not
        // configured — all fine, nothing further to do here.
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(
                error = ?error,
                outbox_id = %outbox_id,
                "could not claim outbox row for immediate dispatch"
            );
        }
    }
}

/// Periodic durable pass. Returns the number of rows claimed this tick.
pub async fn run_transactional_email_dispatch_pass(state: &AppState) -> Result<u64, AppError> {
    if !state.email_transport.is_configured() {
        return Ok(0);
    }
    let claims =
        email_outbox_repo::claim_due_transactional(&state.system_db, LEASE_SECONDS, BATCH_LIMIT)
            .await?;
    let claimed = claims.len() as u64;
    for claim in claims {
        let id = claim.id;
        if let Err(error) = process_claim(state, claim).await {
            tracing::error!(error = ?error, outbox_id = %id, "transactional email dispatch failed");
        }
    }
    Ok(claimed)
}

async fn process_claim(
    state: &AppState,
    claim: email_outbox_repo::ClaimedTransactionalEmail,
) -> Result<(), AppError> {
    let rendered = match resolve_and_render(state, &claim).await {
        Ok(Some(rendered)) => rendered,
        Ok(None) => {
            // The referenced entity (or a row it depends on) is gone or in
            // an unexpected state — not a transient condition retrying would
            // fix, and not something we can render a truthful email about.
            email_outbox_repo::mark_transactional_permanent_failed(
                &state.system_db,
                claim.id,
                "Referenced entity could not be resolved at dispatch time",
            )
            .await?;
            return Ok(());
        }
        Err(error) => {
            schedule_retry_or_fail(
                state,
                &claim,
                &format!("Content resolution failed: {error}"),
            )
            .await?;
            return Ok(());
        }
    };
    let message = EmailMessage {
        recipient: &claim.recipient_email,
        subject: &rendered.subject,
        plain_text_body: &rendered.text_body,
        html_body: Some(&rendered.html_body),
    };
    match state.email_transport.send_email(&message).await {
        EmailOutcome::SentToProvider {
            provider_message_id,
        } => {
            email_outbox_repo::mark_transactional_sent(
                &state.system_db,
                claim.id,
                provider_message_id.as_deref(),
            )
            .await?;
        }
        EmailOutcome::NoProviderConfigured => {
            email_outbox_repo::mark_transactional_permanent_failed(
                &state.system_db,
                claim.id,
                "No email provider configured",
            )
            .await?;
        }
        EmailOutcome::Permanent { reason } => {
            email_outbox_repo::mark_transactional_permanent_failed(
                &state.system_db,
                claim.id,
                &reason,
            )
            .await?;
        }
        EmailOutcome::Retryable { reason } => {
            schedule_retry_or_fail(state, &claim, &reason).await?;
        }
    }
    Ok(())
}

async fn schedule_retry_or_fail(
    state: &AppState,
    claim: &email_outbox_repo::ClaimedTransactionalEmail,
    reason: &str,
) -> Result<(), AppError> {
    let attempts_after_this_one = claim.delivery_attempts + 1;
    if attempts_after_this_one >= MAX_ATTEMPTS {
        email_outbox_repo::mark_transactional_permanent_failed(&state.system_db, claim.id, reason)
            .await?;
        return Ok(());
    }
    let exponent = (attempts_after_this_one - 1).clamp(0, 6) as u32;
    let delay_secs = (30_i64 * 2_i64.pow(exponent)).min(3600);
    let next_attempt_at = Utc::now() + chrono::Duration::seconds(delay_secs);
    email_outbox_repo::mark_transactional_retry(
        &state.system_db,
        claim.id,
        reason,
        next_attempt_at,
    )
    .await?;
    Ok(())
}

async fn resolve_and_render(
    state: &AppState,
    claim: &email_outbox_repo::ClaimedTransactionalEmail,
) -> Result<Option<RenderedEmail>, AppError> {
    match claim.email_type.as_str() {
        "PATIENT_WELCOME" => {
            let Some(first_name) =
                resolve_patient_first_name(state, claim.related_entity_id).await?
            else {
                return Ok(None);
            };
            Ok(Some(tmpl::render_patient_welcome(
                &first_name,
                &state.email_assets,
                &state.email_links,
            )))
        }
        "DOCTOR_APPLICATION_RECEIVED" => {
            let Some(name) = resolve_doctor_full_name(state, claim.related_entity_id).await? else {
                return Ok(None);
            };
            Ok(Some(tmpl::render_doctor_application_received(
                &name,
                &state.email_assets,
            )))
        }
        "DOCTOR_APPROVED" => {
            let Some(name) = resolve_doctor_full_name(state, claim.related_entity_id).await? else {
                return Ok(None);
            };
            Ok(Some(tmpl::render_doctor_approved(
                &name,
                &state.email_assets,
                &state.email_links,
            )))
        }
        "DOCTOR_REJECTED" => {
            let Some(name) = resolve_doctor_full_name(state, claim.related_entity_id).await? else {
                return Ok(None);
            };
            Ok(Some(tmpl::render_doctor_rejected(
                &name,
                &state.email_assets,
            )))
        }
        "CONSULTATION_CONFIRMED" => {
            resolve_consultation_confirmed(state, claim.related_entity_id).await
        }
        "CONSULTATION_RESCHEDULED" => resolve_rescheduled(state, claim.related_entity_id).await,
        "PAYMENT_CONFIRMED" => resolve_payment_confirmed(state, claim.related_entity_id).await,
        "REFUND_PROCESSED" => resolve_refund_processed(state, claim.related_entity_id).await,
        other => {
            tracing::error!(
                email_type = other,
                outbox_id = %claim.id,
                "unknown transactional email_type claimed by dispatch worker"
            );
            Ok(None)
        }
    }
}

async fn resolve_patient_first_name(
    state: &AppState,
    user_id: Uuid,
) -> Result<Option<String>, AppError> {
    let full_name =
        crate::repository::patient_repo::find_by_user_id(&state.system_db, UserId::from(user_id))
            .await?
            .map(|p| p.full_name);
    Ok(full_name
        .map(|n| n.split_whitespace().next().unwrap_or_default().to_string())
        .filter(|s| !s.is_empty()))
}

async fn resolve_doctor_full_name(
    state: &AppState,
    doctor_profile_id: Uuid,
) -> Result<Option<String>, AppError> {
    Ok(crate::repository::doctor_repo::find_by_id(
        &state.system_db,
        DoctorProfileId::from(doctor_profile_id),
    )
    .await?
    .map(|d| d.full_name))
}

async fn resolve_consultation_confirmed(
    state: &AppState,
    appointment_id: Uuid,
) -> Result<Option<RenderedEmail>, AppError> {
    let Some(details) =
        crate::services::AppointmentService::consultation_email_details(state, appointment_id)
            .await?
    else {
        return Ok(None);
    };
    let params = tmpl::ConsultationConfirmedParams {
        first_name: &details.patient_name,
        consultation_id: &details.appointment_number,
        appointment_date: &details.date_label,
        appointment_time: &details.time_label,
        timezone: "EAT",
        doctor_name: &details.doctor_name,
    };
    Ok(Some(tmpl::render_consultation_confirmed(
        &params,
        &state.email_assets,
        &state.email_links,
    )))
}

async fn resolve_rescheduled(
    state: &AppState,
    reschedule_id: Uuid,
) -> Result<Option<RenderedEmail>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        appointment_id: Uuid,
        new_start: chrono::DateTime<Utc>,
    }
    let row: Option<Row> =
        sqlx::query_as("SELECT appointment_id, new_start FROM appointment_reschedules WHERE id=$1")
            .bind(reschedule_id)
            .fetch_optional(&state.system_db)
            .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let Some(details) =
        crate::services::AppointmentService::consultation_email_details(state, row.appointment_id)
            .await?
    else {
        return Ok(None);
    };
    let local = row
        .new_start
        .with_timezone(&crate::services::notification_service::nairobi_offset());
    let params = tmpl::ConsultationRescheduledParams {
        first_name: &details.patient_name,
        consultation_id: &details.appointment_number,
        new_appointment_date: &local.format("%A, %d %B %Y").to_string(),
        new_appointment_time: &local.format("%-I:%M %p").to_string(),
        timezone: "EAT",
        doctor_name: &details.doctor_name,
    };
    Ok(Some(tmpl::render_consultation_rescheduled(
        &params,
        &state.email_assets,
        &state.email_links,
    )))
}

async fn resolve_payment_confirmed(
    state: &AppState,
    transaction_id: Uuid,
) -> Result<Option<RenderedEmail>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        appointment_id: Uuid,
        transaction_number: String,
        currency: String,
        gross_amount: bigdecimal::BigDecimal,
    }
    let row: Option<Row> = sqlx::query_as(
        "SELECT appointment_id, transaction_number, currency, gross_amount \
         FROM transactions WHERE id=$1",
    )
    .bind(transaction_id)
    .fetch_optional(&state.system_db)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let Some(details) =
        crate::services::AppointmentService::consultation_email_details(state, row.appointment_id)
            .await?
    else {
        return Ok(None);
    };
    let params = tmpl::PaymentConfirmedParams {
        first_name: &details.patient_name,
        transaction_id: &row.transaction_number,
        consultation_id: &details.appointment_number,
        currency: &row.currency,
        amount_paid: &format!("{}", row.gross_amount),
        payment_date: &details.date_label,
    };
    Ok(Some(tmpl::render_payment_confirmed(
        &params,
        &state.email_assets,
        &state.email_links,
    )))
}

async fn resolve_refund_processed(
    state: &AppState,
    refund_request_id: Uuid,
) -> Result<Option<RenderedEmail>, AppError> {
    let Some(refund) =
        crate::repository::grievance_repo::find_refund_request(&state.system_db, refund_request_id)
            .await?
    else {
        return Ok(None);
    };
    // Defense in depth: the enqueueing call site only ever enqueues this
    // email_type when `to_status == "REFUNDED"`, but re-checking the live
    // row here means a row that was somehow later reverted (there is no
    // such transition today, but this never trusts that absence) is never
    // mis-rendered as "your refund was processed."
    if refund.status != "REFUNDED" {
        return Ok(None);
    }
    #[derive(sqlx::FromRow)]
    struct TxnRow {
        appointment_id: Uuid,
        transaction_number: String,
    }
    let txn: Option<TxnRow> =
        sqlx::query_as("SELECT appointment_id, transaction_number FROM transactions WHERE id=$1")
            .bind(refund.transaction_id)
            .fetch_optional(&state.system_db)
            .await?;
    let Some(txn) = txn else {
        return Ok(None);
    };
    let Some(details) =
        crate::services::AppointmentService::consultation_email_details(state, txn.appointment_id)
            .await?
    else {
        return Ok(None);
    };
    let refund_date = refund
        .refunded_at
        .unwrap_or_else(Utc::now)
        .with_timezone(&crate::services::notification_service::nairobi_offset())
        .format("%A, %d %B %Y")
        .to_string();
    let params = tmpl::RefundProcessedParams {
        first_name: &details.patient_name,
        refund_reference: refund.provider_reference.as_deref().unwrap_or_default(),
        transaction_id: &txn.transaction_number,
        consultation_id: &details.appointment_number,
        currency: &refund.currency,
        refund_amount: &format!("{}", refund.amount),
        refund_date: &refund_date,
    };
    Ok(Some(tmpl::render_refund_processed(
        &params,
        &state.email_assets,
        &state.email_links,
    )))
}
