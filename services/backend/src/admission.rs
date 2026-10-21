//! Fail-fast admission before Redis, session SQL, body decoding or handlers.
//! Counters have fixed labels; dependency incidents are coalesced, never one
//! database log row per rejected request.
use crate::repository::AppState;
use axum::{
    extract::{Request, State},
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use parking_lot::Mutex;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cost {
    Health,
    Webhook,
    Consultation,
    Auth,
    Upload,
    Payment,
    Expensive,
    Normal,
}
impl Cost {
    pub fn label(self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::Webhook => "webhook",
            Self::Consultation => "consultation",
            Self::Auth => "auth",
            Self::Upload => "upload",
            Self::Payment => "payment",
            Self::Expensive => "expensive",
            Self::Normal => "normal",
        }
    }
    fn reserved(self) -> bool {
        matches!(self, Self::Health | Self::Webhook | Self::Consultation)
    }
    fn index(self) -> usize {
        self as usize
    }
}

/// Classify from the method/path, never User-Agent, caller role, or IP.
pub fn classify(method: &Method, path: &str) -> Cost {
    if matches!(path, "/healthz" | "/metrics") {
        return Cost::Health;
    }
    if path.starts_with("/api/v1/webhooks/payment/") {
        return Cost::Webhook;
    }
    if path.starts_with("/api/v1/auth/") {
        return Cost::Auth;
    }
    if *method == Method::POST
        && (path.ends_with("/prescriptions")
            || path.ends_with("/prescription")
            || path.ends_with("/attachments"))
    {
        return Cost::Upload;
    }
    if path == "/api/v1/appointments/book" || path.ends_with("/payment") {
        return Cost::Payment;
    }
    if path.starts_with("/api/v1/documents/")
        || path.contains("/documents/")
        || path.ends_with("/attachment")
        || path.starts_with("/api/v1/admin/")
    {
        return Cost::Expensive;
    }
    if path == "/api/v1/doctor/chats" || path.starts_with("/api/v1/doctor/consultations/") {
        return Cost::Consultation;
    }
    if path.starts_with("/api/v1/consultations/") || path.starts_with("/api/v1/chat/") {
        return Cost::Consultation;
    }
    Cost::Normal
}

struct Bucket {
    tokens: f64,
    last: Instant,
    rate: f64,
}
impl Bucket {
    fn new(rate: u32) -> Self {
        Self {
            tokens: rate as f64,
            last: Instant::now(),
            rate: rate as f64,
        }
    }
    fn take(&mut self, now: Instant) -> bool {
        self.tokens = (self.tokens
            + now.saturating_duration_since(self.last).as_secs_f64() * self.rate)
            .min(self.rate);
        self.last = now.max(self.last);
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }
}

pub struct Admission {
    ordinary: Arc<Semaphore>,
    other_work: Arc<Semaphore>,
    lanes: [Arc<Semaphore>; 8],
    service_tokens: Mutex<Bucket>,
    lane_tokens: [Mutex<Bucket>; 8],
    last_limiter_report: Mutex<Option<Instant>>,
}

pub struct Lease {
    _ordinary: Option<OwnedSemaphorePermit>,
    _other: Option<OwnedSemaphorePermit>,
    _lane: OwnedSemaphorePermit,
    cost: Cost,
}
impl Drop for Lease {
    fn drop(&mut self) {
        metrics::gauge!("admission_inflight", "class" => self.cost.label()).decrement(1.0);
    }
}

