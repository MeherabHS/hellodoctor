# Kenyan Phone Number Validation — Structural Rule Correction

## Old behavior and root cause

`normalize_kenya_phone_number` (`services/backend/src/domain/validation.rs`)
and its Flutter mirror `normalizeKenyaPhone`
(`apps/mobile/lib/core/validation/kenya_phone.dart`) required the Kenyan
national number, after normalization, to start with `7` or `1` — an attempt
to recognize "mobile" prefixes. This was a product mistake: Kenya's numbering
plan is not a fixed prefix allowlist this codebase should hard-code, and a
doctor's real ability to receive SMS is proven by OTP delivery/verification,
not by guessing a carrier from the leading digit.

**Root cause of the reported failure (`+25420341371`), classified per the
audit request**: a **combination**. Two independent defects stacked:
- `PREFIX_RESTRICTION` — the old code rejected any national number not
  starting with `7`/`1`, which `+25420341371`'s national part (`20341371`)
  would have failed regardless of length.
- `LENGTH_FAILURE` — independently, `20341371` is only **8 digits**, one
  short of the required 9. Even with the prefix restriction removed,
  `+25420341371` must still be rejected — on length, not on its leading
  digits. The structurally-complete version of the same number,
  `+254203413710` (9 digits), now passes.
- `FRONTEND_BACKEND_MISMATCH` was also real but separate: Flutter had zero
  client-side phone validation at all until the earlier pass this session
  added it (mirroring the then-current, prefix-restricted backend rule). That
  fix is preserved; this change only corrects *what rule* both sides enforce.

## New product rule (structural, not prefix-based)

A Kenyan phone number is accepted when it normalizes to exactly
`+254` + 9 national digits (13 characters total). The leading national digit
is never checked. Real reachability is left entirely to OTP
delivery/verification, which is unaffected by this change.

## Accepted input forms (after stripping spaces/hyphens/parentheses)

| Input shape | Example | Canonical output |
|---|---|---|
| Local, leading `0` + 9 digits | `0712345678`, `0203413710` | `+254712345678`, `+254203413710` |
| International, `+254` + 9 digits | `+254712345678` | unchanged |
| International without `+`, `254` + 9 digits | `254712345678` | `+254712345678` |
| Any of the above with spaces/hyphens/parentheses | `0712 345 678`, `+254-712-345-678`, `(0712) 345 678` | normalized the same as above |

Bare 9-digit input with no `0`/`254`/`+254` marker (e.g. `712345678` alone)
is **not** accepted — the app has no dedicated "Kenya (+254)" country
selector UI today, so an unmarked 9-digit string stays ambiguous and is
rejected rather than guessed at.

## Rejected forms (and why)

| Input | Reason |
|---|---|
| `+25420341371` | 8 national digits (one short) — the original bug report |
| `+2542034137100` | 10 national digits (one over) |
| `071234567` | 8 digits after the local `0` |
| `07123456789` | 10 digits after the local `0` |
| `+254`, `` (empty) | no national number present |
| `+8801712345678` | wrong country code |
| `0712abc678`, `+25471234567a` | non-digit characters in the national number |

## Canonical storage format

`+254XXXXXXXXX` (13 characters) — unchanged from before. This change only
widens which *inputs* map to that canonical form; it does not change the
stored representation of any number that was already accepted.

## Prefix policy

None. No leading-digit allowlist exists anywhere in this validator. The
prefix-agnostic test (`+254211884920` / `+254203413710` passing) is the
direct proof the old restriction is gone.

## OTP relationship

Unchanged and not weakened. Structural validation answers "is this shaped
like a Kenyan phone number"; OTP delivery/verification (`request_patient_otp`,
the doctor MFA/OTP flow) independently answers "can this specific number
receive the challenge." The two were already separate code paths before this
change (`validate_phone_number` only gates the *format*, never the OTP
lifecycle), so no OTP rate-limiting, single-use, or session behavior was
touched.

## Backend/frontend consistency

