# Current public-exposure/session hardening and Doctor Articles ? 2026-10-08

Access JWTs now carry `sid` and consult the authoritative active refresh-family state on every protected request. Logout invalidates current-family access immediately; DB validation and Redis rate limiting fail closed. Explicit environment/seed/OTP/simulation guards, trusted HTTPS ingress and protected monitoring are documented in [PUBLIC INTERNET EXPOSURE REQUIREMENTS](../../docs/PUBLIC_EXPOSURE_HARDENING.md). External TLS remains required and Admin step-up remains open. No host infrastructure was installed.

[Doctor Articles](../../docs/DOCTOR_ARTICLES.md) publish directly from a verified Doctor. There is no Admin article review or slot approval workflow. Earlier dated results below retain their historical scope; current evidence is in `docs/audit_evidence/public-exposure-security.json`.

# Kenya professional-review boundary - verified - 2026-10-08

Professional verification has PENDING / VERIFIED / REJECTED states. Existing
ClinicalAdmin/PlatformAdmin ABAC and the five database capability roles are
preserved. A pending-only conditional update, database write guard and shared
transaction prevent self-review, cross-Doctor review, conflicting terminal
mutations and partial audit/notification commits. Account suspension, MFA,
sessions and payout controls remain separate; approval does not reactivate an
account or switch duty on. Compatibility `is_verified` is derived from the enum.

Regressions cover mass assignment, unauthorized roles, database status and metadata
writes, concurrent verify/reject (one 200 / one 409), terminal repetition, rollback,
conservative audit-backed migration and rejected clinical access. Final sequential
verification: fmt/clippy PASS, **50/50 lib, 113/113 API**.
See [current contract/evidence](../../docs/KENYA_PROFESSIONAL_VERIFICATION.md).
Earlier authentication-hardening results below retain their original scope.

# Authentication boundary hardening — 2026-10-08

The three authentication/session defects were addressed in the requested order:
idempotency, refresh-token rotation, then patient OTP consumption. The final
verification includes the entire existing integration suite, serialized.

## Idempotency boundary

Protected routes authenticate and recheck the account before preparing an
idempotency context. Middleware cannot return a cached response itself.
Booking, patient intake uploads, and doctor prescription uploads explicitly
request replay after their handler's ABAC/identity checks; replay also rechecks
current appointment ownership through RLS. Doctor prescription replay checks
the current clinical authorization window and professional verification.

Keys include authenticated user, role, sorted permissions, HTTP method, full
path/query, and the caller's key. The request body has a separate fingerprint;
changing it produces 409. Stored entries contain the authenticated user and
appointment resource. A versioned namespace isolates legacy anonymous entries.
Replayed JSON retains its content type.

Reads and public authentication, download-grant, and webhook endpoints execute
their checks on every request. Generic replay for other mutations is disabled;
those handlers execute their existing authorization and transaction logic.
Further replay support requires an explicit current resource-authorization gate.

Before the fix, a regression reproduced an unauthenticated booking replay
returning 201 with the original key/body. The regression now expects 401 and passes.

## Refresh-token lineage

Each token keeps its own immutable hash/row. Rotation marks the original consumed
and inserts an unconsumed successor in the same transaction and family. The
successor inherits the family's original expiry. Consumed identities remain
available for reuse detection throughout every descendant's usable lifetime.

Refresh, reuse detection, and logout share a PostgreSQL transaction-level family
lock. After acquiring it, refresh rereads the presented token. Reusing a consumed
token revokes the complete family, including any racing successor. Logging out
with a consumed ancestor also revokes the family. Successor-insert failure rolls
back consumption. Unique indexes enforce token identity and one live family leaf.

Migration `20261023000001_refresh_token_lineage` revokes legacy families that
already rotated: overwritten hashes cannot be reconstructed. Stop legacy server
instances before applying this migration and starting the new code. Affected
clients must authenticate again. The down migration removes the indexes and
never reverses security revocations. Access JWTs retain their existing short TTL;
this change does not add immediate access-JWT family revocation.

## Patient OTP and startup

OTP lookup uses `FOR UPDATE` within an explicit auth-worker transaction. Expiry,
attempt-limit checks, failed-attempt increments, and deletion happen under that
lock. Successful consumption commits before account/profile bootstrap, matching
the Doctor MFA consumption pattern. Concurrent requests for both existing and
new patients produce one successful login and one 401, with one session/profile.

`ENVIRONMENT` is required and limited to development/test/staging/production.
`SEED_DEV_DATA` defaults to false, accepts only true/false, and startup rejects
true outside development/test before opening a database pool. Both confirmed-
expired JWT artifacts (`admin_token.txt`, `services/backend/patient_token.txt`)
were removed. Git and both Docker build contexts exclude local `.env` files and
exported token artifacts; example environment files remain reviewable in Git.

## Regression coverage

All 13 new integration tests are in [api_tests.rs](tests/api_tests.rs):

| Area | Specific tests |
|---|---|
| Idempotency | `test_security_idempotency_requires_fresh_authentication_and_role`; `test_security_idempotency_subject_permissions_query_and_ownership_isolation`; `test_security_idempotency_reads_keep_query_semantics_and_current_authorization`; `test_security_idempotency_upload_rechecks_ownership_and_clinical_window` |
| Refresh | `test_security_refresh_successors_remain_valid_and_preserve_consumed_identity`; `test_security_refresh_reuse_revokes_descendants_but_not_another_family`; `test_security_refresh_concurrent_consumption_has_one_winner_and_revokes_family`; `test_security_refresh_insert_failure_rolls_back_consumption`; `test_security_refresh_logout_race_cannot_leave_live_descendants`; `test_security_refresh_migration_revokes_legacy_rotated_families` |
| Patient OTP | `test_security_patient_otp_single_use_even_with_idempotency_key`; `test_security_patient_otp_concurrent_consumption_issues_one_session`; `test_security_patient_otp_attempt_limit_and_expiry_are_atomic` |

Three new unit tests cover startup mode/seed validation. The existing encrypted
document-download regression was strengthened with a reused idempotency key to
prove an expired clinical authorization cannot be bypassed.

## Verification evidence

| Check | Result |
|---|---|
| `cargo fmt --check` | Exit 0 |
| `cargo clippy --all-targets -- -D warnings` | Exit 0, zero warnings |
| `cargo test --lib` | 50 passed, 0 failed/ignored, 103.74s |
| `cargo test --test api_tests -- --test-threads=1` | 103 passed, 0 failed/ignored/filtered, 730.86s |
| Built-executable startup refusal checks | 5/5 passed: missing/unknown environment, production/staging seeding, invalid seed flag |
| Local backend readiness/schema | `HELLODOCTOR_BACKEND_READY`; source = binary = database = `20261023000001`; separate schema verifier exit 0 |

Integration count increased from 90 to 103; library count increased from 47 to 50.
Raw local logs are under `target/security-hardening-verification/`: `clippy.log`,
`lib-tests.log`, `api-tests.log`, `startup-guards.log`, `backend-readiness.log`,
and `schema-verification.log`. Deliberately injected 500/503 cases log errors while
their assertions pass; they are not failed tests.

## Production verdict

Continued development and controlled synthetic-data testing remain acceptable.
These fixes and local checks do not justify unrestricted clinical production or
real-money operation. Remaining provider, operational, and product readiness
requirements still need their own evidence.
