# Peer Navigator — Plan

Date: 2026-11-14. Product-owner instruction, with the two "AfyaMkononi Peer Navigator
Workflow" boards (patient journey and peer journey) as the visual reference.

## What a Peer Navigator is

An adult who lives with, or has been affected by, HIV and is trained to support
adolescents and young people who are shy to talk to a doctor. Peer support is
non-clinical: no prescriptions, no referrals, no clinical records, no earnings.

## Decisions (product owner, 2026-11-14)

| Topic | Decision |
|---|---|
| Becoming a peer | Signs up in the Care app ("Register as Peer Navigator"); an Admin approves in Admin Web before patients can see or book them. No KMPDC. |
| Booking | Same as doctor booking (pick peer, slot, topic) plus a support type: Chat or Voice/Video. 10-minute slots, created by the peer like doctor slots. |
| After booking | No doctor-style confirmation. Patient sees "Support Request Sent ✓ — waiting for the peer to accept". |
| Acceptance | The peer accepts or declines. Only after acceptance does the chat open. Decline frees the slot and tells the patient. |
| Chat | Same chat experience as doctor chat, but never closes: no 24-hour lock. "Mark Session Complete" changes the status only; the conversation stays open. |
| Calls | Voice/video through Agora, reusing the doctor call screen and token minting, within the accepted slot. |
| Patient's chat list | One list showing doctor consultation chats and peer chats together, each labelled. |
| What the peer sees | Patient first name + initial (e.g. "Amina M."), age range, the topic they wrote, and the patient's saved profile photo if they set one (product owner, 2026-11-19). Never phone, email, address, records, prescriptions or doctor consultations. |
| Not for peers | Rx, referrals/escalations, 20% earnings, KMPDC verification, grievances against clinical care. |
| Payment | None (finance is deferred platform-wide). |

**Default to confirm:** a request the peer never answers stays pending until the
peer acts or the patient cancels it (the session answer was "never closes").

## Architecture

Peers get their **own role, tables and endpoints**, following the doctor
patterns, rather than being stored as a kind of doctor. Doctor rules (KMPDC,
earnings, Rx, 24-hour lock, clinical access windows, doctor directory) are
enforced across many queries and RLS policies; folding peers into
`doctor_profiles` would need a "not for peers" exception in each. Shared code is
reused as functions (slot generation and overlap checks, chat message
validation and attachments, Agora token minting) and the Flutter screens are
reused as widgets.

- **Role:** `PEER` added to `user_role_enum`; ABAC actions for peer endpoints.
- **Tables (one reversible migration set):** `peer_profiles` (display name,
  location, bio, approval status), `peer_slots` (10-minute, non-overlapping per
  peer), `peer_requests` (patient, peer, slot, support type, topic, status:
  PENDING → ACCEPTED / DECLINED / CANCELLED, completed_at), `peer_conversations`
  and `peer_messages` (no expiry), with RLS so a peer sees only their own
  requests and a patient only their own.
- **Calls:** Agora channel per accepted request; token minted only for the two
  participants, only during the slot window.
- **Notifications:** new request (peer), accepted / declined (patient), new
  message. No PHI and no topic text in push payloads.

## Phases

1. **Backend foundation** — PEER role, peer sign-up and Admin approval,
   peer profile, peer slots (create/list/cancel), peer directory for patients.
2. **Requests** — patient requests a slot (PENDING), peer accepts/declines,
   patient cancels; slot locking; notifications.
3. **Chat and calls** — peer conversations and messages (attachments as
   doctor chat), mark complete, Agora tokens for voice/video.
4. **Care app (peer side)** — wire the existing Peer screens to the API:
   Home counts, Requests (accept/decline), Availability & Slots (reusing the
   doctor slot screen), Chats, Support Session (start call, open chat, mark
   complete), real sign-up.
5. **Patient app** — Peer Navigator module: peer list, Book Peer Support
   (reusing doctor booking), Request Sent screen, combined chat list, peer chat,
   peer call.
