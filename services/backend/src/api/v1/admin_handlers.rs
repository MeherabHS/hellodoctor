use crate::error::AppError;
use crate::repository::AppState;
use crate::services::DisbursementService;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct InitiateDisbursementRequest {
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn get_doctor_dossier(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let docs = state.doctor_profiles.read();
    let doc = docs.get(&id).cloned().ok_or_else(|| {
        AppError::NotFound("Doctor profile not found.".into())
    })?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "id": doc.id,
            "full_name": doc.full_name,
            "license_number": doc.license_number,
            "license_authority": doc.license_authority,
            "license_country": doc.license_country,
            "primary_specialty": doc.primary_specialty,
            "experience_years": doc.experience_years,
            "current_hospital": doc.current_hospital,
            "residential_address": doc.residential_address,
            "verified_phone": doc.verified_phone,
            "qualifications": doc.qualifications,
            "disciplinary_warnings": []
        }),
    }))
}

pub async fn initiate_disbursement(
    State(state): State<AppState>,
    Json(payload): Json<InitiateDisbursementRequest>,
) -> Result<impl IntoResponse, AppError> {
    let admin_id = Uuid::parse_str("a0000001-0000-0000-0000-000000000001").unwrap();
    let batch = DisbursementService::initiate_monthly_disbursement(
        &state,
        admin_id,
        payload.period_start,
        payload.period_end,
    )?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: batch,
        }),
    ))
}
