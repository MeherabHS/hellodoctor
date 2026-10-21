use crate::abac::{self, Action};
use crate::api::middleware::idempotency::Idempotency;
use crate::auth_context::AuthContext;
use crate::domain::ids::{DoctorProfileId, PatientProfileId};
use crate::domain::models::*;
use crate::error::AppError;
use crate::rate_limit;
use crate::repository::{
    appointment_repo, chat_repo, consultation_repo, payment_methods_repo, AppState,
};
use crate::services::AppointmentService;
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct BookAppointmentRequest {
    pub doctor_id: DoctorProfileId,
    pub slot_id: Uuid,
    pub modality: Modality,
    /// Required only when `payment_method_id` is omitted — selecting a
    /// saved method resolves the gateway from it instead (see
    /// `patient_payment_methods`), so a client that already lets the
    /// patient pick a saved method never needs to send this too.
    pub gateway: Option<Gateway>,
    /// A previously-saved `patient_payment_methods` row to pay with — the
    /// patient picks it at checkout instead of entering payment details
    /// fresh every time. Re-validated server-side (ownership + active) in
    /// `AppointmentService::book_appointment`, never trusted from the
    /// client alone.
    pub payment_method_id: Option<Uuid>,
    pub chief_complaint: Option<String>,
    /// Which of the caller's profiles (self or a guardianed dependent) this
    /// consultation is for — the caller's own self profile when omitted. See
    /// `AuthContext::acting_patient_profile_id`.
    pub profile_id: Option<PatientProfileId>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn book_appointment(
    State(state): State<AppState>,
    ctx: AuthContext,
    idempotency: Idempotency,
    Json(payload): Json<BookAppointmentRequest>,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::BookAppointment)?;
    rate_limit::check_booking_limit(&state.redis, &ctx.user_id.to_string()).await?;

    let chief_complaint = payload
        .chief_complaint
        .map(|c| crate::domain::validation::validate_text_field(&c, "Chief complaint", 1000))
        .transpose()?;

    let patient_id = ctx
        .acting_patient_profile_id(&state, payload.profile_id)
        .await?;

    // Resolving the gateway here (read-only, before the idempotency replay
    // check and before AppointmentService opens its own write transaction)
    // keeps that service function's own re-validation of ownership/active
    // status as the single authoritative check — this lookup only decides
    // *which* gateway to proceed with, never whether the booking itself is
    // allowed to use it.
    let gateway = match (payload.gateway, payload.payment_method_id) {
        (Some(g), _) => g,
        (None, Some(method_id)) => {
            // `patient_payment_methods` is RLS-protected by caller identity
            // (`fn_patient_profile_owned_by_caller`), which needs
            // `app.current_user_id` set — `state.db` carries no such
            // session context, so this must run inside a properly authed
            // transaction, not against the raw pool (which would silently
            // see zero rows for everyone, including the rightful owner).
            let mut lookup_tx = state.begin_authed(&ctx).await?;
            let method =
                payment_methods_repo::find_patient_payment_method(&mut *lookup_tx, method_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::NotFound("No saved payment method with that id.".into())
                    })?;
            lookup_tx.commit().await?;
            method.gateway
        }
        (None, None) => {
            return Err(AppError::Validation(
                "Either gateway or payment_method_id is required.".into(),
            ))
        }
    };

    if let Some(response) = idempotency.replay(&state, &ctx, None).await? {
        return Ok(response);
    }
    let res = AppointmentService::book_appointment(
        &state,
        &ctx,
        patient_id,
        payload.doctor_id,
        payload.slot_id,
        payload.modality,
        gateway,
        chief_complaint,
        payload.payment_method_id,
    )
    .await?;

    Ok(idempotency.finish(
        res.appointment_id,
        (
            axum::http::StatusCode::CREATED,
            Json(ApiResponse {
                success: true,
                data: serde_json::json!({
                    "appointment_id": res.appointment_id,
                    "appointment_number": res.appointment_number,
                    "status": res.status,
                    "payment_session_id": res.payment_session_id,
                    "payment_redirect_url": res.payment_redirect_url,
                    "amount": res.amount
                }),
            }),
        )
            .into_response(),
    ))
}

