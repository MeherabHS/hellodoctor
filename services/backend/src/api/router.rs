use crate::api::middleware::{auth_middleware, global_guard_middleware, idempotency_middleware};
use crate::api::v1::*;
use crate::repository::AppState;
use crate::telemetry;
use axum::{
    error_handling::HandleErrorLayer,
    extract::State,
    http::{HeaderValue, Method, StatusCode},
    middleware::{from_fn, from_fn_with_state},
    response::IntoResponse,
    routing::{get, post},
    BoxError, Json, Router,
};
use metrics_exporter_prometheus::PrometheusHandle;
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

/// Hard ceiling on request bodies at the HTTP layer (prescription uploads are capped
/// well below this in application logic already; this is the backstop).
const MAX_REQUEST_BODY_BYTES: usize = 15 * 1024 * 1024;
const REQUEST_TIMEOUT_SECS: u64 = 10;

/// Converts a timeout or load-shed rejection (the only ways this stage can fail) into
/// a real HTTP response, since axum requires the outermost service to be infallible.
async fn handle_overload_error(err: BoxError) -> (StatusCode, String) {
    if err.is::<tower::timeout::error::Elapsed>() {
        (
            StatusCode::REQUEST_TIMEOUT,
            "Request timed out.".to_string(),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Server is under heavy load — please retry shortly.".to_string(),
        )
    }
}

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
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(tower_http::cors::Any);

    // Layers execute from last to first: authenticate before preparing any
    // idempotency context. Public credentials/download grants never use it.
    let protected = protected_routes()
        .route_layer(from_fn_with_state(state.clone(), idempotency_middleware))
        .route_layer(from_fn_with_state(state.clone(), auth_middleware));

    let api_v1 = public_routes().merge(protected).with_state(state.clone());

    let metrics_router = Router::new()
        .route("/metrics", get(telemetry::metrics_endpoint))
        .route_layer(from_fn_with_state(
            state.clone(),
            telemetry::metrics_auth_guard,
        ))
        .with_state(metrics_handle);

    let health_router = Router::new()
        .route("/healthz", get(health_check))
        .with_state(state.clone());

    Router::new()
        .merge(health_router)
        .nest("/api/v1", api_v1)
        .merge(metrics_router)
        .layer(from_fn_with_state(state.clone(), global_guard_middleware))
        .layer(from_fn_with_state(state.clone(), crate::api::middleware::transport_guard))
        .layer(cors)
        .layer(TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<axum::body::Body>| {
            tracing::debug_span!("request", method=%request.method(), path=request.uri().path())
        }))
        .layer(from_fn(telemetry::track_metrics))
        .layer(RequestBodyLimitLayer::new(MAX_REQUEST_BODY_BYTES))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_overload_error))
                .load_shed()
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS)),
        )
}

/// `GET /healthz` — a real readiness check, not a static "OK". Verifies the
/// live database's applied migrations actually match what this compiled
/// binary's embedded migrator expects (see `migration_status` module doc
/// comment for the incident — B-01 — this closes). Never leaks raw SQL
/// errors or schema internals in the response body; any underlying DB error
/// is logged via `tracing` instead.
async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    match crate::migration_status::check(&state.db, &crate::migration_status::MIGRATOR).await {
        Ok(status) if status.schema_ready => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "ready",
                "database": "ready",
                "migrations": "current",
                "binary_latest_migration": status.binary_latest.to_string(),
                "database_latest_migration": status.database_latest.map(|v| v.to_string()),
            })),
        ),
        Ok(status) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "not_ready",
                "database": "reachable",
                "migrations": "behind",
                "binary_latest_migration": status.binary_latest.to_string(),
                "database_latest_migration": status.database_latest.map(|v| v.to_string()),
            })),
        ),
        Err(e) => {
            tracing::error!("healthz: database unreachable or migration-status query failed: {e}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "status": "not_ready",
                    "database": "unreachable",
                    "migrations": "unknown",
                    "binary_latest_migration": crate::migration_status::MIGRATOR
                        .migrations
                        .iter()
                        .map(|m| m.version)
                        .max()
                        .unwrap_or(0)
                        .to_string(),
                    "database_latest_migration": serde_json::Value::Null,
                })),
            )
        }
    }
}

/// Explicit, auditable allow-list of routes reachable with no bearer token at
/// all: OTP/password login steps (there is no caller identity yet to
/// authenticate — that's the whole point), token refresh, and payment
/// webhooks (authorized instead by their own HMAC signature check, see
/// `webhook_handlers`). Everything else lives in `protected_routes` and is
/// gated by `auth_middleware` as a group — adding a new route here is a
/// deliberate, visible choice to make it public, not an accident.
fn public_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/documents/download",
            get(prescription_handlers::download_document),
        )
        .route(
            "/auth/register-otp",
            axum::routing::post(auth_handlers::register_otp),
        )
        .route(
            "/auth/verify-otp-and-login",
            axum::routing::post(auth_handlers::verify_otp_and_login),
        )
        .route(
            "/auth/doctor/login",
            axum::routing::post(auth_handlers::doctor_login),
        )
        .route(
            "/auth/doctor/register",
            axum::routing::post(auth_handlers::doctor_register),
        )
        .route(
            "/legal/documents",
            get(legal_handlers::list_current_documents),
        )
        .route(
            "/auth/doctor/verify-otp",
            axum::routing::post(auth_handlers::doctor_verify_otp),
        )
        .route(
            "/auth/admin/login",
            axum::routing::post(auth_handlers::admin_login),
        )
        .route(
            "/auth/refresh",
            axum::routing::post(auth_handlers::refresh_token),
        )
        .route("/auth/logout", axum::routing::post(auth_handlers::logout))
        .route(
            "/webhooks/payment/:provider",
            axum::routing::post(webhook_handlers::handle_payment_webhook),
        )
}

