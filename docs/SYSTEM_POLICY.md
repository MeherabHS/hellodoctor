# AfyaMkononi (HelloDoctor) — System Policy

The engineering and product rules every change to this repository must follow. They apply to
everyone who works on the code, whatever tools they use. Where a rule was set or changed by the
product owner, the date is given.

Related: [PROGRESS.md](PROGRESS.md) (current state), [CHECKPOINT.md](CHECKPOINT.md) (resume
point), [assurance/SECURITY_STATUS.md](assurance/SECURITY_STATUS.md) (security posture),
[OPEN_QUESTIONS.md](OPEN_QUESTIONS.md) (undecided items and their safe defaults).

---

## 1. Technology

| Part | Technology |
|---|---|
| Mobile apps | Flutter (Dart): Patient app `apps/mobile_patient`, Care app (Doctor and Peer Navigator) `apps/mobile`; iOS, Android and tablet |
| Backend | Rust: Axum, Tokio, SQLx, Tower; REST and WebSocket |
| Database | PostgreSQL 16+: normalized schema, connection pooling, ACID transactions, FORCE row-level security |
| Web | Next.js: Admin Web `apps/admin-web`, Doctor Web `apps/doctor-web` |
| Video and audio | Agora RTC only (`agora_rtc_engine` in Flutter; tokens minted by the Rust backend) |
| Clinical chat | HTTP with adaptive polling and read receipts |

**Boundaries.**
- Mobile apps are Flutter. No web wrappers (Capacitor, Cordova, WebView shells).
- The backend is Rust with Axum and SQLx. The database is PostgreSQL.
- All live video and audio uses Agora. No custom WebRTC signalling, SDP exchange, ICE routing,
  STUN/TURN, coturn, LiveKit or self-hosted SFU.

## 2. Sources of truth for the user interface

| Surface | Source of truth |
|---|---|
| Patient app | The reference image [`design/afyamkononi-patient-ui-reference.jpeg`](design/afyamkononi-patient-ui-reference.jpeg), pixel for pixel (2026-11-12). Its 11 panels: Home, Prevention & Education, Confidential HIV Support Chat, Peer Navigator, Reminders, Health Records, Facilities & Services, Video Consultation, Dashboard for GHI, Multi-channel Access, Privacy & Confidentiality. Bottom navigation: Home, Services, Messages, Profile. Where it and the old `index.html` prototype disagree, the image wins. |
| Care app (Doctor) | The 11-panel board [`design/afyamkononi-doctor-ui-reference-11-panel.png`](design/afyamkononi-doctor-ui-reference-11-panel.png), pixel for pixel (2026-11-13): Login, Home Dashboard, Appointment Queue, Patient Details, Waiting Room, Video Consultation, Clinical Notes & Outcome, Patient Records list and detail, Profile & Availability, Notifications; bottom navigation Home, Appointments, Patients, More. The 14-panel board adds Registration, Availability (10-minute slots) and Prescription. Plan: [`doctor-ui/`](doctor-ui/). |
| Admin Web | The AfyaMkononi design language, adapted to a desktop console, from the five boards in [`design/admin/`](design/admin/) with the decisions in [`admin-ui/ADMIN_REDESIGN_PLAN.md`](admin-ui/ADMIN_REDESIGN_PLAN.md) (2026-11-13). |
| Doctor Web | The AfyaMkononi design language from the six proposal boards in [`design/doctor-web/`](design/doctor-web/), with the decisions in [`doctor-web-ui/DOCTOR_WEB_REDESIGN_PLAN.md`](doctor-web-ui/DOCTOR_WEB_REDESIGN_PLAN.md) (2026-11-14). Proposals may be polished and simplified. |
| Everything else | The `index.html` prototype and the specification in `docs/`. |

**Rules for the reference images.**
- Reproduce them exactly. Do not redesign, simplify or reinterpret.
- Read exact values from the image: sample colours from pixels, measure spacing against the
  9:41 status-bar frame. If a value cannot be read reliably, ask rather than guess. Show a
  screenshot of each screen next to its panel for approval before calling it done.
- The images' artwork was made for this project and may be extracted, cropped, adapted and
  recreated. Text, buttons, cards and navigation are always native widgets, never pasted
  screenshots. Never present an upscaled extraction as an original asset; never invent
  institutional logos or imply endorsements.
- App headers use the app's own logo (`assets/brand/afya_logo.webp`), not the boards' logos.
- Sample content for features without a backend may appear only in a clearly labelled
  design-preview environment. The signed-in app shows real data or honest empty or
  "coming soon" states.
- A redesign changes presentation only. Auth, booking, join-window gating, API clients, BFF
  routes, localization wiring, workflows and tests keep their behaviour. Screens not in a
  reference are restyled in its design language.

## 3. Product rules that must never be broken

1. **No star ratings or vanity reviews.** No ratings, reviews or popularity ranking. Doctors
   are presented by manually reviewed KMPDC registration, years of experience and verified
   hospital affiliations. Dissatisfaction goes through the Clinical Grievance Redressal
   System.
2. **Doctor earnings: gross, 20 % platform charge, net.** Always computed and shown in that
   order. The note "Total calculation is based including the platform charge 20%" appears on
   every earnings view except Doctor Web's Earnings page and Statement (2026-11-14).
