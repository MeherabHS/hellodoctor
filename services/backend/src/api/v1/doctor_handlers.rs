use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::AppState;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct DoctorQuery {
    pub modality: Option<String>,
    pub specialty: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn list_doctors(
    State(state): State<AppState>,
    Query(query): Query<DoctorQuery>,
) -> Result<impl IntoResponse, AppError> {
    let docs = state.doctor_profiles.read();
    let mut list: Vec<DoctorProfile> = docs.values().cloned().collect();

    if let Some(spec) = query.specialty {
        list.retain(|d| d.primary_specialty.eq_ignore_ascii_case(&spec));
    }

    if let Some(s) = query.search {
        list.retain(|d| {
            d.full_name.to_lowercase().contains(&s.to_lowercase())
                || d.current_hospital.to_lowercase().contains(&s.to_lowercase())
        });
    }

    Ok(Json(ApiResponse {
        success: true,
        data: list,
    }))
}

pub async fn get_doctor(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let docs = state.doctor_profiles.read();
    let doc = docs.get(&id).cloned().ok_or_else(|| {
        AppError::NotFound("Doctor profile not found.".into())
    })?;

    Ok(Json(ApiResponse {
        success: true,
        data: doc,
    }))
}

pub async fn get_doctor_slots(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let slots = state.schedule_slots.read();
    let doc_slots: Vec<ScheduleSlot> = slots
        .values()
        .filter(|s| s.doctor_id == id && s.status == SlotStatus::Available)
        .cloned()
        .collect();

    Ok(Json(ApiResponse {
        success: true,
        data: doc_slots,
    }))
}
