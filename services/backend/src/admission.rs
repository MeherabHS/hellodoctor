//! Fail-fast admission before Redis, session SQL, body decoding or handlers.
//! Counters have fixed labels; dependency incidents are coalesced, never one
//! database log row per rejected request.
//!
//! Milestone 8.0R (admission-control correctness remediation): a Milestone
//! 8.0 load benchmark measured 46-56% of ordinary `GET /doctors`-class
//! traffic rejected at 150-250 offered RPS from 1,200 *independent*
//! synthetic Patient/Doctor accounts, while real error rate stayed under
//! 0.06% and p50 latency stayed flat at 3-5ms throughout — i.e. no
//! hardware/DB saturation, a pure policy ceiling. Root-caused to two
//! redundant, fully account-agnostic global ceilings that used to live here
//! (a local in-process token bucket fixed at 100/s shared across every
//! non-reserved cost class combined, and a duplicate Redis-backed copy of
//! the same ~100/s ceiling keyed `ratelimit:service:all`): the measured
//! aggregate non-reserved success rate matched that hardcoded 100/s almost
//! exactly, confirming it — not the Normal lane's old 8-slot concurrency
//! cap, which flat latency shows had headroom to spare — as the dominant
//! cause. Both were removed, so independent accounts stop competing for one
//! shared allowance — `api/middleware/auth.rs::auth_middleware` already
//! applies a correctly-scoped, *tighter* per-account budget for
//! authenticated Normal-class traffic (2/s/account; real measured
//! per-account traffic ran at ~0.1/s, so this was never the active
//! constraint), which this change leaves untouched. In its place here: a
//! per-principal (authenticated user, or IP for anonymous/invalid-token
//! traffic) Redis budget specifically closing the gap that layer can't
//! cover — nothing previously bounded an anonymous flood per-source before
//! it reached that post-auth check — and the concurrency ceilings that
//! previously assumed a tiny, mostly-unbenchmarked pool capacity were
//! widened to scale with the real combined five-pool capacity this
//! deployment actually has (see `Admission::new`). Sensitive classes
//! (Auth/Upload/Payment/Expensive) and every existing per-account/per-IP
//! anti-abuse control (login/OTP attempt limits, booking limit, `ip_jail`)
//! are untouched.
use crate::repository::AppState;
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use parking_lot::Mutex;
use std::{
    net::SocketAddr,
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
    lane_tokens: [Mutex<Bucket>; 8],
    last_limiter_report: Mutex<Option<Instant>>,
}

pub struct Lease {
    _ordinary: Option<OwnedSemaphorePermit>,
    _other: Option<OwnedSemaphorePermit>,
    _lane: OwnedSemaphorePermit,
    cost: Cost,
    // Milestone 8 benchmark-methodology correction (2026-11-04): additive-only
    // observability, does not affect admission decisions or timing of permit
    // grants/releases in any way. Answers "how long did an admitted request
    // actually hold its lane permit" (distinct from DB connection-acquire
    // latency, which would need a separate sqlx-level hook this change does
    // not add — see restore_canonical_benchmark_baseline.ps1's header for the
    // fuller methodology context this was built to support).
    started: Instant,
    // Milestone 8 hold-duration investigation (2026-11-04): the route
    // TEMPLATE (e.g. "/api/v1/doctors/:id/slots" via axum's `MatchedPath`),
    // never the resolved path — same bounded, ID-free vocabulary
    // `telemetry::track_metrics` already uses for its own `path` label, so
    // this carries no new PII/cardinality risk. Defaults to "__unset" when
    // left at its `enter()`-time value; `middleware()` overwrites it with the
    // real matched path immediately after a successful `enter()`, before any
    // `.await` point, so Drop always sees the real value for a lease that
    // ever reaches `next.run()`.
    endpoint: String,
}
impl Drop for Lease {
    fn drop(&mut self) {
        metrics::gauge!("admission_inflight", "class" => self.cost.label()).decrement(1.0);
        let elapsed = self.started.elapsed().as_secs_f64();
        metrics::histogram!("admission_permit_hold_seconds", "class" => self.cost.label())
            .record(elapsed);
        metrics::histogram!("admission_permit_hold_by_endpoint_seconds", "class" => self.cost.label(), "endpoint" => self.endpoint.clone())
            .record(elapsed);
    }
}