3. **Prescription and clinical images** (2026-11-18).
   - A patient uploads 1 to 20 images per consultation (`PATIENT_MAX_IMAGES_PER_CONSULTATION =
     20`), counted per consultation only; extra selections are rejected or capped with a
     notice.
   - Every clinical image (patient intake, doctor Rx, chat, peer chat, verification) is
     compressed toward its own 600 KB ceiling (`MAX_PRESCRIPTION_IMAGE_BYTES = 600 * 1024`),
     never a shared budget.
   - Profile photos have their own 40 KB ceiling (`MAX_PROFILE_PICTURE_BYTES`).
   - Phones resize photos to at most 2000 px before upload; the server is the size authority.
   - The doctor's own Rx cap of 5 pages per appointment is a separate limit.
4. **Patient Home** (2026-11-12, 2026-11-13, 2026-11-17).
   - Layout: header, greeting, Upcoming Consultation card, hero banner, 3 × 3 grid of nine
     modules, bottom navigation. All nine modules fit without scrolling on ordinary phones. No
     emergency banner and no search field.
   - The Upcoming Consultation card appears 15 minutes before the start, using the server's
     join information (`join.join_open_at`, `server_time`), never the device clock alone. It
     stays while the consultation is joinable or in progress and disappears when it is
     completed, cancelled or the server's window has closed. Its button opens the waiting
     room; joining stays gated by the server.
   - Offline, Home replaces the hero with the offline banner, greys out modules that need a
     connection and marks Reminders and Health Records as available offline.
   - An emergency contact is never labelled "24/7". The number is configurable per country.
5. **Languages.** English and Kiswahili, switchable instantly. Deployment scope is Kenya only.
6. **Consultation outcomes.** Completion does not require a prescription: COMPLETED_WITH_RX,
   COMPLETED_NO_RX, REFERRED, ESCALATED.
7. **Admins publish directly** (2026-11-14). No submit, approve or return step in the Admin UI
   for articles, videos, quizzes, quiz revisions or notification campaigns. The backend still
   records its internal submit → approve steps for the audit trail; Admin Web runs them
   automatically (`apps/admin-web/src/lib/publishFlow.ts`). The `sensitive` flag is a label
   only. Doctor verification and grievance decisions are unaffected.
8. **No unilateral features.** A behaviour not in a reference, the prototype or the
   specification is not invented. Use [OPEN_QUESTIONS.md](OPEN_QUESTIONS.md) or its documented
   safe defaults, or ask the product owner.

## 4. Security and privacy

- Passwords are hashed with Argon2id; never stored in plain text.
- No secrets, API keys or tokens in source code or client apps. The Agora App Certificate
  exists only on the backend.
- Never log protected health information, patient complaints or raw national identity
  credentials. Push notification payloads never contain health information.
- Medical images and prescriptions are never kept in generic disk caches; they use encrypted,
  lifecycle-controlled storage cleaned up with the session.
- **Admin never sees patient medical conditions**: no column, chip, filter, tile or detail
  field, whatever a reference image shows. Admin never sees a peer request's topic or peer
  chat content; peer data reaches Admin only as times, statuses and counts.
- **Consultation trace:** kept 5 days after each event (configurable 3 to 5,
  `CONSULTATION_TRACE_RETENTION_DAYS`), then deleted automatically, unless a grievance places
  a preservation hold. It never deletes appointments, sessions, chat messages, prescriptions,
  documents, payments, grievances, adjudications or the audit log. See
  [CONSULTATION_TRACE_AND_GRIEVANCE_ARCHITECTURE.md](CONSULTATION_TRACE_AND_GRIEVANCE_ARCHITECTURE.md).
- Sensitive Admin actions require a fresh password confirmation (step-up).

## 5. Database and migrations

- Every migration has a matching down-migration. Production columns are never dropped without
  a deprecation window.
- `sqlx::migrate!` applies migrations only when the backend starts. After a migration is added
  or changed, restart the running backend before trusting it (this caused a 14-hour outage of
  doctor registration in October 2026).
- A green `cargo test` does not prove a long-running database is current; each test gets its
  own freshly migrated database. Before reporting a backend fix against a shared database, run
  `services/backend/scripts/verify_backend_schema.ps1`.
- Never insert or edit rows in `_sqlx_migrations` by hand, and never run pieces of a
  migration's SQL manually. Always go through `sqlx migrate run` or the normal start-up path.

## 6. Workflow

Work feature by feature following [IMPLEMENTATION_ROADMAP.md](IMPLEMENTATION_ROADMAP.md);
record progress in [PROGRESS.md](PROGRESS.md). Read the relevant documents in `docs/` before
architectural or structural decisions.

```
read spec → inspect reference → pick task → implement → test → fix → audit → commit → update progress
```

**Rust backend** (`services/backend`)
- Format: `cargo fmt`
- Lint (the CI gate): `cargo clippy --all-targets -- -D warnings`
- Migrations: `sqlx migrate run`
- Tests: `cargo test` (the database suite runs single-threaded in CI:
  `cargo test --test api_tests -- --test-threads=1`)
- Run: `cargo run`

**Flutter**
- Code generation: `dart run build_runner build --delete-conflicting-outputs`
- Integration tests: `flutter test integration_test/app_test.dart`

**Commits and pull requests** carry no AI-tool attribution trailers or footers.

## 7. Definition of done

Nothing is reported complete while tests fail or the code does not compile. Before marking a
milestone or task complete in [PROGRESS.md](PROGRESS.md):

- [ ] Compiles with zero errors and no unhandled warnings.
- [ ] Unit, service and integration tests are written and pass.
- [ ] Form input is validated on both the client and the server.
- [ ] Database entities and migrations match [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md).
- [ ] The UI matches its source of truth (section 2).
- [ ] [ACCEPTANCE_CRITERIA.md](ACCEPTANCE_CRITERIA.md) is satisfied.
- [ ] No credentials, tokens or personal data in logs or test fixtures.
