# Doctor UI — Redesign Plan

Date: 2026-11-13. Status: **proposed, not started.**

Target: the Doctor App reference board `docs/design/afyamkononi-doctor-ui-reference-11-panel.png`, reproduced pixel for pixel by product-owner instruction of 2026-11-13 (recorded in `CLAUDE.md` rule 2). Panels 12–14 come from the 14-panel board. Screens with no panel follow the same design language.

Principles: build reference-matched screens first, with components and tokens created as each screen needs them; presentation only, logic preserved (component map, section 2); real data or honest empty states in the real app; sample content only in the design preview.

## 1. Screen-by-screen plan

| Panel | Target screen | Current source | Change | Data and backend | Main risk |
|---|---|---|---|---|---|
| 1 | Login | `app.dart` `LoginPage` | Logo, "Doctor App", scene illustration, "Welcome, Doctor", icon fields, password eye, "Sign In", 2FA note. Keep the code step, forgot password and register link (not drawn; styled to match) | Existing auth endpoints | Breaking `auth_state` and `auth_security` tests; development auto-code path |
| 2 | Home dashboard | new; replaces top of `QueuePage` | Logo bar with bell and avatar; greeting card; four counters; three quick actions | Counters derived client-side from the appointments list and history. "Patient Records" quick action depends on decision 1 | Counter accuracy with paged history; keep duty toggle somewhere visible |
| 3 | Appointments | `QueuePage`, `ConsultationHistoryPage` | Today / Upcoming / Completed tabs; card with avatar, time range, modality, status pill | `GET /doctor/appointments` (`view=all`, `view=history`) | Avatars: patients have no photos; use initials. Chat appointments must remain reachable |
| 4 | Patient details | `EncounterHistoryPage`, `HistoryPage`, `AppointmentCard` | Header, Overview / History / Files tabs, info rows, "Open Consultation" | Name, modality, time, reason, intake notes, files: real. Patient ID, sex, age, "Key Information": **not available** | Showing fields the server does not return |
| 5 | Waiting room | new, in front of `AgoraPage` | Illustration, "Patient is waiting", patient card, hint, "Start Consultation" | `patient_present` exists only for an active session; no join time. Honest state: "Start Consultation" enabled by the join window, presence shown only when known | Must not weaken join gating |
| 6 | Video consultation | `agora_page.dart` | Title with timer, remote video, local preview, five controls, Chat / Notes / Checklist | Video and controls: real. In-call chat could reuse the appointment conversation where one exists. Notes and checklist: no backend. Screen share: not implemented | Call lifecycle regressions; overlaying widgets on platform video views |
| 7 | Consultation notes and outcome | `ConsultationPage` (unreachable) | Form to the panel | Outcome codes and no-Rx reason: real. Notes, diagnosis, plan, follow-up: **no backend** | Collecting clinical text that is then discarded would be unsafe; fields must be disabled or absent until a backend exists |
| 8 | Patients list | new | Search and rows | **No backend** | Honest empty state in the real app |
| 9 | Patient record | new | Summary / Visits / Results / Files | **No backend** beyond per-appointment history | Same |
| 10 | My Profile | `SettingsHomePage`, `ProfilePage`, `SecuritySettingsPage` | Header card with verified badge; four rows | Profile and verification: real. "Consultation Settings": none. Security: logout only | Verified badge only when status is `VERIFIED` |
| 11 | Notifications | `NotificationCenterPage` | Filter chips; rows with icon tile and unread dot | Real list; filter can be client-side by notification type | Golden tests |
| 12 | Registration | `RegistrationPage`, `LegalConsentPage` | Three-step look: account, professional details, under review | Same single request; all ten fields and the legal agreements stay | Dropping a required field; legal consent is not drawn but is mandatory |
| 13 | Availability | `SchedulePage` | Day chips, hours, slot list with Available / Booked / Blocked legend | Per-date slots only. No weekly template, no stored duration, no modality switch | Bulk creation must keep reporting partial failures |
| 14 | Prescription | `ScannerPage` | Panel shows structured medicines; app has photographed pages | **Structured Rx has no backend.** Keep page capture, restyled; "Prescription Submitted" only after confirmed upload | Presenting typed medicines that are never stored |
| none | Chat list and thread, Q&A, Articles, Wallet, notification settings, password reset, splash, gate, document viewers | existing | Restyle in the board's language | Existing | Placement in four-tab navigation (decision 2) |

## 2. Batches

| Batch | Contents | Backend | Depends on |
|---|---|---|---|
| A | Splash, login and code step, password reset, registration, legal agreements, submitted, pending and rejected gate | none | Logo and login scene artwork |
| B | Four-tab shell, Home, Appointments, Availability | none | Decision 2 and 4 |
| C | Patient details, waiting room, call chrome, chat thread | none | Waiting-room artwork |
| D | Consultation outcome, prescription, history and document screens; patient list and record as honest states | none for the restyle; new work if notes or records are approved | Decisions 1 and 3 |
| E | Notifications, profile, settings, security, Q&A, articles, wallet | none | — |
| F | Role choice, Peer registration form and pending state in the design preview only; role-aware shell once the backend exists | Peer steps 2–5 | Peer decisions |
| G | Peer requests, conversations, calls, notifications | Peer steps 6–8 | Batch F and backend |

Each batch: analyzer, tests, emulator captures beside the panel at 411 dp, then 360 dp, Swahili and text scale 1.3, and a behaviour check of anything it touched. No release APK.

Localisation: almost all current copy is hardcoded English. Move each rebuilt screen's strings into the `.arb` files as it is built; Swahili text needs a native speaker's review.

## 3. Recommended first batch

**Batch A.** It has complete references (panels 1 and 12), no backend dependency and no open product decision, it establishes the tokens, logo, buttons and fields every later screen reuses, and its risk is contained to presentation around an unchanged `AuthCubit`.

Before starting: create a `feature/doctor-peer-ui` branch as the spec asks. The working tree currently holds uncommitted Patient work on `feature/patient-ui-replication`; how to separate the two is the product owner's call.
