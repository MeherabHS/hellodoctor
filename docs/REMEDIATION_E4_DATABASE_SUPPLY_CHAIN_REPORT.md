# HelloDoctor.Asia — Remediation E4: Database, Credential and Supply-Chain Hardening

Date: 2026-10-30 (repo-internal migration timeline: 2026-12-02).

Scope: five confirmed findings from the independent cybersecurity audit —
stale Admin-role references in three database objects, missing Flutter
dependency vulnerability scanning, plaintext-secret exposure through `Debug`
on authentication request DTOs, a non-constant-time OTP digest comparison,
and FCM payload/documentation accuracy. No application functionality was
changed, no architecture was rewritten, no existing migration was edited,
and no destructive operation was run against the persistent development
database. Remediations A–D, E1, E2, E3 and Milestones 7A–7C.1B were not
touched and remain as they were.

**Final classification: GREEN.** All five findings are fixed, each fix is
covered by a passing automated regression test, and the complete existing
test surface (Rust unit, Rust integration, both Flutter suites) is green
with no regressions.

---

## 1. Phase 1 — Verified findings

All five were independently reproduced against the actual code/migrations
(not taken on the audit's word) before anything was changed.

### 1.1 Stale Admin-role references — CONFIRMED, root-caused

`20261106000002_single_admin` consolidated six legacy Admin role names
(`PLATFORM_ADMIN`, `CLINICAL_ADMIN`, `FINANCE_ADMIN`, `COMPLIANCE`,
`SECURITY_ADMIN`, `SUPPORT`) into a single `ADMIN` role, and its own dynamic
`DO $$` sweep correctly rewrote every *other* live policy/function that
referenced them (45 policies + 2 functions, recorded in
`admin_policy_rollback`). But `20261112000001_profile_pictures` — dated six
days *after* that consolidation — had to `DROP`/`CREATE` both
`doctor_public_directory` (a view) and `doctor_articles_published_read` (a
policy depending on that view) to add new profile-picture columns, and
copied the pre-consolidation six-role list back in verbatim instead of
`ADMIN`. The new `doctor_public_photo()` function created in that same
migration never used `ADMIN` either.

Verified by applying the complete migration history (82 migration pairs) to
a disposable PostgreSQL 16 container and inspecting the live definitions:

```
pg_get_viewdef('doctor_public_directory')        -- contained PLATFORM_ADMIN,...,SUPPORT
pg_get_functiondef('doctor_public_photo')        -- contained PLATFORM_ADMIN,...,SUPPORT
pg_policies.qual WHERE policyname='doctor_articles_published_read'  -- contained PLATFORM_ADMIN,...,SECURITY_ADMIN
```

Effect: under the consolidated role model, an authenticated `ADMIN` session
was treated like any other authenticated caller by these three objects, so
an inactive doctor's public-directory row, profile photo, and published
articles were unintentionally hidden from Admin review. This is a
correctness/availability regression (Admin under-privileged), **not** a
data-exposure defect — no unauthorized party gained access to anything.

### 1.2 Flutter dependency vulnerability scanning gap — CONFIRMED

`.github/workflows/security-scan.yml` ran `cargo-audit` (Rust) and
TruffleHog (secrets) only. No job scanned either Flutter app's
`pubspec.lock` (`apps/mobile`, the Doctor app; `apps/mobile_patient`, the
Patient app) against any vulnerability database. `backend-migrations.yml`
and `application-checks.yml` don't cover this either.

### 1.3 Plaintext password/OTP/token exposure via `Debug` — CONFIRMED

Ten request DTOs in `services/backend/src/api/v1/auth_handlers.rs` derived
`Debug` over a field carrying a live secret:

| Struct | Secret field(s) |
|---|---|
| `PatientRegistrationRequest` | `password` |
| `PatientLoginRequest` | `password` |
| `PatientVerifyOtpRequest` | `challenge_token`, `otp_code` |
| `VerifyOtpRequest` | `otp_code` |
| `DoctorLoginRequest` | `password` |
| `DoctorVerifyOtpRequest` | `session_token`, `otp_code` |
| `DoctorRegistrationRequest` | `password` |
| `AdminLoginRequest` | `password` |
| `AdminVerifyOtpRequest` | `session_token`, `otp_code`, `totp_code` |
| `RefreshRequest` | `refresh_token` |

This is the same class of defect Remediation E3 fixed for `AppConfig`
(`config.rs`) and explicitly called out as a known, un-actioned limitation
for these exact DTOs ("a different class of object ... out of this
remediation's scope" — `REMEDIATION_E3_AZURE_CONTAINER_HARDENING_REPORT.md`
§3/§8). Any `tracing::debug!(?payload, ...)`, panic message, or assertion
failure formatting one of these structs with `{:?}` would have printed the
real secret.

### 1.4 Non-constant-time OTP digest comparison — CONFIRMED

`crypto::verify_code(code, hash)` computed `hash_code(code) == hash` — a
`String` equality that short-circuits on the first differing byte. A
reviewed, already-correct constant-time comparison (`constant_time_eq`,
byte-wise XOR-accumulate, no early exit) already existed in the codebase but
was private to `services/dr_erasure_journal.rs` for HMAC verification only,
confirming the audit's framing ("evaluate whether OTP/MFA digest
comparisons should reuse an existing constant-time comparison mechanism").
Admin's separate `admin_totp::verify` was unaffected — it already compared
recomputed `u32` TOTP values arithmetically, not via hash-string `==`.

### 1.5 FCM payload privacy / notification doc staleness — PARTIALLY CONFIRMED

The FCM payload itself (`services/notification_transport.rs`) was verified
safe: its `data` block carries only `notification_id`, `notification_type`,
and optionally `related_entity_type`/`related_entity_id` — opaque
identifiers, never PHI, a clinical complaint, or prescription content,
consistent with CLAUDE.md §7. No code defect was found here.

`docs/FCM_SETUP.md`, however, was stale: it asserted "this app only ever
sends a `notification` block ... never a silent/data-only push," which is
inaccurate — a `data` block is always sent alongside `notification`, and
both Flutter apps actively read it (`message.data` in
`handleOpenedMessage`) to drive in-app deep-linking on tap. The underlying
conclusion (no `onBackgroundMessage` handler is needed) is still correct,
for a different reason than the doc gave: the OS auto-displays because
`notification` is always present, not because `data` is absent.

---

## 2. Phase 2 — Database correctness

### 2.1 Migration

New forward migration (no existing migration was edited):
`services/backend/migrations/20261202000001_fix_stale_admin_role_references.{up,down}.sql`.

The up-migration drops and recreates `doctor_articles_published_read` (the
dependent policy must go first), drops and recreates
`doctor_public_directory` (identical column set to its current definition —
no schema change, only the role predicate), recreates the policy with
`ADMIN` replacing the six legacy names, and `CREATE OR REPLACE`s
`doctor_public_photo()` with the same substitution. The down-migration is
the exact byte-for-byte inverse (restores the six-role predicate), so a
revert returns to precisely the pre-fix state — verified, not assumed (§2.2).

### 2.2 Verification method and evidence

A disposable, throw-away `postgres:16` container
(`hellodoctor-e4-disposable`, port 5433, **never** the persistent
`hellodoctor-postgres` dev container) received the complete migration
history via `sqlx migrate run`:

```
$ sqlx migrate run        # 82 migrations, including the new one, applied clean
Applied 20261202000001/migrate fix stale admin role references (11.9ms)

$ sqlx migrate revert     # down-migration applies clean
Applied 20261202000001/revert fix stale admin role references (7.4ms)

$ sqlx migrate run        # re-applied clean
Applied 20261202000001/migrate fix stale admin role references (6.2ms)
```

Static confirmation — zero stale references remain anywhere in the schema,
and the one legitimate reference (the role-rejection guard trigger, which
must still recognize the legacy names to reject them) is untouched:

```sql
-- policies referencing a legacy role literal
SELECT count(*) FROM pg_policies WHERE qual LIKE '%PLATFORM_ADMIN%';            -- 0
-- views
SELECT count(*) FROM pg_class ... WHERE pg_get_viewdef(...) LIKE '%PLATFORM_ADMIN%';  -- 0
-- functions, excluding the legitimate guard
SELECT count(*) FROM pg_proc ... WHERE proname<>'reject_active_legacy_admin_role'
  AND pg_get_functiondef(oid) LIKE '%PLATFORM_ADMIN%';                          -- 0
```

Functional confirmation — a live, role-scoped session under the real
`hellodoctor_api` role (not the `postgres` superuser, which bypasses RLS
entirely) against an inserted inactive-but-verified doctor fixture:

| Session role | `doctor_public_directory` row visible | `doctor_public_photo()` returns non-null |
|---|---|---|
| `ADMIN` | **yes** (was: no, before the fix) | **yes** (was: no) |
| `PATIENT` | no (correct — doctor is inactive) | no (correct) |

This is also covered by a new, permanent HTTP-level regression test
(`test_admin_sees_inactive_doctors_photo_patient_does_not`,
`tests/cases/profile_pictures.rs`), exercised through the real router/DB
stack as part of the full suite in §6, not just this one-off manual check.

`doctor_articles_published_read`'s own business condition
(`doctor_public_directory.account_active`, i.e. the published-article
visibility rule still requires the *author's* account to be literally
active, for every role including Admin) is unrelated pre-existing business
logic, not part of the stale-role defect, and was deliberately left
unchanged — only the role-literal substitution was touched, per the
restriction against rewriting unrelated behavior.

### 2.3 Role isolation / RLS / SECURITY DEFINER preserved

No RLS policy was disabled, no `FORCE ROW LEVEL SECURITY` was removed, the
view keeps `security_barrier=true`, and `doctor_public_photo()` keeps
`SECURITY DEFINER SET search_path=public` with `REVOKE ALL ... FROM PUBLIC`
plus an explicit `GRANT EXECUTE ... TO hellodoctor_api` (re-granted, since
`DROP`/`CREATE VIEW` drops prior grants; `CREATE OR REPLACE FUNCTION`
preserves them but the grant statement is kept for clarity and idempotency).

---

## 3. Phase 3 — Credential safety

All ten DTOs from §1.3 now have hand-written `Debug` impls (the derive is
removed) that print every field explicitly, redacting only the secret ones
as `<redacted>`/`<empty>`/`<unset>` — the same pattern Remediation E3
established for `AppConfig`, so a field added to one of these structs later
is simply absent from `{:?}` until someone deliberately decides how to show
it, rather than `derive(Debug)`'s default of printing a new field's raw
value.

No change to deserialization, authentication behavior, or response
contracts: field types, `serde` attributes (`flatten`, `rename`, `default`,
`deny_unknown_fields`), and all call sites are untouched — only the
`Debug` derive was replaced.

11 new regression tests (`auth_handlers.rs::remediation_e4_debug_redaction_tests`)
assert, for each struct, that formatting it with `{:?}` using a synthetic,
recognizable secret (e.g. `SynthSecretPW_7f2c9a`) never contains that
secret and does contain a redaction marker — including the "absent TOTP"
case (`Option<String>` = `None` → `<unset>`, not a panic or empty string).

---

## 4. Phase 4 — Flutter dependency security

Added a new job, `flutter-dependency-audit`, to
`.github/workflows/security-scan.yml`, using Google's OSV-Scanner reusable
workflow pinned to its release commit:
`google/osv-scanner-action/.github/workflows/osv-scanner-reusable.yml@b1d267aea5211fa648d9d442fa64b90dbdcbdd36 # v2.7.0`
(commit resolved by dereferencing the `v2.7.0` annotated tag through the
GitHub API, matching this repo's existing pin-by-commit-SHA convention).

Compatibility was verified for real, not assumed: OSV-Scanner's own
supported-lockfiles documentation lists `pubspec.lock`/Pub, and it was run
locally against both of this repo's actual lockfiles:

```
$ docker run --rm -v "<repo>:/src" ghcr.io/google/osv-scanner:v2.7.0 \
    --lockfile=/src/apps/mobile/pubspec.lock \
    --lockfile=/src/apps/mobile_patient/pubspec.lock --format table
Scanned /src/apps/mobile/pubspec.lock file and found 156 packages
Scanned /src/apps/mobile_patient/pubspec.lock file and found 157 packages
No issues found
```

The CI job scans only these two explicit lockfiles (`scan-args:
--lockfile=apps/mobile/pubspec.lock`, `--lockfile=apps/mobile_patient/pubspec.lock`),
not `--recursive ./` — the Rust lockfile already has its own dedicated
`cargo-audit` job, and a repo-wide scan would also pick up Node lockfiles
with no established triage policy. `upload-sarif` is off (this repo has
not enabled GitHub code scanning) and `fail-on-vuln` is on, so an actionable
advisory fails this job exactly the way `cargo-audit --deny warnings` fails
its own job. No vulnerability result is fabricated or suppressed — the
scan found none to triage. A future confirmed-unfixable advisory would be
excluded the same documented way `RUSTSEC-2023-0071` is in the Rust job: a
dated, linked inline comment, never a silent drop.

No unrelated dependency was updated.

---

## 5. Phase 5 — Additional defensive checks

- **Constant-time OTP comparison**: `constant_time_eq` was moved from
  `services/dr_erasure_journal.rs` (private) to `crypto.rs`
  (`pub(crate)`), and both of its original call sites there now import it
  from `crypto` instead of keeping an independent copy. `crypto::verify_code`
  now calls `constant_time_eq(&hash_code(code), hash)` instead of `==`. 5
  new tests (`crypto::remediation_e4_constant_time_tests`) cover equal
  values, differing values of the same length, differing lengths, the
  empty-string edge case, and that `verify_code` still verifies correctly
  end-to-end (accepts the right code, rejects a wrong one and a
  length-mismatched one).
- **FCM template audit**: every `notify_*` function in
  `services/notification_service.rs` was reviewed; all render titles/bodies
  from `notification_templates` or a hardcoded fallback containing only
  non-clinical facts (e.g. "Consultation earning credited: KES 450"),
  several with an explicit in-code comment confirming no PHI is included
  (e.g. `notify_professional_verification_revoked`,
  `notify_prescription_available`). No implementation change was made here
  — the templates were already correct.
- **Documentation correction**: `docs/FCM_SETUP.md`'s "what's already
  wired" section was corrected to describe the real `data` block
  (non-PHI IDs) and how both apps consume it via `handleOpenedMessage` for
  deep-linking, without changing the actual notification implementation.

---

## 6. Phase 6 — Comprehensive dependency verification

### 6.1 Database change verification (§2 covers the detail; summarized here)

| Check | Result |
|---|---|
| Fresh migration history applies clean (82 migrations) | Pass |
| Down-migration reverts clean; re-apply clean | Pass |
| No stale legacy-role reference remains (policy/view/function scan) | Pass — 0 matches |
| Admin sees inactive doctor's directory row/photo | Pass (was failing before fix) |
| Patient still cannot see inactive doctor's directory row/photo | Pass (unchanged) |
| RLS/`FORCE ROW LEVEL SECURITY`/`SECURITY DEFINER`/grants preserved | Pass |

### 6.2 Credential-redaction verification

11/11 new tests pass (part of the `cargo test --lib` run in §6.4), each
built from a synthetic secret, asserting both "the real secret never
appears" and "a redaction marker does appear."

### 6.3 CI/dependency-scanning verification

| Check | Result |
|---|---|
| OSV-Scanner finds and parses both real lockfiles | Pass — 156 + 157 packages |
| Current vulnerability findings | None |
| Tool/lockfile-format compatibility | Confirmed (official docs + live run) |
| Action/workflow pinned by commit SHA | Pass |
| `security-scan.yml` YAML validity | Pass (`yaml.safe_load`) |

Not exercised in this pass (requires pushing to GitHub Actions, which this
task does not do): the reusable workflow actually executing inside GitHub's
runners, and the `fail-on-vuln`/`upload-sarif` input plumbing end-to-end.
This is the "outstanding cloud verification" called out in §7.

### 6.4 Full regression suite

All four full-suite runs below are against this change set in its entirety
(§§2–5 combined), using disposable fixtures only — the persistent
`hellodoctor-postgres`/`hellodoctor-redis` dev containers were used solely
for the already-established `#[sqlx::test]` ephemeral-per-test-database
convention (each test gets its own freshly-migrated, auto-dropped database
on that server; the server's own persistent `hellodoctor` database was
never read from or written to), and a dedicated, explicitly-owned disposable
Redis fixture was used for anything requiring Redis, per the suite's own
built-in safety check (`TEST_REDIS_URL`/`TEST_REDIS_FIXTURE_ID`, rejecting
the shared dev Redis by construction):

| Suite | Command | Result |
|---|---|---|
| Format | `cargo fmt --check` | Clean |
| Lint | `cargo clippy --all-targets -- -D warnings` | Clean |
| Rust library | `cargo test --lib` | **131 passed**, 0 failed (115 baseline + 16 new) |
| Rust integration | `cargo test --test api_tests -- --test-threads=1` (disposable Redis, port 16380, ownership-marker-gated) | **454 passed**, 0 failed, 4 ignored (453 baseline + 1 new) |
| Doctor Flutter | `flutter test` (`apps/mobile`) | **302 passed**, 0 failed (baseline) |
| Patient Flutter | `flutter test` (`apps/mobile_patient`) | **364 passed**, 0 failed (baseline) |

Every command above exited 0. No existing test was weakened, skipped, or
deleted to make this pass.

**Process note, disclosed rather than glossed over**: the first integration
run in this session failed en masse (443/453) with
`TEST_REDIS_URL must select an explicitly owned disposable Redis fixture: NotPresent`
— a missing test-harness fixture in this session's environment, confirmed
by immediately re-running one failing test in isolation and reading its
exact panic message, not a code regression. A disposable `redis:7-alpine`
container (`hellodoctor-e4-test-redis`, port 16380) was started with the
required `hd:test-fixture:owner` marker and the suite re-run clean. This is
the same failure mode and fix already documented in
`REMEDIATION_E3_AZURE_CONTAINER_HARDENING_REPORT.md` §"known limitations"
for a prior remediation pass.

---

## 7. Changed files

| File | Change |
|---|---|
| `services/backend/migrations/20261202000001_fix_stale_admin_role_references.up.sql` | New migration: `ADMIN` replaces the six legacy role names in the view/function/policy |
| `services/backend/migrations/20261202000001_fix_stale_admin_role_references.down.sql` | New migration: exact inverse |
| `services/backend/src/api/v1/auth_handlers.rs` | Removed `Debug` derive from 10 DTOs; added hand-written redacting impls; added 11 regression tests |
| `services/backend/src/crypto.rs` | Added shared `pub(crate) fn constant_time_eq`; `verify_code` now uses it; added 5 regression tests |
| `services/backend/src/services/dr_erasure_journal.rs` | Removed its private `constant_time_eq` copy; imports the shared one from `crypto` instead |
| `services/backend/tests/cases/profile_pictures.rs` | Added `test_admin_sees_inactive_doctors_photo_patient_does_not` |
| `.github/workflows/security-scan.yml` | Added `flutter-dependency-audit` job (OSV-Scanner, pinned SHA) |
| `docs/FCM_SETUP.md` | Corrected the `data`-block/background-handler claim |

No file outside this list was modified for E4. (A separate, explicitly
user-requested change to consolidate Admin login onto the same OTP-only
flow as Doctor/Patient — removing the mandatory TOTP second factor — was
started in this session in `services/auth_service.rs` after this E4 work
was confirmed complete; it is **out of E4's scope**, is **not yet finished
or compiling**, and is not included in this report or its test results. It
will be documented separately once done.)

---

## 8. Outstanding native/cloud verification

- The new `flutter-dependency-audit` CI job has not actually executed
  inside GitHub Actions (would require a push/PR); it was verified by
  running the identical scanner/lockfile combination locally instead.
- No Azure, Firebase, or other cloud resource was created, inspected, or
  modified in this pass (none were in scope).
- No Android/iOS device or emulator verification was performed (none of
  this pass's changes touch client-rendered UI).

---

## 9. Final classification: GREEN

All five Phase-1 findings are independently confirmed fixed; every fix has
a passing automated regression test exercising the real HTTP/DB stack where
applicable (not an isolated unit mock); the complete pre-existing test
surface — Rust library, Rust integration, Doctor Flutter, Patient Flutter —
passes at 100% with zero regressions; `cargo fmt`/`clippy` are clean; and
the one piece of work not executable from inside this environment (the CI
job actually running on GitHub's infrastructure) is explicitly disclosed in
§8 rather than assumed.
