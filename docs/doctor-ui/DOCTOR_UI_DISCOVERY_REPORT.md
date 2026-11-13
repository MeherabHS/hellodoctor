# Doctor UI — Discovery Report

Date: 2026-11-13. Branch at the time: `feature/patient-ui-replication` (Patient work uncommitted on it). Scope: `apps/mobile` (Doctor Flutter) and the Peer Navigator role. Discovery and documentation; the only code added is a debug-only preview entry point (section 8).

Companion documents in this folder: screen inventory, component map, emulator workflow, redesign plan, Peer Navigator gap analysis.

## 1. What was read

| Read in full | Not re-read |
|---|---|
| `CLAUDE.md`, `AGENTS.md`, `checkpoint.md`, `progress.md`, `apps/mobile/README.md` | The roughly 190 historical reports in `docs/` (audits, milestone and remediation reports). Their claims were not relied on; behaviour below was checked in source |
| The design specification `AfyaMkononi_Doctor_Peer_App_UI_Fable52_Emulator_Spec_v1.docx` (13 sections) and both embedded boards | |
| Doctor Flutter: `app.dart`, auth cubit, tokens, shared widgets, settings, the call screen, the Rx, schedule, queue, history, completion and profile code; copy and navigation extracted from the rest of `doctor_pages.dart` (wallet, chat, Q&A) | Wallet, chat-thread and Q&A internals line by line |
| Backend: role enum, doctor registration contract, verification migrations, slot creation, chat schema, RTC token service, prescription service, router | |

Nothing requested was missing. The "Doctor App flow diagrams" are the two boards inside the Word file; the first is byte-identical to the PNG supplied in chat.

## 2. Size and structure

55 Dart files, 15,823 lines, 79 widget classes, 35 screens (34 reachable). State: `flutter_bloc` cubits (`AuthCubit`, `DoctorCubit`, `NotificationCubit`) plus `setState` in pages. One repository per area (`DoctorRepository`, `NotificationRepository`, `ArticlesRepository`) over a single `ApiClient` (Dio). Navigation is imperative; `go_router` is declared but unused. 60 backend endpoints are called; all exist in the router.

Navigation: six bottom tabs — Queue, Chat, Scan Rx, Schedule, Wallet, Settings — behind a verification gate.

## 3. Registration and approval as built

1. "Register as Doctor" → one form of ten required fields including KMPDC number, specialty, experience, hospital, qualifications and address → legal agreements (four required documents) → `POST /auth/doctor/register`.
2. The account is created `PENDING`. The app shows "Registration submitted".
3. A pending doctor **can sign in** (password plus emailed code). The shell then shows the verification gate with status and any admin note; clinical endpoints refuse an unverified profile server-side (`verified_doctor_profile_id`).
4. An admin verifies or rejects in Admin Web's Verification Queue (`/admin/doctors/{id}/verify`, `/reject`, `/revoke`).

Gaps: the backend has a `REVOKED` status that the client does not know; it falls through to "Verification unavailable". There is no "suspended" wording. Rejected doctors see the admin note but no next step.

## 4. Scheduling and consultation as built

- Slots are individual date-and-time records. The server enforces: end after start, start in the future, no overlap with a non-cancelled slot (row-locked). **It does not enforce a duration.** Ten minutes is a client convention: "Create 10-Min Slots" posts six consecutive 10-minute slots one by one; "Add Slot" lets the doctor pick any start and end.
- Slot states shown: Open for Booking, Past opening, Upcoming, In Progress, Completed, Blocked. Booked slots cannot be cancelled by the doctor.
- There is no accept or decline step. A booked appointment is confirmed by booking.
- Joining video is gated by the server's join window (`/consultations/{id}/join-eligibility`); the button shows "Room opens at …" until then.
- Video: Agora through `CallController`; mute, camera, switch camera, end; reconnect and rejoin with a grace period; presence flags from the server.
- Chat consultations: full thread with image attachments, expiry, read-only after close.
- There is no doctor-side waiting-room screen.

## 5. Clinical documentation as built

| Capability | Doctor Flutter | Backend |
|---|---|---|
| Patient intake (chief complaint, notes, uploaded photos) | Read | Exists |
| Prior episodes for the same patient | Read, within the server's history window; detail shown as raw JSON | Exists |
| Prescription | Photographed or uploaded pages, max 5, plus optional advice text | Exists |
| Consultation outcome (with Rx, no Rx, referred, escalated) | **Screen exists but is unreachable** | Exists; Doctor Web's allow-list includes it |
| Doctor-authored notes, diagnosis, plan, follow-up | None | **None** |
| Patient list, longitudinal record, lab results | None | **None** |
| Structured e-prescription | None | **None** |

