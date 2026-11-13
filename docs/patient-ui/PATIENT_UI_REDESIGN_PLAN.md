# Patient UI — Redesign Plan (proposal, not started)

Date: 2026-11-12. This plan is for approval. No application source has been changed. It follows the batch order in the v2.1 brief (section 13A, "Implementation order update") and the constraints in `CLAUDE.md` rule 2.

Companion documents: `PATIENT_UI_DISCOVERY_REPORT.md` (findings and decisions), `PATIENT_UI_SCREEN_INVENTORY.md`, `PATIENT_UI_COMPONENT_MAP.md`, `PATIENT_UI_EMULATOR_WORKFLOW.md`.

---

## 1. How the presentation layer is replaced without touching logic

The boundary already exists in most features. It is kept exactly where it is.

| Layer | Files | Treatment |
|---|---|---|
| Transport, session, security | `core/network/*`, `core/cache/*`, `core/notifications/*`, `core/video/*`, `core/validation/*`, `core/config/*` | **Not edited** |
| Data | every `*_repository.dart`, `*_models.dart` | **Not edited** |
| State and rules | every `*_cubit.dart`, `*_controller.dart`, `booking/join_eligibility.dart`, `video/mint_failure.dart` | **Not edited** |
| App wiring | `app.dart` (`_AppState`, `_SignedInRoot`) | Not edited, except `PatientShell` and `PatientBottomNav` (tab list and labels) |
| Presentation | `*_page.dart`, `*_sheet.dart`, `*_modal.dart`, widgets, `core/theme/tokens.dart`, `core/widgets/*` | Rebuilt |

Rules for presentation edits:

1. A page keeps its constructor, the cubit it creates, the cubit methods it calls and the order of its navigation calls. Only the `build` output changes.
2. Four pages hold logic inside the widget state: `PatientChatListPage` (paging, Active/Past split), `ConsultationStatusPage` (hold countdown, payment launch), `HealthVaultPage` (MIME verification), `DataRightsPage` (direct API calls). Their state classes are restyled in place. Extracting that logic into cubits is a separate, optional refactor and is not part of a visual batch.
3. Widget types and keys that tests rely on are kept: `PatientShell`, `PatientBottomNav`, `PatientCard`, `PatientField` (with `.label`), `FilledButton` for primary actions, `DoctorDirectoryFilters`, `ArticleQuizSection`, `ArticleMarkdown`, `ClinicalPdfViewer`, `ProfilePhotoAvatar`, `Key('join-gate-notice')`, `Key('qna-question')`, `Key('qna-submit')`, `ValueKey('blogs-load-more')`.
4. Visible copy that tests assert on changes only together with the test, in the same batch, and each such change is listed in the batch report.
5. No screen adds a network request at boot without the two strict-request tests being extended (`auth_navigation_test`, `patient_navigation_safe_area_test`).
6. New modules with no backend (Peer Navigator, Reminders, Facilities, Videos, Audio) get the reference layout with an explicit unavailable state and no sample data.

---

## 2. Prerequisites before batch 1

| # | Item | Owner |
|---|---|---|
| P1 | Decisions D1–D15 in the discovery report, section I | Product owner |
| P2 | Reference designs for **Messages** and **Profile**, or written approval to derive them from the image's design language | Product owner |
| P3 | Brand assets: logo, wordmark, illustrations, with usage rights; or approval of marked placeholders | Product owner |
| P4 | A higher-resolution reference (the source file behind the JPEG), if one exists | Product owner |
| P5 | A second, compact AVD (360 dp wide). Only `Pixel_9` (411 dp) exists | Developer machine |
| P6 | Remove or relocate the stray Android Studio template project created at the repository root on 2026-11-12 (`app/`, `gradle/`, `build.gradle.kts`, and related files) | Product owner to confirm |
| P7 | "Before" screenshots: 31 captured (tabs, Account children, empty states, Swahili, text scale 1.3). Data-dependent screens are not captured because the local database is empty; capturing them needs seeded doctors, slots and an appointment. The 26 renders in `docs/audit_evidence/patient_ui_correction/` can serve as "before" images for directory, doctor profile and Q&A | Product owner to confirm |

---

## 3. Batches

Each batch ends with: `flutter analyze`, `flutter test`, an emulator run on both AVDs, before/after screenshots beside the reference crop, a tap-through of every changed control, and a short changed/remaining list. Work stops for approval after each batch.

### Batch 1 — Design system

- Extend `PatientTokens`: sampled palette, spacing scale, radii, type scale, tone surfaces (mint, info, urgent, amber).
- Restyle the existing shared widgets in place; add the new shared widgets listed in the component map (app bar, segmented tabs, service tile, list row, info card, hero card, async body).
- Move the 35 stray colour literals into tokens.
- Add an asset pipeline (`assets/brand/`, `assets/illustrations/`) with supplied or placeholder assets.
- Visible effect: colours and shapes shift across every screen; no layout changes yet.
- Tests at risk: none expected by type or copy. The font asset stays.

### Batch 2 — Home and bottom navigation

- Bottom navigation becomes Home, Services, Messages, Profile.
  - Messages hosts the chat list.
  - Blogs leaves the bar and is reached through the education module.
  - The fourth tab is relabelled Profile.
