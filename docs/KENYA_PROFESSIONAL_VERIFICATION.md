# Kenya professional-registration correction — 2026-10-08

HelloDoctor's active deployment scope is Kenya only. Professional registration
uses **Kenya Medical Practitioners and Dentists Council (KMPDC)** and country
**KE**. `license_number` is the existing generic registration-number field;
no duplicate regulator-specific number column was added.

This is **manual HelloDoctor review of submitted registration information**.
It does not establish real-time registry confirmation, current annual practice
licensure, or an official KMPDC API connection. KMPDC distinguishes
[practitioner registration](https://kmpdc.go.ke/registering-as-a-practitioner/)
from [annual practice licensing](https://kmpdc.go.ke/application-for-annual-license/).
No external integration, scraping or annual-licence subsystem was introduced.

## Persisted model and transitions

`professional_verification_status` is a PostgreSQL/Rust enum with exactly
`PENDING`, `VERIFIED`, `REJECTED`. `is_verified` on Doctor profiles is retained
solely as a database-derived compatibility field. All professional business gates
use the explicit state or `is_professionally_verified()`. `users.is_verified`
is a separate account field and was not repurposed.

Both public and Admin-created new Doctors start PENDING and off duty. Submitted
authority/country/status/review fields are not accepted as self-service input.
The existing ClinicalAdmin/PlatformAdmin authorization is preserved. The five
database capability roles, request AuthContext, auth/session architecture and
RLS ownership boundaries remain intact. Account activation/suspension, deletion,
MFA, duty and payout readiness remain separate controls. Review never reactivates
an account or automatically switches duty on.

| Request | Before | After | Repeated/opposite terminal review |
| --- | --- | --- | --- |
| Verify | PENDING | VERIFIED | 409 |
| Reject | PENDING | REJECTED | 409 |

A conditional PostgreSQL UPDATE consumes PENDING exactly once. Status, review
timestamp, reviewer, optional note, audit event and Doctor notification commit
together. Concurrent verify/reject produces one 200 and one 409. No reapplication
or terminal-state reversal was added. Review metadata is available in the own
profile, while the public Doctor directory excludes internal notes/reviewer IDs.

## Current API and client mappings

All routes below are under `/api/v1`:

| Route | Authority / behavior |
| --- | --- |
| `GET /admin/doctors/pending-verification` | ClinicalAdmin or PlatformAdmin; SQL filters PENDING only |
| `POST /admin/doctors/{id}/verify` | Same roles; optional `{ "note": "..." }`; PENDING → VERIFIED |
| `POST /admin/doctors/{id}/reject` | Same roles; optional note; PENDING → REJECTED |
| `GET /admin/overview` | Pending count field is `pending_professional_verification` |
| `POST /auth/doctor/register` | Public rate-limited; KMPDC/KE/PENDING set by server |
| `GET /doctor/profile` | Own profile; explicit review state and Kenya registration fields |

The previous country-specific review routes are removed, with no compatibility
alias. Backend and both portals are built/restarted together. Admin navigation is
**Doctor Verification**. Its queue shows actual submitted details and only
PENDING Doctors. The unsupported clarification button was removed rather than
claiming to send a request. The Doctor directory exposes approved Doctors only;
patient availability also excludes non-approved Doctors.

Doctor Web uses the explicit state for clinical readiness and presents rejected
registration distinctly. Settings → Professional Details is read-only and names
KMPDC and Kenya. Flutter maps the same state; it fetches the own profile before
clinical collections so a 403 cannot hide rejection as a generic load failure.
Missing/old cached state fails closed. The user confirms the Doctor app is verified;
the automated checks in this pass cover the changed contract and status handling.

## Migration and fixture safety

Normal SQLx migration: `20261024000001_professional_verification`.
No earlier applied migration or `_sqlx_migrations` row is manually modified.

Backfill uses the latest successful historical review audit event, ordered by
timestamp and UUID to break ties. Its verification/rejection action determines
the state and review metadata. Without review evidence, true becomes VERIFIED
and false conservatively becomes PENDING. False alone never means rejected.
The compatibility boolean is synchronized afterward. Rollback refuses to discard
rejection/review evidence; archival is required before a lossy rollback.

New seed/capacity fixtures use synthetic Kenyan registration numbers and clinics.
The fixed login emails/passwords/UUIDs remain stable. Persistent cleanup targets
only exact seed UUIDs and expected old values; unknown Doctor records are not
rewritten. No schedules, appointments or finances are changed by fixture cleanup.

## Verification and residue evidence

Final verification is recorded below and in PROGRESS.md. Raw logs and inventories are under
`services/backend/target/kenya-professional-verification/`.
The source inventory tool is
`services/backend/scripts/audit_professional_registration.py`.
It classifies every matching source line, excludes credentials/build output, and
checks that every pre-existing migration retains its original SHA-256 hash.

Historical prototypes, dated audit snapshots and old SQL migrations retain their
original terminology as historical evidence. Legacy-country negative tests and
source rejection patterns are TEST_LEGACY safeguards. Legacy finance enum values
and historical transaction parsing are retained for existing records; payment
providers and financial/session architecture are outside this correction.
No production professional-registration path may depend on the old regulator.

**HOST_SOFTWARE_INSTALLED = NONE.** Only existing dependencies, installed Flutter,
Rust/Node/Python, installed Edge, and the existing PostgreSQL/Redis services are used.

## Final regression evidence

The actual checkout already included later security/scheduling work: 50 library
tests and 106 API tests before this correction. The supplied 47/90 baseline was an
earlier snapshot. This pass adds seven API test functions and strengthens existing
registration, queue, audit and notification regressions.

The four required Rust commands ran **sequentially**, with no concurrent API suite:

| Command | Result | Elapsed |
| --- | --- | --- |
| `cargo fmt --check` | PASS | 0.49 s |
| `cargo clippy --all-targets -- -D warnings` | PASS | 10.33 s |
| `cargo test --lib` | 50 passed, 0 failed, 0 ignored | 121.53 s |
| `cargo test --test api_tests -- --test-threads=1` | 113 passed, 0 failed, 0 ignored | 810.08 s |

| Client check | Result |
| --- | --- |
| `flutter analyze --no-pub` | No issues found |
| `flutter test --no-pub` | 74 passed |
| Doctor Web local TypeScript check | PASS |
| Doctor Web existing Node test runner | 33 passed |
| Doctor Web source/format guard | PASS |
| Doctor Web production build | PASS |
| Admin local TypeScript check | PASS |
| Admin existing Node test runner | 4 passed |
| Admin production build | PASS |

Both TypeScript checks use the existing project-local TypeScript CLI with
`--noEmit`; no dependency resolution or installation is needed. Node tests use the
existing `--experimental-strip-types --test` runner. Production builds use each
application's existing Next CLI. Flutter explicitly uses `--no-pub`.

Initial regression execution exposed an old payout masking fixture's foreign
phone, and one new regression's incorrect booking URL. The masking fixture now
uses a synthetic Kenyan M-Pesa identifier; the new regression uses the existing
`/appointments/book` helper. Both targeted tests passed, followed by the complete
clean sequential run above. No financial implementation was changed for those
test corrections.

Portable results: [verification evidence](audit_evidence/kenya-professional-verification-results.json).
Raw command logs remain in `services/backend/target/kenya-professional-verification/`.

## Coverage mapping

All API coverage is in `services/backend/tests/api_tests.rs`, using its existing
SQLx fixtures, token helpers and HTTP helpers.

| Requirement | Specific regression |
| --- | --- |
| New registration PENDING; KMPDC/KE; no status/metadata mass assignment | `test_doctor_registration_is_pending_and_cannot_set_privileged_fields` |
| Admin-created Doctor also PENDING/off duty | `test_admin_create_doctor_issues_temporary_password` |
| Pending included; VERIFIED and REJECTED excluded; repeat list/overview correct | `test_admin_professional_verification_queue_pending_only_and_abac` |
| Pending verification succeeds with reviewer/timestamp/note; repeated/opposite reviews conflict; account stays suspended | `test_professional_verify_is_terminal_with_metadata_and_cannot_reactivate_suspended_account` |
| Pending rejection succeeds and persists, with one notification | `test_reject_doctor_professional_registration_creates_exactly_one_notification_via_http` |
| Notification exactly once; repeated terminal review conflicts | `test_registration_verification_notifications_are_exactly_once` |
| Doctor self-review and cross-Doctor review denied; Patient and other Admin roles denied; both permitted roles work | `test_professional_review_denies_self_cross_doctor_patient_and_other_admin_roles` |
| Rejected own profile stays REJECTED; duty/clinical/Q&A/token access denied; directory/availability and stale-slot booking blocked; boolean cannot approve | `test_professional_rejection_is_persistent_and_blocks_verified_only_business_gates` |
| Simultaneous verify/reject has one 200 and one 409, consistent persisted state, one audit and one notification | `test_professional_verify_reject_race_has_one_terminal_winner` |
| Audit failure rolls back state, metadata and notification together | `test_professional_review_audit_failure_rolls_back_status_metadata_and_notification` |
| Direct own/other profile status and review-metadata writes cannot bypass the review guard | `test_professional_review_database_guard_denies_doctor_status_and_metadata_writes` |
| Audit-evidence backfill; false defaults PENDING; latest successful review wins; lossy rollback refused | `test_professional_review_migration_backfill_uses_latest_evidence_and_conservative_pending` |

The existing before/after audit regression now asserts explicit review states.
Doctor Web and Flutter tests also cover contradictory legacy booleans, distinct
pending/rejected states and missing/unknown cached status failing closed. Flutter
Cubit coverage checks that unapproved profiles load without making clinical reads.
Admin tests assert generic routes, returned KMPDC/state fields and 409 propagation.

## Live development proof

The installed Edge browser exercised the running Admin and Doctor Web portals
against the current backend and persistent development database. One new synthetic
Kenyan Doctor was rejected, another verified. Both began PENDING, appeared in the
Admin queue, were reviewed through actual Admin buttons, and disappeared from
the refreshed queue. Two fresh profile/list reads confirmed each terminal state
and its review metadata; Doctor Web showed the distinct rejection screen or
read-only verified KMPDC details. Neither account was put on duty.

**14 checks passed; zero browser exceptions.** The final database check confirms:

| Synthetic Doctor profile ID | Persisted state | Authority | Country | Slots |
| --- | --- | --- | --- | --- |
| `27eafc90-9559-47d0-8595-d5a2542c45c4` | REJECTED | KMPDC | KE | 0 |
| `cb304c29-c919-42e5-8e46-6469ea94b949` | VERIFIED | KMPDC | KE | 0 |

Screenshots and the full check manifest are in the raw artifact directory. The
browser walkthrough disables only its own background Admin polling timer to avoid
multiplying unrelated real dossier reads against the shared development IP limit;
responses and review actions are real. No authorization or rate-limit guard was
disabled. An optional repeat reached the existing Doctor-login ceiling (429).

After the complete walkthrough, the final audit found and corrected two inert
attributes in the generated Doctor Web template: a legacy field ID and a foreign
phone placeholder. The generator and case-insensitive source guard now prevent
their return. The final rebuilt portal passes typecheck, all 33 tests and the
stronger guard. The complete review walkthrough's component/handler behavior is
unchanged by those attribute corrections.

Backend and both production web builds are running on ports 8080, 3001 and 3000.
The changed Flutter source passes analysis and tests; this pass does not claim a
new installed APK or a new physical-device visual audit. The user has confirmed
the Doctor app is verified.

## Persistent data and migration verification

Exactly **2** existing proven synthetic Doctor profiles were corrected to Kenya
fixture names, registration numbers and clinics. Their two user-phone fields and
the two known Admin/Patient seed phone fields were also updated: **4 phone fields**.
The guarded repair script checks exact UUIDs and expected previous values. Unknown
Doctor identities, scheduling, appointments and financial records were not updated.
The two live-review accounts above are new fixtures, separate from that count.

Normal SQLx startup applied `20261024000001`. The read-only B-01 check confirms:

```text
SOURCE_LATEST_MIGRATION   = 20261024000001
BINARY_LATEST_MIGRATION   = 20261024000001
DATABASE_LATEST_MIGRATION = 20261024000001
```

All **62 pre-existing SQL migration files** retain their original SHA-256 hashes.
No migration ledger was manually changed. No new database role was introduced.

## Remaining source occurrences

[Every remaining legacy search occurrence, grouped by file and exact line](audit_evidence/kenya-professional-verification-residue.json)
is classified as HISTORICAL, TEST_LEGACY or DEAD_CODE. The original full inventory
also records the requested active-code/API/UI/test/seed/document/migration classes.

HISTORICAL includes immutable migrations, old audit/prototype snapshots and
compiled financial compatibility for old records. It does **not** mean all such
code is unreachable: legacy provider parsing and development-only financial
simulation remain intentionally outside this professional-registration correction.
Old design documents' financial examples are historical scope, not authorization
to enable another country or provider. TEST_LEGACY includes foreign-phone denial
tests, historical review-action backfill tests and source rejection patterns.
DEAD_CODE identifies unshipped scratch helpers and unused prototype CSS.

There are no active professional-registration BMDC references in the four shipped
source roots and no active Bangladesh professional seed fixtures. Historical audit
action names occur only where needed to migrate/test old evidence; new events use
`DOCTOR_PROFESSIONAL_VERIFY` / `DOCTOR_PROFESSIONAL_REJECT`.

## Requested final status

```text
PROFESSIONAL_VERIFICATION_MODEL = PENDING_VERIFIED_REJECTED
BOOLEAN_ONLY_VERIFICATION = LEGACY_COMPAT_ONLY
KMPDC_AUTHORITY = VERIFIED_IN_CURRENT_KENYA_FLOW
KENYA_COUNTRY_CODE = KE
PENDING_QUEUE_FILTER = PENDING_ONLY
REJECTED_EXCLUDED_FROM_PENDING = PASS
VERIFIED_EXCLUDED_FROM_PENDING = PASS
VERIFY_REJECT_RACE = PASS
DOCTOR_SELF_VERIFICATION = DENIED
ADMIN_ABAC = PASS
ACTIVE_BMDC_BACKEND_REFERENCES = 0
ACTIVE_BMDC_DOCTOR_WEB_REFERENCES = 0
ACTIVE_BMDC_FLUTTER_REFERENCES = 0
ACTIVE_BMDC_ADMIN_REFERENCES = 0
ACTIVE_BANGLADESH_PROFESSIONAL_FIXTURES = 0
PERSISTENT_DEV_FIXTURES_UPDATED = 2
MIGRATION_SCHEMA_VERIFIED = YES
RUST_LIB = 50/50
API_TESTS = 113/113
FLUTTER_TESTS = 74/74
ADMIN_BUILD = PASS
HOST_SOFTWARE_INSTALLED = NONE
```

**KENYA_PROFESSIONAL_VERIFICATION_VERIFIED**
