use crate::domain::ids::DoctorProfileId;
use crate::domain::models::{PaymentStatus, Transaction};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

/// The exact `payment_status` set `admin_handlers::contributes_funded_volume`
/// already defines in Rust -- duplicated here as a literal SQL `IN (...)` so
/// the aggregate queries below answer the identical question, pushed into
/// SQL instead of evaluated per-row in the application after a full-table
/// fetch. Keep in sync with that function if the funded-volume definition
/// ever changes.
const FUNDED_VOLUME_STATUSES: &str =
    "'PAYMENT_HELD','SETTLED_TO_DOCTOR','DISBURSED','REFUND_PENDING'";

pub async fn insert<'e, E: PgExecutor<'e>>(
    executor: E,
    txn: &Transaction,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO transactions (id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, created_at, settled_at, currency, payment_method_id,provider_request_state) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,'NOT_STARTED')",
    )
    .bind(txn.id)
    .bind(&txn.transaction_number)
    .bind(txn.appointment_id)
    .bind(txn.payment_session_id)
    .bind(txn.gateway)
    .bind(&txn.gateway_reference)
    .bind(&txn.gross_amount)
    .bind(&txn.platform_fee_amount)
    .bind(&txn.net_amount)
    .bind(txn.payment_status)
    .bind(txn.created_at)
    .bind(txn.settled_at)
    .bind(&txn.currency)
    .bind(txn.payment_method_id)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_by_payment_session<'e, E: PgExecutor<'e>>(
    executor: E,
    payment_session_id: Uuid,
) -> Result<Option<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, reconciliation_required, created_at, settled_at, currency, disbursement_item_id, payment_method_id \
         FROM transactions WHERE payment_session_id = $1",
    )
    .bind(payment_session_id)
    .fetch_optional(executor)
    .await
}

pub async fn find_by_appointment<'e, E: PgExecutor<'e>>(
    executor: E,
    appointment_id: Uuid,
) -> Result<Option<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, reconciliation_required, created_at, settled_at, currency, disbursement_item_id, payment_method_id \
         FROM transactions WHERE appointment_id = $1 ORDER BY (payment_status <> 'FAILED') DESC, created_at DESC, id DESC LIMIT 1",
    )
    .bind(appointment_id)
    .fetch_optional(executor)
    .await
}

pub async fn list_all<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, reconciliation_required, created_at, settled_at, currency, disbursement_item_id, payment_method_id \
         FROM transactions ORDER BY created_at DESC",
    )
    .fetch_all(executor)
    .await
}

/// M8-G-20: scoped alternative to `list_all` for `GET
/// /admin/finance/transactions?month=YYYY-MM` — the month filter previously
/// fetched every transaction ever created (`list_all`) and discarded
/// everything outside the requested month in Rust via `Vec::retain`. Under a
/// measured 10,000-row fixture this cost ~190ms in deserialization alone
/// before a single row was ever examined for relevance; pushing the same
/// filter into SQL (confirmed equivalent to the original
/// `created_at.date_naive() >= start && < end` comparison, since
/// `created_at` is always `DateTime<Utc>` and `start`/`end` are bound here as
/// UTC midnight instants of those same dates) measured at ~1ms for the
/// equivalent result set. `start` is inclusive, `end` is exclusive — same
/// semantics as the `NaiveDate` range `parse_optional_month` already
/// produces in `admin_handlers.rs`.
pub async fn list_in_range<'e, E: PgExecutor<'e>>(
    executor: E,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<Transaction>, sqlx::Error> {
    let start = start.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let end = end.and_hms_opt(0, 0, 0).unwrap().and_utc();
    sqlx::query_as::<_, Transaction>(
        "SELECT id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, reconciliation_required, created_at, settled_at, currency, disbursement_item_id, payment_method_id \
         FROM transactions WHERE created_at >= $1 AND created_at < $2 ORDER BY created_at DESC",
    )
    .bind(start)
    .bind(end)
    .fetch_all(executor)
    .await
}

/// Milestone 8 Phase 7 Finance SQL remediation (2026-11-03): `GET
/// /admin/overview` only ever needed two lifetime sums, but previously called
/// `list_all()` (every transaction row, including its `transaction_number`/
/// `gateway_reference`/every other column, fetched and deserialized) and
/// summed `gross_amount`/`platform_fee_amount` in Rust after filtering by
/// `contributes_funded_volume`. `EXPLAIN (ANALYZE, BUFFERS)` against the real
/// `hellodoctor_api` role confirmed that query's `ORDER BY created_at DESC`
/// (needed by `list_all`'s other callers, not by this one) forces a full
/// sequential scan plus an external-merge disk sort — measured at ~20MB of
/// temp-file I/O per call against this deployment's transaction volume, with
/// zero `WHERE`/`LIMIT`. A pure aggregate needs no sort at all (nothing here
/// is ordered), so this eliminates the spill at its source instead of only
/// narrowing it. The `IN (...)` filter is applied in SQL via
/// `FILTER (WHERE ...)`, matching `admin_handlers::contributes_funded_volume`
/// exactly — same question, computed by Postgres instead of by Rust after a
/// full-table transfer.
pub async fn funded_volume_totals<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<(BigDecimal, BigDecimal), sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT COALESCE(SUM(gross_amount) FILTER (WHERE payment_status IN ({statuses})),0), \
                COALESCE(SUM(platform_fee_amount) FILTER (WHERE payment_status IN ({statuses})),0) \
         FROM transactions",
        statuses = FUNDED_VOLUME_STATUSES
    ))
    .fetch_one(executor)
    .await
}