- Home is rebuilt to panel 1: brand header with bell and initials avatar, greeting card, hero banner, 3 × 3 service grid in the approved order.
- The Emergency Contact banner and the Upcoming Consultation card stay on Home in that order (`CLAUDE.md` rule 2.4), restyled, below the grid, unless D2 decides otherwise. The family-profile switcher stays next to the consultation card.
- All nine cards navigate: five to existing screens, four to explicit unavailable-state screens until their batches or backends exist.
- Tests to update: `patient_navigation_safe_area_test` (labels), and the `ClientErrorReporter` screen names in `PatientShell`. A new widget test pins the banner-above-card order, which has no test today.

### Batch 3 — Services directory and the nine module screens

| Module | Screen work | Data |
|---|---|---|
| Services tab | Directory of the nine modules; removes Medicine Delivery and Emergency Dispatch rows (D8) | none |
| HIV Prevention & Sexual Health | Panel 2: tabs Learn, Videos, Quizzes, Audio; six topic cards; article list beneath | Learn uses the existing article feed. Topic filtering needs a backend filter (D10); until then topic cards open the full list. Videos and Audio show unavailable states. Quizzes lists nothing standalone today (quizzes exist only inside admin articles) |
| HIV Support Chat | Panel 3 layout over the existing anonymous Q&A: category rows, "Ask a question (anonymous)", privacy card, existing feed | Existing Q&A endpoints. Category list updated client-side (free-text field on the server) |
| Book Consultation | Panel 8 landing ("Consult a Doctor") leading into the existing directory; the button opens discovery and booking, never a call | Existing |
| Peer Navigator | Panel 4 layout; "Start Chat" disabled with an explicit not-yet-available state | none exists |
| Reminders | Panel 5 layout with an empty state; no fake reminders | none exists |
| Health Records | Panel 6 visual treatment over the existing vault; only categories the backend has (prescriptions, uploads) are live | Existing vault |
| Videos & Quizzes | Shell that reuses the education Videos and Quizzes tabs | as above |
| Facilities & Services | Panel 7 layout with an empty state | none exists |
| More | Three rows: Profile Settings, Help & Support, Privacy Policies | Profile exists; legal documents endpoint exists; Help & Support has no backend (D11) |

### Batch 4 — Messages

- Inbox: conversation cards with avatar, doctor name, specialty, status, Active / Past, loading, empty and error states, all strings localized.
- Thread: bubbles, composer, expiry and locked bars, older-message paging, image viewer.
- Consultation status page and intake modal, which sit between the inbox and the thread.
- Last-message preview, timestamps and unread badges are **not** shown unless a patient conversations endpoint is approved and built separately (D5). Peer conversations are not shown.
- Tests to update: `consultation_flow_test`, `consultation_booking_test`, `pre_e2e_availability_payment_test`, `patient_chat_list_deferred_fee_test` where copy or button structure changes.

### Batch 5 — Profile

- Overview: avatar, name, account details, family profiles, settings rows, logout.
- Children: edit sheet, photo sheet, add-dependent sheet, language modal, notification settings, data rights (localized and styled), and the policy reader used by More.
- "Payment Methods" and "Report an Issue" rows either get real destinations that already exist (the payment-method sheet; the grievance flow needs a consultation, so it cannot be a general entry) or are removed (D12).
- A profile widget test is added first, since none exists.

### Batch 6 — Nested flows and refinements

- Doctor directory, doctor profile, booking page, waiting room, call screen chrome, post-consultation, grievance modal, notification center and appointment page, article reader and quiz.
- Sign-in, registration, OTP, password reset, profile completion, splash, startup error. These have no reference panel; they take the design language (D13).
- Localize the remaining hardcoded English strings.
- 360 dp and 411 dp, text scale 1.0 and 1.3, English and Swahili, keyboard-open passes.

### Batch 7 — Verification and hand-off

- Full regression checklist from the discovery report, section G, on the emulator.
- Patient ↔ Doctor mobile call regression on test accounts.
- Reference-to-implementation comparison matrix with screenshot pairs.
- Update the stale specs (`UI_DESIGN_SYSTEM.md`, `VISUAL_FIDELITY.md`, `SCREEN_INVENTORY.md`, `ASSET_SPEC.md`, `PROGRESS.md`).
- One signed APK, same applicationId and keystore, with its SHA-256.

---

## 4. Work that is not part of the visual redesign

Each needs its own contract, backend change, tests and approval. None is started by this plan.

| Item | Why it is separate |
|---|---|
| Peer Navigator role, approval, chat requests, calls | New role, tables, routes and authorisation |
| Patient-authored reminders | New local scheduling, optional sync, lock-screen privacy decision |
| Facilities directory | New Admin CRUD and patient endpoints |
| Videos and audio content | No content model exists |
| Article topic filter | Feed needs a category or tag parameter and a patient-readable category list |
| Patient conversations endpoint | Needed for message previews and unread counts |
| Help & Support | No endpoint |
| Two suspected client defects (password-reset OTP check; logout clearing language) | Outside presentation; need runtime confirmation first |