pub async fn get_appointment(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;

    let mut tx = state.begin_authed(&ctx).await?;
    let apt = appointment_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
    crate::api::middleware::request_context::bind_consultation(id, None);
    let payment: Option<String> =
        sqlx::query_scalar("SELECT payment_status::text FROM transactions WHERE appointment_id=$1 ORDER BY (payment_status<>'FAILED') DESC,created_at DESC,id DESC LIMIT 1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    tx.commit().await?;
    let mut data = serde_json::to_value(apt).map_err(|e| AppError::internal(e.to_string()))?;
    data["payment_status"] = serde_json::json!(payment);
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

/// Resume the existing payment attempt, never book or charge again. There is
/// currently no configured collection adapter; fail honestly after ownership
/// and lifecycle checks rather than returning the old placeholder URL.
pub async fn initiate_payment(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::BookAppointment)?;
    rate_limit::check_and_increment(
        &state.redis,
        &format!("ratelimit:payment-initiate:{}", ctx.user_id),
        10,
        60,
    )
    .await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let apt = appointment_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
    if apt.status != AppointmentStatus::PendingPayment {
        return Err(AppError::Conflict(
            "Payment is not required for this consultation state.".into(),
        ));
    }
    let future: bool = sqlx::query_scalar(
        "SELECT start_time > clock_timestamp() FROM doctor_schedule_slots WHERE id=$1",
    )
    .bind(apt.slot_id)
    .fetch_one(&mut *tx)
    .await?;
    if !future {
        return Err(AppError::Conflict(
            "This appointment time has passed. Payment cannot be started.".into(),
        ));
    }
    // INV-BOOK-03: never start a payment against a reservation whose deadline
    // has passed - the slot may already be offered to someone else. Legacy
    // holds without a deadline fall back to created_at + the configured window.
    let hold_live: bool = sqlx::query_scalar(
        "SELECT COALESCE(hold_expires_at, created_at + make_interval(secs => $2)) > clock_timestamp() FROM appointments WHERE id=$1",
    )
    .bind(id)
    .bind(state.config.payment_reservation_window_secs as f64)
    .fetch_one(&mut *tx)
    .await?;
    if !hold_live {
        return Err(AppError::Conflict(
            "This booking hold has expired. Please choose a time again.".into(),
        ));
    }
    let existing: Option<Uuid> =
        sqlx::query_scalar("SELECT payment_session_id FROM transactions WHERE appointment_id=$1 AND payment_status='INITIATED' ORDER BY created_at DESC,id DESC LIMIT 1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    tx.commit().await?;
    // Release the first API connection before resolving guardian ownership.
    // Concurrent payment taps must not exhaust the pool with nested reads.
    ctx.acting_patient_profile_id(&state, Some(apt.patient_id))
        .await?;
    if existing.is_none() {
        return Err(AppError::Conflict(
            "Payment is unavailable for this booking. Please contact support.".into(),
        ));
    }
    crate::services::consultation_trace::rejected(
        &state,
        &ctx,
        id,
        None,
        "PAYMENT_INITIATION_REJECTED",
        "PROVIDER_UNAVAILABLE",
    )
    .await?;
    Err(AppError::Conflict(
        "Payment service is currently unavailable. Please try again later.".into(),
    ))
}

/// Resolves the `conversation_id` for an appointment's 24-hour clinical chat
/// (plan's gap #3 — "no way to resolve this" for a patient, confirmed by
/// direct router grep before this was added; `GET /doctor/chats` already
/// gave doctors a list, but a patient had no equivalent lookup at all).
/// `chat_repo::find_conversation_by_appointment` already existed — this is
/// purely a missing read endpoint, not new persistence. Authorization is
/// entirely RLS (`chat_conversations_access`, already extended for
/// guardian/dependent in the §5b migration): a caller who cannot see the
/// conversation under RLS gets a 404, the same non-leaky shape as
/// `get_appointment`, not a specially-worded "forbidden".
pub async fn get_conversation_for_appointment(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;

    let mut tx = state.begin_authed(&ctx).await?;
    let appointment = appointment_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
    let mut conversation = chat_repo::find_conversation_by_appointment(&mut *tx, id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound("No chat conversation exists for this appointment.".into())
        })?;
    tx.commit().await?;

    // Read-only history remains accessible to authorized participants. A closed
    // appointment must not present an editable thread, even before lazy locking.
    if !matches!(
        appointment.status,
        crate::domain::models::AppointmentStatus::Confirmed
            | crate::domain::models::AppointmentStatus::Waiting
            | crate::domain::models::AppointmentStatus::InConsultation
            | crate::domain::models::AppointmentStatus::Completed
    ) {
        conversation.is_locked = true;
    }

    Ok(Json(ApiResponse {
        success: true,
        data: conversation,
    }))
}

/// Resolves the `consultation_id` a patient needs to file a grievance
/// (`POST /grievances`'s `consultation_id` field) — no prior endpoint exposed
/// this to a patient at all, the same shape of gap as #3's conversation
/// lookup. `consultation_repo::find_by_appointment` already existed.
/// Deliberately returns a filtered projection, not the raw
/// `ConsultationSession` row: `agora_channel_name` is internal RTC-session
/// detail with no reason to reach a patient-facing response. A session only
/// exists once either participant has minted an RTC token at least once
/// (`ConsultationService::mint_agora_token` creates it on first use) — a 404
/// here is real and expected before that, surfaced by the Flutter Grievance
/// entry point as "nothing to report yet," not a generic error.
pub async fn get_consultation_for_appointment(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;

    let mut tx = state.begin_authed(&ctx).await?;
    appointment_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
    let session = consultation_repo::find_by_appointment(&mut *tx, id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound("No consultation session exists for this appointment.".into())
        })?;
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "id": session.id,
            "appointment_id": session.appointment_id,
            "status": session.status,
            "started_at": session.started_at,
            "ended_at": session.ended_at,
        }),
    }))
}