6. **Admin Web** — Peer Navigators list and approval (same design as doctor
   verification, no KMPDC). No peer chat content shown to Admins.
7. **Tests, screenshots against the boards, docs/PROGRESS.md.**

Each phase ships with backend tests (`cargo test`), Flutter widget tests and
screenshots of the new screens next to the board panels.

## Progress

### Phase 1 — Backend foundation (done 2026-11-14)

- Migrations `20261222000001_peer_role` (adds `PEER` to `user_role_enum`; the
  down-migration cannot remove an enum value, as with `ADMIN`) and
  `20261222000002_peer_navigators` (`peer_profiles`, `peer_slots`, RLS, role and
  approval guards, `PEER_OTP` email type; reversible).
- Sign-in shared with doctors (`AuthService::care_login_step1` /
  `care_verify_otp_with_device`, `CareRole::{Doctor, Peer}`); doctor behaviour
  and jail keys unchanged. Peer sign-in email: `render_peer_login_otp`.
- Endpoints: `POST /auth/peer/register|login|verify-otp`; `GET|PATCH
  /peer/profile`; `GET|POST /peer/slots`, `DELETE /peer/slots/:id` (10-minute,
  approved peers only, no overlap); `GET /peers`, `GET /peers/:id/slots`
  (patients; display fields only); `GET /admin/peers`, `POST
  /admin/peers/:id/{approve|reject|suspend|reinstate}` (audited; reject and
  suspend need a note).
- Tests: `tests/cases/peer_navigators.rs` (2 tests).

**Open question:** peers have no legal documents of their own yet (the Care-app
sign-up shows a Terms / Privacy checkbox). Peer-specific terms need product and
legal text before acceptance can be recorded like the doctor agreements.

### Phase 1b — Peer legal documents (done 2026-11-14)

- Migrations `20261222000003_peer_legal_document_types` and
  `20261222000004_peer_legal_documents`: `PEER_TERMS`, `PEER_PRIVACY_NOTICE`,
  `PEER_CONDUCT_SAFEGUARDING` (v1.0, pilot text drafted from CATIE peer
  navigation practice guidelines and the Kenya Data Protection Act 2019; to be
  reviewed by counsel before public use). Like the Patient documents they are
  stored `required=false` so Doctor sign-up ignores them; Peer sign-up requires
  all three and records each acceptance. Consent is required for every role:
  Doctor (doctor documents), Patient (four patient documents), Peer (three
  peer documents). Each role sees only its own documents at sign-up.

### Phase 2 — Requests (done 2026-11-15)

