# Patient UI correction verification ? 2026-10-14

This report covers responsive discovery, consultation actions, long text, and Patient Q&A. The earlier broad visual refactor remains historical; this is a focused correction pass, not a claim that the whole product is production accepted.

## Findings and corrections

- Discovery forced three filters into equal-width columns; Availability and Consultation had empty callbacks. Filters now use content-sized, single-line controls in an intentionally horizontal scroll area with 48dp targets. Specialty, actual on-duty status, and consultation type all work. On-duty is not advertised as a guaranteed appointment slot.
- Directory offered video unconditionally. `supportsVideo` and `supportsChat` now follow the existing backend `DoctorQuery` rule: the corresponding fee must be positive. Neither mode is offered for zero/negative/malformed fees. Both-mode doctors open a choice sheet from Consult; one-mode doctors have a direct action. Narrow cards stack the actions instead of truncating them. Prices are shown for each offered modality.
- Chat is a paid appointment with a schedule slot in the existing contract. Discovery, profile, and the choice sheet therefore use **Book chat**, leading into the existing paid chat explanation, slot picker and payment flow. No instant chat semantics were introduced.
- Doctor profile names are centered with an intentional two-line limit; hospital names wrap and credentials are outside the hero. Redundant status metrics and duplicate fee rows were removed. Profile actions are full-width and conditional on the configured modes.
- The exact `47278ee148 Approve` defect comes from `docs/audit_evidence/kenya_professional_verification_live.py`, which generated `Dr. Synthetic Kenya Review {nonce} {label}` as the real `full_name`. The fixture generator now keeps the nonce and review action in email/license/evidence metadata. A narrowly anchored presentation mapper removes only the proven legacy suffix shape from existing fixtures; it does not edit real names or the database. Directory, profile, booking, appointment/vault DTOs and chat headers use this mapping.
- `WEB_FIXTURE` comes from the Doctor Web verification script's seeded Q&A category. The Patient repository excludes those fixture articles and the presentation also guards them. Other unrecognized topic metadata maps to General Health rather than exposing internal category strings. Synthetic development identities remain synthetic; no replacement doctor identities or clinical answers were invented.
- Q&A is now an editorial list with a prominent header action, anonymity/topic metadata, OPEN status, doctor attribution, a four-line answer preview, subtle dividers and a full-answer route. The nested mint answer panel and floating/bottom CTA were removed.
- The form is now a dedicated keyboard-safe route. It validates 1?4,000 characters, retains failed input, prevents duplicate in-flight submits, preserves dependent-profile selection, explains mandatory anonymity, and provides an authoritative success view. Submission state resets on every new entry, fixing repeat-entry failure.
- The old repository discarded the POST response and the GET feed exposed only answered questions. It now retains the server-created OPEN DTO immediately and reloads from the backend when returning to the feed. It never invents an answer or reports success before HTTP success.

## Necessary bounded API extension

The missing persisted owner-read capability was demonstrated in `list_answered_questions`. Existing `GET /patients/me/qna/questions` now accepts optional `include_own_open=true`. Its default still returns the public answered feed. Responses add `status` and the actual answering-doctor verification flag; unanswered answer fields are null.

Migration `20261105000001_patient_own_open_questions` adds an ID-only, context-bound SQL function. It returns OPEN question IDs for the authenticated Patient's self/guardian profiles only. No author column SELECT permission was granted; no author identity is returned to Patient or Doctor clients. No business rule or answer arbitration was changed. The down migration removes the helper.

## Long-text and fixture audit

Source audit covered doctor/name, hospital, specialty, price and CTA call sites throughout `lib/features/patient`. Primary actions do not use ellipsis. Names have two-line contracts where space is limited; hospital details wrap, prices can move onto a separate line, and booking/payment actions remain readable. Text previews, filenames and search hints may still intentionally truncate. Booking chat sheets scroll on smaller screens. Skeleton loading timers now cancel on disposal, a lifecycle issue caught by full-route widget checks.

This is not a claim that every legacy Patient page was physically verified. The automated visual matrix targets the routes/components changed in this correction pass.

## Evidence

- Flutter analysis: **0 issues**.
- Flutter suite: **171/171 passed**, including 27 new correction tests.
- Backend: **9 relevant tests passed**: two owner-read/guardian isolation regressions, six existing submission/validation/authorization/rate-limit/answered-flow checks, and the existing concurrent-answer 409 regression.
- Live local HTTP: email MFA login; question POST **201 OPEN**; two subsequent GETs preserve OPEN; no answer fabricated; author identity absent. See [live result](audit_evidence/patient_ui_correction/live_qna_result.json).
- Captured **26 Flutter-rendered PNGs** using the bundled font and Material icons. These use clearly test-only transport fixtures and **are not physical-device captures**. Representative images:
  - [Find Doctor, 320dp](audit_evidence/patient_ui_correction/find_doctor_route_320.png)
  - [Find Doctor, 412dp](audit_evidence/patient_ui_correction/find_doctor_route_412.png)
  - [Long-name Doctor Profile, 320dp](audit_evidence/patient_ui_correction/doctor_profile_route_320.png)
  - [Video only](audit_evidence/patient_ui_correction/doctor_video_320.png), [chat only](audit_evidence/patient_ui_correction/doctor_chat_320.png), [both](audit_evidence/patient_ui_correction/doctor_both_320.png)
  - [Consultation choices](audit_evidence/patient_ui_correction/consultation_choices_320.png)
  - [OPEN and answered Q&A](audit_evidence/patient_ui_correction/qna_320.png)
  - [Question form](audit_evidence/patient_ui_correction/question_form_320.png)
  - [Submission success](audit_evidence/patient_ui_correction/question_submitted_320.png)
