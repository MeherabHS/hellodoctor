# Canonical ownership and Doctor Q&A correction

Security identity is `users.id`. Patient and Doctor profile IDs are distinct
domain identifiers, never substitutes for the authenticated user UUID. Each
profile's `user_id` remains UNIQUE and FK-backed. Database triggers reject
wrong-role profiles, identity reassignment, and role changes on profile-bearing
accounts. Normal Patient OTP bootstrap and Doctor registration create the
account and its single matching profile transactionally. An OTP consumption
remains single-use even when account bootstrap subsequently fails.

The existing five capability roles remain unchanged. None owns application
tables or has SUPERUSER/BYPASSRLS. Protected database transactions bind user,
role, resolved Patient/Doctor profile, session and request context with
`set_config(..., true)` (SET LOCAL). The profile identifiers are resolved from
the canonical user, not supplied by the client. Worker transactions clear
inapplicable profile settings. Commit and rollback both clear local context.

## Database enforcement

Migration `20261026000001_canonical_user_isolation` adds FORCE RLS to users,
private Doctor profiles, idempotency responses and system-error records.
Clinical and financial tables retain their existing FORCE RLS and capability
grants. Public Doctor discovery reads a security-barrier professional projection
that excludes account linkage, residential address, private phone and review
notes. Pending/rejected professional details remain distinct; booking and
available-slot listing still require VERIFIED. No professional-review policy
or suspension control is bypassed.

Composite FKs ensure slot/Doctor, appointment/Patient, and appointment/Patient/
Doctor relationships agree in intake, documents, prescriptions and chats.
Constraints are normally immediate; privileged test fixture transfers defer
them inside one transaction and must still be consistent at commit. Grievance
subject checks also protect the consultation/appointment chain. Existing
inconsistencies cause migration failure and require explicit repair; migration
does not guess identities or rewrite user records.

Restrictive write policies prevent permissive FOR ALL policies from allowing
sender spoofing and persisted sender/conversation reassignment (message UPDATE is limited to read receipts), non-Patient booking/document/grievance insertion, another
Doctor's prescriptions, or another Doctor's wallet creation. Logout now disables device registrations inside the Auth Worker transaction, using the canonical subject resolved from the refresh token; the ordinary API role no longer has broad device-token SELECT/UPDATE exceptions. Password hashes are also unavailable to the ordinary API DB role. Treating Doctors
cannot update Patient profiles. Clinical read access retains the approved
appointment/follow-up window and the exact assigned Doctor relationship.
The legacy treatment-relationship definer helper also binds its supplied Doctor
UUID to the transaction's canonical user and requires an active verified Doctor.
Assigned active/upcoming appointments retain operational Patient demographics;
completed appointments retain access only within the existing 24-hour follow-up
window. Cancelled or expired historical relationships do not grant indefinite
private-profile access. Private clinical records retain their narrower clinical
window, including the start-time gate for upcoming appointments.

## Controlled investigation primitive

`POST /api/v1/admin/compliance/trace` accepts exactly:

```json
{"target_type":"APPOINTMENT","target_id":"<internal UUID>","reason":"CASE-123 legitimate investigation"}
```

Allowed target types are USER, PATIENT, DOCTOR, APPOINTMENT and CONSULTATION.
Only COMPLIANCE and PLATFORM_ADMIN can invoke it. SUPPORT, SECURITY_ADMIN,
ordinary Doctors and Patients are denied. The database function independently
checks the active actor and actual role. There is no arbitrary SQL, external
authority account, phone/email search, credential return, or normal Admin
database browser.

Results contain internal IDs and canonical participant links, not clinical
content or payment credentials. Appointment/consultation requests stay scoped
to that case. Account/profile requests resolve the account's related resources.
Every successful or not-found lookup inserts COMPLIANCE_TRACE with reason,
actor, role, target, session and server-generated request ID in the same
transaction. If that write fails, retrieval fails. Denied authenticated attempts
produce COMPLIANCE_TRACE_DENIED. `X-Request-ID` links the response to stored
audit metadata. Audit rows remain append-only; no application role receives
UPDATE or DELETE. Historical events are not rewritten.

FORMAL_LEGAL_EXPORT_WORKFLOW_NOT_IMPLEMENTED. Lawful requests require an
authorized internal Compliance process; this primitive does not grant outside
authorities direct database access or create a formal legal export process.

Open Q&A keeps its optional internal Patient FK for account-originated content.
Legacy/imported anonymous questions may lack an account by design. Doctor APIs
return only public question/answer fields, and the API DB role cannot SELECT
the internal author link. Known author linkage is retained with ON DELETE
RESTRICT; no identity is fabricated for anonymous historical questions.

The table-by-table ownership and retention inventory is in
[USER_OWNERSHIP_INVENTORY.md](USER_OWNERSHIP_INVENTORY.md).

## Doctor Flutter Q&A

The canonical structure is prototype/index.html `doc-view-5`: its dedicated
flow header, filter strip and scrollable question containers are preserved.
The prototype's obsolete regulator label is not restored. The active app keeps
KMPDC terminology.

Reproduction found an unconstrained registration badge beside a single-line
title. Long registrations and enlarged text caused RenderFlex overflow.
The second Queue entry point also opened a height-limited default bottom sheet.
Both entry points now use one full-height, SafeArea-aware sheet. The header
wraps within available width; cards grow with content; the publish label wraps;
the list remains scrollable above the keyboard. Q&A-only emoji artwork is
replaced with existing Material icons/plain labels.

