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
    let ip = addr.ip().to_string();

    if let Err(error) = rate_limit::check_global_ip_limit(
        &state.redis,
        &ip,
        state.config.global_ip_rate_limit_per_min,
    )
    .await
    {
        return error.into_response();
    }
    match ip_jail::is_ip_jailed(&state.redis, &ip).await {
        Ok(true) => {
            return crate::error::AppError::Forbidden(
                "Too many failed authentication attempts from this network. Try again later."
                    .into(),
            )
            .into_response()
        }
        Ok(false) => {}
        Err(error) => return error.into_response(),
    }
    next.run(request).await
}
