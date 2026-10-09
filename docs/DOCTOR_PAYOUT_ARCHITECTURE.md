# Doctor payout architecture

The Doctor wallet is read-only. Earnings are calculated from the existing
transaction, settlement, wallet, and disbursement records; Finance controls
scheduled disbursement. The Doctor client never exposes a withdraw or cash-out
operation.

## Provider binding boundary

A payout destination is a provider-bound destination, not a trusted number
typed into Flutter. The intended flow is:

Doctor app â†’ HelloDoctor backend binding attempt â†’ official provider flow â†’
server-side provider confirmation â†’ masked provider reference â†’ eligible
payout method.

Provider PINs, passwords, OTPs, merchant credentials, PAN, and CVV are never
stored in Flutter, PostgreSQL, Redis, logs, or audit records. Doctor payout
methods and future Patient payment methods remain separate domains.

The current `doctor_payout_methods` migration predates this provider-binding
extension and accepts an encrypted identifier through
`PUT /doctor/wallet/payout-method`. That endpoint is retained only for
backward-compatible data inspection while the external provider integration is
deferred; it is not used by the Flutter UI and must not be treated as
`VERIFIED`, `ACTIVE`, or payout-ready. Replacing it with a provider binding
attempt and authoritative callback/status contract is classified as
`GENUINE_BACKEND_GAP_PROVIDER_PAYOUT_BINDING`.

## Monthly statements

The wallet now exposes lifetime summary, paginated ledger data, and a
server-calculated KES monthly statement. Finalized periods are immutable
snapshots containing gross, platform charge, adjustments, final payable, period
status, and payout metadata. Open periods are explicitly estimated. The
authenticated Doctor scope is enforced by the backend and currencies are never
merged. Flutter formats and caches these responses; it does not calculate
authoritative financial totals.

For local field testing only, builds with `DEV_PAYOUT_BINDING=true` expose a
clearly labelled `Connect M-Pesa (Dev Simulation)` action. It creates an
ephemeral masked UI state and a `DEV MODE: ... (simulated)` popup; it makes no
provider call, no backend payout record, and no production authorization claim.

## Kenya routing contract implemented

The backend now includes `doctor_payout_destinations`, restricted to KES, with
provider reference, masked destination, binding/verification state, payout
capability, and primary selection. New read APIs are:

- `GET /api/v1/doctor/wallet/payout-methods`
- `POST /api/v1/doctor/wallet/payout-methods/dev-bind` (development only)
- `GET /api/v1/doctor/wallet/statements?month=YYYY-MM`
- `GET /api/v1/admin/finance/payout-routing`

Disbursement items carry a destination link and safe destination snapshot. The
development binding generates the provider reference on the server; it does not
accept or store a raw wallet/card number. Real merchant/provider authorization
and payout execution remain `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`.

## 2026-10-06 - Finance state machine and Kenya-only routing correction

The internal finance workflow now distinguishes `PENDING_APPROVAL`, `APPROVED`, `PROCESSING`, `PAID`, `FAILED`, and reconciliation-required outcomes. Batch preparation allocates settled transactions to a disbursement item without changing their payment status, reducing the Doctor wallet, or marking a batch complete. Finance approval is a separate audited transition and explicitly reports `money_moved: false`. The external provider boundary remains `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`; no local UUID is presented as a provider transaction reference.

A persisted KES financial period and monthly statement snapshot provide controlled month close. Current/open months are estimated; only Finance finalization creates a final statement snapshot. Finance routing is month-filtered and returns Doctor-specific KES payable, adjustment, statement, disbursement, payout-method and masked-destination fields. Payout destinations are snapshotted on prepared items, so a later primary-method change cannot redirect a locked obligation. The allocation index was corrected so one item can aggregate many transactions. Kenyan bank transfer has a distinct gateway value; historical Bangladesh gateway values remain only for backward-compatible old records and are not active Kenya options.

Validation after this pass: `cargo fmt --all`, `cargo check --lib`, `cargo test --lib` (14/14), the disbursement regression subset (2/2), and `npm run build` for the Admin portal. The full serialized integration baseline previously passed 43/43 before the final additive gateway/query changes; the current targeted regression remains green. Flutter CLI is not installed in this environment, so a new APK/analyze run is not claimed here.

Final regression correction: after preserving the legacy BKASH setter solely for backward-compatible historical API tests (active Kenya onboarding still rejects it), the complete serialized integration suite is green again: 43/43. `cargo clippy --all-targets -- -D warnings` also passes.



Mobile verification after the wallet month-selector/status changes: Flutter analyze passed, Flutter tests passed (41 tests), and the ARM64 release build passed with `--dart-define=DEV_PAYOUT_BINDING=true`. Artifact: `apps/mobile/build/app/outputs/flutter-apk/app-arm64-v8a-release.apk` (18.0 MB). The development binding is explicitly simulated and cannot claim a real provider payout.

The legacy `PUT /doctor/wallet/payout-method` compatibility endpoint is development-only and disabled outside development; Kenya production onboarding must use provider-bound payout destinations. Historical BKASH records remain readable solely to avoid breaking old data/tests and are not offered by the active Kenya UI.

## 2026-10-07 — Four-agent audit corrections

Full findings and the verified current test counts are in
`docs/DOCTOR_SYSTEM_FINAL_MULTI_AGENT_AUDIT.md`. Fixes made to this subsystem
specifically:

- `doctor_wallets.pending_disbursement` was a dead counter — credited at
  settlement, never decremented by any reachable code path — and drove the
  Admin dashboard's "Unpaid Doctor Earnings" KPI, so every doctor who ever
  earned anything showed as permanently "Pending." Admin's unpaid figures
  (`/admin/overview`, `/admin/doctors`, the doctor dossier view) now derive
  from the real finalized-statement/disbursement-item aggregate
  (`wallet_repo::list_true_unpaid_totals`) instead.
- A refund approved against an already-wallet-settled transaction now reverses
  that wallet credit (`wallet_repo::debit`), so a doctor's lifetime earnings
  figure does not permanently overstate true earnings after a refund.
- An approved refund previously had no resolution path at all once approved —
  `financial_refund_requests.status` could never move past `APPROVED`. Finance
  can now list and transition refund requests
  (`GET/POST /admin/finance/refund-requests[/:id/transition]`).
- `GET /admin/finance/summary`'s `gateway_volume` no longer surfaces
  bkash/nagad buckets on this Kenya-only deployment; `total_gross` still
  includes any historical BDT rows (flagged, not solved — true multi-currency
  separation needs a currency-scoped total, not just a hidden bucket).
- `GET /admin/finance/summary` and `GET /admin/finance/transactions` now
  accept the same `?month=YYYY-MM` filter `payout-routing` already had.