- Migration `20261222000005_peer_requests`: `peer_requests` (snapshot of the
  patient's display name "Amina O." and age band; topic; CHAT or CALL), one live
  request per slot, database-enforced transitions (patient: PENDING ->
  CANCELLED; peer: PENDING -> ACCEPTED/DECLINED, ACCEPTED -> COMPLETED) and slot
  transitions; four notification types with no topic or health detail.
- Endpoints: `POST|GET /peer-requests`, `POST /peer-requests/:id/cancel`
  (patient); `GET /peer/requests?status=`, `POST /peer/requests/:id/accept|decline`
  (peer). A patient may have at most 3 waiting requests. Unanswered requests stay
  pending until the peer answers or the patient cancels.
- Tests: 6 in `tests/cases/peer_navigators.rs`.

### Phase 3 — Chat and calls (done 2026-11-15)

- **Chat** (migration `20261222000006_peer_chat`): `peer_conversations` /
  `peer_messages`, opened when the peer accepts and never locked (the doctor
  24-hour lock does not apply; "Mark session complete" — `POST
  /peer/requests/:id/complete` — changes the request status only). Same rules
  as doctor chat: 2,000 characters, `Idempotency-Key`, chat rate limit, images
  compressed and stored encrypted, read receipts, message notifications without
  text. Endpoints: `GET /peer-chats`, `GET|POST /peer-chats/:id/messages`,
  `POST /peer-chats/:id/attachments`, `GET /peer-chats/:id/messages/:mid/attachment`,
  `POST /peer-chats/:id/read`. Separate tables (not `chat_conversations`) because
  doctor chat is bound to appointments in its trace, policies and attachments.
- **Calls** (migration `20261222000007_peer_calls`, `peer_call_service.rs`):
  voice and video share one Agora session per request. Same timing as doctor
  video: `join_window::evaluate` with the configured minutes, tokens capped to
  the remaining window, presence from the same Agora webhook (channels prefixed
  `peer_`), out-of-order events ignored, and the same 180-second
  `EMPTY_ROOM_GRACE` swept with the doctor rooms; an ended room issues no new
  token, the chat stays open. Endpoints: `GET /peer-requests/:id/call`,
  `POST /peer-requests/:id/rtc-token`. Not reused: the doctor one-live-
  consultation reservation (appointment-bound).
- Tests: 8 in `tests/cases/peer_navigators.rs`; doctor call recovery and live
  reservation tests unchanged and passing.

### Phase 4 — Care app, peer side (done 2026-11-15, rebuilt to new references)

- **References:** the product owner supplied twelve Care-app peer screens on
  2026-11-15, saved in `docs/design/peer-navigator/care-app/` (01 Home,
  02/02b Support Requests and empty, 03 Request Details, 04/04b Schedule
  Create Slots and My Schedule, 05/05b Chats and empty, 06 Chat, 07 Profile,
  08/08b Notifications and empty). They replace the earlier workflow board for
  the peer side. Illustrations (hero friends, mission icon, calendars, empty
  states) are cropped from those references into `apps/mobile/assets/peer/`.
- One sign-in for doctors and peers (`/auth/care/login`, `/auth/care/verify-otp`);
  the role comes from the access token and a PEER session opens `PeerApp`.
  Until an Admin approves, the peer sees "Application under review".
- Code: `apps/mobile/lib/features/peer/` — `peer_ui.dart` (tokens, header,
  pill tabs, cards, empty states), `peer_pages.dart` (gate, shell, Home,
  Profile), `peer_requests.dart` (list, search, mode filter, details, accept /
  decline with optional note, start call, open chat, mark complete),
  `peer_schedule.dart` (week strip, start/end time, presets, 10-minute preview,
  create; My Schedule with names from requests, remove open slots, summary),
  `peer_chats.dart` (All / Active / Archived, chat with read ticks, photos via
  the existing encrypted peer attachment endpoint, held in memory only, voice/
  video), `peer_notifications.dart` (Today / Yesterday / Earlier, unread dots,
  Decline / View Request while pending).
- **Differences from the references, on purpose:**
  - No online/offline dots on avatars: there is no presence data.
  - No settings gear on Profile, Schedule or Notifications and no Support
    Preferences card on Profile: nothing exists behind them.
  - Slot Duration shows the only option, 10 min (the backend's slot length).
  - Notification text comes from the server and never names the patient
    ("Someone has requested a support session…"), unlike the sample
    "Grace W. requested…"; names in notifications are sensitive here.
  - Requests empty state keeps the populated screen's tabs (Pending / Accepted /
    All Requests) instead of the empty reference's four tabs.
  - "Archived" chats are sessions marked complete; the conversation stays open.
  - The chat subtitle reads "Peer Support Chat" (the other person is a patient,
    not a Peer Navigator as in the sample).
- Design preview: `flutter run -t lib/dev/design_preview_main.dart
  --dart-define=PREVIEW_ROLE=PEER` (add `--dart-define=PREVIEW_PEER=EMPTY` for
  the empty states). Screenshots: `screenshots/care-f01_home.png` …
  `care-f08b_notifications_empty.png`.
- Tests: `apps/mobile/test/peer_screens_test.dart` (16); full Care app suite
  371 passed; `flutter analyze` clean.

### Phase 5 — Patient app (done 2026-11-15)

- The Peer Navigator module (patient reference panel 4) keeps its layout and
  copy; "Start Chat" now opens **Choose a Peer Navigator** (approved peers,
  next open time), then **Book Peer Support** (support type Chat or
  Voice/Video, open 10-minute slots grouped by day in EAT, topic up to 200
  characters), then **Request Sent** ("Waiting for peer acceptance"). The
  module page lists the patient's own requests with Cancel (pending) or Open
  Chat (accepted). Board: `docs/design/peer-navigator/peer-workflow-patient-journey.jpeg`.
- Messages tab shows peer chats with the doctor chats (under Active, labelled
  "Peer support"); only for the account holder, not a dependant profile.
- Peer chat with Start Call (shared Agora screen; token from
  `/peer-requests/:id/rtc-token`, no consultation telemetry).
- English and Swahili strings for everything new.
- Code: `apps/mobile_patient/lib/features/patient/peer/`; debug preview
  `lib/dev/peer_preview_main.dart` (`PREVIEW_SCREEN=LIST|BOOK|SENT|CHAT`).
  Screenshots: `screenshots/patient-peer-*.png`.
- Tests: `test/peer_navigator_test.dart` (6); full Patient suite 520 passed;
  analyze clean.
- Not yet: tapping a peer notification opens the inbox, not the chat.

### Phase 6 — Admin Web (done 2026-11-15)

- People & Care → **Peer Navigators**: stat tiles, Pending / Approved /
  Rejected / Suspended / All tabs, search, detail with approve, reject and
  suspend (note required) and reinstate, mirroring the backend rules. No chat
  content or patient information is shown.
- Peer sign-up is now audited (`PEER_NAVIGATOR_CREATED`, county only) and is
  an Admin attention notification that opens the page.
- Code: `apps/admin-web/src/components/people/PeerNavigators.tsx`,
  `src/lib/api.ts` (`getAdminPeers`, `decideAdminPeer`).
- Tests: `tests/peer-navigators.test.tsx`, `tests/notification-text.test.ts`;
  admin suite 123 of 124 pass (the failing one, `refresh.test.ts`, needs a
  Redis fixture that is not set up locally); typecheck clean.
- Backend: peer tests 9 passed; clippy clean.

### Phase 7 — Follow-ups (2026-11-15)

- **Patient peer chat rebuilt** to the product owner's reference
  `docs/design/peer-navigator/patient-peer-chat.png`: circle back button,
  avatar, "Name (Peer Navigator)", Trained Peer Navigator, Accepted pill,
  call button and menu (Session details, Refresh); the "conversation active"
  banner; day chips; patient bubbles on the right with read ticks (one grey
  tick sent, two green ticks read); peer bubbles on the left with the peer's
  avatar; paperclip to send a photo (JPEG/PNG, 10 MB, viewed from memory
  only). Difference: no online dot on the avatar (presence is not tracked).
- **Care Home** greets the peer by first name ("Welcome, Grace").
- **Peer emails** (outbox, rendered at send time, migration
  `20261222000008_peer_emails`): application received, approved, declined,
  paused, reinstated. Greeting by first name; decisions never include the
  Admin's note. Previews: `docs/mail/previews/peer_*.html`.
- **Password reset** now works for peers. Password-reset and
  password-changed emails greet every role by name: the name used to be read
  on the API pool without a caller context, where row-level security hides
  every profile, so the greeting fell back to "there".
- **Doctor sign-in email** said "Dear Dr. Colleague": the committed code read
  the doctor's name the same way and got nothing. It now greets by full name
  and drops a stored "Dr."/"Doctor" title ("Dear Dr. Jane Mwangi").
- **Admin Peer Directory** (People & Care → Peer Directory; migration
  `20261222000009_admin_peer_history`): every peer with patients supported,
  sessions and last session; a record page with Support sessions (time,
  patient with a link to their record, chat or call, status, message count,
  call length), Patients supported and Account history. Admin reads through
  two SECURITY DEFINER functions that return counts and dates only, and only
  for an Admin session: never the topic a patient wrote (it can describe a
  health condition) or any message. Viewing a record is audited
  (`PEER_NAVIGATOR_HISTORY_VIEWED`, kept out of Admin pop-ups).

### Phase 8 — Admin audit, patient record, search, security (2026-11-15)

- **Admin patient record** lists Peer Navigator sessions with the doctor
  consultations: on the timeline ("Peer support chat/call · With Neema Kamau
  (Peer Navigator)") and as a "Peer support sessions" table in the
  Consultations tab, plus a "Peer sessions" summary row. Dates, peer, chat or
  call, status and message count only (`GET /admin/patients/:id/peer-sessions`,
  `fn_admin_patient_peer_sessions`, migration `20261222000010`).
- **Every Admin action is audited.** A layer around every `/admin/...` route
  (`src/api/middleware/admin_audit.rs`) writes `ADMIN_ACTION` for any change
  and `ADMIN_RECORD_VIEWED` for any read of one record (route template,
  record id, status; no body or query text) whenever the handler did not
  write its own audit row; failed attempts are recorded as FAILED. Opening a
  record the console already has (patient, doctor, Peer Navigator) is
  recorded through `POST /admin/audit/views` (`PATIENT_RECORD_VIEWED`,
  `DOCTOR_RECORD_VIEWED`, `PEER_NAVIGATOR_RECORD_VIEWED`). Lists and polling
  are not recorded; views stay out of Admin pop-ups.
- **Indexed search** in the Patients Directory, Doctor Verification, Peer
  Navigators and Peer Directory (`src/lib/searchIndex.ts`): fields are
  normalised once per load; every word must match in any order; accents and
  case ignored; IDs match with or without "#PT-" punctuation; phone numbers
  match by digits with or without +254.
- **Patient peer chat** says "Verified Peer" under the peer's name.
- **Care Create Slots**: the slot preview strip is removed; a hint explains
  when no slots would be created.
- **Security review of uploads and input:**
  - Peer chat accepted any declared type through the shared sanitizer, which
    passes PDFs through; it now takes JPEG or PNG only (like doctor chat).
  - Images are always verified by their bytes (declared type must match),
    fully decoded within size limits and re-encoded, so anything appended
    to or hidden in an image (scripts, PHP, executables) is dropped.
  - PDFs (doctor prescription uploads) were scanned only in their raw bytes;
    scripts hidden in compressed streams went unseen. Each Flate stream is
    now inflated within a 32 MB budget and scanned; larger is refused.
  - Peer text fields now refuse bidirectional-override characters, like the
    rest of the API. All SQL is parameterised (the few `format!` queries
    interpolate only server constants); an SQL string in a topic is stored
    as text (tested).
- Debug preview for the patient's doctor chat:
  `apps/mobile_patient/lib/dev/doctor_chat_preview_main.dart`.
- **Profile photos (product owner, 2026-11-19).** Peers save a profile photo
  from the Care app's Profile card (`POST/GET/DELETE /peer/profile/photo`,
  migration `20261222000014_counterpart_photos`, same storage and
  sanitising as patient and doctor photos); the Care Home header shows it.
  The other person's photo in a session comes from
  `GET /peer-requests/:id/counterpart-photo` (the peer gets the patient's
  photo, the patient the peer's; anyone else, Admins included, gets 404),
  shown on the request card, the chat list, the chat header and message
  avatars in both apps. Consultations use the same shape:
  `GET /appointments/:id/counterpart-photo` (the doctor gets the patient's
  photo, the patient the doctor's). Photos are held in memory only and
  cleared on sign-out; initials remain the fallback.
