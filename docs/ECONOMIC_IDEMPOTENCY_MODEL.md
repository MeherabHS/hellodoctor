# Economic idempotency model

## Inspection map (recorded before source edits)

The current collection integration is a development simulator. Production booking
and payment callbacks fail closed. `POST /appointments/:id/payment` (handler
`initiate_payment`) validates ownership/lifecycle, finds an existing transaction,
and returns provider-unavailable; it does not create a provider checkout. There
is no live provider collection, payout execution, or refund driver to verify.

| Operation | Business identity | Current authoritative gate | Inspection gap |
| --- | --- | --- | --- |
| Booking | patient profile + schedule slot | conditional slot lock and appointment/intake/transaction in PaymentWorker transaction | different request keys return conflict; lost-response recovery must find the same patient's existing booking |
| Payment resume | appointment ID | read existing payment session; fail closed without provider | DB allows multiple nonfailed transaction rows for an appointment |
| Provider payment | gateway + payment session; gateway + reference | unique payment-session ID and partial unique gateway/reference | webhook reads transaction without lock and unconditionally rewrites PAYMENT_HELD, including after settlement/refund |
| Callback evidence | gateway + provider event ID | INSERT ON CONFLICT, unique index, in payment transaction | same event ID reused against another payment silently appears to be a replay |
| Payment confirmation | appointment/transaction ID | appointment conditional PENDING_PAYMENT transition | distinct simultaneous success events can both emit PAYMENT_CONFIRMED using stale state |
| Consultation activation | appointment ID | unique session/conversation appointment IDs; conversation advisory lock | activation also runs for an expired payment hold |
| Consultation settlement | appointment/transaction ID | doctor_complete_consultation SECURITY DEFINER function, accounting lock, appointment/payment row locks; wallet credit in same transaction | notification/trace/audit use unlocked pre-call state; no independent immutable economic-effect row |
| Monthly close | currency/year/month | period advisory lock, immutable finalized period/statements; unique Doctor/period and statement transaction membership | no new race found in inspected close path |
| Disbursement preparation | monthly statement ID | ordered statement row locks; unique noncancelled statement/item index; conditional transaction allocation | repeat creates empty batches after sources already allocated |
| Admin payout approval | item ID | conditional PENDING_APPROVAL UPDATE; winning approved_by; batch row lock; same-transaction audit | no new race found in inspected approval path |
| Payout paid confirmation | item ID; provider/reference | conditional APPROVED/PROCESSING UPDATE; evidence CHECK; same-transaction notification/audit | provider payout reference not unique across items |
| Refund approval | payment transaction ID | payment row lock + accounting lock; conditional grievance adjudication; unique transaction refund request; wallet reversal same transaction | immutable economic-effect evidence absent; no provider refund execution exists |
| Refund workflow | refund request ID | compare-and-swap status transition | provider execution remains external/manual; not proof of money returned |

Saved patient payment methods and Doctor destinations are routing/configuration,
not payment execution. Wallet counters are projections; no patient balance debit
or distinct platform cash ledger exists in this implementation. Existing
`transactions`, frozen statement membership, and refund requests carry financial
history. Claims about patient debits or provider effects are not applicable until
the relevant integration exists.

## Intended authoritative rules

An appointment may have historical FAILED attempts, but at most one transaction
whose status is not FAILED. Settled, disbursed, refund-pending and refunded rows
continue to occupy that obligation: another payment cannot charge it again.
Different HTTP idempotency keys cannot bypass this partial unique index.

Callbacks lock the appointment then re-read/lock the transaction, verify the
gateway and immutable reference, record provider event evidence atomically, and
allow only INITIATED -> PAYMENT_HELD. Later retries recover current state without
rewriting settled/refund history. Expired/cancelled appointments never activate.

Settlement and refund approval retain existing transaction/row locks. Immutable,
uniquely identified economic-effect evidence adds an independent database gate.
UI disabling remains a convenience; source and database controls are authoritative.

## Verification status

Source inspection completed. No runtime test has passed merely because its source
exists. No external or production endpoint may be used for validation.

## Implemented controls

Migration `20261110000001_economic_idempotency` adds the non-FAILED appointment
unique index and provider-code/reference payout unique index. It deliberately
fails if existing data violates either invariant; review conflicting history
before rollout, never silently delete financial records. FAILED history remains
permitted. All payment lookup code that formerly assumed one row now chooses the
nonfailed row first; the SQL completion function explicitly excludes FAILED
history. Payment resume remains unavailable until a real provider is integrated.

`financial_economic_effects` is a prospective, append-only financial event journal.
Its primary key is `(effect_type, business_id)`: appointment ID for collection,
settlement and refund approval; disbursement item ID for payout. Database
transition triggers write amounts, currency, actor and source identity inside
the same transaction. Application roles cannot insert, update or delete journal
rows. Duplicate economic identities abort the transaction, including wallet
projection writes. Existing historical balances are not presented as backfilled
events. A consultation normally has one PAYMENT_CONFIRMED and one
CONSULTATION_SETTLED event; count each kind separately, not every journal row as
a separate consultation settlement. The journal is evidence for existing money
flows, not a new patient cash wallet or platform bank ledger.

