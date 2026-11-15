# Doctor Web redesign plan (2026-11-14)

Product-owner instruction (2026-11-14): Doctor Web (`apps/doctor-web`) gets a full UI
redesign. The six proposal boards in [`docs/design/doctor-web/`](../design/doctor-web/)
are **proposals, not pixel references**: the new UI may be more visually polished and
simpler than them, in the AfyaMkononi green design language shared with the Patient,
Doctor and Admin apps. The `index.html` prototype is no longer Doctor Web's source of truth.

Only presentation is rebuilt. The HttpOnly BFF, auth/MFA/session restore, proxy
allow-list, Agora call, join-window gating, Rx compression/upload, chat, schedule sync,
contracts and tests are preserved.

## Boards

| File | Content |
|---|---|
| `doctor-web-dashboard-scheduling.png` | Dashboard, Consultation Schedule, Add Slot, Appointments, History |
| `doctor-web-telehealth-workflow.png` | Dashboard, Appointments, Live consultation, Complete consultation, Encounter detail |
| `doctor-web-patient-care-communication.png` | Patient directory, Patient profile, Chats, Chat detail, Encounter history |
| `doctor-web-knowledge-publishing-settings.png` | Q&A hub, Q&A answer, My Articles, Create article, Settings |
| `doctor-web-portal-states.png` | Dashboard, Workspace, Rx, Completed modal, New-consultation pop-up, Verification steps/success/rejection |
| `doctor-web-consultation-history.png` | Consultation history table + detail drawer |

## Product-owner decisions (2026-11-14)

1. **Backend additions approved:**
   - Patient **age and sex** on the Doctor's own appointment list (derived from the
     existing `date_of_birth`/`gender`, same access rules as `patient_display_name`).
   - **Verification resubmission**: a rejected Doctor uploads documents and resubmits;
     Admin review picks it up.
   - **Article cover image, topic and optional Swahili version** for Doctor articles.
2. **Not approved (omitted from the UI):** Doctor-written consultation notes, summary and
   follow-up plan. The completion screen keeps outcome + Rx / no-Rx reason only.
3. **Patients page:** own patients only, grouped from the Doctor's own appointments
   (name, age/sex, last consultation, visit count, link to appointment-scoped history and
   documents). No allergies, chronic conditions, blood group or cohort filters.
4. **Earnings:** shown only when `GET /config/booking-policy` reports payment active;
   always Gross → 20% platform charge → Net with the mandatory note. Otherwise an honest
   "not yet active" state.
5. **No export:** no PDF summary download and no history export. Documents open through
   the existing short-lived secure viewer.

## Proposal items omitted (no backend, not approved)

Structured Rx form (medication/dosage/frequency): Rx stays photo pages (max 5, 40 KB each) ·
general clinical attachments · screen share and device pickers · consultation type per
slot/appointment · No-Show status · aggregate stats endpoints (tiles computed only where the
loaded data is complete, otherwise omitted) · doctor-initiated chats, lab orders,
follow-up booking from chat · chat previews/unread counts · Q&A drafts, resolve-without-
publish, in-progress status, upvotes · article views, scheduling, comments · Pending Rx
count, period comparisons · Reports, Help & Support.

## Newly wired existing endpoints

`doctor/profile/photo` (POST/DELETE through the proxy; GET through the binary
`/api/photo` route), `auth/change-password`, and the password-reset flow through
`/api/auth/reset-*`. Notification preferences and payout binding were not wired (see
"Not wired" below).

## Old-UI defects fixed by the rebuild

Session-revalidation Retry button stretched full height; Consultation History header
overlap; missing nav icons (Consultation History, My Articles); Patients table showing
columns and filters for data that does not exist.

## Implementation status (2026-11-14)

**Doctor Web (`apps/doctor-web`)** — presentation rebuilt; the prototype template layer
(`Prototype.tsx`, `templates.json`, `prototype.css`) is removed. New design system in
`src/app/web.css` (sidebar selection matches Admin Web: solid green item, no edge glow).
Screens: Dashboard, Appointments, Schedule, Consultation Room, Prescription, Consultation
History, My Patients, Patient Chats, Health Q&A, My Articles (topic, Kiswahili, cover),
Earnings, Settings (photo, change password), Notifications, sign-in, apply, 2FA,
**forgot password** (email → code → new password; reset authorization held in an HttpOnly
cookie, never in the browser), verification pending/rejected with **document resubmission**,
custom 404 / 500 / global error screens, and a "consultation ready" pop-up driven by the
server join window. Screenshots: [`screenshots/`](screenshots/) (labelled preview fixtures;
all `/api` traffic answered in-process by the capture script, no real account).

