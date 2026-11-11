use crate::repository::AppState;
use crate::{ip_jail, rate_limit};
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};
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
    // A restored environment starts read-mostly. Authentication and health
    // endpoints remain available for the recovery operator, while every other
    // mutating API is rejected until the process is restarted with
    // RECOVERY_MODE=false. This prevents payments, registration, consultation
    // mutations and arbitrary recovered outbox replay during reconciliation.
    let operator_authenticated = recovery_operator_authenticated(&state, request.headers());
    if state.config.recovery_mode
        && !recovery_request_allowed(
            request.method(),
            request.uri().path(),
            operator_authenticated,
        )
    {
        return crate::admission::reject(
            StatusCode::SERVICE_UNAVAILABLE,
            crate::admission::classify(request.method(), request.uri().path()),
            "recovery_mode",
        );
    }
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

pub(crate) fn recovery_operator_authenticated(state: &AppState, headers: &HeaderMap) -> bool {
    let Some(expected) = state.config.recovery_operator_token_sha256.as_deref() else {
        return false;
    };
    let Some(presented) = headers
        .get("x-recovery-operator-token")
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    operator_digest_matches(expected, presented)
}

/// `pub` (not just `pub(crate)`) so the operator-run `dr_reconcile` example
/// binary (Remediation E1) can authenticate itself with the exact same
/// constant-time comparison this HTTP guard uses, rather than a second,
/// independently-maintained implementation.
pub fn operator_digest_matches(expected: &str, presented: &str) -> bool {
    let actual = hex::encode(Sha256::digest(presented.as_bytes()));
    let expected = expected.to_ascii_lowercase();
    actual.len() == expected.len()
        && actual
            .as_bytes()
            .iter()
            .zip(expected.as_bytes())
            .fold(0u8, |difference, (left, right)| difference | (left ^ right))
            == 0
}

fn recovery_request_allowed(method: &Method, path: &str, operator_authenticated: bool) -> bool {
    if path == "/healthz" {
        return true;
    }
    *method == Method::GET && path == "/api/v1/recovery/status" && operator_authenticated
}

#[cfg(test)]
mod tests {
    use super::{operator_digest_matches, recovery_request_allowed};
    use axum::http::Method;
    use sha2::Digest;

    #[test]
    fn recovery_mode_allows_only_health_and_operator_status() {
        assert!(recovery_request_allowed(&Method::GET, "/healthz", false));
        assert!(recovery_request_allowed(
            &Method::GET,
            "/api/v1/recovery/status",
            true
        ));
        assert!(!recovery_request_allowed(
            &Method::GET,
            "/api/v1/recovery/status",
            false
        ));
        assert!(!recovery_request_allowed(
            &Method::GET,
            "/api/v1/articles",
            true
        ));
        assert!(!recovery_request_allowed(
            &Method::POST,
            "/api/v1/auth/admin/login",
            true
        ));
    }

    #[test]
    fn recovery_operator_token_is_digest_bound_and_rejects_stale_values() {
        let expected = hex::encode(sha2::Sha256::digest(b"synthetic-recovery-token"));
        assert!(operator_digest_matches(
            &expected,
            "synthetic-recovery-token"
        ));
        assert!(!operator_digest_matches(&expected, "stale-token"));
    }
}
