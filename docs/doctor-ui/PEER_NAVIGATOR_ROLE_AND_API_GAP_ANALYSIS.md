# Peer Navigator — Role and API Gap Analysis

Date: 2026-11-13. Analysis only. Nothing in this document has been built. Endpoints under "Verified existing" were read in `services/backend/src/api/router.rs` and the handlers; everything under "Proposed" is a proposal and does not exist.

## 1. Verdict

**There is no Peer Navigator support anywhere in the backend, Admin Web, Doctor Web or Doctor Flutter.** A search for `peer navigator`, `PEER_NAVIGATOR` and `peer_nav` across `services/backend/src`, `services/backend/migrations`, `apps/admin-web/src`, `apps/doctor-web/src` and `apps/mobile/lib` returns no file. The only occurrence in the product is the Patient app's Peer Navigator screen, which is layout with a disabled action.

The 12 November assessment therefore still holds: this is a new role with its own registration, approval, directory, conversation and call authorisation, spanning backend, Admin Web, Patient Flutter and the professional app.

## 2. What exists and can be reused

| Area | Verified in code | Reusable for Peer? |
|---|---|---|
| Accounts | `users` table with `role user_role_enum` (`PATIENT`, `DOCTOR`, `ADMIN` plus legacy admin roles), Argon2id passwords, unique phone and email | Yes, with a new role value |
| Sign-in | `POST /auth/doctor/login` (password) then `POST /auth/doctor/verify-otp` (emailed code); the login handler accepts only `role == Doctor` | The mechanism yes; the endpoint no. It is role-locked, which is correct |
| Tokens | `POST /auth/refresh`, `POST /auth/logout`, device proof, session expiry handling in `ApiClient` | Yes, unchanged |
| Password reset | `/auth/password-reset/request`, `/verify-otp`, `/complete` | Yes, if the service admits the new role |
| Legal consent ledger | `GET /legal/documents`, `accepted_documents` on registration (ADR-011) | Yes, with new document types for peer terms and peer conduct |
| Approval state machine | `professional_verification_status_enum` (`PENDING`, `VERIFIED`, `REJECTED`, `REVOKED`) on `doctor_profiles`, with a trigger that only allows `PENDING → VERIFIED/REJECTED` | The pattern yes; the column no. It lives on `doctor_profiles` and is tied to KMPDC fields |
| Admin review | `GET /admin/doctors/pending-verification`, `POST /admin/doctors/{id}/verify`, `/reject`, `/revoke`; Admin Web "Verification Queue" | The pattern yes; needs a separate peer queue |
| Authorisation | `abac::authorize(&ctx, Action::…)`, `ctx.verified_doctor_profile_id()`, row-level security keyed on `app.current_role` | The framework yes; new actions and policies are required |
| Chat transport | `chat_conversations`, `chat_messages`, attachments, pagination | **No.** `chat_conversations.appointment_id` is `NOT NULL UNIQUE` and `doctor_id` references `doctor_profiles`. A peer conversation has no appointment |
| Calls | `AgoraTokenService::mint`, deterministic uid per channel and user; `POST /consultations/{id}/rtc-token` is authorised against an appointment and its join window | The token minting yes; the authorisation no |
| Notifications | `notifications` table, device registration, FCM transport, preferences; the Doctor app polls every minute, runs a background sync worker and receives FCM | Yes, with new notification types |
| Flutter client | `ApiClient`, `AuthCubit`, `CallController` and `AgoraEngineAdapter`, `ClinicalMessageBubble`, notification stack | Yes |

## 3. What is missing

### 3.1 Database (new migrations, each with a down-migration)

- A `PEER_NAVIGATOR` value in `user_role_enum`.
- `peer_profiles`: `user_id`, `full_name`, `location` (required), `facility_name` (nullable), photo reference, `review_status`, `review_note`, reviewer, timestamps, soft delete. No licence, specialty, qualification, experience, fee or slot columns.
- A peer review status type. The spec asks for Submitted → Pending → Approved / Rejected / Further information required; the doctor enum has no "further information" state, so reuse would either lose that state or change doctor behaviour. A separate enum is cleaner.
- `peer_chat_requests`: patient, peer, status (`REQUESTED`, `ACCEPTED`, `DECLINED`, `EXPIRED`, `CANCELLED`), timestamps.
- `peer_conversations` and `peer_messages`, separate from the clinical chat tables so clinical retention, trace and history rules do not silently apply.
- `peer_call_sessions`: conversation, requester, status (`CALL_REQUESTED`, `CALL_ACCEPTED`, `CALL_DECLINED`, `RTC_CONNECTED`, `ENDED`), channel name, timestamps.
- Row-level security for every new table: a patient sees only their own requests and conversations; a peer sees only rows addressed to them and only while approved.
- New legal document types and new notification types.

### 3.2 Backend endpoints

Verified existing (reused as they are): `/auth/refresh`, `/auth/logout`, `/auth/password-reset/*`, `/legal/documents`, `/notifications*`.

Proposed (none exist):

| Purpose | Proposed endpoint | Caller |
|---|---|---|
| Register | `POST /auth/peer/register` | Professional app |
| Sign in | `POST /auth/peer/login`, `POST /auth/peer/verify-otp` | Professional app |
| Own profile and status | `GET/PATCH /peer/profile`, `/peer/profile/photo` | Peer |
| Review queue | `GET /admin/peers/pending-review`, `GET /admin/peers` | Admin |
| Decisions | `POST /admin/peers/{id}/approve`, `/reject`, `/request-information`, `/suspend` | Admin |
| Directory | `GET /peers` (approved only) | Patient |
| Request chat | `POST /peer-requests`, `GET /peer-requests` | Patient |
| Inbox and decision | `GET /peer/requests`, `POST /peer/requests/{id}/accept`, `/decline` | Approved peer |
| Messaging | `GET /peer-conversations`, `GET/POST /peer-conversations/{id}/messages` | Both parties |
| Call | `POST /peer-conversations/{id}/call`, `POST /peer-calls/{id}/accept`, `/decline`, `/end`, `POST /peer-calls/{id}/rtc-token` | Both parties |

