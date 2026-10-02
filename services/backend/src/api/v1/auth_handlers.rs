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
    pub phone_number: String,
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
    let _ = payload.role;
    let (session_token, expires_in, debug_otp_code) = AuthService::request_patient_otp(&state, &payload.phone_number).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "session_token": session_token,
            "expires_in": expires_in,
            "debug_otp_code": debug_otp_code
        }),
    }))
}

pub async fn verify_otp_and_login(
    State(state): State<AppState>,
    Json(payload): Json<VerifyOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    let login_resp = AuthService::verify_otp_and_login(&state, &payload.phone_number, &payload.otp_code).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: login_resp,
    }))
}

pub async fn doctor_login(
    State(state): State<AppState>,
    Json(payload): Json<DoctorLoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (session_token, debug_code) = AuthService::doctor_login_step1(&state, &payload.license_number, &payload.password).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "session_token": session_token,
            "requires_otp": true,
            "debug_otp_code": debug_code
        }),
    }))
}

pub async fn doctor_verify_otp(
    State(state): State<AppState>,
    Json(payload): Json<DoctorVerifyOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    let login_resp = AuthService::doctor_verify_otp(&state, &payload.session_token, &payload.otp_code).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: login_resp,
    }))
}

pub async fn admin_login(
    State(state): State<AppState>,
    Json(payload): Json<AdminLoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let login_resp = AuthService::admin_login(&state, &payload.email, &payload.password, &payload.totp_code).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: login_resp,
    }))
}

pub async fn refresh_token(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (access_token, new_refresh_token) = AuthService::refresh_session(&state, &payload.refresh_token).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "access_token": access_token,
            "refresh_token": new_refresh_token
        }),
    }))
}
