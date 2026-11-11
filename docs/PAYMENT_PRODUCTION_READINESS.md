# Payment production readiness

| Control | Status |
|---|---|
| Internal PAYMENT_HELD -> settlement state machine | IMPLEMENTED_AND_TESTED |
| Wallet settlement exactly-once guard | IMPLEMENTED_AND_TESTED |
| Disbursement claim exactly-once guard | IMPLEMENTED_AND_TESTED |
| Signed/fresh webhook envelope | IMPLEMENTED_AND_TESTED |
| Provider-to-transaction gateway binding | IMPLEMENTED_AND_TESTED |
| Provider-specific amount/currency/merchant validation | DEFERRED_EXTERNAL_DEPENDENCY |
| Abandoned-payment expiry and slot release worker | IMPLEMENTED_AND_TESTED |
| Late-payment-after-expiry reconciliation flagging/audit/admin visibility | IMPLEMENTED_AND_TESTED |
| Accounting-period-close block on unresolved provider-ambiguous transactions | IMPLEMENTED_AND_TESTED |
| Active UNKNOWN-provider-status polling (query a live gateway for an ambiguous transaction) | DEFERRED_EXTERNAL_DEPENDENCY |
| Provider refund execution/status query | DEFERRED_EXTERNAL_DEPENDENCY |
| Provider payout execution/status query | DEFERRED_EXTERNAL_DEPENDENCY |
| Currency-separated ledger/reporting | NOT_IMPLEMENTED |
| Sandbox end-to-end tests | DEFERRED_EXTERNAL_DEPENDENCY |

No provider integration is fabricated by this document. Real-money enablement
requires provider adapters, sandbox tests, and finance signoff.

## 2026-11-01 correction

The three rows above marked `IMPLEMENTED_AND_TESTED` were previously recorded
as a single `NOT_IMPLEMENTED` row ("UNKNOWN payment status and reconciliation
worker") plus a second `NOT_IMPLEMENTED` row ("Abandoned-payment expiry and
slot release worker"). Both predated the Stage A work (migrations
`20261111000001_schedule_exclusivity`, `20261113000001_payment_hold_cleanup`,
`20261130000001_remediation_a_finance`) and were stale.

Verified on re-inspection, with exact evidence:

- **10-minute hold is server-side and immutable.** `hold_expires_at` is set at
  INSERT from `clock_timestamp() + 600s`
  (`services/backend/src/repository/appointment_repo.rs:31-43,195`,
  `BOOKING_HOLD_SECONDS = 600`) and protected by an immutable-update trigger
  (`20261113000001_payment_hold_cleanup.up.sql`). The Flutter UI only displays
  the server's `hold_expires_at`/`server_time`; it never computes its own
  deadline.
- **Auto-expiry and slot release run independent of the client.**
  `run_hold_expiry_sweep` ticks on the backend scheduler
  (`services/backend/src/services/notification_scheduler.rs:872-897`) and
  releases the slot to `AVAILABLE`. A lazy reclaim
  (`expire_abandoned_hold_for_slot`) additionally runs inside a new booking's
  own transaction so a sweep-cycle gap can never block a rebooker. Tested:
  `tests/cases/schedule_exclusivity.rs::test_expired_payment_hold_reclaimed_by_new_booking`,
  `::test_concurrent_hold_expiry_workers_partition_without_double_processing`.
- **Payment-vs-expiry races are transactional and tested.**
  `confirm_payment_if_pending` is an atomic compare-and-set under row lock;
  the full race is verified for exactly-one-winner semantics across 10
  concurrent trials:
  `tests/cases/schedule_exclusivity.rs::test_payment_confirmation_races_hold_expiration_exactly_one_outcome`.
- **A late provider success callback (after expiry) is handled, not
  ignored.** The transaction is flagged `reconciliation_required=true`, an
  audit event `LATE_PAYMENT_RECONCILIATION_REQUIRED` is recorded, and it
  surfaces in `/admin/finance/transactions` — verified end-to-end at the HTTP
  layer: `tests/cases/remediation_a.rs`,
  `tests/api_tests.rs::test_late_webhook_preserves_expired_booking_and_reports_authoritative_state`.
- **Provider-ambiguous transactions block monthly close rather than being
  silently resolved either way.** The `provider_request_state`
  (`NOT_STARTED`/`STARTED`/`UNKNOWN`) column deliberately leaves anything the
  provider might still be mid-flight on untouched on hold expiry, and
  `/admin/finance/periods/finalize` returns 409 until a human resolves it:
  `tests/cases/remediation_a.rs::a1_unknown_and_provider_started_attempts_keep_close_blocked`.

What genuinely remains open is an **active** worker that queries a live
payment provider's API to resolve an ambiguous transaction proactively. This
cannot be built usefully without a real payment-provider integration (the
gateway adapter itself is `DEFERRED_EXTERNAL_DEPENDENCY` above), so it is not
separate local engineering work — it is the same blocker as the rest of this
document's provider-dependent rows.