The registration contract must be its own request type. `DoctorRegistrationRequest` requires `license_number`, `primary_specialty`, `experience_years`, `current_hospital`, `qualifications` and `residential_address`; making those optional to admit peers would weaken doctor validation, which the brief forbids.

### 3.3 Admin Web

- A peer review queue separate from the doctor Verification Queue, showing name, contact, location and optional facility.
- Approve, reject, request information, suspend, with a note and an audit entry.
- A peer directory view. Peers must never be labelled as doctors or clinicians.

### 3.4 Patient Flutter

- Replace the disabled Peer Navigator screen with the approved-peer directory, request chat, request status, conversation and in-conversation call.
- New notification handling for request accepted or declined, message, incoming call.

### 3.5 Professional (Doctor) Flutter

- Role choice at registration; a separate peer form; pending, rejected and information-requested states.
- A role-aware shell: after sign-in the server's role decides whether the doctor tabs or the peer tabs are shown.
- Peer home, request inbox, conversations, call request and call screen, profile, notifications.
- Today the client hard-codes `/auth/doctor/*`, and `ProfileDto` is the only source of approval state. Both need a peer branch.

## 4. Permission matrix

| Capability | Patient | Doctor | Peer Navigator | Admin |
|---|---|---|---|---|
| Register in professional app | no | yes, with KMPDC credentials | yes, no clinical credentials | no (bootstrap only) |
| Needs admin approval before use | no | yes | yes | n/a |
| Appear in doctor directory | no | when verified | **never** | no |
| Appear in peer directory | no | no | when approved | no |
| Manage schedule slots | no | own | **no** | no |
| Book or hold appointments | own | receives | **no** | view |
| Join appointment video or chat | own | own | **no** | no |
| Record consultation outcome | no | own | **no** | no |
| Issue or view prescriptions | own | own appointments | **no** | per policy |
| Read patient history or documents | own | only within the server's history window for an appointment | **no** | per policy |
| Earnings, wallet, payout | no | own | **no** | finance views |
| Send peer chat request | yes | no | no | no |
| Accept or decline peer request | no | no | own, when approved | no |
| Peer conversation | own | no | own, after acceptance | audit only |
| Peer call | own, inside an accepted conversation | no | own, inside an accepted conversation and after acceptance | no |
| Approve or reject professionals | no | no | no | yes |

Enforcement points, all server-side:

1. Every existing doctor action is authorised by role `DOCTOR` and a verified doctor profile. A peer token must fail these by role, not by a missing profile row. This needs an explicit negative test per doctor route group.
2. Every peer endpoint requires role `PEER_NAVIGATOR` **and** `review_status = APPROVED`. A pending peer may call only its own profile, legal, notification and auth endpoints.
3. The peer RTC token is minted only when a call session for that conversation is in `CALL_ACCEPTED` and the caller is one of its two parties.
4. Row-level security repeats rules 1–3 at the database, as it does for clinical tables today.

## 5. Notifications

New types needed: peer application received, approved, rejected, information requested; new chat request (peer); request accepted or declined (patient); new peer message; incoming peer call; missed call. Payloads must carry no health information, per the existing push rule: a title such as "New support request" and an opaque id, with content fetched after an authorised read.

## 6. Security and privacy requirements

- Peer support in this product is HIV-related, so the fact that a patient contacted a peer is itself sensitive. Request previews shown to the peer before acceptance should carry the minimum (no free text until accepted, or a short capped message, to be decided).
- No clinical data crosses into peer tables: no joins from peer conversations to appointments, intake, documents or prescriptions.
- Message and call retention for peer conversations needs its own policy; do not inherit the 24-hour clinical chat window by accident.
- Calls are never auto-joined. Recording stays off unless a separate decision is made.
- Rate limits on request creation, and block and report controls for both sides.
- Audit entries for every admin decision and every state transition.
- Suspension must revoke directory visibility and close the ability to accept or call at once.

## 7. Open product decisions (from the spec, section 12)

Location granularity and whether it is a controlled Kenya list; whether a peer can edit location after approval; any identity check beyond admin review; public profile fields and presence; who may start a call; request expiry; moderation and escalation; retention.

## 8. Implementation order

1. Decide the items in section 7 that shape the schema (location model, review states, call initiation).
2. Migrations: role value, peer profile, review status, legal document types.
3. Peer registration, sign-in and own-profile endpoints, with negative tests proving a peer token is refused on every doctor route.
4. Admin endpoints and Admin Web peer review queue.
5. Professional app: role choice, peer form, pending and decision states, role-aware shell. At this point a peer can be approved and can sign in, but has nothing to do; the directory stays empty of actions.
6. Directory, chat request, acceptance, conversation: backend first, then Patient app and professional app together.
7. Notifications for steps 5 and 6.
8. Call request, acceptance and peer RTC token; then call screens on both apps.
9. End-to-end and security testing across both roles.

Steps 2–5 must ship together before any peer account is treated as operational, as the brief requires: a registration form on its own must not create an active peer.

Size: unchanged from the earlier estimate. This is on the order of several weeks across backend, Admin Web, Patient app and professional app, and it is independent of the Doctor visual redesign.
