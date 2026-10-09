//! Redis-backed rate limiting, implementing the per-scope matrix from
//! `docs/AUTH_SECURITY.md` §9 (global per-IP, OTP, login, booking, uploads, grievances).
//!
//! Fixed-window counters (INCR + EXPIRE-once) rather than a true sliding log — adequate
//! for abuse/brute-force protection at this scale and far cheaper than a sorted-set
//! sliding window per request.

use crate::error::AppError;
use deadpool_redis::Pool;

pub async fn check_and_increment(
    pool: &Pool,
    key: &str,
    limit: u32,
    window_secs: u64,
) -> Result<(), AppError> {
    let mut conn = pool.get().await.map_err(|e| {
        AppError::rate_limit_service_unavailable(format!("Rate limiter store unavailable: {e}"))
    })?;

    let count: u32 = deadpool_redis::redis::cmd("EVAL").arg(
        "local n=redis.call('INCR',KEYS[1]); if n==1 then redis.call('EXPIRE',KEYS[1],ARGV[1]) end; return n"
    ).arg(1).arg(key).arg(window_secs).query_async(&mut conn).await
     .map_err(|e| AppError::rate_limit_service_unavailable(format!("Rate limiter store error: {e}")))?;

    if count > limit {
        return Err(AppError::RateLimitExceeded(
            "Too many requests — please slow down and try again shortly.".into(),
        ));
    }

    Ok(())
}

/// Global per-IP ceiling applied to every request (120 req/min/IP per §9 by
/// default). Fails closed with RATE_LIMIT_SERVICE_UNAVAILABLE (503) when
/// Redis is unavailable; an outage never disables the abuse boundary.
///
/// Configurable (`GLOBAL_IP_RATE_LIMIT_PER_MIN`, default 120) specifically so
/// a single-origin load-testing tool (`examples/load_test.rs`) — which,
/// unlike real distributed user traffic, necessarily sends every simulated
/// user's requests from the one real IP of the machine running it — can be
/// pointed at a deliberately raised ceiling to measure the *application's*
/// throughput ceiling instead of immediately re-measuring this already-known,
/// already-deliberate control. The production default is unchanged.
pub async fn check_global_ip_limit(
    pool: &Pool,
    ip: &str,
    limit_per_min: u32,
) -> Result<(), AppError> {
    check_and_increment(pool, &format!("ratelimit:global:{ip}"), limit_per_min, 60).await
}

/// Brute-force-adjacent (bounds how many OTPs can be requested for a phone
/// number): fails closed.
pub async fn check_otp_request_limit(pool: &Pool, phone: &str) -> Result<(), AppError> {
    check_and_increment(pool, &format!("ratelimit:otp:{phone}"), 3, 600).await
}

/// Directly brute-force-shaped: fails closed, same principle as `ip_jail`.
pub async fn check_login_attempt_limit(pool: &Pool, ip: &str) -> Result<(), AppError> {
    check_and_increment(pool, &format!("ratelimit:login:{ip}"), 5, 900).await
}

/// Resource-exhaustion guard: fails closed when the store is unavailable.
pub async fn check_booking_limit(pool: &Pool, user_id: &str) -> Result<(), AppError> {
    check_and_increment(pool, &format!("ratelimit:booking:{user_id}"), 10, 3600).await
}

/// Resource-exhaustion guard: fails closed when the store is unavailable.
pub async fn check_upload_limit(pool: &Pool, user_id: &str) -> Result<(), AppError> {
    check_and_increment(pool, &format!("ratelimit:upload:{user_id}"), 10, 600).await
}

/// Resource-exhaustion guard: fails closed when the store is unavailable.
pub async fn check_grievance_limit(pool: &Pool, user_id: &str) -> Result<(), AppError> {
    check_and_increment(pool, &format!("ratelimit:grievance:{user_id}"), 3, 86400).await
}

/// Resource-exhaustion / spam guard for the 24-hour clinical chat, fails
/// closed. Added after a security audit found `send_chat_message` was the
/// only patient/doctor-facing write endpoint with no rate limiter at all —
/// unlike `check_booking_limit`/`check_grievance_limit` this is a genuinely
/// interactive, two-way channel (a real consultation can reasonably involve
/// several dozen short back-and-forth messages), so a tight hourly cap in
/// the style of booking (10/3600s) would break ordinary clinical use. 40
/// messages per 5-minute window comfortably covers real human typing pace
/// (well under 1 message every 7.5s sustained) while still bounding a
/// scripted flood that would otherwise cheaply spam the other party and
/// burn DB/storage writes for the 24h duration of a conversation. Keyed by
/// sender, not by conversation, since a sender could otherwise spread the
/// same flood across several of their own conversations.
pub async fn check_chat_message_limit(pool: &Pool, user_id: &str) -> Result<(), AppError> {
    check_and_increment(pool, &format!("ratelimit:chat:{user_id}"), 40, 300).await
}
