use crate::api::middleware::idempotency_middleware;
use crate::api::v1::*;
use crate::repository::AppState;
use axum::{
    middleware::from_fn,
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_v1 = Router::new()
        // Auth
        .route("/auth/register-otp", post(auth_handlers::register_otp))
        .route("/auth/verify-otp-and-login", post(auth_handlers::verify_otp_and_login))
        .route("/auth/doctor/login", post(auth_handlers::doctor_login))
        .route("/auth/doctor/verify-otp", post(auth_handlers::doctor_verify_otp))
        .route("/auth/admin/login", post(auth_handlers::admin_login))
        .route("/auth/refresh", post(auth_handlers::refresh_token))
        // Doctors
        .route("/doctors", get(doctor_handlers::list_doctors))
        .route("/doctors/:id", get(doctor_handlers::get_doctor))
        .route("/doctors/:id/slots", get(doctor_handlers::get_doctor_slots))
        // Appointments
        .route("/appointments/book", post(appointment_handlers::book_appointment))
        .route("/appointments/:id", get(appointment_handlers::get_appointment))
        .route("/appointments/:id/prescriptions", post(prescription_handlers::upload_patient_intake))
        .route("/appointments/:id/documents/:document_id", get(prescription_handlers::get_document_download_url))
        // Consultations & Agora
        .route("/consultations/:appointment_id/rtc-token", post(consultation_handlers::mint_agora_token))
        .route("/consultations/:appointment_id/telemetry", post(consultation_handlers::submit_telemetry))
        .route("/consultations/:appointment_id/complete", post(consultation_handlers::complete_consultation))
        .route("/consultations/:appointment_id/prescription", post(prescription_handlers::upload_doctor_prescription))
        .route("/consultations/:appointment_id/complete-no-rx", post(prescription_handlers::complete_no_rx))
        // 24-Hour Clinical Chat
        .route("/chat/:conversation_id/messages", post(chat_handlers::send_chat_message))
        .route("/chat/:conversation_id/messages", get(chat_handlers::get_chat_messages))
        // Doctor Wallet
        .route("/doctor/wallet/summary", get(wallet_handlers::get_doctor_wallet))
        // Grievances
        .route("/grievances", post(grievance_handlers::submit_grievance))
        .route("/admin/grievances/:id/refund", post(grievance_handlers::adjudicate_refund))
        .route("/admin/grievances/:id/warn", post(grievance_handlers::adjudicate_warn))
        // Admin & Disbursements
        .route("/admin/doctors/:id/dossier", get(admin_handlers::get_doctor_dossier))
        .route("/admin/disbursements/initiate", post(admin_handlers::initiate_disbursement))
        // Payment Webhooks
        .route("/webhooks/payment/:provider", post(webhook_handlers::handle_payment_webhook))
        .layer(from_fn(idempotency_middleware));

    Router::new()
        .route("/healthz", get(|| async { "OK" }))
        .nest("/api/v1", api_v1)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