impl Admission {
    pub fn new(pool_capacity: u32) -> Self {
        // Milestone 8.0R: the Normal lane (ordinary, cheap reads/writes —
        // directory browsing, slot lookups, wallet summaries) and the
        // shared `ordinary`/`other_work` ceilings were capped at tiny fixed
        // maximums (8 / 24 / 23) regardless of how large `pool_capacity`
        // actually was, on the stated assumption that this was "a
        // conservative initial policy, not benchmarked capacity." A real
        // Configuration-A load sweep benchmarked it: flat 3-5ms p50 latency
        // at up to 250 offered RPS showed those fixed caps were binding far
        // below any real CPU/DB/Redis pressure. They now scale with
        // `pool_capacity` instead of topping out at a small constant —
        // still bounded (never unlimited), just sized off the deployment's
        // actual combined five-pool capacity rather than an arbitrary
        // placeholder. Auth/Upload/Payment/Expensive — the sensitive,
        // already-separately-protected classes — are intentionally left at
        // their original small, protective ceilings.
        let n = pool_capacity.max(1) as usize;
        // Auth login work must leave a connection for already-authenticated
        // session checks when the deployment has at least two auth connections.
        let auth_limit = n.saturating_sub(1).clamp(1, 4);
        // Milestone 8.0 booking-fairness remediation (2026-10-31): a live,
        // clean-metrics 1,200-active-session run (5-20s think time, the
        // documented `capacity_test.rs` "sessions" mode default — see
        // docs/MILESTONE_8_0_BOOKING_FAIRNESS_REMEDIATION.md) measured
        // `POST /appointments/book` arriving at ~8.2-8.7 req/s aggregate
        // from over a thousand *independent* Patient accounts, none anywhere
        // near their own separate, untouched `check_booking_limit`
        // (10/hour/account). Against that real, legitimate, account-agnostic
        // demand, Payment's concurrency ceiling (previously `n.min(2)` —
        // never scaled with `pool_capacity`, unlike every other class here)
        // was the single largest rejection cause: server-side
        // `admission_rejected_total{class="payment",reason="lane_concurrency"}`
        // (240, surfacing as a misleading-looking 503/"system error") alone
        // exceeded the Redis `shared_class_budget` 429 rejections (219) in
        // the same clean run — i.e. the admission-layer concurrency
        // semaphore, not even the rate budget, was the dominant ceiling.
        // Raised to `n.min(8)`, matching the already-accepted Upload/
        // Consultation precedent shape, not an unbounded value — booking's
        // own p95/p99 latency (401ms/1164ms in that run) means Little's Law
        // (concurrency ~= rate x latency ~= 8.5 x 0.15s ~= 1.3, with real
        // headroom for bursts) comfortably fits under 8 without approaching
        // Expensive/Auth/Upload's still-untouched, still-protective n.min(4)
        // ceilings.
        // Milestone 8 Step 3 (docs/MILESTONE_8_SIX_SERVICE_PERFORMANCE_REMEDIATION.md):
        // Consultation (chat/rtc-token/telemetry) was the single largest
        // `lane_concurrency` rejection source in the Phase A six-service
        // evidence (31,687 of 117,310 total admission rejections), capped at
        // `n.min(4)` regardless of how large the DB pool actually was — the
        // same "fixed ceiling independent of pool_capacity" shape already
        // corrected once for Payment above. Raised to `n.min(6)` to test
        // whether real headroom exists, following that exact precedent.
        //
        // Milestone 8 Phase 7 (2026-11-03): Webhook (index 1) was the one
        // sibling of this exact "fixed, unscaled ceiling" shape never
        // corrected — still the original constant `2` while Payment,
        // Consultation, Expensive and Normal were each raised. A controlled
        // 200-sustained-RPS six-service benchmark measured
        // `admission_rejected_total{class="webhook",reason="lane_concurrency"}`
        // = 322, an exact match (not merely close) to the 322 client-observed
        // `POST webhook confirm` system errors in that same run — over 90% of
        // the run's entire error budget, confirmed via EXPLAIN-free, purely
        // metrics-based root-causing (no query was slow; the fixed lane
        // ceiling was hit by ordinary booking-confirmation burstiness, not by
        // genuine payment-processing load). Payment and Webhook are
        // sequential steps of the same booking+payment-confirmation flow
        // (see the local-rate-matching comment above), so raised to match
        // Payment's own `n.min(8)` precedent exactly rather than picking an
        // arbitrary new number — scoped narrowly to 4 first per product-owner
        // instruction, to be validated before considering 8. Reviewed first
        // for payment-processing safety: `AppointmentService::
        // process_payment_webhook` is idempotency-protected by a unique
        // `(gateway, provider_event_id)` constraint and per-transaction
        // `SELECT ... FOR UPDATE` locking (not a global lock), so raising
        // this ceiling changes only how much legitimate concurrent
        // confirmation traffic is admitted, never payment correctness under
        // concurrency.
        let limits = [
            2,
            4,
            n.min(6),
            auth_limit,
            n.min(4),
            n.min(8),
            n.min(2),
            n.min(64),
        ];
        // Milestone 8.0R: Normal's local per-process rate (index 7) raised
        // from 100 to 400/s — still a bounded, evidence-justified global
        // backstop for this one cheap-read class, not the per-account
        // fairness mechanism (that is the new `ratelimit:principal:normal:*`
        // Redis check in `middleware`, below). The removed `service_tokens`
        // bucket (previously a *second*, cross-class-combined 100/s ceiling
        // shared by every non-reserved class together) was the dominant,
        // fully account-agnostic cause of the measured 46-56% rejection
        // rate and had no distinct purpose once each class already has its
        // own `lane_tokens` entry and its own `ratelimit:service:{class}`
        // Redis budget.
        //
        // Milestone 8.0 booking-fairness remediation: Payment's local rate
        // (index 5) raised from 10 to 20/s, matching Webhook's own local
        // rate exactly — the two are sequential steps of the same booking
        // + payment-confirmation flow, so giving Payment a *lower* local
        // ceiling than the webhook call that follows every successful
        // booking never made sense independently of the 1,200-session
        // evidence above. Kept at/above (never below) the widened Redis
        // budget below, same relationship Webhook's own pair already has,
        // so the per-process bucket is not reintroduced as a tighter,
        // un-benchmarked bottleneck once the concurrency/Redis ceilings are
        // widened.
        let rates = [10, 20, 60, 12, 8, 20, 20, 400];
        Self {
            ordinary: Arc::new(Semaphore::new(n.saturating_mul(2).min(128))),
            other_work: Arc::new(Semaphore::new(
                n.saturating_mul(2).min(128).saturating_sub(1),
            )),
            lanes: limits.map(|n| Arc::new(Semaphore::new(n))),
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
    /// Milestone 8.0R: returns the exact reason a request was turned away
    /// (`(StatusCode, reason_label)`) instead of a bare status code, so
    /// `admission_rejected_total{reason=...}` can distinguish a global
    /// concurrency ceiling from a per-lane concurrency ceiling from a local
    /// rate-limit bucket — the three were previously indistinguishable
    /// (all surfaced as the single generic reason `"budget"`), which is
    /// exactly the ambiguity that made this class of issue hard to
    /// root-cause from metrics alone.
    pub fn enter(&self, cost: Cost) -> Result<Lease, (StatusCode, &'static str)> {
        let ordinary = if cost.reserved() {
            None
        } else {
            Some(
                self.ordinary
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "ordinary_concurrency"))?,
            )
        };
        let other = if cost.reserved() || cost == Cost::Normal {
            None
        } else {
            Some(
                self.other_work
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "other_work_concurrency"))?,
            )
        };
        let lane = self.lanes[cost.index()]
            .clone()
            .try_acquire_owned()
            .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "lane_concurrency"))?;
        let now = Instant::now();
        if !self.lane_tokens[cost.index()].lock().take(now) {
            return Err((StatusCode::TOO_MANY_REQUESTS, "lane_rate_limit"));
        }
        metrics::gauge!("admission_inflight", "class" => cost.label()).increment(1.0);
        metrics::counter!("admission_accepted_total", "class" => cost.label()).increment(1);
        Ok(Lease {
            _ordinary: ordinary,
            _other: other,
            _lane: lane,
            cost,
            started: now,
            endpoint: String::from("__unset"),
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

/// Milestone 8.0R: a best-effort identity for *fairness keying only*, never
/// for authorization — an invalid, expired or missing token simply falls
/// back to the IP bucket rather than rejecting here; the real auth check
/// still happens downstream exactly as before. Deliberately a pure,
/// stateless signature+expiry check (the same primitive
/// `api/middleware/auth.rs` uses), not a session/DB lookup, so this keeps
/// admission's own "fail-fast before session SQL" property intact.
fn principal(state: &AppState, request: &Request, addr: SocketAddr) -> String {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    match token.and_then(|t| {
        crate::crypto::verify_access_token(t, &state.config.jwt_ed25519_public_key_b64).ok()
    }) {
        Some(claims) => format!("acct:{}", claims.user_id),
        None => format!("anon:{}", addr.ip()),
    }
}

pub async fn middleware(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let cost = classify(request.method(), request.uri().path());
    let mut _lease = match state.admission.enter(cost) {
        Ok(lease) => lease,
        Err((status, reason)) => return reject(status, cost, reason),
    };
    // Milestone 8 hold-duration investigation (2026-11-04): same bounded,
    // ID-free route-template extraction `telemetry::track_metrics` already
    // uses for its own `path` label -- axum populates `MatchedPath` for any
    // middleware added via `Router::layer` on the matched router, regardless
    // of layering order, which is exactly why `track_metrics` (also a
    // `Router::layer` middleware) can already read it today.
    _lease.endpoint = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| "__unmatched".to_owned());
    // Reserved paths do not consume ordinary permits or depend on API/auth
    // pool pressure. Webhooks use their existing separate payment role.
    //
    // Milestone 8 benchmark-methodology correction (2026-11-04): the
    // `relevant_pools` list and the `admission_dependency_pressure_pool_total`
    // counter below are additive-only observability answering "which specific
    // role-pool(s) were exhausted when this class rejected" -- requested
    // after per-role pool saturation turned out to be unobservable from the
    // aggregate-only pg_stat_activity sampling the benchmark harness had been
    // using. This does NOT change `pressure`'s value or how it's computed
    // (still the exact same boolean OR of the exact same `exhausted()` calls
    // on the exact same pools per class) -- only records, for a rejection
    // that already happened under the pre-existing unchanged logic, which of
    // the pools that class depends on were actually the exhausted one(s).
    let relevant_pools: &[(&str, &sqlx::PgPool)] = match cost {
        Cost::Health => &[],
        Cost::Webhook => &[("payment", &state.payment_db)],
        Cost::Auth => &[("auth", &state.auth_db)],
        Cost::Payment => &[("auth", &state.auth_db), ("payment", &state.payment_db)],
        Cost::Expensive => &[
            ("auth", &state.auth_db),
            ("api", &state.db),
            ("finance", &state.finance_db),
        ],
        _ => &[("auth", &state.auth_db), ("api", &state.db)],
    };
    let pressure = relevant_pools.iter().any(|(_, pool)| exhausted(pool));
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
        for (name, pool) in relevant_pools {
            if exhausted(pool) {
                metrics::counter!("admission_dependency_pressure_pool_total", "class" => cost.label(), "pool" => *name)
                    .increment(1);
            }
        }
        return reject(StatusCode::SERVICE_UNAVAILABLE, cost, "dependency_pressure");
    }
    // All subsequent Redis work is bounded by the admitted request count.
    // Shared service and route budgets span replicas, independently of IP.
    //
    // Milestone 8.0R: the old cross-class global ceiling
    // (`ratelimit:service:all`, 6000/60s shared by every non-reserved class
    // from every account combined) is removed — it was the dominant,
    // account-agnostic cause of rejecting legitimate diverse traffic (see
    // module doc comment).
    //
    // For AUTHENTICATED callers, `api/middleware/auth.rs::auth_middleware`
    // already applies a correctly-scoped, *tighter* per-account budget
    // post-authentication (`ratelimit:authenticated:{user_id}:normal`,
    // 120/60s = 2/s — real per-account traffic in the benchmark ran at
    // ~0.1/s, far under this; it was never the active constraint and is
    // left untouched). What nothing previously bounded per-source at all is
    // NORMAL-shaped traffic that never reaches that check — anonymous or
    // invalid-token requests, rejected only by auth_middleware *after*
    // admission has already spent a lane/semaphore slot and a DB round
    // trip on them. This principal-budget check closes that gap (Milestone
    // 8.0R Phase 4 scenario 8, "anonymous high-volume requests"): generous
    // for a real authenticated account (30/s, intentionally looser than
    // auth_middleware's own 2/s, which will always bind first for them),
    // but now something bounds a single anonymous source specifically,
    // instead of it being able to consume the whole shared per-class budget
    // alongside legitimate accounts.
    if cost != Cost::Health {
        if cost == Cost::Normal {
            let who = principal(&state, &request, addr);
            // Milestone 8 hold-duration investigation (2026-11-04): additive-
            // only timing around this Redis round-trip -- does not change
            // the timeout, the outcome, or any admission decision, only
            // records how long this specific check took while the lane
            // permit was already held.
            let redis_check_start = Instant::now();
            let outcome = tokio::time::timeout(
                Duration::from_millis(300),
                crate::rate_limit::check_and_increment(
                    &state.redis,
                    &format!("ratelimit:principal:normal:{who}"),
                    1800,
                    60,
                ),
            )
            .await;
            metrics::histogram!("admission_redis_check_seconds", "class" => cost.label(), "check" => "principal_budget")
                .record(redis_check_start.elapsed().as_secs_f64());
            match outcome {
                Ok(Ok(())) => {}
                Ok(Err(crate::error::AppError::RateLimitExceeded(_))) => {
                    return reject(StatusCode::TOO_MANY_REQUESTS, cost, "principal_budget")
                }
                _ => {
                    state.admission.report_limiter_outage();
                    return reject(StatusCode::SERVICE_UNAVAILABLE, cost, "limiter_unavailable");
                }
            }
        }
        let limit = match cost {
            Cost::Webhook => 1200,
            Cost::Consultation => 3600,
            Cost::Auth => 240,
            Cost::Upload => 120,
            // Milestone 8.0 booking-fairness remediation (2026-10-31): raised
            // from 300 (5/s) to 1200 (20/s), matching Webhook's own budget
            // exactly rather than an arbitrary new number — Payment
            // (booking creation) and Webhook (its payment confirmation) are
            // sequential steps of the one flow, so the earlier step having a
            // *lower* account-agnostic ceiling than the step after it was
            // never a deliberate design choice. A clean, 1,200-active-session
            // live run (docs/MILESTONE_8_0_BOOKING_FAIRNESS_REMEDIATION.md)
            // measured real, independent, legitimate booking demand at
            // ~8.2-8.7 req/s aggregate under the project's own documented
            // active-session target, exceeding the old 5/s ceiling by
            // roughly 60-75% and accounting for most of that run's `POST
            // /appointments/book` rejections — none of them attributable to
            // `check_booking_limit` (10/hour/account, untouched) or to any
            // real slot conflict. 1200 keeps real headroom over the measured
            // figure without being unbounded, and does not touch
            // `check_booking_limit`, booking-conflict detection, or payment
            // webhook signature/idempotency integrity.
            Cost::Payment => 1200,
            Cost::Expensive => 600,
            _ => 30000,
        };
        let key = format!("ratelimit:service:{}", cost.label());
        let budget = crate::rate_limit::check_and_increment(&state.redis, &key, limit, 60);
        let shared_check_start = Instant::now();
        let outcome = tokio::time::timeout(Duration::from_millis(300), budget).await;
        metrics::histogram!("admission_redis_check_seconds", "class" => cost.label(), "check" => "shared_class_budget")
            .record(shared_check_start.elapsed().as_secs_f64());
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(crate::error::AppError::RateLimitExceeded(_))) => {
                return reject(StatusCode::TOO_MANY_REQUESTS, cost, "shared_class_budget")
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
                n.min(64)
            );
            assert_eq!(
                gate.lanes[Cost::Consultation.index()].available_permits(),
                n.min(6)
            );
            assert_eq!(gate.lanes[Cost::Webhook.index()].available_permits(), 4);
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
        // Milestone 8.0R: Normal's lane cap scales with pool capacity
        // (n.min(64)) instead of a fixed 8, so Admission::new(10) now admits
        // 10 concurrently, not 8 — this test's actual point (one logical
        // source cannot buy more than the lane's concurrency ceiling by
        // claiming many identities) is unaffected by the exact number.
        assert_eq!(leases.len(), 10);
        assert_eq!(rejected, 29_990);
        assert!(gate.enter(Cost::Webhook).is_ok());
        assert!(gate.enter(Cost::Health).is_ok());
        assert!(gate.enter(Cost::Consultation).is_ok());
        drop(leases);
        assert!(gate.enter(Cost::Normal).is_ok());
    }
    #[test]
    fn expensive_flood_does_not_take_ordinary_slots() {
        // Milestone 8.0 booking-fairness remediation: Payment's own cap is
        // now n.min(8) (was n.min(2)), still a small, bounded, deliberately
        // sensitive-class ceiling like its three siblings here — just no
        // longer small enough to misattribute real legitimate booking
        // concurrency as "abuse". Asserted per-class so raising any one of
        // these in the future can't silently hide a regression in another.
        for (cost, expected_cap) in [
            (Cost::Auth, 4),
            (Cost::Upload, 4),
            (Cost::Payment, 8),
            (Cost::Expensive, 2),
        ] {
            let gate = Admission::new(10);
            let mut held = Vec::new();
            while let Ok(lease) = gate.enter(cost) {
                held.push(lease);
            }
            assert_eq!(held.len(), expected_cap, "unexpected cap for {cost:?}");
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
