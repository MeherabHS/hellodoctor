# Doctor Business Logic & Financial Correctness Gap Audit (Agent C)

Scope: independent trace of the Doctor lifecycle (registration → verification →
payout setup → duty → booking → payment → consultation → settlement →
statement → disbursement → payout) against the ACTUAL code in
`services/backend` (migrations `20261001...` through `20261017...`) and
`apps/admin-web`, `apps/mobile`. `docs/DOCTOR_APP_FINAL_SYSTEM_AUDIT.md` and
`docs/DOCTOR_PAYOUT_ARCHITECTURE.md` were read for orientation only; every
claim below is re-derived from source, not accepted from those docs.

Environment note: `cargo test --test api_tests -- --test-threads=1` could not
be completed in this session — the shared `target/` directory is being
written concurrently by other cargo processes (other audit agents / a running
`hellodoctor-backend.exe`), and the linker repeatedly failed with
`Permission denied` deleting the previous `api_tests-*.exe` on both attempts.
This is a read-only audit and I did not stop other processes to force a
clean build. All findings below are therefore based on static
code/migration/trigger/constraint analysis, not a live test run or a live
two-doctor DB experiment. This is flagged explicitly wherever it limits
confidence (see GAP-C06/C07 verification notes).

---

## Summary of what is actually sound (context for the gaps below)

Before the gaps: the financial core built across the `2026100[6-9]`–`20261017`
migrations is materially more rigorous than "a doc's self-description" —
several invariants are enforced with real triggers/constraints, not just
application code:

- `transactions.gross_amount = platform_fee_amount + net_amount` and
  `monthly_statement_transactions.gross = platform_charge + doctor_earning`
  are DB-level `CHECK` constraints on `NUMERIC(10,2)`/`NUMERIC(12,2)` columns
  (no floats anywhere in the money path).
- `protect_finalized_statement` / `protect_finalized_period` triggers raise on
  any `UPDATE OR DELETE` once `status='FINALIZED'` — immutability is enforced
  at the Postgres level, not merely by omitting an edit button from an API.
- `protect_disbursement_snapshot` trigger freezes doctor/amounts/destination
  on a prepared `disbursement_items` row, and separately refuses to let any
  `UPDATE` reach `status IN ('PAID','CONFIRMED','DISBURSED')` unless
  `gateway_reference`/`paid_at` are set **and** `provider_code <> 'DEV_SIMULATED'`
  — confirmed exercised by `tests/api_tests.rs:1406`, which asserts a raw
  `UPDATE ... SET status='PAID', gateway_reference='fake' ...` is rejected by
  the DB itself.
- Doctor↔destination↔statement ownership is tied together by composite
  foreign keys (`disbursement_destination_owner`, `disbursement_statement_owner`
  on `(id/doctor_id/currency)`), not just an apparently-matching `doctor_id`
  column that a bug could silently decouple.
- `GET /api/v1/admin/finance/payout-routing` (`list_payout_routing` in
  `admin_handlers.rs`) genuinely binds the `month` query parameter into the
  date-range predicates (`$1`/`$2`) and the adjustment-year/month predicates
  (`$3`/`$4`) — changing the month changes what is returned; it is not an
  ignored parameter.
- Finance "approve" (`wallet_repo::approve_disbursement_item`) is a single
  conditional `UPDATE ... SET status='APPROVED', approved_by=$2, approved_at=NOW()
  WHERE id=$1 AND status='PENDING_APPROVAL'` — it does not touch amount,
  currency, or destination, and the audit log for batch preparation explicitly
  records `money_moved: false`.
- Grievance adjudication (`adjudicate_refund`) is serialized against
  settlement/close via the same `pg_advisory_xact_lock(61012, ...)` key used
  by period close and consultation completion, and is blocked once the source
  transaction is already in `monthly_statement_transactions` (closed).

These are real, load-bearing correctness properties. The gaps below are real
holes *within* that otherwise-careful design, not a wholesale rejection of it.

---

## GAP-C01 — Doctor wallet's "pending_disbursement" ledger is a dead
counter that never decreases, and drives the Admin Finance dashboard's
headline "unpaid" KPI

**DOMAIN:** Doctor earning settlement / Admin Finance dashboard

