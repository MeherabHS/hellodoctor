use axum::{extract::MatchedPath, http::Request, middleware::Next, response::IntoResponse};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::{Once, OnceLock};
use std::time::Instant;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static METRICS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();
static TRACING_INIT: Once = Once::new();

/// Initializes the global tracing subscriber. Safe to call more than once
/// (e.g. once per test building its own router) — only the first call takes effect.
pub fn init_tracing() {
    TRACING_INIT.call_once(|| {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "hellodoctor_backend=debug,tower_http=debug,axum=info".into()),
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

/// Request-count and latency instrumentation middleware.
pub async fn track_metrics(req: Request<axum::body::Body>, next: Next) -> impl IntoResponse {
    let start = Instant::now();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| req.uri().path().to_owned());
    let method = req.method().clone();

    let response = next.run(req).await;

    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let labels = [
        ("method", method.to_string()),
        ("path", path),
        ("status", status),
    ];

    metrics::counter!("http_requests_total", &labels).increment(1);
    metrics::histogram!("http_request_duration_seconds", &labels).record(latency);

    response
}
