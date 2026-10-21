use crate::repository::AppState;
use crate::{ip_jail, rate_limit};
use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::SocketAddr;

/// Runs ahead of everything else (auth, ABAC, handlers): jails IPs that have recently
/// failed authentication repeatedly, and enforces the blanket per-IP request ceiling.
/// `transport_guard` has already validated the physical proxy peer and, for a
/// trusted ingress only, replaced ConnectInfo with its sanitized client address.
/// Redis outage is a distinct 503; it cannot disable this security boundary.
pub async fn global_guard_middleware(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    // Local admission already bounded these independent recovery/callback lanes.
    // IP jail or Redis failures for public-user traffic must not block health.
    // Authenticated provider callbacks have their own signature and shared budget.
    if request.uri().path() == "/healthz"
        || request
            .uri()
            .path()
            .starts_with("/api/v1/webhooks/payment/")
    {
        return next.run(request).await;
    }
    let ip = addr.ip().to_string();

    let checks = async {
        rate_limit::check_global_ip_limit(
            &state.redis,
            &ip,
            state.config.global_ip_rate_limit_per_min,
        )
        .await?;
        match ip_jail::is_ip_jailed(&state.redis, &ip).await {
            Ok(true) => {
                return Err(crate::error::AppError::Forbidden(
                    "Too many attempts. Please try again later.".into(),
                ));
            }
            Ok(false) => {}
            Err(error) => return Err(error),
        }
        Ok::<(), crate::error::AppError>(())
    };
    match tokio::time::timeout(std::time::Duration::from_millis(300), checks).await {
        Ok(Ok(())) => next.run(request).await,
        Ok(Err(crate::error::AppError::RateLimitExceeded(_))) => crate::admission::reject(
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            crate::admission::classify(request.method(), request.uri().path()),
            "ip_budget",
        ),
        Ok(Err(crate::error::AppError::RateLimitServiceUnavailable(_))) | Err(_) => {
            state.admission.report_limiter_outage();
            crate::admission::reject(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                crate::admission::classify(request.method(), request.uri().path()),
                "limiter_unavailable",
            )
        }
        Ok(Err(error)) => error.into_response(),
    }
}
