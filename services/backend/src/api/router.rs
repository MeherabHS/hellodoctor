use crate::api::middleware::{auth_middleware, global_guard_middleware, idempotency_middleware};
use crate::api::v1::*;
use crate::domain::models::ErrorSeverity;
use crate::error::{ErrorBody, ErrorResponse, ResponseMeta};
use crate::repository::AppState;
use crate::system_error::{self, ErrorEvent};
use crate::telemetry;
use axum::{
    error_handling::HandleErrorLayer,
    extract::State,
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{from_fn, from_fn_with_state},
    response::{IntoResponse, Response},
    routing::{get, post},
    BoxError, Json, Router,
};
use metrics_exporter_prometheus::PrometheusHandle;
use std::any::Any;
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

/// Hard ceiling on request bodies at the HTTP layer (prescription uploads are capped
/// well below this in application logic already; this is the backstop).
const MAX_REQUEST_BODY_BYTES: usize = 15 * 1024 * 1024;
const REQUEST_TIMEOUT_SECS: u64 = 10;

fn error_envelope(status: StatusCode, code: &'static str, message: String) -> Response {
    let body = ErrorResponse {
        success: false,
        error: ErrorBody {
            code: code.to_string(),
            message,
            details: None,
        },
        meta: ResponseMeta {
            timestamp: chrono::Utc::now(),
            request_id: crate::api::middleware::request_context::request_id(),
        },
    };
    (status, Json(body)).into_response()
}

/// Converts a timeout or load-shed rejection (the only ways this stage can fail) into
/// a real HTTP response, since axum requires the outermost service to be infallible.
/// Both are genuine backend-distress signals — something hung past
/// `REQUEST_TIMEOUT_SECS`, or the server is rejecting work outright under
/// load — so both are reported into the same `system_error_logs` pipeline
/// every other backend failure uses, instead of only ever being visible as
/// a client-side timeout with nothing to correlate it against admin-side.
pub async fn handle_overload_error(err: BoxError) -> Response {
    let (status, code, message, subsystem, severity) = if err.is::<tower::timeout::error::Elapsed>()
    {
        (
            StatusCode::REQUEST_TIMEOUT,
            "REQUEST_TIMEOUT",
            "Request timed out.".to_string(),
            "INTERNAL",
            ErrorSeverity::Error,
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_OVERLOADED",
            "Server is under heavy load — please retry shortly.".to_string(),
            "INTERNAL",
            ErrorSeverity::Warning,
        )
    };
    tracing::warn!("Request stage failure: {code}");
    let event = ErrorEvent::new(
        severity,
        subsystem,
        "RequestStage::Overload",
        code,
        message.clone(),
        crate::api::middleware::request_context::request_id(),
    )
    .with_http_status(status.as_u16());
    system_error::report(event);
    error_envelope(status, code, message)
}

/// Converts an unwinding panic from inside any handler into a normal 500
/// response and a `system_error_logs` row, instead of the connection simply
/// dropping with nothing recorded anywhere. Every *handled* failure
/// (`AppError`'s `Internal`/`Database`/`ServiceUnavailable` variants) already
/// reports into this same pipeline (see `error.rs`) — a genuine code bug
/// (an unexpected `None`/`unwrap`, an out-of-bounds index) previously had no
/// equivalent path at all and was invisible to the admin panel.
pub fn handle_panic(payload: Box<dyn Any + Send + 'static>) -> Response {
    let message = if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "Handler panicked with a non-string payload.".to_string()
    };
    tracing::error!("Handler panicked: {}", system_error::redact(&message));
    // `CatchPanicLayer`'s callback is only ever given the panic's message
    // payload — never its `PanicInfo` (confirmed by reading tower-http's own
    // source, not assumed) — so the real file:line has to come from the
    // sidecar panic hook `system_error::ensure_panic_location_capture`
    // installs, read here on this same thread immediately after
    // `catch_unwind` returns, before anything else can touch it.
    let mut event = ErrorEvent::new(
        ErrorSeverity::Critical,
        "INTERNAL",
        "HandlerPanic",
        "HANDLER_PANIC",
        message,
        crate::api::middleware::request_context::request_id(),
    )
    .with_http_status(500);
    if let Some(loc) = system_error::take_last_panic_location() {
        event = event.with_stack_trace(loc);
    }
    system_error::report(event);
    error_envelope(
        StatusCode::INTERNAL_SERVER_ERROR,
        "INTERNAL_SERVER_ERROR",
        "An unexpected error occurred.".to_string(),
    )
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
        .layer(from_fn_with_state(state.clone(), crate::api::middleware::web_auth::middleware))
        .layer(from_fn_with_state(state.clone(), global_guard_middleware))
        .layer(from_fn_with_state(state.clone(), crate::admission::middleware))
        .layer(from_fn_with_state(state.clone(), crate::api::middleware::transport_guard))
        .layer(cors)
        .layer(TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<axum::body::Body>| {
            tracing::debug_span!("request", method=%request.method(), path=request.uri().path())
        }))
        .layer(from_fn(telemetry::track_metrics))
        .layer(RequestBodyLimitLayer::new(MAX_REQUEST_BODY_BYTES))
        // Placed inside (closer to the handler than) `request_context`'s
        // layer below, so a caught panic still runs while that request's
        // task-local context is active — `handle_panic` calling
        // `request_id()` resolves the real per-request id, not a fallback.
        .layer(CatchPanicLayer::custom(handle_panic))
        .layer(from_fn(crate::api::middleware::request_context::middleware))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_overload_error))
                .load_shed()
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS)),
        )
}