**Backend** — migrations `20261220000001_doctor_article_enrichment` (topic, complete
Swahili version, encrypted cover media with owner/published RLS and cleanup) and
`20261220000002_doctor_verification_resubmission` (submissions + documents, guard extended
with one REJECTED→PENDING transition by the owning Doctor only). Patient age/sex on the
Doctor appointment list (no migration). New routes: `POST /doctor/articles/:id/cover`,
`GET|POST /doctor/verification/submissions`, `GET /admin/doctors/:id/verification-submissions`,
`GET /admin/doctors/:id/verification-documents/:document_id`. The public
`/articles/:id/cover` now also serves Doctor covers. Admin Web shows submitted documents on
the verification review. Patient app shows Doctor covers, topics and Swahili summaries.

**Not wired (decided during build):** notification preferences (fields are patient-oriented)
and payout destination binding (provider integration still pending).

**Restart required:** the running dev backend must be restarted (and
`verify_backend_schema.ps1` run) before live verification, because two migrations were added.

## Security review (2026-11-14)

| Item | Result |
|---|---|
| HTTP/2 Rapid Reset | h2 0.4.19 / hyper 1.x; Caddy 2.11.7 (digest-pinned) — patched |
| 52-char username / bcrypt truncation | Argon2id only; no bcrypt |
| SAML bypass, nOAuth email spoof, GraphQL aliases | No SAML, OAuth/SSO or GraphQL — not applicable |
| 2FA code replay | Challenge row locked `FOR UPDATE`, deleted on success, attempt-capped; reset OTP single-use |
| ReDoS | Rust `regex` is linear-time; web regexes have no nested quantifiers and run on length-capped input |
| JWT alg confusion | `Validation::new(EdDSA)` with issuer, audience and expiry |
| Request smuggling | Caddy re-serialises requests; ambiguous CL/TE rejected |
| Cache poisoning | `private, no-store` everywhere; Host only used for loopback-only redirect; content cache keyed by role, published content only |
| Spoofed IP header | Backend trusts `X-Forwarded-For` only from configured proxy IPs, exactly one address; web BFFs require the edge proxy secret; Caddy overwrites client values |
| Views bypass RLS | Only `doctor_public_directory` (deliberate, column-limited public fields, endpoint-only) |
| Math.random tokens | Only polling jitter; tokens from OS-seeded CSPRNGs |
| Timing attack | Constant-time code/hash comparison; dummy hashing for unknown accounts |
| Padding oracle | AES-256-GCM (AEAD) — not applicable |
| Prototype pollution | Middleware rejects `__proto__`/`constructor`/`prototype` JSON keys; built-ins never frozen |
| Template injection | Email variables HTML-escaped; no server template engine on user input |
| XXE | No XML parsing of user input (quick-xml only reads storage-provider responses) |
| WebSocket hijack | No WebSocket endpoints; **unused axum `ws` feature removed** |
| Unicode lookalikes ("SPOTIFY") | Emails restricted to printable ASCII |
| Side/back doors | Simulation and dev-bind endpoints refuse outside development; seed data and dev OTP refuse to start outside development; `/metrics` guarded; diagnostics BFF same-origin + session + size-capped |
| XSS / CSRF | Nonce CSP, no inline scripts, `dangerouslySetInnerHTML` banned by the source check; exact-Origin + `Sec-Fetch-Site` checks, `SameSite=Strict` HttpOnly cookies |
| Bot spam | Turnstile on login, apply and password reset; backend rate limits on login, OTP, chat and uploads |
| Uploads | Magic-byte + MIME match, decode/re-encode (metadata stripped), size and dimension limits, encrypted storage, hash-verified reads |
| Chat input | 2,000-char cap, control characters stripped, rate-limited, rendered as text; **bidirectional override characters now stripped** |
| Dependencies | `npm audit` clean; Next.js 15.5.28 (past the middleware bypass fix) |
| **Fixed in this pass** | HSTS added in production; custom 404/500 screens; bidi stripping; `ws` feature removed |
