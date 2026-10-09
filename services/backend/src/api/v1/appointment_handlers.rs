use crate::abac::{self, Action};
use crate::api::middleware::idempotency::Idempotency;
use crate::auth_context::AuthContext;
use crate::domain::ids::DoctorProfileId;
use crate::domain::models::*;
use crate::error::AppError;
use crate::rate_limit;
use crate::repository::{appointment_repo, AppState};
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
    pub gateway: Gateway,
    pub chief_complaint: Option<String>,
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

    let patient_id = ctx.patient_profile_id(&state).await?;
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
        payload.gateway,
        chief_complaint,
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
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: apt,
    }))
}
