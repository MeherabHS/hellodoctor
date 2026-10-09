# Doctor Mobile Profile — Notifications Verification

Status snapshot of the Doctor Engagement notification system as of 2026-10-09: what exists, what was audited, what was fixed, and what was newly wired. This supersedes any assumption that the notification system was undocumented or that `PAYOUT_PAID` had a live caller — neither was true before this pass.

## 1. Implementation inventory

- Schema: `migrations/20261020000001_doctor_engagement_notifications` (`notifications`, `notification_device_tokens`, RLS) plus `migrations/20261029000001_notification_finance_worker_insert` (adds `hellodoctor_finance_worker` to `notifications_backend_insert`, added by this pass — see §3). Documented in `docs/DATABASE_SCHEMA.md` ("Notifications — Doctor Engagement extension").
- Repository: `src/repository/notification_repo.rs` — `insert_if_new` (savepoint + catch `23505` unique-violation on `dedup_key`), `list_for_user`, `count_unread`, `mark_read`, `mark_all_read`, `exists_with_dedup_key`, `device_tokens::upsert`.
- Service: `src/services/notification_service.rs` — `nairobi_offset()`/`nairobi_local_date()` (fixed `UTC+3`, no DST in Kenya) and one creation function per notification kind.
- Scheduler: `src/services/notification_scheduler.rs` — a single 60s `tokio::spawn` loop (`start(state)`) running two independent, idempotent passes: `run_daily_summary_pass` and `run_t15_reminder_pass`. Deliberately a DB-backed worker, not a new broker — this project already has one Redis instance for rate-limiting/sessions, not a job queue.
- Transport: `src/services/notification_transport.rs` — `NotificationTransport` trait, exactly one implementation, `NoopTransport`, which always returns `PushOutcome::NoProviderConfigured`.
- API: `GET /api/v1/notifications`, `GET /api/v1/notifications/unread-count`, `POST /api/v1/notifications/read-all`, `POST /api/v1/notifications/:id/read`, `POST /api/v1/notifications/devices` (`src/api/v1/notification_handlers.rs`, routed in `router.rs`).

## 2. Event inventory

| Event | Backend trigger | Dedup key | Push | In-app | Current status |
|---|---|---|---|---|---|
| `DAILY_CONSULTATION_SUMMARY` | `notification_scheduler::run_daily_summary_pass`, once per verified/active Doctor per Kenya-local calendar day, at/after `DOCTOR_DAILY_SUMMARY_TIME_LOCAL` (default `07:00`) | `daily-consultation-summary:{doctor_profile_id}:{local_date}` | `PUSH_PROVIDER_CONFIGURATION_REQUIRED` | Live | Working as audited; see §4 for the zero-appointment behavior flag |
| `CONSULTATION_REMINDER_15M` | `notification_scheduler::run_t15_reminder_pass`, for a `CONFIRMED`/`WAITING` appointment whose `[start_time - 15m, start_time)` window has opened | `consultation-reminder-15m:{appointment_id}:{start_time_rfc3339}` | `PUSH_PROVIDER_CONFIGURATION_REQUIRED` | Live | Working as audited |
| `EARNING_CREDITED` | `consultation_service::doctor_complete_consultation`, same transaction as the authoritative wallet credit | `earning-credited:{appointment_id}` | `PUSH_PROVIDER_CONFIGURATION_REQUIRED` | Live | Pre-existing, re-confirmed by this pass's test run (not re-audited in depth; `test_duplicate_consultation_completion_creates_one_earning_notification` already covers it) |
| `PAYOUT_PAID` | `DisbursementService::confirm_disbursement_item_paid`, same transaction as a disbursement item's genuine, provider-confirmed transition to `PAID` | `payout-paid:{disbursement_item_id}` | `PUSH_PROVIDER_CONFIGURATION_REQUIRED` | Live | **Newly wired by this pass** — see §3; previously defined but with zero callers anywhere |
| `REGISTRATION_VERIFIED` | Admin approves a Doctor's professional registration | `registration-verified:{doctor_profile_id}` | `PUSH_PROVIDER_CONFIGURATION_REQUIRED` | Live | Pre-existing, re-confirmed (`test_registration_verification_notifications_are_exactly_once`) |
| `REGISTRATION_REJECTED` | Admin rejects a Doctor's professional registration | `registration-rejected:{doctor_profile_id}` | `PUSH_PROVIDER_CONFIGURATION_REQUIRED` | Live | Pre-existing, re-confirmed (same test as above) |
| `ARTICLE_STATUS_CHANGED` | A Doctor Article is published/archived | `article:{article_id}:{version}` | `PUSH_PROVIDER_CONFIGURATION_REQUIRED` | Live | Pre-existing, not in scope of this pass |