The callback service locks appointment then payment and re-reads after waiting,
validates event ownership/reference, and uses a conditional transition. Unique
event IDs cannot reset a settled or refunded payment. Immutable identities and
amounts have database protection. Callbacks after expired/cancelled holds retain
the actual appointment state and skip session/chat creation. Captured late funds
require Finance reconciliation; no automatic refund is fabricated. If a FAILED
historical attempt later reports success while another active attempt exists,
the unique index rejects a second obligation; a future provider reconciler must
resolve that case rather than inventing a second local payable obligation.

The existing atomic completion SQL function now reports ALREADY_COMPLETED while
holding its locks. Rust returns before repeating completion trace, earning
notification or audit on this result. Admin approval and payout-paid confirmation
retain their existing conditional updates, winning actor and transaction-scoped
audit/notification behavior; the payout reference index and journal are extra
database backstops. Refund approval retains its transaction lock, unique refund
request and wallet reversal, and gains the journal's REFUND_APPROVED identity.
Repeated disbursement preparation with no unallocated payable statements now
returns a conflict before inserting an empty batch or another initiation audit.
Investigation and appointment status reads choose the current nonfailed attempt
deterministically while financial history endpoints retain FAILED attempts.
Payment resume uses a separate per-account budget of 10 requests/minute before
database acquisition in addition to the outer service/payment admission budget.

## Other mutation classification (source review)

| Mutation | Classification | Existing scope/control |
| --- | --- | --- |
| Booking / slot reservation | IDEMPOTENT REQUIRED for one occupied slot | `try_lock_slot` locks/rechecks the slot; a competing new booking conflicts; request-key replay is optional and does not replace slot exclusivity |
| Reschedule | DUPLICATE-SAFE for same target; intentionally repeatable for a different target | `trace_handlers::reschedule` locks the appointment; retry to its already-current slot conflicts; old/new slot and change record commit together |
| Consultation completion | IDEMPOTENT REQUIRED | atomic SQL completion, one settlement journal identity and first-completion-only trace/audit/notification |
| Chat send | INTENTIONALLY MULTIPLE, IDEMPOTENT per supplied send key | same-key body mismatch conflicts; conversation/sender/key unique index and advisory lock; no-key identical messages may be intentional |
| Grievance creation | INTENTIONALLY MULTIPLE | a user can report different incidents for one consultation; each grievance has a new identity; no content-based deduplication is imposed |
| Patient document upload finalization | IDEMPOTENT REQUIRED per upload request; multiple different uploads legitimate | existing request-key replay plus appointment advisory lock/page cap; different keys can add another document within that cap |
| Doctor prescription upload | DUPLICATE-SAFE for identical sanitized bytes | appointment lock and persisted integrity hash lookup reuse the existing prescription |
| Doctor verification/revocation | IDEMPOTENT REQUIRED per permitted transition | conditional PENDING->VERIFIED/REJECTED or VERIFIED->REVOKED updates; repeat conflicts; database trigger protects reviewer/state evidence |
| Notification creation | IDEMPOTENT REQUIRED per semantic event | stable unique dedup key; savepoint handles unique conflict without poisoning surrounding financial transaction |

These classifications are a source review, not a claim that every nonfinancial
route received a new adversarial runtime test in this milestone.

## Test inventory and limits

`services/backend/tests/economic_idempotency.rs` adds a disposable-database suite:
100 simultaneous distinct transaction creations for one appointment, FAILED
history, 100 identical-event and 100 distinct-event callback repository calls,
100 atomic SQL completions, two Admins/100 approvals, and 100 payout confirmations.
Observer queries read committed transaction/event/journal/wallet rows after the
workers finish. The suite invokes actual repositories and SQL completion; it
does not replace the separate HTTP attacker/observer acceptance harness.

Execution status: **INCONCLUSIVE / NOT RUN** in this implementation agent.
Target-controlled execution requires an OS-enforced sandbox with no external
network, clean allowlisted environment, read-only source/toolchain and bounded
scratch/resources. Validation is coordinated separately. Run only against a
new disposable PostgreSQL cluster with synthetic data and execute integration
tests single-threaded because capability roles are cluster-wide. A safe command
inside that sandbox is `cargo test --offline --test economic_idempotency --
--test-threads=1`. The existing HTTP regression suite must also be run; do not
alter assertions merely to accommodate a new control.

No external provider requests, real checkout reuse, real refunds, or authoritative
provider reconciliation have been tested. HTTP same-key/different-key payment
success remains **INCONCLUSIVE** because collection is unavailable. The current
endpoint contract permits a missing Idempotency-Key, and resume creates no new
transaction in every key variant. Existing booking retries still return the
established slot-conflict response; finding the already-created appointment is
the recovery route when a booking response is lost.