/// Aggregate totals `GET /admin/finance/summary` needs. Mirrors
/// `funded_volume_totals`'s rationale: the handler never returns individual
/// transaction rows, only sums, so there is no reason to fetch rows (or sort
/// them) at all. `start`/`end` reuse `list_in_range`'s exact semantics (UTC
/// midnight instants of the given dates, start inclusive/end exclusive,
/// already confirmed equivalent to the prior `created_at.date_naive()`
/// comparison); `None` means "lifetime", matching `get_finance_summary`'s
/// previous unfiltered `list_all()` path for the no-`?month=` case.
pub struct FinanceSummaryTotals {
    pub total_gross: BigDecimal,
    pub total_platform_fee: BigDecimal,
    pub total_net: BigDecimal,
    pub card_volume: BigDecimal,
    pub mpesa_volume: BigDecimal,
}

pub async fn finance_summary_totals<'e, E: PgExecutor<'e>>(
    executor: E,
    range: Option<(NaiveDate, NaiveDate)>,
) -> Result<FinanceSummaryTotals, sqlx::Error> {
    let (start, end) = match range {
        Some((start, end)) => (
            Some(start.and_hms_opt(0, 0, 0).unwrap().and_utc()),
            Some(end.and_hms_opt(0, 0, 0).unwrap().and_utc()),
        ),
        None => (None, None),
    };
    // Bank transfer is grouped into `card_volume` here, matching the
    // pre-existing Rust-side `match` in `get_finance_summary` exactly
    // (`Gateway::Card | Gateway::BankTransfer => card_vol += ...`).
    // BKASH/NAGAD (historical, pre-Kenya-cutover rows only) are deliberately
    // excluded from both buckets, same as before.
    let row: (BigDecimal, BigDecimal, BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as(&format!(
        "SELECT \
           COALESCE(SUM(gross_amount) FILTER (WHERE payment_status IN ({statuses})),0), \
           COALESCE(SUM(platform_fee_amount) FILTER (WHERE payment_status IN ({statuses})),0), \
           COALESCE(SUM(net_amount) FILTER (WHERE payment_status IN ({statuses})),0), \
           COALESCE(SUM(gross_amount) FILTER (WHERE payment_status IN ({statuses}) AND gateway IN ('CARD','BANK_TRANSFER')),0), \
           COALESCE(SUM(gross_amount) FILTER (WHERE payment_status IN ({statuses}) AND gateway='MPESA'),0) \
         FROM transactions \
         WHERE ($1::timestamptz IS NULL OR created_at >= $1) AND ($2::timestamptz IS NULL OR created_at < $2)",
        statuses = FUNDED_VOLUME_STATUSES
    ))
    .bind(start)
    .bind(end)
    .fetch_one(executor)
    .await?;
    Ok(FinanceSummaryTotals {
        total_gross: row.0,
        total_platform_fee: row.1,
        total_net: row.2,
        card_volume: row.3,
        mpesa_volume: row.4,
    })
}

pub async fn mark_payment_held<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    gateway_reference: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE transactions SET payment_status = 'PAYMENT_HELD', gateway_reference = $2,provider_request_state='STARTED' WHERE id = $1 AND payment_status IN ('INITIATED','FAILED') AND gateway_reference IS NULL")
        .bind(id)
        .bind(gateway_reference)
        .execute(executor)
        .await?;
    Ok(result.rows_affected() == 1)
}