impl Admission {
    pub fn new(pool_capacity: u32) -> Self {
        // Conservative initial policy, not benchmarked capacity. The cap scales
        // down with small pools; no queue is created when it is exhausted.
        let n = pool_capacity.max(1) as usize;
        // Auth login work must leave a connection for already-authenticated
        // session checks when the deployment has at least two auth connections.
        let auth_limit = n.saturating_sub(1).clamp(1, 4);
        let limits = [
            2,
            2,
            n.min(4),
            auth_limit,
            n.min(4),
            n.min(2),
            n.min(2),
            n.min(8),
        ];
        let rates = [10, 20, 60, 12, 8, 10, 20, 100];
        Self {
            ordinary: Arc::new(Semaphore::new(n.saturating_mul(2).min(24))),
            other_work: Arc::new(Semaphore::new(
                n.saturating_mul(2).min(24).saturating_sub(1),
            )),
            lanes: limits.map(|n| Arc::new(Semaphore::new(n))),
            service_tokens: Mutex::new(Bucket::new(100)),
            lane_tokens: rates.map(|n| Mutex::new(Bucket::new(n))),
            last_limiter_report: Mutex::new(None),
        }
    }
    pub fn report_limiter_outage(&self) {
        let now = Instant::now();
        let mut last = self.last_limiter_report.lock();
        if last.is_some_and(|previous| {
            now.saturating_duration_since(previous) < Duration::from_secs(30)
        }) {
            return;
        }
        *last = Some(now);
        drop(last);
        // One dependency incident per 30 seconds, not one DB write per reject.
        crate::system_error::report(
            crate::system_error::ErrorEvent::new(
                crate::domain::models::ErrorSeverity::Warning,
                "REDIS",
                "Admission::RateLimitStore",
                "RATE_LIMIT_SERVICE_UNAVAILABLE",
                "Rate limiter temporarily unavailable.",
                crate::api::middleware::request_context::request_id(),
            )
            .with_http_status(503),
        );
    }
    pub fn enter(&self, cost: Cost) -> Result<Lease, StatusCode> {
        let ordinary = if cost.reserved() {
            None
        } else {
            Some(
                self.ordinary
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
            )
        };
        let other = if cost.reserved() || cost == Cost::Normal {
            None
        } else {
            Some(
                self.other_work
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
            )
        };
        let lane = self.lanes[cost.index()]
            .clone()
            .try_acquire_owned()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let now = Instant::now();
        if !cost.reserved() && !self.service_tokens.lock().take(now) {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        if !self.lane_tokens[cost.index()].lock().take(now) {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        metrics::gauge!("admission_inflight", "class" => cost.label()).increment(1.0);
        metrics::counter!("admission_accepted_total", "class" => cost.label()).increment(1);
        Ok(Lease {
            _ordinary: ordinary,
            _other: other,
            _lane: lane,
            cost,
        })
    }
}

pub fn reject(status: StatusCode, cost: Cost, reason: &'static str) -> Response {
    metrics::counter!("admission_rejected_total", "class" => cost.label(), "reason" => reason)
        .increment(1);
    let code = match status {
        StatusCode::SERVICE_UNAVAILABLE if reason == "limiter_unavailable" => {
            "RATE_LIMIT_SERVICE_UNAVAILABLE"
        }
        StatusCode::TOO_MANY_REQUESTS => "RATE_LIMIT_EXCEEDED",
        StatusCode::FORBIDDEN => "AUTH_CHALLENGE_FAILED",
        StatusCode::BAD_REQUEST => "INVALID_REQUEST",
        _ => "SERVICE_OVERLOADED",
    };
    let mut response = (status, Json(serde_json::json!({
        "success": false, "error": { "code": code, "message": "We couldn't complete this request. Please try again shortly." },
        "meta": { "timestamp": chrono::Utc::now(), "request_id": crate::api::middleware::request_context::request_id() }
    }))).into_response();
    response.headers_mut().insert(
        header::RETRY_AFTER,
        axum::http::HeaderValue::from_static("1"),
    );
    response
}

fn exhausted(pool: &sqlx::PgPool) -> bool {
    pool.is_closed()
        || (pool.size() >= pool.options().get_max_connections() && pool.num_idle() == 0)
}

pub async fn middleware(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let cost = classify(request.method(), request.uri().path());
    let _lease = match state.admission.enter(cost) {
        Ok(lease) => lease,
        Err(status) => return reject(status, cost, "budget"),
    };
    // Reserved paths do not consume ordinary permits or depend on API/auth
    // pool pressure. Webhooks use their existing separate payment role.
    let pressure = match cost {
        Cost::Health => false,
        Cost::Webhook => exhausted(&state.payment_db),
        Cost::Auth => exhausted(&state.auth_db),
        Cost::Payment => exhausted(&state.auth_db) || exhausted(&state.payment_db),
        Cost::Expensive => {
            exhausted(&state.auth_db) || exhausted(&state.db) || exhausted(&state.finance_db)
        }
        _ => exhausted(&state.auth_db) || exhausted(&state.db),
    };
    for (name, pool) in [
        ("api", &state.db),
        ("auth", &state.auth_db),
        ("payment", &state.payment_db),
        ("finance", &state.finance_db),
    ] {
        metrics::gauge!("db_pool_size", "role" => name).set(pool.size() as f64);
        metrics::gauge!("db_pool_idle", "role" => name).set(pool.num_idle() as f64);
    }
    if pressure {
        return reject(StatusCode::SERVICE_UNAVAILABLE, cost, "dependency_pressure");
    }
    // All subsequent Redis work is bounded by the admitted request count.
    // Shared service and route budgets span replicas, independently of IP.
    if cost != Cost::Health {
        let budget = async {
            if !cost.reserved() {
                crate::rate_limit::check_and_increment(
                    &state.redis,
                    "ratelimit:service:all",
                    6000,
                    60,
                )
                .await?;
            }
            let limit = match cost {
                Cost::Webhook => 1200,
                Cost::Consultation => 3600,
                Cost::Auth => 240,
                Cost::Upload => 120,
                Cost::Payment => 300,
                Cost::Expensive => 600,
                _ => 6000,
            };
            crate::rate_limit::check_and_increment(
                &state.redis,
                &format!("ratelimit:service:{}", cost.label()),
                limit,
                60,
            )
            .await
        };
        match tokio::time::timeout(Duration::from_millis(300), budget).await {
            Ok(Ok(())) => {}
            Ok(Err(crate::error::AppError::RateLimitExceeded(_))) => {
                return reject(StatusCode::TOO_MANY_REQUESTS, cost, "shared_budget")
            }
            _ => {
                state.admission.report_limiter_outage();
                return reject(StatusCode::SERVICE_UNAVAILABLE, cost, "limiter_unavailable");
            }
        }
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn small_pools_keep_auth_and_reserved_lane_bounds() {
        for (capacity, auth_limit) in [
            (0, 1),
            (1, 1),
            (2, 1),
            (3, 2),
            (4, 3),
            (5, 4),
            (10, 4),
            (u32::MAX, 4),
        ] {
            let gate = Admission::new(capacity);
            let n = capacity.max(1) as usize;
            assert_eq!(
                gate.lanes[Cost::Auth.index()].available_permits(),
                auth_limit
            );
            assert_eq!(
                gate.lanes[Cost::Normal.index()].available_permits(),
                n.min(8)
            );
            assert_eq!(
                gate.lanes[Cost::Consultation.index()].available_permits(),
                n.min(4)
            );
            assert_eq!(gate.lanes[Cost::Webhook.index()].available_permits(), 2);
            assert_eq!(gate.lanes[Cost::Health.index()].available_permits(), 2);
        }
    }
    #[test]
    fn unique_source_identities_do_not_multiply_concurrency() {
        let gate = Admission::new(10);
        let mut leases = Vec::new();
        let mut rejected = 0;
        for _logical_ip in 0..30_000 {
            match gate.enter(Cost::Normal) {
                Ok(lease) => leases.push(lease),
                Err(_) => rejected += 1,
            }
        }
        assert_eq!(leases.len(), 8);
        assert_eq!(rejected, 29_992);
        assert!(gate.enter(Cost::Webhook).is_ok());
        assert!(gate.enter(Cost::Health).is_ok());
        assert!(gate.enter(Cost::Consultation).is_ok());
        drop(leases);
        assert!(gate.enter(Cost::Normal).is_ok());
    }
    #[test]
    fn expensive_flood_does_not_take_ordinary_slots() {
        for cost in [Cost::Auth, Cost::Upload, Cost::Payment, Cost::Expensive] {
            let gate = Admission::new(10);
            let mut held = Vec::new();
            while let Ok(lease) = gate.enter(cost) {
                held.push(lease);
            }
            assert!(held.len() <= 4);
            assert!(gate.enter(Cost::Normal).is_ok());
            assert!(gate.enter(Cost::Webhook).is_ok());
        }
    }
    #[test]
    fn route_priority_cannot_bypass_upload_lane() {
        assert_eq!(
            classify(&Method::POST, "/api/v1/chat/a/attachments"),
            Cost::Upload
        );
        assert_eq!(
            classify(&Method::POST, "/api/v1/consultations/a/prescription"),
            Cost::Upload
        );
        assert_eq!(
            classify(&Method::POST, "/api/v1/appointments/a/payment"),
            Cost::Payment
        );
        assert_eq!(
            classify(&Method::GET, "/api/v1/chat/a/messages"),
            Cost::Consultation
        );
    }
    #[test]
    fn local_rate_budget_bounds_sequential_work_too() {
        let mut bucket = Bucket::new(2);
        let now = Instant::now();
        assert!(bucket.take(now));
        assert!(bucket.take(now));
        assert!(!bucket.take(now));
        assert!(bucket.take(now + Duration::from_millis(500)));
        assert!(!bucket.take(now + Duration::from_millis(500)));
    }
}
