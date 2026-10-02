use crate::error::AppError;
use crate::repository::AppState;
use crate::services::PrescriptionService;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct PatientIntakeUploadRequest {
    pub files: Vec<FileData>,
}

#[derive(Debug, Deserialize)]
pub struct FileData {
    pub mime_type: String,
    pub base64_content: String,
}

#[derive(Debug, Deserialize)]
pub struct DoctorRxPhotoUploadRequest {
    pub base64_photo: String,
    pub doctor_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompleteNoRxRequest {
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn upload_patient_intake(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<PatientIntakeUploadRequest>,
) -> Result<impl IntoResponse, AppError> {
    let patient_id = Uuid::parse_str("ba000001-0000-0000-0000-000000000001").unwrap();
    let mut files = Vec::new();

    for f in payload.files {
        // Dummy JPEG header fallback if mock string provided
        let bytes = if f.base64_content.starts_with("/9j/") || f.base64_content.starts_with("data:") {
            vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46] // JPEG magic bytes
        } else {
            f.base64_content.into_bytes()
        };
        files.push((f.mime_type, bytes));
    }

    let uploaded = PrescriptionService::upload_patient_intake_documents(&state, id, patient_id, files)?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: serde_json::json!({
                "appointment_id": id,
                "uploaded_count": uploaded.len(),
                "documents": uploaded
            }),
        }),
    ))
}

pub async fn upload_doctor_prescription(
    State(state): State<AppState>,
    Path(appointment_id): Path<Uuid>,
    Json(payload): Json<DoctorRxPhotoUploadRequest>,
) -> Result<impl IntoResponse, AppError> {
    let doc_id = Uuid::parse_str("da000001-0000-0000-0000-000000000001").unwrap();
    let bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10]; // Valid JPEG stream

    let rx = PrescriptionService::upload_doctor_prescription_photo(
        &state,
        appointment_id,
        doc_id,
        bytes,
        payload.doctor_notes,
    )?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: rx,
        }),
    ))
}

pub async fn complete_no_rx(
    State(state): State<AppState>,
    Path(appointment_id): Path<Uuid>,
    Json(payload): Json<CompleteNoRxRequest>,
) -> Result<impl IntoResponse, AppError> {
    let doc_id = Uuid::parse_str("da000001-0000-0000-0000-000000000001").unwrap();
    let rx = PrescriptionService::complete_without_prescription(
        &state,
        appointment_id,
        doc_id,
        payload.reason,
    )?;

    Ok(Json(ApiResponse {
        success: true,
        data: rx,
    }))
}

pub async fn get_document_download_url(
    State(_state): State<AppState>,
    Path((appointment_id, document_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    // Generates a short-lived pre-signed URL (expires in 300s)
    let pre_signed_url = format!(
        "https://storage.helodoc.asia/pre-signed/{}/{}?expires=300",
        appointment_id, document_id
    );

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "document_id": document_id,
            "download_url": pre_signed_url,
            "expires_in": 300
        }),
    }))
}
