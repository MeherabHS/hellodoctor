use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{doctor_repo, AppState};
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
    let _ = query.modality;
    let mut list: Vec<DoctorProfile> = doctor_repo::list(&state.db).await?;

    if let Some(spec) = query.specialty {
        list.retain(|d| d.primary_specialty.eq_ignore_ascii_case(&spec));
    }

    if let Some(s) = query.search {
        list.retain(|d| {
            d.full_name.to_lowercase().contains(&s.to_lowercase())
                || d.current_hospital.to_lowercase().contains(&s.to_lowercase())
        });
    }

    Ok(Json(ApiResponse { success: true, data: list }))
}

pub async fn get_doctor(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<impl IntoResponse, AppError> {
    let doc = doctor_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;

    Ok(Json(ApiResponse { success: true, data: doc }))
}

pub async fn get_doctor_slots(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<impl IntoResponse, AppError> {
    let slots = doctor_repo::list_slots_for_doctor(&state.db, id).await?;
    Ok(Json(ApiResponse { success: true, data: slots }))
}