**Push delivery is `PUSH_PROVIDER_CONFIGURATION_REQUIRED` for every event, with no exception.** `NotificationTransport` has exactly one implementation in this codebase, `NoopTransport`, which always returns `PushOutcome::NoProviderConfigured`. No FCM/APNs credential, SDK call, or provider integration exists anywhere. Nothing in this system should ever be described as "push notifications are working" — only the in-app inbox is live.

## 3. `PAYOUT_PAID` wiring (this pass)

**Finding:** `NotificationService::notify_payout_paid` (`src/services/notification_service.rs`) was fully implemented — correct signature, correct dedup key, correct doc comments explicitly warning "never call this from a `DEV_SIMULATED` path" — but had **zero callers anywhere in the codebase** (confirmed: no "notify" string hits in `disbursement_service.rs` before this pass). This matched the task's starting assumption.

**Deeper finding, not assumed by the original task brief:** there was also no application code path anywhere that ever transitioned a `disbursement_items` row to `status='PAID'` at all. The only existing transition was `wallet_repo::approve_disbursement_item` (`PENDING_APPROVAL` → `APPROVED`, via `POST /admin/disbursements/:item_id/approve`). `PROCESSING` → `PAID` was a state the schema anticipated (the `disbursement_paid_evidence` CHECK constraint and `protect_disbursement_snapshot` trigger in `migrations/20261012000001`/`20261017000001` already enforce `gateway_reference`/`paid_at`/non-`DEV_SIMULATED` invariants for any `PAID` row) but that no Rust code ever reached — payout execution itself remains `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION` (no live Mpesa/bank-transfer provider call exists anywhere in this codebase). Wiring the notification therefore required adding the one missing write path, not just a function call.

**What was added:**
- `wallet_repo::confirm_disbursement_item_paid` (`src/repository/wallet_repo.rs`) — a single conditional `UPDATE disbursement_items SET status='PAID', gateway_reference=$2, paid_at=NOW() WHERE id=$1 AND status IN ('APPROVED','PROCESSING') AND provider_code IS NOT NULL AND provider_code <> 'DEV_SIMULATED' RETURNING ...`. Returns `None` (no transition, no caller-visible row) for anything not currently `APPROVED`/`PROCESSING`, and for any `DEV_SIMULATED` destination — matching `notify_payout_paid`'s own contract and the DB's existing CHECK/trigger.
- `DisbursementService::confirm_disbursement_item_paid` (`src/services/disbursement_service.rs`) — runs under `hellodoctor_finance_worker` (the only role with `UPDATE` on `disbursement_items`), performs the transition, looks up the doctor's `user_id`, calls `NotificationService::notify_payout_paid` **in the same transaction**, writes a `DISBURSEMENT_PAID_CONFIRMED` audit log entry, then commits — mirroring exactly how `EARNING_CREDITED` fires from the same transaction as its wallet credit in `consultation_service.rs`. A transition that affects zero rows returns `AppError::Conflict` before the notification call is ever reached, so a retry against an already-`PAID`/still-`PENDING_APPROVAL`/`DEV_SIMULATED` item can never duplicate or fabricate a notification.
- `POST /api/v1/admin/disbursements/:item_id/confirm-paid` (admin handler `confirm_disbursement_item_paid` in `admin_handlers.rs`, routed in `router.rs`), accepting `{"gateway_reference": "..."}` — Finance's admin-operated confirmation that a payout was genuinely paid by the provider. The caller supplies `gateway_reference` out-of-band (the real provider confirmation, e.g. an Mpesa/bank statement reference); it is never server-fabricated. Same `Action::InitiateDisbursement` ABAC gate as the existing approve endpoint.
- Migration `20261029000001_notification_finance_worker_insert` (up/down) — `hellodoctor_finance_worker` had no `INSERT` grant on `notifications` at all; the first real test run against this wiring failed with `permission denied for table notifications` before this migration existed, confirming the gap empirically rather than by inspection alone. The migration extends `notifications_backend_insert` to include `hellodoctor_finance_worker` and grants it `INSERT` (not `SELECT`/`UPDATE` — nothing in this path needs to read or update another row).

This migration was applied to the shared local dev database (`sqlx migrate run`) during this pass. **Per CLAUDE.md's migration discipline, a long-running `hellodoctor-backend.exe` process was observed already listening on port 8080 (PID 7092) before this migration existed — it must be restarted before anyone trusts live/manual HTTP verification of `confirm-paid` against the shared dev database.** It was deliberately left running (not restarted by this pass) since other work may be in progress against it concurrently; this is a flag for whoever does that live verification next, not a completed step.

## 4. Audit findings (scheduler)

`notification_scheduler.rs` was read in full before any test was written, per instruction not to assume its behavior. All of the following were verified by running real `#[sqlx::test]`s that call `run_daily_summary_pass`/`run_t15_reminder_pass` directly against seeded data — no bug was found in the scheduler itself; everything below is "confirmed correct as audited," except one item flagged as a product-intent question, not a bug.

