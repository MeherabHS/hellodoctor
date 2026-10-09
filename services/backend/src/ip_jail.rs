//! Multi-dimensional abuse tracking for password-based logins (doctor/admin).
//!
//! A pure IP-keyed jail (the previous design here) is unsafe for Kenya mobile traffic: carrier-grade NAT means hundreds of unrelated real
//! users can share one public IP, so one attacker's — or even one confused
//! legitimate user's — failed attempts could lock out everyone behind that
//! address. This module instead tracks two independent dimensions:
//!
//! - **Account-level**: has *this specific account* (license number / admin
//!   email) failed enough recent attempts to warrant a temporary lock,
//!   regardless of which IP the attempts came from? This is the primary
//!   brute-force defense, and it's carrier-NAT-safe by construction — it
//!   never penalizes a device for another user's mistakes, and it still
//!   stops a distributed attack against one account from many IPs.
//! - **IP-breadth-level**: has *this IP* recently failed logins against many
//!   DISTINCT accounts (a credential-stuffing/scanning signature, not normal
//!   user behavior — a real carrier-NAT IP's legitimate traffic fails its own
//!   small set of accounts' passwords sometimes, it does not fail dozens of
//!   different accounts in a quarter hour)? Only this breadth pattern jails
//!   the IP itself, and it gets a longer cooldown because it's a stronger signal.
//!
//! Both checks fail **closed** on a Redis outage — a brute-force/credential
//! endpoint must not silently disable its own abuse control just because the
//! store is unreachable, even though that means login is unavailable during
//! the outage too. See each function's doc comment; this is a deliberate
//! choice, not an accident of error propagation.

use crate::error::AppError;
use deadpool_redis::redis::AsyncCommands;
use deadpool_redis::Pool;

const ACCOUNT_FAILURE_THRESHOLD: u32 = 5;
const ACCOUNT_FAILURE_WINDOW_SECS: i64 = 900; // 15 minutes
const ACCOUNT_JAIL_COOLDOWN_SECS: i64 = 900; // 15 minutes

/// How many distinct accounts one IP may fail logins against before the IP
/// itself (not just those accounts) gets jailed. Set well above what a real
/// carrier-NAT IP's organic traffic would ever produce in the same window.
const IP_BREADTH_THRESHOLD: usize = 20;
const IP_BREADTH_WINDOW_SECS: i64 = 900; // 15 minutes
/// Longer than the account cooldown: IP-breadth jailing is a stronger,
/// rarer signal (scanning/credential-stuffing), so a longer lockout is an
/// acceptable tradeoff even against shared-IP collateral impact.
const IP_JAIL_COOLDOWN_SECS: i64 = 1800; // 30 minutes

fn redis_unavailable(e: impl std::fmt::Display) -> AppError {
    // A distinct variant (503, "SERVICE_UNAVAILABLE"), not `AppError::Internal`
    // (500, "INTERNAL_SERVER_ERROR") — the fail-closed design above is
    // deliberate, and must be operator-distinguishable from a genuine
    // unhandled bug (see B-05 in docs/audit/DOCTOR_SECURITY_BUG_AUDIT.md).
    AppError::service_unavailable(format!("Abuse-control store unavailable: {e}"))
}

/// True if this specific account has recently failed enough attempts to be
/// locked out, independent of IP. Fails closed: if Redis can't be reached,
/// this returns `Err`, which the caller propagates as a request failure
/// rather than silently treating the account as not-jailed.
pub async fn is_account_jailed(pool: &Pool, account_key: &str) -> Result<bool, AppError> {
    let mut conn = pool.get().await.map_err(redis_unavailable)?;
    let jailed: Option<String> = conn
        .get(format!("jailed:acct:{account_key}"))
        .await
        .map_err(redis_unavailable)?;
    Ok(jailed.is_some())
}

/// True if this IP has recently shown a credential-stuffing/scanning pattern
/// (failed logins against many distinct accounts). Also fails closed.
pub async fn is_ip_jailed(pool: &Pool, ip: &str) -> Result<bool, AppError> {
    let mut conn = pool.get().await.map_err(redis_unavailable)?;
    let jailed: Option<String> = conn
        .get(format!("jailed:ip:{ip}"))
        .await
        .map_err(redis_unavailable)?;
    Ok(jailed.is_some())
}

/// Records a failed login attempt against `account_key` from `ip`. Updates
/// both dimensions independently: the account's own failure counter (jailing
/// just that account past the threshold), and the IP's set of distinct
/// targeted accounts in the current window (jailing the IP itself only once
/// its breadth — not raw volume — crosses the threshold).
pub async fn record_failure(pool: &Pool, ip: &str, account_key: &str) -> Result<(), AppError> {
    let mut conn = pool.get().await.map_err(redis_unavailable)?;

    let acct_key = format!("failcount:acct:{account_key}");
    let acct_count: u32 = conn.incr(&acct_key, 1).await.map_err(redis_unavailable)?;
    if acct_count == 1 {
        let _: () = conn
            .expire(&acct_key, ACCOUNT_FAILURE_WINDOW_SECS)
            .await
            .map_err(redis_unavailable)?;
    }
    if acct_count >= ACCOUNT_FAILURE_THRESHOLD {
        let _: () = conn
            .set_ex(
                format!("jailed:acct:{account_key}"),
                "1",
                ACCOUNT_JAIL_COOLDOWN_SECS as u64,
            )
            .await
            .map_err(redis_unavailable)?;
        tracing::warn!(account = %account_key, "account temporarily locked after repeated authentication failures");
    }

    let targets_key = format!("ip_targets:{ip}");
    let _: () = conn
        .sadd(&targets_key, account_key)
        .await
        .map_err(redis_unavailable)?;
    let _: () = conn
        .expire(&targets_key, IP_BREADTH_WINDOW_SECS)
        .await
        .map_err(redis_unavailable)?;
    let breadth: usize = conn.scard(&targets_key).await.map_err(redis_unavailable)?;
    if breadth >= IP_BREADTH_THRESHOLD {
        let _: () = conn
            .set_ex(format!("jailed:ip:{ip}"), "1", IP_JAIL_COOLDOWN_SECS as u64)
            .await
            .map_err(redis_unavailable)?;
        tracing::warn!(ip = %ip, distinct_accounts = breadth, "IP jailed after credential-stuffing-shaped failure pattern");
    }

    Ok(())
}

/// Clears only this account's own failure counter on a successful login —
/// never clears the IP breadth set, which remains valid attack evidence
/// regardless of whether one of the targeted accounts eventually succeeded
/// (e.g. the legitimate owner logging in while an attacker is also probing).
pub async fn record_success(pool: &Pool, account_key: &str) -> Result<(), AppError> {
    let mut conn = pool.get().await.map_err(redis_unavailable)?;
    let _: () = conn
        .del(format!("failcount:acct:{account_key}"))
        .await
        .map_err(redis_unavailable)?;
    Ok(())
}