- Width matrix: **320, 360, 390, 412dp**. Doctor components also checked at **1.5x text**; Swahili consultation/Q&A checked at 320dp and 1.5x. Full directory routes exercise working filters and profile navigation. No layout overflow exceptions in this matrix.
- `adb devices -l`: **no connected devices**. No emulator was started and no host software was installed.

Reproduce captures:

```powershell
cd apps/mobile_patient
flutter test test/patient_discovery_qna_correction_test.dart --dart-define=CAPTURE_PATIENT_UI=true
```

## Final status

| Requested status | Evidence-based result |
| --- | --- |
| PATIENT_DOCTOR_DISCOVERY_RESPONSIVE_VERIFIED | PASS ? rendered widget matrix; hardware acceptance pending |
| PATIENT_DOCTOR_CHAT_ACTION_VERIFIED | PASS ? both-mode choice and chat-only routing; existing paid booking flow retained |
| PATIENT_LONG_TEXT_LAYOUT_VERIFIED | PASS ? changed routes/components, width/text-scale matrix |
| PATIENT_QNA_VISUAL_REDESIGN_VERIFIED | PASS ? rendered editorial feed/form captures |
| PATIENT_QNA_OPEN_QUESTION_SUBMISSION_VERIFIED | PASS ? widget, repository, backend isolation and live HTTP evidence |
| PATIENT_FIXTURE_METADATA_NOT_EXPOSED | PASS ? known category/nonce/action shapes guarded; test source fixed |
| PATIENT_PHYSICAL_DEVICE_VISUALLY_VERIFIED | PENDING_NO_DEVICE ? no claim of physical verification |

## Device test build

ARM64 development release, version 1.0.1+2; current LAN API is `http://192.168.0.105:8080/api/v1`. This differs from the earlier .106 address. Use the same network with the backend running. Development OTP is explicitly enabled. This is a synthetic-data mobile test artifact, not production approval. Real payment/provider acceptance remains outside this correction pass.

## Consultation completion follow-up (2026-10-14)

The new device test build supersedes the discovery/Q&A APK above: Patient **1.0.1+3**, ARM64, approximately 20 MB. See the [device test guide](../releases/PATIENT_CONSULTATION_TESTING_20261014.md) and [consultation engineering record](PATIENT_CHAT_CONSULTATION_VERIFICATION.md).

Home and Services Chat entries now open the profile-scoped Active/Past list and paid booking flow. Booking shows authoritative pending/failed/confirmed payment and enables access only after server confirmation. Chat history can be reopened, receiving replies from either Doctor client. The chat expiry banner wraps at narrow widths; new localized copy removes emoji and unsupported permanent-retention/prescription promises. Doctor Flutter/Web actions use the paid modality and authorized Patient identity.

Rendered full-flow evidence covers 320/360/390/412dp: [chat details](audit_evidence/patient_consultation/paid_chat_details_320.png), [slot selection](audit_evidence/patient_consultation/chat_slot_booking_320.png), [payment pending](audit_evidence/patient_consultation/chat_payment_pending_320.png), [confirmed](audit_evidence/patient_consultation/chat_payment_confirmed_320.png), [open chat](audit_evidence/patient_consultation/patient_chat_320.png). These are 20 new UI captures. Additional [Patient](audit_evidence/patient_consultation/patient_flutter_shared_chat.png), [Doctor Flutter](audit_evidence/patient_consultation/doctor_flutter_shared_chat.png) and [Doctor Web](audit_evidence/patient_consultation/doctor_web_shared_chat.png) captures use the same live local backend conversation. They are rendered-widget/browser evidence, not physical-device screenshots.

Patient Flutter regression is **181/181 passed**, with zero skips and clean analysis. Test integrity and all other suite results are recorded in the focused consultation report. `PATIENT_CHAT_CONSULTATION_ACTION_IMPLEMENTED`, `PATIENT_CLINICAL_CHAT_SEND_RECEIVE_VERIFIED`, `PATIENT_CHAT_AUTHORIZATION_VERIFIED`, `PATIENT_CHAT_DUPLICATE_SUBMISSION_SAFE` and `PATIENT_CHAT_EXPIRATION_VERIFIED` have automated evidence. Payment activation is verified with the existing **signed development callback only**. `PATIENT_CHAT_PHYSICAL_DEVICE_VERIFIED` remains `PENDING_NO_CONNECTED_DEVICE`; live M-Pesa and Agora media remain unconfigured.
