use axum::{extract::MatchedPath, http::Request, middleware::Next, response::IntoResponse};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::{Once, OnceLock};
use std::time::Instant;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static METRICS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Anonymous scrapes exist only in explicit development/test with no configured
/// credential. Deployed scrapes require Authorization, never a URL secret.
pub async fn metrics_auth_guard(
    axum::extract::State(state): axum::extract::State<crate::repository::AppState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let expected = &state.config.metrics_scrape_token;
    if expected.is_empty() && state.config.is_development_or_test() {
        return next.run(request).await;
    }
    let candidate = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));
    let valid = candidate.is_some_and(|candidate| {
        if expected.is_empty() {
            return false;
        }
        // Constant-time tag verification rather than a string equality check.
        let mut correct =
            Hmac::<Sha256>::new_from_slice(b"HelloDoctor metrics credential comparison")
                .expect("HMAC key");
        correct.update(expected.as_bytes());
        let mut actual =
            Hmac::<Sha256>::new_from_slice(b"HelloDoctor metrics credential comparison")
                .expect("HMAC key");
        actual.update(candidate.as_bytes());
        correct
            .verify_slice(&actual.finalize().into_bytes())
            .is_ok()
    });
    if !valid {
        return crate::error::AppError::Unauthorized("Monitoring credential required.".into())
            .into_response();
    }
    next.run(request).await
}
static TRACING_INIT: Once = Once::new();

/// Initializes the global tracing subscriber. Safe to call more than once
/// (e.g. once per test building its own router) — only the first call takes effect.
pub fn init_tracing() {
    TRACING_INIT.call_once(|| {
        // The fallback below only applies when `RUST_LOG` is genuinely unset
        // anywhere — it must itself be production-safe, since a deployment
        // that simply forgot to set `RUST_LOG` should get quiet, not
        // unconditional per-request debug I/O (confirmed live: every single
        // request logged two synchronous DEBUG lines under the old
        // `...=debug,tower_http=debug...` fallback). Local development sets
        // its own verbose value explicitly via `.env`/`.env.example`'s
        // documented `RUST_LOG=debug`, rather than relying on this fallback.
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                    "hellodoctor_backend=info,tower_http=warn,axum=warn".into()
                }),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();
    });
}

/// Installs the process-wide Prometheus recorder (once) and returns a handle whose
/// `render()` serves the `/metrics` endpoint. Safe to call repeatedly (e.g. once per
/// test building its own router) — later calls just return the cached handle.
pub fn init_metrics() -> PrometheusHandle {
    METRICS_HANDLE
        .get_or_init(|| {
            PrometheusBuilder::new()
                .install_recorder()
                .expect("failed to install Prometheus metrics recorder")
        })
        .clone()
}

pub async fn metrics_endpoint(
    axum::extract::State(handle): axum::extract::State<PrometheusHandle>,
) -> impl IntoResponse {
    handle.render()
}

/// HTTP permits extension methods; never retain their caller-controlled text as
/// a metric label, even for requests rejected by routing or admission.
fn method_label(method: &axum::http::Method) -> &'static str {
    match method.as_str() {
        "GET" => "GET",
        "HEAD" => "HEAD",
        "POST" => "POST",
        "PUT" => "PUT",
        "DELETE" => "DELETE",
        "CONNECT" => "CONNECT",
        "OPTIONS" => "OPTIONS",
        "TRACE" => "TRACE",
        "PATCH" => "PATCH",
        _ => "OTHER",
    }
}

/// Request-count and latency instrumentation middleware.
pub async fn track_metrics(req: Request<axum::body::Body>, next: Next) -> impl IntoResponse {
    let start = Instant::now();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| "__unmatched".to_owned());
    let method = method_label(req.method());

    let response = next.run(req).await;

    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let labels = [
        ("method", method.to_owned()),
        ("path", path),
        ("status", status),
    ];

    metrics::counter!("http_requests_total", &labels).increment(1);
    metrics::histogram!("http_request_duration_seconds", &labels).record(latency);

    response
}

#[cfg(test)]
mod tests {
    use super::method_label;
    use axum::http::Method;

    #[test]
    fn method_labels_have_a_fixed_vocabulary() {
        for name in [
            "GET", "HEAD", "POST", "PUT", "DELETE", "CONNECT", "OPTIONS", "TRACE", "PATCH",
        ] {
            assert_eq!(
                method_label(&Method::from_bytes(name.as_bytes()).unwrap()),
                name
            );
        }
        for number in 0..1_000 {
            let method = Method::from_bytes(format!("SYNTHETIC{number}").as_bytes()).unwrap();
            assert_eq!(method_label(&method), "OTHER");
        }
    }
}