/// Everything else. Gated as a group by `auth_middleware` (see
/// `create_router_with_metrics`) — a route added here is authenticated
/// automatically; resource-level ownership/role checks still happen inside
/// each handler via `abac::authorize` and `AuthContext`.
fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/client-errors", axum::routing::post(client_error_handlers::submit_client_error)
            .layer(axum::extract::DefaultBodyLimit::max(16 * 1024)))
        .route("/doctor/articles", get(article_handlers::list_own).post(article_handlers::create))
        .route("/doctor/articles/:id", get(article_handlers::own_detail).patch(article_handlers::update))
        .route("/doctor/articles/:id/publish", axum::routing::post(article_handlers::publish))
        .route("/doctor/articles/:id/archive", axum::routing::post(article_handlers::archive))
        .route("/articles", get(article_handlers::feed))
        .route("/articles/:id", get(article_handlers::detail))
        // Doctors
        .route("/doctors", get(doctor_handlers::list_doctors))
        .route("/doctors/:id", get(doctor_handlers::get_doctor))
        .route("/doctors/:id/slots", get(doctor_handlers::get_doctor_slots))
        .route("/doctor/profile", get(doctor_handlers::get_own_profile).patch(doctor_handlers::update_own_profile))
        .route("/doctor/duty", get(doctor_handlers::get_own_duty).put(doctor_handlers::update_own_duty))
        .route("/doctor/slots", get(doctor_handlers::list_own_slots).post(doctor_handlers::create_own_slot))
        .route("/doctor/slots/:slot_id", axum::routing::delete(doctor_handlers::cancel_own_slot).patch(doctor_handlers::update_own_slot))
        .route("/doctor/appointments", get(doctor_handlers::list_own_appointments))
        // Notifications (own-inbox only; see notification_handlers.rs)
        .route("/notifications", get(notification_handlers::list_own_notifications))
        .route("/notifications/unread-count", get(notification_handlers::unread_count))
        .route("/notifications/read-all", axum::routing::post(notification_handlers::mark_all_read))
        .route("/notifications/:id/read", axum::routing::post(notification_handlers::mark_read))
        .route("/notifications/devices", axum::routing::post(notification_handlers::register_device))
        .route("/doctor/chats", get(doctor_handlers::list_own_chats))
        .route("/doctor/appointments/:current_appointment_id/patient-history", get(doctor_handlers::list_patient_history))
        .route("/doctor/appointments/:current_appointment_id/patient-history/:historical_consultation_id", get(doctor_handlers::get_patient_history_detail))
        .route("/doctor/consultations/:appointment_id/summary", get(doctor_handlers::consultation_summary))
        .route("/doctor/wallet/ledger", get(doctor_handlers::wallet_ledger))
        .route("/doctor/qna/questions", get(doctor_handlers::list_open_questions))
        .route("/doctor/qna/questions/:question_id/answers", axum::routing::post(doctor_handlers::answer_question))
        .route("/doctor/legal/acceptances", get(legal_handlers::get_own_acceptances))
        // Appointments
        .route(
            "/appointments/book",
            axum::routing::post(appointment_handlers::book_appointment),
        )
        .route(
            "/appointments/:id",
            get(appointment_handlers::get_appointment),
        )
        .route(
            "/appointments/:id/prescriptions",
            axum::routing::post(prescription_handlers::upload_patient_intake),
        )
        .route(
            "/appointments/:id/documents/:document_id",
            get(prescription_handlers::get_document_download_url),
        )
        // Consultations & Agora
        .route(
            "/consultations/:appointment_id/rtc-token",
            axum::routing::post(consultation_handlers::mint_agora_token),
        )
        .route(
            "/consultations/:appointment_id/telemetry",
            axum::routing::post(consultation_handlers::submit_telemetry),
        )
        .route(
            "/consultations/:appointment_id/telemetry/events",
            axum::routing::post(consultation_handlers::submit_telemetry_events),
        )
        .route(
            "/consultations/:appointment_id/complete",
            axum::routing::post(consultation_handlers::complete_consultation),
        )
        .route(
            "/consultations/:appointment_id/prescription",
            axum::routing::post(prescription_handlers::upload_doctor_prescription),
        )
        .route(
            "/consultations/:appointment_id/complete-no-rx",
            axum::routing::post(prescription_handlers::complete_no_rx),
        )
        // 24-Hour Clinical Chat
        .route(
            "/chat/:conversation_id/messages",
            axum::routing::post(chat_handlers::send_chat_message)
                .get(chat_handlers::get_chat_messages),
        )
        // Doctor Wallet
        .route(
            "/doctor/wallet/summary",
            get(wallet_handlers::get_doctor_wallet),
        )
        .route(
            "/doctor/wallet/payout-method",
            axum::routing::get(wallet_handlers::get_payout_method)
                .put(wallet_handlers::set_payout_method),
        )
        .route(
            "/doctor/wallet/payout-methods",
            get(wallet_handlers::list_payout_destinations),
        )
        .route(
            "/doctor/wallet/payout-methods/dev-bind",
            axum::routing::post(wallet_handlers::dev_bind_payout_method),
        )
        .route(
            "/doctor/wallet/statements",
            get(wallet_handlers::get_monthly_statement),
        )
        // Grievances
        .route(
            "/grievances",
            axum::routing::post(grievance_handlers::submit_grievance),
        )
        .route(
            "/admin/grievances/:id/refund",
            axum::routing::post(grievance_handlers::adjudicate_refund),
        )
        .route(
            "/admin/grievances/:id/warn",
            axum::routing::post(grievance_handlers::adjudicate_warn),
        )
        .route(
            "/admin/grievances/:id/dismiss",
            axum::routing::post(grievance_handlers::adjudicate_dismiss),
        )
        .route(
            "/admin/finance/refund-requests",
            get(grievance_handlers::list_refund_requests),
        )
        .route(
            "/admin/finance/refund-requests/:id/transition",
            axum::routing::post(grievance_handlers::transition_refund_request),
        )
        // Admin & Telemetry & Governance
        .route("/admin/overview", get(admin_handlers::get_admin_overview))
        .route(
            "/admin/doctors",
            get(admin_handlers::list_admin_doctors).post(admin_handlers::create_doctor),
        )
        .route(
            "/admin/doctors/:id/dossier",
            get(admin_handlers::get_doctor_dossier),
        )
        .route(
            "/admin/disbursements/initiate",
            axum::routing::post(admin_handlers::initiate_disbursement),
        )
        .route(
            "/admin/disbursements/:item_id/approve",
            axum::routing::post(admin_handlers::approve_disbursement_item),
        )
        .route(
            "/admin/disbursements/:item_id/confirm-paid",
            axum::routing::post(admin_handlers::confirm_disbursement_item_paid),
        )
        .route(
            "/admin/finance/summary",
            get(admin_handlers::get_finance_summary),
        )
        .route(
            "/admin/finance/transactions",
            get(admin_handlers::list_admin_transactions),
        )
        .route(
            "/admin/finance/payout-routing",
            get(admin_handlers::list_payout_routing),
        )
        .route(
            "/admin/finance/periods/finalize",
            axum::routing::post(admin_handlers::finalize_financial_period),
        )
        .route(
            "/admin/command/telemetry",
            get(admin_handlers::get_command_telemetry),
        )
        .route(
            "/admin/slots",
            get(admin_handlers::list_admin_slots).post(admin_handlers::create_or_toggle_slot),
        )
        .route("/admin/patients", get(admin_handlers::list_admin_patients))
        .route(
            "/admin/patients/:id/dossier",
            get(admin_handlers::get_patient_dossier),
        )
        .route("/admin/doctors/pending-verification", get(admin_handlers::list_pending_doctor_verifications))
        .route(
            "/admin/doctors/:id/verify",
            axum::routing::post(admin_handlers::verify_doctor_professional_registration),
        )
        .route(
            "/admin/doctors/:id/reject",
            axum::routing::post(admin_handlers::reject_doctor_professional_registration),
        )
        .route(
            "/admin/compliance/alerts",
            get(admin_handlers::get_compliance_alerts),
        )
        .route("/admin/compliance/trace", post(admin_handlers::trace_identity))
        .route("/admin/legal/acceptances/:user_id", get(legal_handlers::admin_list_acceptances))
        .route("/admin/logs", get(admin_handlers::list_admin_logs))
        .route(
            "/admin/logs/simulate",
            axum::routing::post(admin_handlers::simulate_admin_log),
        )
        .route(
            "/admin/grievances",
            get(admin_handlers::list_admin_grievances),
        )
        .route(
            "/admin/system-errors",
            get(admin_handlers::list_system_errors),
        )
        .route(
            "/admin/system-errors/:id",
            get(admin_handlers::get_system_error),
        )
        .route(
            "/admin/system-errors/:id/acknowledge",
            axum::routing::post(admin_handlers::acknowledge_system_error),
        )
        .route(
            "/admin/system-errors/:id/resolve",
            axum::routing::post(admin_handlers::resolve_system_error),
        )
        .route(
            "/admin/system-errors/simulate",
            axum::routing::post(admin_handlers::simulate_system_error),
        )
        .route(
            "/admin/consultations/:session_id/timeline",
            get(admin_handlers::get_consultation_timeline),
        )
}