Doctor Flutter and Doctor Web are not equivalent; in particular, completion is not available on mobile today.

## 6. Notifications as built

Server notification types the app routes: booking confirmed, rescheduled and hold events, 15-minute reminder, consultation ready, rejoin, completed, interrupted, needs review, recovery resolved, prescription available, new chat message, article status. Delivery has three layers: in-app polling every minute, a background sync worker, and FCM. Tapping re-reads the target with an ownership check before navigating. Preferences cover language, four categories and quiet hours. There is no category filter in the list.

## 7. Current design system

`DoctorTokens`: primary `#059669`, dark `#047857`, light `#D1FAE5`, background `#F5F9F7`, text `#0F291E`; radius 16; Plus Jakarta Sans (bundled); Material 3. Shared widgets: `DoctorCard`, `StatusChip`, `DoctorPageHeader`, `NotificationBellAction`, `ErrorPanel`, `InlineError`, `CacheStatusBar`. **There are no image assets at all** — no logo, no illustrations; only the font. Localisation covers 46 strings (notifications, settings, cache bar); every other string is hardcoded English.

## 8. Emulator, analyzer and tests

- `flutter analyze`: no issues.
- `flutter test`: **324 passed, 10 failed.** All ten are golden-image comparisons (two notification-centre, eight Q&A) differing by 1.1%–4.9%. `test/flutter_test_config.dart` documents a 0.47%–5.94% drift between Windows and the Linux CI where the goldens were captured, with a 1% tolerance. The failures are consistent with that; this was not confirmed on Linux.
- Emulator: `Pixel_9` (1080 × 2424, 420 dpi, Android 17) was already running. The Doctor app builds and runs in debug.
- The local database has two patients, one admin, no doctor, no slots and no appointments, so no signed-in screen is reachable through real sign-in.
- At the product owner's instruction, sign-in was bypassed for UI viewing with a **debug-only preview entry point**, `apps/mobile/lib/dev/design_preview_main.dart`. It opens the signed-in shell with invented, labelled sample data, answers every request in-process, refuses writes, shows a "PREVIEW DATA" ribbon and will not start in a release build. `main.dart`, `app.dart` and the backend are unchanged.
- Baseline: 18 screenshots in `screenshots/baseline/` (list in the emulator workflow). Not captured: video call, OTP step, reset-code and new-password steps, legal agreements, notification settings, article editor, payout sheet, document viewers.

Defects seen in the baseline: history rows print raw ISO timestamps and a "?" where a separator character was lost ("VIDEO consultation ? 2026-11-10 19:39:15.671474Z ? COMPLETED"); the blocked-slot line has the same "?" ("Blocked ? unavailable…").

## 9. Peer Navigator

Not present in any backend, Admin Web, Doctor Web or Doctor Flutter file. Full analysis and permission matrix: `PEER_NAVIGATOR_ROLE_AND_API_GAP_ANALYSIS.md`.

## 10. References and artwork

Both boards are 1215 × 1295 pixels; one phone is about 190 pixels wide. That is enough to measure layout and sample colours, **not enough to extract artwork**. Needed as separate high-resolution files: the AfyaMkononi logo (the Patient app's `assets/brand/afya_logo.png` can be reused), the login scene (two clinicians and hospital), the waiting-room monitor illustration, patient and doctor portrait avatars, the review-pending shield, the prescription-submitted tick. Portrait avatars of real-looking patients should not be shown for real patients; initials or the patient's own photo policy applies.

## 11. Decisions needed before building

1. Panels 7, 8, 9 and 14 (clinical notes, patient list and record, structured Rx) have no backend. Proposed handling, as in the Patient app: build them to the board, sample content only in the design preview, honest empty states in the real app, until backend work is approved.
2. Where Chat, Scan Rx, Wallet, Q&A, Articles and the duty toggle live in a four-tab layout. Proposed: Chat inside Appointments and the consultation; the rest under More; duty toggle on Home.
3. Whether to wire the existing completion screen into the mobile flow (it restores a missing capability and is the natural home for panel 7).
4. Ten-minute slots: keep as a client convention or enforce on the server.
5. High-resolution artwork (section 10).