/// Conditional on the transaction still being `PAYMENT_HELD` — this is the
/// fix for a real double-credit bug: the caller previously read
/// `payment_status` once in an earlier, already-committed transaction, then
/// trusted that stale snapshot here. Two concurrent `complete_consultation`
/// calls for the same appointment (double-click, client retry) could both
/// pass that stale check and both credit the wallet. Making *this* UPDATE
/// itself the gate — and having the caller check `rows_affected()` before
/// ever crediting the wallet — makes the whole operation safe to attempt
/// twice: only the call that actually flips the status proceeds to credit.
pub async fn mark_settled<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE transactions SET payment_status = 'SETTLED_TO_DOCTOR', settled_at = $2 \
         WHERE id = $1 AND payment_status = 'PAYMENT_HELD'",
    )
    .bind(id)
    .bind(Utc::now())
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Transactions already settled to a doctor's wallet but not yet included in a disbursement batch.
pub async fn list_settled_undisbursed_for_doctor<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: DoctorProfileId,
    period_start: chrono::NaiveDate,
    period_end: chrono::NaiveDate,
) -> Result<Vec<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT t.id, t.transaction_number, t.appointment_id, t.payment_session_id, t.gateway, t.gateway_reference, \
                t.gross_amount, t.platform_fee_amount, t.net_amount, t.payment_status, t.reconciliation_required, t.created_at, t.settled_at, t.currency, t.disbursement_item_id, t.payment_method_id \
         FROM transactions t \
         JOIN appointments a ON a.id = t.appointment_id \
         WHERE a.doctor_id = $1 AND t.payment_status = 'SETTLED_TO_DOCTOR' AND t.disbursement_item_id IS NULL \
           AND t.created_at >= $2::date AND t.created_at < ($3::date + INTERVAL '1 day')",
    )
    .bind(doctor_id)
    .bind(period_start)
    .bind(period_end)
    .fetch_all(executor)
    .await
}

/// Conditional claim: only transitions a transaction from
/// `SETTLED_TO_DOCTOR` to `DISBURSED` if it's still in that state — the fix
/// for a real double-payment bug. `list_settled_undisbursed_for_doctor`'s
/// `SELECT` is unlocked, so two concurrent "Initiate Disbursement" calls can
/// both read the exact same candidate rows before either commits; without
/// this guard, both would sum and pay out the same settled transactions.
/// Postgres's own row-level locking on `UPDATE` serializes concurrent
/// callers here — whichever commits first "wins" each row, and the second
/// caller's `WHERE` clause simply no longer matches once it proceeds.
/// Returns true if *this* call actually claimed the row (safe to include its
/// amounts in the current batch), false if something else already did.
pub async fn claim_for_disbursement<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE transactions SET payment_status = 'DISBURSED' \
         WHERE id = $1 AND payment_status = 'SETTLED_TO_DOCTOR'",
    )
    .bind(id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Allocates a settled obligation to a prepared item without claiming that
/// money was paid. The unique transaction allocation is the exactly-once gate.
pub async fn assign_to_disbursement<'e, E: PgExecutor<'e>>(
    executor: E,
    transaction_id: Uuid,
    item_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE transactions SET disbursement_item_id = $2 WHERE id = $1 AND payment_status = 'SETTLED_TO_DOCTOR' AND disbursement_item_id IS NULL",
    )
    .bind(transaction_id)
    .bind(item_id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn mark_status<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    status: PaymentStatus,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE transactions SET payment_status = $2 WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(executor)
        .await?;
    Ok(())
}