/// `GET /healthz` — a real readiness check, not a static "OK". Verifies the
/// live database's applied migrations actually match what this compiled
/// binary's embedded migrator expects, and checks the required Redis dependency
/// with a bounded PING (see `migration_status` module doc
/// comment for the incident — B-01 — this closes). Never leaks raw SQL
/// errors or schema internals in the response body; any underlying DB error
/// is logged via `tracing` instead.
async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    match tokio::time::timeout(
        Duration::from_millis(250),
        crate::migration_status::check(&state.db, &crate::migration_status::MIGRATOR),
    )
    .await
    {
        Ok(Ok(status)) if status.schema_ready => {
            let redis_ready = matches!(
                tokio::time::timeout(Duration::from_millis(250), async {
                    let mut connection = state.redis.get().await.ok()?;
                    let reply: String = deadpool_redis::redis::cmd("PING")
                        .query_async(&mut connection)
                        .await
                        .ok()?;
                    Some(reply == "PONG")
                })
                .await,
                Ok(Some(true))
            );
            if !redis_ready {
                metrics::counter!("readiness_dependency_failures_total").increment(1);
            }
            (
                if redis_ready {
                    StatusCode::OK
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                },
                Json(serde_json::json!({
                    "status": if redis_ready { "ready" } else { "not_ready" },
                    "database": "ready",
                    "redis": if redis_ready { "ready" } else { "unreachable" },
                    "migrations": "current",
                    "binary_latest_migration": status.binary_latest.to_string(),
                    "database_latest_migration": status.database_latest.map(|v| v.to_string()),
                })),
            )
        }
        Ok(Ok(status)) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "not_ready",
                "database": "reachable",
                "migrations": "behind",
                "binary_latest_migration": status.binary_latest.to_string(),
                "database_latest_migration": status.database_latest.map(|v| v.to_string()),
            })),
        ),
        _ => {
            metrics::counter!("readiness_dependency_failures_total").increment(1);
            // A readiness probe remains cheap during an outage; do not write a
            // database error per probe or wait the full pool acquisition timeout.
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

/// Recovery operator probe. It deliberately exposes only safe operational
/// state and is available solely with the separately issued recovery token;
/// ordinary Patient, Doctor and Admin credentials are not accepted here.
async fn recovery_status(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !crate::api::middleware::global_guard::recovery_operator_authenticated(&state, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(serde_json::json!({
        "recovery_mode": state.config.recovery_mode,
        "outbound_operations": "disabled",
        "scheduler": "stopped",
        "operator_authorization": "verified"
    }))
    .into_response()
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
        .route("/recovery/status", get(recovery_status))
        .route(
            "/documents/download",
            get(prescription_handlers::download_document),
        )
        .route(
            "/auth/register-otp",
            axum::routing::post(auth_handlers::register_otp),
        )
        .route(
            "/auth/patient/register",
            axum::routing::post(auth_handlers::patient_register),
        )
        .route(
            "/auth/patient/login",
            axum::routing::post(auth_handlers::patient_login),
        )
        .route(
            "/auth/patient/verify-otp",
            axum::routing::post(auth_handlers::patient_verify_otp),
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
            "/auth/admin/verify-otp",
            axum::routing::post(auth_handlers::admin_verify_otp),
        )
        .route(
            "/auth/refresh",
            axum::routing::post(auth_handlers::refresh_token),
        )
        .route("/auth/logout", axum::routing::post(auth_handlers::logout))
        .route(
            "/auth/password-reset/request",
            axum::routing::post(auth_handlers::request_password_reset),
        )
        .route(
            "/auth/password-reset/verify-otp",
            axum::routing::post(auth_handlers::verify_password_reset_otp),
        )
        .route(
            "/auth/password-reset/complete",
            axum::routing::post(auth_handlers::complete_password_reset),
        )
        .route(
            "/webhooks/payment/:provider",
            axum::routing::post(webhook_handlers::handle_payment_webhook),
        )
        .route(
            "/webhooks/agora/presence",
            axum::routing::post(webhook_handlers::handle_agora_presence_webhook),
        )
}

/// Everything else. Gated as a group by `auth_middleware` (see
/// `create_router_with_metrics`) — a route added here is authenticated
/// automatically; resource-level ownership/role checks still happen inside
/// each handler via `abac::authorize` and `AuthContext`.
fn protected_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/auth/change-password",
            axum::routing::post(auth_handlers::change_password),
        )
        .route("/client-errors", axum::routing::post(client_error_handlers::submit_client_error)
            .layer(axum::extract::DefaultBodyLimit::max(16 * 1024)))
        .route("/doctor/articles", get(article_handlers::list_own).post(article_handlers::create))
        .route("/doctor/articles/:id", get(article_handlers::own_detail).patch(article_handlers::update))
        .route("/doctor/articles/:id/publish", axum::routing::post(article_handlers::publish))
        .route("/doctor/articles/:id/archive", axum::routing::post(article_handlers::archive))
        .route("/articles", get(article_handlers::feed))
        .route("/articles/:id", get(article_handlers::detail))
        .route("/articles/:id/quiz", get(patient_quiz_handlers::read))
        .route("/articles/:id/quiz/check", post(patient_quiz_handlers::check).layer(axum::extract::DefaultBodyLimit::max(1024)))
        .route("/articles/:id/cover", get(crate::services::article_media_service::cover))
        .route("/admin/article-categories", get(admin_article_handlers::categories).post(admin_article_handlers::create_category))
        .route("/admin/article-categories/:id", axum::routing::patch(admin_article_handlers::edit_category).delete(admin_article_handlers::delete_category))
        .route("/admin/articles", get(admin_article_handlers::list).post(admin_article_handlers::create))
        .route("/admin/articles/:id", get(admin_article_handlers::detail).patch(admin_article_handlers::edit))
        .route("/admin/articles/:id/cover", get(crate::services::article_media_service::admin_cover).post(crate::services::article_media_service::upload).layer(axum::extract::DefaultBodyLimit::max(crate::services::article_media_service::MAX_COVER_UPLOAD)))
        .route("/admin/articles/:id/:action", post(admin_article_handlers::transition))
        .route("/admin/articles/:id/quiz", get(crate::api::v1::admin_quiz_handlers::get).patch(crate::api::v1::admin_quiz_handlers::save).layer(axum::extract::DefaultBodyLimit::max(crate::domain::article_quizzes::MAX_REQUEST_BYTES)))
        .route("/admin/articles/:id/quiz/import-preview", post(crate::api::v1::admin_quiz_handlers::import_preview).layer(axum::extract::DefaultBodyLimit::max(crate::domain::article_quizzes::MAX_REQUEST_BYTES)))
        .route("/admin/articles/:id/quiz/import", post(crate::api::v1::admin_quiz_handlers::import_confirm).layer(axum::extract::DefaultBodyLimit::max(crate::domain::article_quizzes::MAX_REQUEST_BYTES)))
        .route("/admin/articles/:id/quiz/:revision/:action", post(crate::api::v1::admin_quiz_handlers::transition).layer(axum::extract::DefaultBodyLimit::max(1024)))
        // Doctors
        .route("/doctors", get(doctor_handlers::list_doctors))
        .route("/doctors/:id", get(doctor_handlers::get_doctor))
        .route("/doctors/:id/slots", get(doctor_handlers::get_doctor_slots))
        .route("/doctor/profile", get(doctor_handlers::get_own_profile).patch(doctor_handlers::update_own_profile))
        .route("/doctor/profile/photo", axum::routing::post(crate::api::v1::profile_picture_handlers::upload_doctor_photo)
            .get(crate::api::v1::profile_picture_handlers::get_own_doctor_photo)
            .delete(crate::api::v1::profile_picture_handlers::delete_doctor_photo))
        .route("/doctors/:id/photo", get(crate::api::v1::profile_picture_handlers::get_directory_doctor_photo))
        .route("/doctor/duty", get(doctor_handlers::get_own_duty).put(doctor_handlers::update_own_duty))
        .route("/doctor/slots", get(doctor_handlers::list_own_slots).post(doctor_handlers::create_own_slot))
        .route("/doctor/slots/:slot_id", axum::routing::delete(doctor_handlers::cancel_own_slot).patch(doctor_handlers::update_own_slot))
        .route("/doctor/appointments", get(doctor_handlers::list_own_appointments))
        .route("/appointments/:id/trace-ack", axum::routing::post(crate::api::v1::trace_handlers::acknowledge))
        .route("/admin/grievances/:id/timeline", get(crate::api::v1::trace_handlers::grievance_timeline))
        .route("/admin/appointments/:id/reschedule", axum::routing::post(crate::api::v1::trace_handlers::reschedule))
        // Notifications (own-inbox only; see notification_handlers.rs)
        .route("/notifications", get(notification_handlers::list_own_notifications))
        .route("/notifications/unread-count", get(notification_handlers::unread_count))
        .route("/notifications/read-all", axum::routing::post(notification_handlers::mark_all_read))
        .route("/notifications/:id/read", axum::routing::post(notification_handlers::mark_read))
        .route("/notifications/devices", axum::routing::post(notification_handlers::register_device))
        .route("/notifications/devices/revoke", axum::routing::post(notification_handlers::revoke_device))
        .route("/notifications/preferences", get(notification_handlers::get_preferences).put(notification_handlers::update_preferences))
        .route("/notifications/:id", get(notification_handlers::detail))
        .route("/doctor/chats", get(doctor_handlers::list_own_chats))
        .route("/doctor/appointments/:current_appointment_id/patient-history", get(doctor_handlers::list_patient_history))
        .route("/doctor/appointments/:current_appointment_id/patient-history/:historical_consultation_id", get(doctor_handlers::get_patient_history_detail))
        .route("/doctor/consultations/:appointment_id/summary", get(doctor_handlers::consultation_summary))
        .route("/doctor/wallet/ledger", get(doctor_handlers::wallet_ledger))
        .route("/doctor/qna/questions", get(doctor_handlers::list_open_questions))
        .route("/doctor/qna/questions/:question_id/answers", axum::routing::post(doctor_handlers::answer_question))
        .route("/doctor/legal/acceptances", get(legal_handlers::get_own_acceptances))
        .route(
            "/patients/legal/acceptances",
            get(legal_handlers::get_patient_acceptances),
        )
        // Patients
        .route("/patients/me/data-requests", get(crate::api::v1::data_rights_handlers::list_own).post(crate::api::v1::data_rights_handlers::create))
        .route("/admin/patient-data-requests", get(crate::api::v1::data_rights_handlers::list_for_review))
        .route("/admin/patient-data-requests/:id", axum::routing::patch(crate::api::v1::data_rights_handlers::review))
        .route("/patients/me", get(patient_handlers::get_own_profile).patch(patient_handlers::update_own_profile))
        .route("/patients/me/photo", axum::routing::post(crate::api::v1::profile_picture_handlers::upload_patient_photo)
            .get(crate::api::v1::profile_picture_handlers::get_patient_photo)
            .delete(crate::api::v1::profile_picture_handlers::delete_patient_photo))
        .route("/patients/me/appointments", get(patient_handlers::list_own_appointments))
        .route("/patients/me/documents", get(patient_handlers::list_own_documents))
        .route("/patients/me/qna/questions", get(patient_handlers::list_answered_questions).post(patient_handlers::submit_question))
        .route("/patients/me/profiles", get(patient_handlers::list_profiles).post(patient_handlers::create_dependent_profile))
        .route("/patients/me/profiles/:id", axum::routing::patch(patient_handlers::update_dependent_profile))
        .route("/patients/me/payment-methods", axum::routing::get(patient_handlers::list_own_payment_methods).post(patient_handlers::save_payment_method))
        .route("/patients/me/payment-methods/:id", axum::routing::delete(patient_handlers::remove_payment_method))
        .route("/patients/me/payment-methods/:id/set-primary", axum::routing::post(patient_handlers::set_primary_payment_method))
        .route(
            "/config/booking-policy",
            get(config_handlers::booking_policy),
        )
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
            "/appointments/:id/conversation",
            get(appointment_handlers::get_conversation_for_appointment),
        )
        .route(
            "/appointments/:id/consultation",
            get(appointment_handlers::get_consultation_for_appointment),
        )
        .route(
            "/appointments/:id/prescriptions",
            axum::routing::post(prescription_handlers::upload_patient_intake),
        )
        .route(
            "/appointments/:id/documents/:document_id",
            get(prescription_handlers::get_document_download_url),
        )
        .route("/appointments/:id/payment", axum::routing::post(appointment_handlers::initiate_payment))
        .route("/chat/:id/attachments", axum::routing::post(crate::api::v1::chat_attachment_handlers::upload))
        .route("/chat/:id/messages/:message_id/attachment", get(crate::api::v1::chat_attachment_handlers::view))
        // Consultations & Agora
        .route(
            "/consultations/:appointment_id/rtc-token",
            axum::routing::post(consultation_handlers::mint_agora_token),
        )
        .route(
            "/consultations/:appointment_id/join-eligibility",
            get(consultation_handlers::join_eligibility),
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
            "/doctor/wallet/payout-methods/bind",
            axum::routing::post(wallet_handlers::bind_payout_destination),
        )
        .route(
            "/doctor/wallet/payout-methods/set-primary",
            axum::routing::post(wallet_handlers::set_primary_payout_destination),
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
        .route(
            "/admin/patients/:id/data-rights/erase",
            axum::routing::post(admin_handlers::execute_patient_data_rights),
        )
        .route(
            "/admin/doctors/:id/data-rights/erase",
            axum::routing::post(admin_handlers::execute_doctor_data_rights),
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
            "/admin/doctors/:id/revoke",
            axum::routing::post(admin_handlers::revoke_doctor_professional_registration),
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
        .route(
            "/admin/consultations/:session_id/recovery-resolution",
            axum::routing::post(admin_handlers::resolve_consultation_recovery),
        )
        .route(
            "/admin/consultations/live-reservations/release",
            axum::routing::post(admin_handlers::release_live_reservation),
        )
        .route(
            "/admin/platform-payment-methods",
            axum::routing::get(admin_handlers::list_platform_payment_methods)
                .post(admin_handlers::save_platform_payment_method),
        )
        .route(
            "/admin/platform-payment-methods/:id/set-primary",
            axum::routing::post(admin_handlers::set_primary_platform_payment_method),
        )
        .route(
            "/admin/platform-payment-methods/:id",
            axum::routing::delete(admin_handlers::deactivate_platform_payment_method),
        )
        // Admin Campaigns (Milestone 6B — draft/review/approval only, no dispatch)
        .route(
            "/admin/campaigns",
            get(campaign_handlers::list).post(campaign_handlers::create),
        )
        .route(
            "/admin/campaigns/audience-preview",
            get(campaign_handlers::audience_preview),
        )
        .route(
            "/admin/campaigns/:id",
            get(campaign_handlers::detail).patch(campaign_handlers::update),
        )
        .route(
            "/admin/campaigns/:id/submit",
            axum::routing::post(campaign_handlers::submit),
        )
        .route(
            "/admin/campaigns/:id/approve",
            axum::routing::post(campaign_handlers::approve),
        )
        .route(
            "/admin/campaigns/:id/cancel",
            axum::routing::post(campaign_handlers::cancel),
        )
        .route(
            "/admin/campaigns/:id/schedule",
            axum::routing::post(campaign_handlers::schedule),
        )
        .route(
            "/admin/campaigns/:id/pause",
            axum::routing::post(campaign_handlers::pause),
        )
        .route(
            "/admin/campaigns/:id/resume",
            axum::routing::post(campaign_handlers::resume),
        )
        .route(
            "/admin/campaigns/:id/stats",
            get(campaign_handlers::stats),
        )
}
