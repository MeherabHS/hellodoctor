use crate::api::middleware::idempotency_middleware;
use crate::api::v1::*;
use crate::repository::AppState;
use crate::telemetry;
use axum::{
    http::{HeaderValue, Method},
    middleware::{from_fn, from_fn_with_state},
    routing::get,
    Router,
};
use metrics_exporter_prometheus::PrometheusHandle;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub fn create_router(state: AppState) -> Router {
    create_router_with_metrics(state, telemetry::init_metrics())
}

pub fn create_router_with_metrics(state: AppState, metrics_handle: PrometheusHandle) -> Router {
    let allowed_origins: Vec<HeaderValue> = state
        .config
        .allowed_origins
        .iter()
        .filter_map(|o| o.parse::<HeaderValue>().ok())
        .collect();

    let cors = CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE, Method::OPTIONS])
        .allow_headers(tower_http::cors::Any);

    let api_v1 = router_v1()
        .layer(from_fn_with_state(state.clone(), idempotency_middleware))
        .with_state(state.clone());

    let metrics_router = Router::new()
        .route("/metrics", get(telemetry::metrics_endpoint))
        .with_state(metrics_handle);

    Router::new()
        .route("/healthz", get(|| async { "OK" }))
        .nest("/api/v1", api_v1)
        .merge(metrics_router)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(from_fn(telemetry::track_metrics))
}

fn router_v1() -> Router<AppState> {
    Router::new()
        // Auth
        .route("/auth/register-otp", axum::routing::post(auth_handlers::register_otp))
        .route("/auth/verify-otp-and-login", axum::routing::post(auth_handlers::verify_otp_and_login))
        .route("/auth/doctor/login", axum::routing::post(auth_handlers::doctor_login))
        .route("/auth/doctor/verify-otp", axum::routing::post(auth_handlers::doctor_verify_otp))
        .route("/auth/admin/login", axum::routing::post(auth_handlers::admin_login))
        .route("/auth/refresh", axum::routing::post(auth_handlers::refresh_token))
        // Doctors
        .route("/doctors", get(doctor_handlers::list_doctors))
        .route("/doctors/:id", get(doctor_handlers::get_doctor))
        .route("/doctors/:id/slots", get(doctor_handlers::get_doctor_slots))
        // Appointments
        .route("/appointments/book", axum::routing::post(appointment_handlers::book_appointment))
        .route("/appointments/:id", get(appointment_handlers::get_appointment))
        .route("/appointments/:id/prescriptions", axum::routing::post(prescription_handlers::upload_patient_intake))
        .route("/appointments/:id/documents/:document_id", get(prescription_handlers::get_document_download_url))
        // Consultations & Agora
        .route("/consultations/:appointment_id/rtc-token", axum::routing::post(consultation_handlers::mint_agora_token))
        .route("/consultations/:appointment_id/telemetry", axum::routing::post(consultation_handlers::submit_telemetry))
        .route("/consultations/:appointment_id/complete", axum::routing::post(consultation_handlers::complete_consultation))
        .route("/consultations/:appointment_id/prescription", axum::routing::post(prescription_handlers::upload_doctor_prescription))
        .route("/consultations/:appointment_id/complete-no-rx", axum::routing::post(prescription_handlers::complete_no_rx))
        // 24-Hour Clinical Chat
        .route(
            "/chat/:conversation_id/messages",
            axum::routing::post(chat_handlers::send_chat_message).get(chat_handlers::get_chat_messages),
        )
        // Doctor Wallet
        .route("/doctor/wallet/summary", get(wallet_handlers::get_doctor_wallet))
        // Grievances
        .route("/grievances", axum::routing::post(grievance_handlers::submit_grievance))
        .route("/admin/grievances/:id/refund", axum::routing::post(grievance_handlers::adjudicate_refund))
        .route("/admin/grievances/:id/warn", axum::routing::post(grievance_handlers::adjudicate_warn))
        .route("/admin/grievances/:id/dismiss", axum::routing::post(grievance_handlers::adjudicate_dismiss))
        // Admin & Telemetry & Governance
        .route("/admin/overview", get(admin_handlers::get_admin_overview))
        .route("/admin/doctors", get(admin_handlers::list_admin_doctors).post(admin_handlers::create_doctor))
        .route("/admin/doctors/:id/dossier", get(admin_handlers::get_doctor_dossier))
        .route("/admin/disbursements/initiate", axum::routing::post(admin_handlers::initiate_disbursement))
        .route("/admin/finance/summary", get(admin_handlers::get_finance_summary))
        .route("/admin/finance/transactions", get(admin_handlers::list_admin_transactions))
        .route("/admin/command/telemetry", get(admin_handlers::get_command_telemetry))
        .route("/admin/slots", get(admin_handlers::list_admin_slots).post(admin_handlers::create_or_toggle_slot))
        .route("/admin/patients", get(admin_handlers::list_admin_patients))
        .route("/admin/patients/:id/dossier", get(admin_handlers::get_patient_dossier))
        .route("/admin/bmdc/queue", get(admin_handlers::get_bmdc_queue))
        .route("/admin/bmdc/:id/verify", axum::routing::post(admin_handlers::verify_bmdc_doctor))
        .route("/admin/bmdc/:id/reject", axum::routing::post(admin_handlers::reject_bmdc_doctor))
        .route("/admin/compliance/alerts", get(admin_handlers::get_compliance_alerts))
        .route("/admin/logs", get(admin_handlers::list_admin_logs))
        .route("/admin/logs/simulate", axum::routing::post(admin_handlers::simulate_admin_log))
        .route("/admin/grievances", get(admin_handlers::list_admin_grievances))
        // Payment Webhooks
        .route("/webhooks/payment/:provider", axum::routing::post(webhook_handlers::handle_payment_webhook))
}
