use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::AppState;
use crate::services::AppointmentService;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct BookAppointmentRequest {
    pub doctor_id: Uuid,
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
    Json(payload): Json<BookAppointmentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let patient_id = Uuid::parse_str("ba000001-0000-0000-0000-000000000001").unwrap();
    let res = AppointmentService::book_appointment(
        &state,
        patient_id,
        payload.doctor_id,
        payload.slot_id,
        payload.modality,
        payload.gateway,
        payload.chief_complaint,
    )?;

    Ok((
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
    ))
}

pub async fn get_appointment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let apts = state.appointments.read();
    let apt = apts.get(&id).cloned().ok_or_else(|| {
        AppError::NotFound("Appointment not found.".into())
    })?;

    Ok(Json(ApiResponse {
        success: true,
        data: apt,
    }))
}
