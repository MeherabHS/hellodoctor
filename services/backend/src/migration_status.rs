//! Verifies the live database's applied migrations actually match what this
//! compiled binary's embedded migrator expects to exist.
//!
//! Closes a real incident (audit finding B-01,
//! docs/DOCTOR_SYSTEM_FINAL_MULTI_AGENT_AUDIT.md §3): a long-running backend
//! process kept serving traffic for hours while the database was several
//! migrations behind the code it was executing, silently 500ing on every
//! `POST /auth/doctor/register` call. `sqlx::migrate!` only ever runs at
//! process start — nothing previously checked, while serving requests,
//! whether the schema a process is *actually* running against still matches
//! what it was compiled against. `GET /healthz` now performs that check on
//! every call.
//!
//! This module intentionally cannot fix the case where the *source
//! repository* gains new migration files after this binary was already
//! compiled and started — no running process can discover files that did
//! not exist yet when it launched. That case is a development/deployment
//! concern, handled by `scripts/verify_backend_schema.ps1`, not here.

use sqlx::migrate::Migrator;
use sqlx::PgPool;
use std::collections::BTreeSet;

/// The migrator this binary was compiled with — the single source of truth
/// for "which migrations does this process expect to exist." Never hardcode
/// a version number in application logic; derive it from here instead.
///
/// `sqlx::migrate!` re-scans `./migrations` only when this file is actually
/// recompiled; a new migration file added to disk does not by itself force
/// that (hit directly while adding 20261118000001/20261118000002 — a prior
/// `cargo build` left this macro's embedded list stale until this file's own
/// content changed too). If `/healthz` ever reports `binary_latest_migration`
/// behind the newest file on disk after a real `cargo build`, touch this
/// comment to force re-expansion rather than assuming the migration itself
/// is wrong. (Re-touched again for 20261119000001.) (Re-touched again for
/// 20261120000001.) (Re-touched again for 20261121000001.) (Re-touched again
/// for 20261122000001.) (Re-touched again after 20261122000001's own content
/// changed in place — template-render grants added — still before this
/// milestone was ever reported complete, so this is an in-flight edit of an
/// unshipped migration, not a rewrite of history.) (Re-touched again after
/// adding the `hellodoctor_finance_worker` preferred_language grant to the
/// same in-flight 20261122000001 file.) (Re-touched again after adding `id`
/// to that same grant — a column referenced only in a WHERE filter still
/// needs its own column-level GRANT, not only the columns in the SELECT
/// list.) (Re-touched again for 20261124000001 — Milestones 4+5 engagement
/// and Kenyan holiday notifications.) (Re-touched again for 20261124000002
/// — HelloDoctor -> AfyaMkononi user-facing rebrand.) (Re-touched again
/// after fixing 20261124000001's `notifications_type_check` rebuild, which
/// had accidentally dropped Milestone 2's own additions to that constraint
/// — `DROP CONSTRAINT` + `ADD CONSTRAINT` replaces the whole list, so the
/// base must always be the MOST RECENT prior owner's list, never an older
/// one.) (Re-touched again for 20261125000001/20261126000001/20261126000002
/// — Admin Campaigns, Milestones 6B+6C.) (Re-touched again after fixing
/// 20261126000002's down migration: it had hardcoded a restored constraint
/// name, `campaigns_check1`, that collided with a still-present, untouched
/// 6B constraint already auto-assigned that exact name — caught by
/// `migrator.undo()`/`migrator.run()` round-tripping in this project's own
/// migration-backfill regression tests, not assumed.)
// Re-expand for Milestone 6D's additive 20261127000001 migration, including
// the cleanup-only row-lock permission and Doctor-ID collision guard.
// Re-expand for the additive 6E quiz-revision migration, 20261128000001.
// Re-expand for 6F's restricted Patient quiz functions (20261129000001).
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationStatus {
    /// True iff every migration this binary's embedded `MIGRATOR` expects is
    /// present (and recorded successful) in the live database.
    pub schema_ready: bool,
    pub binary_latest: i64,
    pub database_latest: Option<i64>,
}

/// Pure comparison — no network/DB access, so it's unit-testable without a
/// live Postgres. `expected` is every version this binary's migrator knows
/// about; `applied` is every version `_sqlx_migrations` records as
/// successfully applied.
///
/// A database that is *ahead* of this binary (has extra, newer versions this
/// binary's migrator has never heard of) is still considered ready **for
/// this binary's own purposes** — that situation means an operator applied
/// newer migrations without yet restarting this older process; the fix is
/// restarting the binary, not this check reporting itself unready when it
/// is, in fact, able to serve every table/column/function it was compiled
/// against. See `scripts/verify_backend_schema.ps1` for the complementary
/// check that *does* flag that situation (`BACKEND_RESTART_REQUIRED`).
pub fn schema_ready(expected: &BTreeSet<i64>, applied: &BTreeSet<i64>) -> bool {
    expected.is_subset(applied)
}

/// Queries the live database and compares it against `migrator`. Requires
/// `SELECT` on `_sqlx_migrations` (granted to `hellodoctor_api` by
/// `migrations/20261019000001_migration_status_grant.up.sql` — that table is
/// otherwise only ever touched by the superuser migrator connection, so this
/// grant did not previously exist).
pub async fn check(pool: &PgPool, migrator: &Migrator) -> Result<MigrationStatus, sqlx::Error> {
    let expected: BTreeSet<i64> = migrator.migrations.iter().map(|m| m.version).collect();
    let binary_latest = *expected.iter().max().unwrap_or(&0);

    let applied_rows: Vec<i64> = sqlx::query_scalar(
        "SELECT version FROM _sqlx_migrations WHERE success = true ORDER BY version",
    )
    .fetch_all(pool)
    .await?;
    let applied: BTreeSet<i64> = applied_rows.into_iter().collect();
    let database_latest = applied.iter().max().copied();

    Ok(MigrationStatus {
        schema_ready: schema_ready(&expected, &applied),
        binary_latest,
        database_latest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_when_applied_matches_expected_exactly() {
        let expected: BTreeSet<i64> = [1, 2, 3].into_iter().collect();
        let applied: BTreeSet<i64> = [1, 2, 3].into_iter().collect();
        assert!(schema_ready(&expected, &applied));
    }

    #[test]
    fn not_ready_when_applied_is_missing_a_version_expected_lists() {
        // A gap in the middle, not just an outdated max — the historical
        // incident left some DDL from later files partially applied out of
        // band while `_sqlx_migrations` itself lagged, so checking only the
        // max version would have missed exactly that shape of drift.
        let expected: BTreeSet<i64> = [1, 2, 3].into_iter().collect();
        let applied: BTreeSet<i64> = [1, 3].into_iter().collect();
        assert!(!schema_ready(&expected, &applied));
    }

    #[test]
    fn ready_when_database_is_ahead_of_this_older_binary() {
        let expected: BTreeSet<i64> = [1, 2].into_iter().collect();
        let applied: BTreeSet<i64> = [1, 2, 3, 4].into_iter().collect();
        assert!(schema_ready(&expected, &applied));
    }

    #[test]
    fn not_ready_on_a_completely_unmigrated_database() {
        let expected: BTreeSet<i64> = [1, 2, 3].into_iter().collect();
        let applied: BTreeSet<i64> = BTreeSet::new();
        assert!(!schema_ready(&expected, &applied));
    }
}