**CURRENT FLOW:** `doctor_wallets` (`lifetime_gross`, `lifetime_fee_withheld`,
`lifetime_net`, `pending_disbursement`) is credited exactly once, atomically,
by the SQL function `doctor_complete_consultation` (migration
`20261016000001_atomic_consultation_completion.up.sql`) at consultation
completion — this part is correct and exactly-once (verified: the only other
write site, `wallet_repo::insert_default`, only zero-inserts at onboarding).

The *decrement* side is dead code. `wallet_repo::mark_disbursed` (decrements
`pending_disbursement`), `transaction_repo::claim_for_disbursement` (flips a
transaction to `DISBURSED`), and `transaction_repo::list_settled_undisbursed_for_doctor`
are all defined but have **zero call sites anywhere in `services/backend/src`**
(confirmed by repo-wide grep). They are orphaned remnants of an earlier,
pre-`financial_state_machine` architecture. The *current*
`DisbursementService::initiate_monthly_disbursement` instead uses
`transaction_repo::assign_to_disbursement`, which only sets
`transactions.disbursement_item_id` and leaves `payment_status` at
`SETTLED_TO_DOCTOR` — by design, because "preparation never moves money." But
because the old `mark_disbursed` path is simply unreachable, **nothing in the
entire codebase ever reduces `doctor_wallets.pending_disbursement`**, under
any circumstance: not on batch preparation (correct — that's intentional),
not on Finance approval (correct — that's intentional), and not even on a
hypothetical future real provider payout reaching `PAID` (incorrect — there is
no code that would fire at that point either, since the only decrement
function that exists is never called).

Meanwhile a *separate*, correctly-derived set of numbers exists side by side:
`DisbursementService::get_doctor_wallet_summary` also computes
`finalized_unpaid`, `scheduled_unpaid`, and `total_paid` straight from
`monthly_payout_statements`/`disbursement_items` (these are correct and do
distinguish paid from unpaid). Both the broken `pending_disbursement` and the
correct `scheduled_unpaid`/`total_paid` are returned in the same
`DoctorWalletSummaryDto` with no reconciliation between them.

**Admin impact (confirmed, not hypothetical):** `GET /api/v1/admin/overview`
(`get_admin_overview`) computes its top-level `pending_disbursement` KPI as a
raw `SUM(doctor_wallets.pending_disbursement)` over every doctor
(`admin_handlers.rs` lines ~288-316), and `GET /api/v1/admin/doctors`
(`list_admin_doctors`) returns the same per-doctor stale field
(lines ~374-378). `apps/admin-web/src/lib/useBackendSync.ts:66` maps this
directly to `pendingDisbursement`, and `apps/admin-web/src/app/page.tsx:379,
384, 841` use it to compute `isSettled = (doc.pendingDisbursement ?? 0) === 0`,
the `doctorsSettledCount`/`doctorsAwaitingCount` filter pills ("All / Pending
/ Settled"), and the headline "Unpaid Doctor Earnings" KPI card text
(`'No pending earning balance'` vs `'N doctors awaiting disbursement'`,
`page.tsx:763-787`).

**WHY IT'S INCOMPLETE/WRONG:** Because this field only ever grows, **every
doctor who has ever completed one paid consultation will show as
"Pending"/"awaiting disbursement" in the Admin Finance dashboard forever**,
even after Finance has correctly finalized their statement, prepared,
approved, and (eventually, once the deferred provider integration exists)
paid every cent they are owed through the correct `disbursement_items` path.
The dashboard's single most important Finance KPI ("Unpaid Doctor Earnings")
is therefore not "unpaid" — it is "lifetime gross ever earned, minus nothing,
ever."

**USER/BUSINESS IMPACT:** Finance staff cannot use the dashboard's primary
settlement indicator to answer "who do we still owe money to" — it says
"everyone, forever." This is exactly the kind of UI-states-that-cannot-be-
proven-from-backend-state problem the brief asks to hunt for: the number is
real (it comes from a real column) but it does not mean what its label says.
At best this is a serious trust/usability defect; at worst, if an operator
ever treats "Settled" as meaning "nothing to review" and only chases doctors
tagged "Pending," there is no doctor the dashboard will ever let them
consider closed.

**REQUIRED RULE:** `doctor_wallets.pending_disbursement` must either (a) be
actively maintained — decremented exactly once per transaction the moment
that transaction is durably allocated to a disbursement item (mirroring
`assign_to_disbursement`'s exactly-once guard) and further adjusted on
provider-confirmed `PAID`/on refund reversal — or (b) be removed from any
"is this doctor settled" decision entirely and replaced everywhere (Admin
overview KPI, Admin doctor list, Admin per-doctor settled/pending filter) with
the already-correct `scheduled_unpaid`/`finalized_unpaid`/`total_paid`
statement-derived figures that `DisbursementService` already computes.

**BACKEND IMPACT:** Remove or repurpose `wallet_repo::mark_disbursed`,
`transaction_repo::claim_for_disbursement`,
`transaction_repo::list_settled_undisbursed_for_doctor` (currently dead);
rewrite `get_admin_overview` and `list_admin_doctors` to source "unpaid" from
the statement/disbursement_item aggregate instead of `doctor_wallets`.

**FLUTTER IMPACT:** None required — `doctor_pages.dart`'s
`EarningsBreakdownCard` already does not render `wallet.pending` and already
labels `wallet.net` "Lifetime earnings before monthly adjustments; not proof
of payout," which is honest. Only the Admin side is actually misled.

**ADMIN IMPACT:** `page.tsx` `isSettled`, `doctorsSettledCount`,
`doctorsAwaitingCount`, and the "Unpaid Doctor Earnings" KPI card must be
re-derived from statement/disbursement data, not `pending_disbursement`.

**TEST REQUIRED:** Finalize a period for a doctor, prepare a disbursement
batch, approve the item (and, once provider integration exists, mark it
paid); assert the Admin `/admin/overview` and `/admin/doctors` "unpaid"
figures for that doctor actually reach zero. Today this test would fail by
construction — there is no code path that could make it pass.

**SEVERITY:** HIGH (confirmed by static analysis of all call sites; not yet
confirmed against a live running query due to the build-lock issue noted
above — recommend re-running once exclusive build access is available).

---

## GAP-C02 — Refund approval never reverses the doctor's wallet ledger

**DOMAIN:** Grievance refund / doctor earning reversal

**CURRENT FLOW:** `GrievanceService::adjudicate_refund`
(`services/grievance_service.rs`) correctly: locks the period, blocks refund
if the transaction is already inside a closed `monthly_statement_transactions`
set, inserts an immutable `financial_refund_requests` row, and transitions
`transactions.payment_status` to `RefundPending` via
`transaction_repo::mark_status`. This correctly removes the transaction from
future `gross_eligible`/`monthly_statement_transactions` aggregation (both
only select `payment_status IN ('SETTLED_TO_DOCTOR','DISBURSED')`), so the
**authoritative statement path is correct**.

However, the function never calls anything in `wallet_repo` to reverse the
`lifetime_gross`/`lifetime_fee_withheld`/`lifetime_net`/`pending_disbursement`
amounts that `doctor_complete_consultation` already credited into
`doctor_wallets` for that same transaction at completion time. There is no
negative-adjustment write of any kind against `doctor_wallets`.

**WHY IT'S INCOMPLETE/WRONG:** A consultation that is completed, settled
into the doctor's wallet, and **then** successfully grieved and refund-
approved still counts permanently toward that doctor's `lifetime_gross` /
`lifetime_net` as shown by `GET /doctor/wallet` and rendered in the Flutter
"Total Earnings (Gross)" / "Final Earning (Net Take-Home)" rows. Combined
with GAP-C01 (the same column is never decremented for *any* reason), this
guarantees permanent overstatement with no corrective ledger row anywhere —
`financial_adjustments` (the real, auditable adjustment table) is only ever
written by Finance for manual corrections, never automatically on refund.

**USER/BUSINESS IMPACT:** A doctor's lifetime earnings dashboard
permanently overstates true lifetime earnings by the sum of any
after-settlement refunds against them. Low immediate monetary risk (the
authoritative statement/payout math is unaffected), but it is a genuine,
unbounded, uncorrected accounting discrepancy exposed to the doctor, and
there is no `financial_adjustments` row a doctor or auditor could point to
that explains the gap between "lifetime net" and "sum of monthly statements."

**REQUIRED RULE:** On refund approval for a transaction already in
`SettledToDoctor`, insert a reversing `financial_adjustments` row (negative,
category e.g. `REFUND_REVERSAL`) in the same transaction, and/or decrement
`doctor_wallets` lifetime figures, so the two views can be reconciled.

**BACKEND IMPACT:** Add the reversal write to `adjudicate_refund`.

**FLUTTER IMPACT:** None beyond already-correct rendering, once reversed.

**ADMIN IMPACT:** None beyond already-correct per-month routing.

**TEST REQUIRED:** Complete a consultation, confirm wallet credited; approve
a grievance refund on it; assert wallet lifetime figures and/or a
`financial_adjustments` row reflect the reversal.

**SEVERITY:** MEDIUM-HIGH.

---

## GAP-C03 — Approved refunds have no resolution path: a permanent dead
state with zero recovery, retry, or reconciliation action

**DOMAIN:** Refund execution / unknown-provider-result handling

**CURRENT FLOW:** `financial_refund_requests.status` is constrained to
`('APPROVED','PROCESSING','REFUNDED','FAILED','RECONCILIATION_REQUIRED')`,
but a repo-wide search shows the table is only ever **inserted**
(`grievance_service.rs:150`) — there is no `UPDATE financial_refund_requests`
anywhere in `services/backend/src`. The migration's own comment says this
plainly: *"Provider result processing is deliberately not exposed to an Admin
action."*

**WHY IT'S INCOMPLETE/WRONG:** This is consistent with the deferred external-
payment-integration boundary for *disbursements* (which the audit brief
accepts as a legitimate, explicitly-labeled gap). But for *refunds* it goes
further than "payment execution is deferred" — there is no manual fallback
either. Once a refund is approved, `transactions.payment_status` sits at
`REFUND_PENDING` and `financial_refund_requests.status` sits at `APPROVED`
**forever**, with no admin action, no retry, no manual "mark resolved," and
no distinct "stuck — needs reconciliation" surface. This is exactly the
"unknown provider result collapsed into neither retry nor failed" case the
brief asks to hunt for, except here it isn't even collapsed into one of
those — it simply has no transition at all.

**USER/BUSINESS IMPACT:** A patient who is approved for a refund has no
traceable, closeable record of ever actually receiving money back through
this system. Finance has no in-app way to know which approved refunds are
still owed, overdue, or need escalation — they would have to query the table
directly. This is worse than "the external payment integration is deferred";
it means the loop is open on both the pay-out *and* the refund side with no
operator-facing recovery surface on either one.

**REQUIRED RULE:** At minimum, give Finance a manual action to transition
`financial_refund_requests.status` to `PROCESSING` / `REFUNDED` (with a
`provider_reference`/`refunded_at`, matching the table's own CHECK
constraint) or `RECONCILIATION_REQUIRED`/`FAILED`, mirroring the disbursement
item's `PENDING_APPROVAL → APPROVED → (deferred)` pattern, so refunds have
the same visible, auditable lifecycle disbursements do.

**BACKEND IMPACT:** New endpoint(s) + repo function to transition
`financial_refund_requests`.

**ADMIN IMPACT:** A refund queue/list view currently has nothing to surface
beyond "approved" — `apps/admin-web` has no UI for this table at all today
(grep found none), so refunds are invisible to Finance after approval.

**TEST REQUIRED:** Approve a refund; assert it is listable and
transitionable by Finance; assert it cannot be silently re-approved or
double-processed.

**SEVERITY:** HIGH (patient-money-facing dead end with zero operator
visibility, not just a deferred-but-tracked external integration).

---

## GAP-C04 — No forcing function between clinical readiness and financial
readiness; earnings can accrue unboundedly against a doctor with zero payout
destination, with only a passive label and no escalation

**DOMAIN:** Registration completeness / clinical vs. financial readiness

**CURRENT FLOW (confirmed separate, as intended):** `is_verified` gates
going on duty and creating slots (`ctx.verified_doctor_profile_id` in
`update_own_duty`/`create_own_slot`/`list_own_appointments`,
`doctor_handlers.rs`), and gates settlement itself (`doctor_complete_consultation`
checks `d.is_verified`). Payout setup is a completely independent table
(`doctor_payout_destinations`) with no FK or trigger requiring its existence
before a doctor can go on duty, create slots, or be booked/paid. This
confirms clinical and financial readiness genuinely are separate concepts in
the schema, not just in a doc's prose — good.

But the only place payout readiness is actually checked is at disbursement
preparation time (`initiate_monthly_disbursement`): if
`wallet_repo::get_primary_payout_destination` returns `None`, the loop does
`continue` — i.e., it silently skips that doctor's otherwise-finalized,
otherwise-payable statement, leaving it allocated to nothing, forever, every
month, with no counter, alert, or escalation beyond the Admin routing table's
per-row `"PAYOUT_SETUP_REQUIRED"` label (`admin_handlers.rs` line 75).

**WHY IT'S INCOMPLETE/WRONG:** There is no cap, reminder cadence, cumulative-
liability alert, or any forcing function at all. A doctor could see patients
and earn money indefinitely — across many months — with zero payout
destination ever configured, and the system's only observable symptom is a
per-doctor badge an operator has to notice on their own, with no "N months,
KES X total overdue, no destination" rollup surfacing this as the structural
problem it is.

**USER/BUSINESS IMPACT:** This is a legitimate, growing, un-owed-but-also-
unpayable-forever liability with no operational safety valve. It's defensible
as intentional (matching CLAUDE.md's "no unilateral feature invention" /
deferred-integration posture), but it is a genuine open gap, not a settled
design: nothing in the roadmap/progress docs commits to ever forcing or even
flagging a cumulative-months-unpaid threshold.

**REQUIRED RULE:** Either (a) require at least one `payout_capable` ACTIVE
destination before a doctor can be marked `is_on_duty=true` (a real gate,
not UI ordering), or (b) keep it fully decoupled as now but add an explicit
cumulative-liability surface (e.g., months-unpaid count, KES total stuck) to
the Admin payout-routing view so this is an operational, not merely visual,
signal.

**BACKEND/ADMIN IMPACT:** New aggregate query / column in
`list_payout_routing`.

**TEST REQUIRED:** A doctor with no destination completes consultations
across 3 statement periods; assert the system surfaces this as a single
rollup ("3 unpaid periods, KES X") rather than three independent per-month
rows an operator must notice individually.

**SEVERITY:** MEDIUM (design gap / open question more than a defect — flagged
as requested by the brief's "Registration Completeness" question rather than
as a confirmed bug).

---

## GAP-C05 — Legacy Bangladesh gateway values remain live-reachable on the
(dev-only) patient payment-confirmation path for a Kenya-only product

**DOMAIN:** Multi-currency / gateway hygiene

**CURRENT FLOW:** `webhook_handlers.rs::parse_gateway` still maps
`"bkash" → Gateway::Bkash` and `"nagad" → Gateway::Nagad` for the inbound
patient-payment webhook (`POST /webhook/payment/:provider`), unconditionally
(not behind any Kenya/country check) — the only gate on this whole handler is
`state.config.environment != "development"` (which disables it entirely in
production, for *any* provider, since "real payment-provider verification is
not configured"). In development, however, a webhook call to
`/webhook/payment/bkash` would successfully parse and proceed into
`AppointmentService::process_payment_webhook`.

Also, `doctor_profiles.license_country` retains `DEFAULT 'BD'` at the column
level (`20261001000001_initial_schema.up.sql` line 97) even though every
actual Rust INSERT path (`seed.rs`, `auth_handlers.rs:234`,
`admin_handlers.rs:451`) explicitly binds `"KE"`, and the
`20261013000001_atomic_doctor_onboarding` trigger only enforces `license_country
= 'KE'` for the self-registration path (`current_role = 'DOCTOR'`), not for
any admin-driven insert.

**WHY IT'S INCOMPLETE/WRONG:** Neither is reachable in production today
(confirmed: production always 403s the webhook regardless of provider, and
every current INSERT path explicitly sets `'KE'`), so this is **not an active
defect**, but both are live landmines: a future admin-only doctor-creation
path that forgets to bind `license_country`, or any direct/manual SQL insert,
would silently default to Bangladesh on a Kenya platform with no trigger to
catch it (the onboarding guard trigger only fires for the `DOCTOR` self-
registration role); and the webhook gateway parser has no allowlist tied to
the deployment's actual supported country/gateways.

**REQUIRED RULE:** Change the column default to `'KE'` (or drop the default
entirely and require explicit binding), and restrict `parse_gateway` to
`MPESA`/`CARD`/`BANK_TRANSFER` for the active deployment, keeping
`BKASH`/`NAGAD` enum values readable for historical data only (as already
done elsewhere in this codebase, e.g. the payout-method backward-compat
comments).

**SEVERITY:** LOW (defense-in-depth; not currently triggerable by any code
path found).

---

## Answers to specific mission questions not already covered by a GAP above

- **Item 7 (platform charge exactness/snapshotting):** CONFIRMED correct.
  `NUMERIC(10,2)`/`NUMERIC(12,2)` throughout, never float; `chk_fee_math`
  and `monthly_statement_transactions`'s own `CHECK (gross_amount =
  platform_charge + doctor_earning)` enforce the 20/80 split arithmetically
  at the DB level on both the live transaction and the frozen statement row,
  so a later global rate change cannot retroactively alter history (there is
  no "rate" column read at render time — figures are stored, not recomputed).
- **Item 9 (period close):** CONFIRMED cannot run twice (`finalize_financial_period`
  explicitly checks `existing_status == "FINALIZED"` → `Conflict`, backed by
  the `protect_finalized_period` trigger as a second, DB-level line of
  defense). CONFIRMED it blocks close while `INITIATED`/`PAYMENT_HELD`
  transactions exist in-period; it does **not** block close while a
  `REFUND_PENDING` transaction from this period exists, but this is
  consistent/correct since such a transaction is already excluded from
  `gross_eligible` either way.
- **Item 11 (disbursement preparation = allocation only):** CONFIRMED —
  `assign_to_disbursement` does not change `payment_status`, item status
  starts at `PENDING_APPROVAL`, and the audit log explicitly records
  `money_moved: false`. (Not independently re-verified against live data this
  session — see environment note at top.)
- **Item 12 (Finance approval):** CONFIRMED to only set
  `status/approved_by/approved_at`; cannot alter amount/currency/destination
  (those columns are frozen by `protect_disbursement_snapshot` the moment the
  row exists, regardless of who calls `UPDATE`).
- **Item 13/14 (destination routing / snapshot immutability):** Enforced by
  composite FKs and the immutability trigger described above (static
  verification only — recommend a live two-doctor run once the shared
  `target/` lock contention clears).
- **Item 15 (max internal state / DEV_SIMULATED boundary):** CONFIRMED —
  `disbursement_paid_evidence` CHECK plus `protect_disbursement_snapshot`
  trigger make it impossible for any `UPDATE`, including from a future buggy
  service, to mark an item `PAID`/`CONFIRMED`/`DISBURSED` while
  `provider_code='DEV_SIMULATED'` or while `gateway_reference`/`paid_at` are
  null; `tests/api_tests.rs:1406` already exercises this.
- **Item 19 (Finance month routing):** CONFIRMED the `month` query parameter
  is genuinely bound into the routing query's date range and adjustment
  period predicates.
- **Item 20 (Admin UI semantics):** The dashboard's Settled/Pending
  classification is the one place a label ("Settled") does **not** match
  backend reality — see GAP-C01. Other Admin labels inspected
  ("Disbursement Approved. No provider payment has been confirmed.",
  "Finalized... does not approve or pay any payout.") are honest and match
  backend state.
- **Item 21 (Doctor wallet semantics in Flutter):** The doctor *can*
  distinguish Finalized unpaid / Assigned for disbursement — unpaid /
  Provider-confirmed paid (`doctor_pages.dart:1609-1625`), and these three
  map directly to the correct statement/disbursement-item aggregate, not the
  broken wallet counter. Good.
- **Item 22 (existing-doctor migration):** No backfill script and no
  "payout setup required" flag column exist on `doctor_profiles`; this is
  genuinely unspecified/implicit rather than a deliberate, documented
  decision — stated here as an open gap per the brief's instruction to say so
  plainly rather than assume intent.