Q&A refresh now fetches questions independently of unrelated wallet/queue
requests. A stable question-container key preserves editor state when a retry/error banner changes list indices. A 409 closes a stale editor even if its subsequent refresh fails, and offers a safe retry. A 409 refreshes the authoritative answered state; its message uses
the surviving ScaffoldMessenger even when the open card is removed. The
backend answered list already exists and is requested with `status=ALL`.
Successful publication also closes the editor before refresh, so a temporary
refresh failure cannot reopen the same answer for duplicate submission.
Loading, empty, safe retry, offline cache and answered states have permanent
widget/golden coverage. Raw exception text is not displayed.

Golden files under `apps/mobile/test/goldens/qna-*.png` are Flutter widget
renders using the existing bundled font and icons, not device screenshots.
Responsive checks cover 360, 390, 412 and 480 logical pixels and doubled text
scale; keyboard tests exercise the real sheet route at the three phone widths.
No emulator or new screenshot dependency is installed. Real-device inspection
must be reported separately when an Android device is connected.

## Verification evidence

Current commands, counts, migration agreement and device availability are
recorded in `services/backend/target/user-isolation/` and the final evidence
JSON under `docs/audit_evidence/`. No host software or infrastructure was
installed for this correction.

The required `dart format --set-exit-if-changed .` encountered an existing
Windows directory-walker limit in deep generated Gradle output. The verification
wrapper temporarily moved only `apps/mobile/build` to a checked location inside
the repository, ran the unchanged format command against the source tree, and
restored the generated output in `finally`. Nothing was deleted or installed.
Flutter analysis and tests ran with `--no-pub`; Rust dependency checks used
`--offline`. Integration tests ran serially without a competing suite or backend
startup. Existing PostgreSQL/Redis containers were already running.

Live preflight found one Doctor account with an extra Patient profile matching
all old OTP-bootstrap defaults and zero dependent records. A guarded maintenance
transaction removed exactly that unused artifact and inserted a
CANONICAL_IDENTITY_REPAIR audit event, explicitly attributed to database-owner
repository maintenance rather than a human session. The Doctor account,
professional profile and histories were preserved. The migration itself still
refuses unexplained mismatches and never performs blanket role/profile repair.

## Final verification result (2026-10-08)

Sequential Rust fmt/clippy/library/API checks passed: **50/50 library, 139/139 API**.
Dart format, Flutter analysis and **98/98 Flutter tests** passed, including 17 Q&A layout/state/golden tests.
The backend restarted normally and applied migration **20261026000001**; source, running binary and database agree.
All 34 applied UP checksums match source; 62 saved historical migration files remain unchanged.
Live synthetic Patient/Doctor/Platform login, private-profile isolation, canonical trace linkage, denied trace access, and session/request audit metadata passed.
Exactly one unused legacy bootstrap profile was removed as described above; zero live role/profile mismatches remain.

No Android device is connected. The installed phone APK was not replaced; actual phone rendering remains pending.
Goldens are widget captures, including [unanswered](../apps/mobile/test/goldens/qna-unanswered.png) and [keyboard editor](../apps/mobile/test/goldens/qna-editor-keyboard.png).

[Machine-readable verification evidence](audit_evidence/user-isolation-qna.json). These are defined regression checks, not a penetration test.

```text
CANONICAL_USER_IDENTITY = users.id
PATIENT_USER_MAPPING = ENFORCED
DOCTOR_USER_MAPPING = ENFORCED
PATIENT_DOCTOR_PROFILE_MIXUP = PREVENTED
SENSITIVE_TABLE_OWNERSHIP_INVENTORY = COMPLETE
OWNERSHIP_CHAIN_GAPS = 0
FORCE_RLS_SENSITIVE_TABLES = VERIFIED
CROSS_PATIENT_DB_ISOLATION = PASS
CROSS_DOCTOR_DB_ISOLATION = PASS
CROSS_DOCTOR_PATIENT_CONTEXT = PASS
CONNECTION_POOL_IDENTITY_LEAK = PREVENTED
AUTHORIZED_TRACE_LOOKUP = VERIFIED
TRACE_LOOKUP_AUDITED = PASS
UNRESTRICTED_ADMIN_BACKDOOR = NO
LEGAL_AUTHORITY_DIRECT_DB_ACCESS = NO
FORMAL_LEGAL_EXPORT_WORKFLOW = NOT_IMPLEMENTED
QNA_RENDERING_ROOT_CAUSE = Unconstrained registration badge/title, height-limited second sheet, and editor state lost when retry banners changed list indices
QNA_GENERIC_HEADER_MISUSE = NOT_APPLICABLE
QNA_HORIZONTAL_OVERFLOW = PASS
QNA_VERTICAL_OVERFLOW = PASS
QNA_KEYBOARD_LAYOUT = PASS
QNA_LONG_QUESTION = PASS
QNA_409_RENDER_STATE = PASS
QNA_ANSWERED_LIST = WIRED
QNA_REAL_DEVICE_RENDER = PENDING_NO_DEVICE
FLUTTER_ANALYZE = PASS
FLUTTER_TESTS = 98/98
RUST_LIB = 50/50
API_TESTS = 139/139
MIGRATION_SCHEMA_VERIFIED = YES
HOST_SOFTWARE_INSTALLED = NONE

TASK_A = USER_LEVEL_DB_ISOLATION_VERIFIED
TASK_B = DOCTOR_QNA_CODE_FIXED_REAL_DEVICE_PENDING
```