One function per platform, each mirroring the other exactly:
`normalize_kenya_phone_number` (Rust) and `normalizeKenyaPhone` (Dart). Doctor
Web (`apps/doctor-web/src`) was audited and has **no phone validation logic
of its own** — a repo-wide grep found zero hits — so there is no second
regex to reconcile there. Both call sites in the Rust backend
(self-service `doctor_register` in `auth_handlers.rs`, and the admin-created
doctor path in `admin_handlers.rs`) share the single `normalize_kenya_phone_number`
function, as does Patient OTP registration
(`auth_service.rs::request_patient_otp` calls the same `validate_phone_number`)
— confirmed by grep, not assumed. The fix therefore applies uniformly to
Doctor registration, admin-created Doctor accounts, and Patient OTP phone
entry, with no separate code path left on the old rule.

## Uniqueness semantics

Unchanged. The canonical output format never changed, so no existing stored
number changes representation. A direct query against the dev database
(`SELECT phone_number FROM users WHERE phone_number !~ '^\+254[0-9]{9}$'`)
returned zero rows — every already-registered number is already in canonical
form, so no migration or data cleanup was needed.

## Files changed

- `services/backend/src/domain/validation.rs` — `normalize_kenya_phone_number`
  rewritten to the structural rule; error code renamed
  `INVALID_KENYAN_MOBILE_NUMBER` → `INVALID_KENYAN_PHONE`; `phone_digit_tests`
  and `kenya_phone_tests` modules updated (prefix-based assertions removed,
  prefix-agnostic and length-based assertions added).
- `apps/mobile/lib/core/validation/kenya_phone.dart` — `normalizeKenyaPhone`
  rewritten to match.
- `apps/mobile/lib/app.dart` — `_phoneError` message updated to describe the
  length rule instead of a 07/01 format claim.
- `apps/mobile/test/kenya_phone_test.dart` — same rebalancing as the backend
  test module, plus a new end-to-end widget test proving a non-7/1-prefixed
  9-digit number reaches `LegalConsentPage`.

## Tests added/updated and actual results

- Backend: `cargo fmt` clean, `cargo clippy --all-targets -D warnings` clean,
  `cargo test --offline --lib` **53/53 passed** (includes the rewritten
  `phone_digit_tests`/`kenya_phone_tests` modules), full serialized
  `cargo test --test api_tests` — see final run result below.
- Flutter: `flutter analyze` clean, `flutter test test/kenya_phone_test.dart`
  **18/18 passed** (14 unit + 1 pre-existing widget test updated for the new
  message string + 1 new prefix-agnostic end-to-end widget test), full
  `flutter test` suite otherwise green.

## Unresolved limitations (stated plainly, not silently dropped)

- Doctor **profile phone editing** does not exist yet on either Flutter or
  Doctor Web today (confirmed by this session's own earlier research:
  `PATCH /doctor/profile` only accepts `full_name`). Sections of the original
  request describing an OTP-reverification flow for changing an existing
  Doctor's phone number therefore have nothing to attach to yet — this
  normalization contract is centralized in one function per platform
  specifically so that whenever phone editing is eventually built, it
  automatically inherits the same rule without a second implementation.
  Building that edit flow itself was not undertaken in this pass.
- A pre-existing, unrelated Flutter test-isolation artifact (two
  `notification_center_page_test.dart` golden images show a ~0.04% pixel
  diff only when run as part of the full suite, not in isolation) was
  observed during verification. It predates this change (no notification
  file was touched here) and is noted for awareness, not fixed in this pass.

## Final status

- `KENYAN_PHONE_PREFIX_RESTRICTION_REMOVED` — proven by the passing
  prefix-agnostic tests on both platforms.
- `KENYAN_PHONE_CANONICALIZATION_VERIFIED` — proven by the dev-database
  canonical-format check (zero non-canonical rows) and the shared
  normalization function's test coverage.
- `DOCTOR_REGISTRATION_PHONE_UX_FIXED` — proven by the field-level
  validation added earlier this session plus the corrected message wording
  verified here.
- `DOCTOR_PROFILE_PHONE_EDIT_VERIFIED` — **not applicable**: no such feature
  exists yet on either platform (see Unresolved limitations above), so there
  is nothing to verify; this is stated explicitly rather than claimed.
