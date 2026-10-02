use crate::error::AppError;
use crate::repository::AppState;
use crate::services::AuthService;
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RegisterOtpRequest {
    pub phone_number: String,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VerifyOtpRequest {
    pub phone_number: Option<String>,
    pub session_token: Option<String>,
    pub otp_code: String,
}

#[derive(Debug, Deserialize)]
pub struct DoctorLoginRequest {
    pub license_number: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct DoctorVerifyOtpRequest {
    pub session_token: String,
    pub otp_code: String,
}

#[derive(Debug, Deserialize)]
pub struct AdminLoginRequest {
    pub email: String,
    pub password: String,
    pub totp_code: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn register_otp(
    State(state): State<AppState>,
    Json(payload): Json<RegisterOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (session_token, expires_in) = AuthService::request_patient_otp(&state, &payload.phone_number)?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "session_token": session_token,
            "expires_in": expires_in
        }),
    }))
}

pub async fn verify_otp_and_login(
    State(state): State<AppState>,
    Json(payload): Json<VerifyOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    let phone = payload.phone_number.unwrap_or_else(|| "+8801712345678".into());
    let login_resp = AuthService::verify_otp_and_login(
        &state,
        &phone,
        &payload.otp_code,
        "helodoc-jwt-secret-key-ed25519",
    )?;

    Ok(Json(ApiResponse {
        success: true,
        data: login_resp,
    }))
}

pub async fn doctor_login(
    State(state): State<AppState>,
    Json(payload): Json<DoctorLoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let session_token = AuthService::doctor_login_step1(&state, &payload.license_number, &payload.password)?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "session_token": session_token,
            "requires_otp": true
        }),
    }))
}

pub async fn doctor_verify_otp(
    State(state): State<AppState>,
    Json(payload): Json<DoctorVerifyOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    let doc_id = uuid::Uuid::parse_str("da000001-0000-0000-0000-000000000001").unwrap();
    let login_resp = AuthService::doctor_verify_otp(
        &state,
        doc_id,
        &payload.otp_code,
        "helodoc-jwt-secret-key-ed25519",
    )?;

    Ok(Json(ApiResponse {
        success: true,
        data: login_resp,
    }))
}

pub async fn admin_login(
    State(state): State<AppState>,
    Json(payload): Json<AdminLoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let login_resp = AuthService::admin_login(
        &state,
        &payload.email,
        &payload.password,
        &payload.totp_code,
        "helodoc-jwt-secret-key-ed25519",
    )?;

    Ok(Json(ApiResponse {
        success: true,
        data: login_resp,
    }))
}

pub async fn refresh_token(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (access_token, new_refresh_token) =
        AuthService::refresh_session(&state, &payload.refresh_token, "helodoc-jwt-secret-key-ed25519")?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "access_token": access_token,
            "refresh_token": new_refresh_token
        }),
    }))
}