- **Zero-appointment day still produces a notification.** The `count == 0` branch builds the body `"You have no consultations scheduled today."` and still calls `insert_if_new` — it does **not** skip the notification. This is the scheduler's actual, current behavior (confirmed by `test_daily_summary_zero_appointments_current_behavior_is_documented`), not a bug introduced or found by this pass. **Flag for product review:** every verified, active Doctor gets paged daily even with a completely empty schedule. If the intended product behavior is "no notification on a zero-consultation day," this is a one-line change (wrap the existing `insert_if_new` call in `if count > 0`) — but that was not requested, so it was left as-is and only documented here.
- **`CANCELLED` appointments are correctly excluded** from the daily count (`a.status NOT IN ('CANCELLED', 'EXPIRED')`) — confirmed directly (not just via the pre-existing T-15 cancellation test) by `test_daily_summary_excludes_cancelled_appointments_from_count`, which seeds 2 active + 1 cancelled appointment and asserts `count == 2`.
- **T-15 window boundaries are correct**: an appointment exactly 14 minutes out is notified; exactly 16 minutes out is not (yet); an appointment whose `start_time` has already passed is never notified even while still `CONFIRMED` (the window is `(now, now+15m]`, strict on the near side) — confirmed by `test_t15_reminder_exact_boundary_times`.
- **Dedup holds under a repeated tick** for both passes (pre-existing tests `test_daily_summary_scheduler_is_exactly_once_per_doctor_per_day` and `test_t15_reminder_scheduler_fires_once_and_cancellation_suppresses_it`, re-run and reconfirmed, not just read).

## 5. Exact test results

All run via `cargo test --offline --test api_tests -- <names> --test-threads=1` against the local Postgres dev container (`hellodoctor-postgres`), 2026-10-09. (`--test-threads=1` was used to avoid an unrelated `sqlx::test` template-database race — "tuple concurrently updated" — that occurs when many `#[sqlx::test]` functions provision their ephemeral databases fully in parallel; it is a test-harness concurrency artifact, not a product bug, and does not affect correctness of any individual test.)

| Test | Result |
|---|---|
| `test_daily_summary_scheduler_is_exactly_once_per_doctor_per_day` (pre-existing) | PASS |
| `test_daily_summary_zero_appointments_current_behavior_is_documented` (new) | PASS |
| `test_daily_summary_excludes_cancelled_appointments_from_count` (new) | PASS |
| `test_t15_reminder_scheduler_fires_once_and_cancellation_suppresses_it` (pre-existing) | PASS |
| `test_t15_reminder_fires_immediately_for_near_term_booking` (pre-existing) | PASS |
| `test_t15_reminder_exact_boundary_times` (new) | PASS |
| `test_payout_paid_notification_fires_once_for_genuine_confirmation` (new) | PASS |
| `test_payout_paid_notification_never_fires_for_unapproved_or_dev_simulated_items` (new) | PASS |
| `test_monthly_finance_disbursement_batch` (pre-existing, disbursement regression check) | PASS |
| `test_disbursement_claims_each_transaction_at_most_once_under_concurrency` (pre-existing, disbursement regression check) | PASS |
| `test_doctor_cannot_list_or_mark_read_another_doctors_notifications` (pre-existing, notification regression check) | PASS |
| `test_registration_verification_notifications_are_exactly_once` (pre-existing, notification regression check) | PASS |
| `test_duplicate_consultation_completion_creates_one_earning_notification` (pre-existing, notification regression check) | PASS |

**13 run, 13 passed, 0 failed** for every test touched or added by this pass.

Additionally:
- `cargo test --offline --lib`: **53 passed, 0 failed.**
- `cargo fmt`: applied cleanly (reformatted `disbursement_service.rs` and `tests/api_tests.rs`).
- `cargo clippy --offline --all-targets -- -D warnings`: **zero warnings.**

**Not run in this pass** (per the task's own instruction, to avoid colliding with other parallel work): the full serialized `cargo test` suite (~17 minutes). It should be run once, in full, before this work is considered fully done — this pass only confirms the tests it touched or added, plus full `--lib` and full `clippy`.

## 6. Known residual gaps (not fixed by this pass — explicitly out of scope)

- No real push provider is configured or implemented (`NoopTransport` only). Any future FCM/APNs integration must keep push payloads PHI-free, per CLAUDE.md §2.7.
- `PROCESSING` remains a schema-valid `disbursement_items.status` value with no code path that ever sets it; only `PENDING_APPROVAL → APPROVED → PAID` is reachable today. `confirm_disbursement_item_paid` accepts `PROCESSING` as a valid source state for forward-compatibility with a future provider-async flow, but nothing currently produces a `PROCESSING` row.
- The zero-appointment daily-summary notification behavior (§4) is a flagged product question, not changed by this pass.
