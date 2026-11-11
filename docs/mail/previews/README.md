# Email preview gallery (Phase 1A)

Open `index.html` in any browser — no server, no database, no Azure
credentials required. Every page here is the **exact HTML** produced by
`services::email_templates` (see `services/backend/examples/generate_email_previews.rs`),
not a hand-written mockup. Regenerate at any time from `services/backend/`:

```
cargo run --example generate_email_previews
```

## Sample data

All names, dates, consultation IDs and transaction IDs in these previews
are fictional (`Amani`, `Njoroge`, `CONS-FICT-10293`, `TXN-FICT-55821`, …).
No live OTPs, secrets, or real account identifiers are used anywhere.

## Open items before production activation

1. **CTA URLs are placeholders.** `https://app.hellodoctor.asia` (patient
   app) and `https://doctor.hellodoctor.asia` (doctor portal) are not
   confirmed production domains — per product direction, every button in
   these templates re-opens the originating app/portal rather than a
   per-record deep link, so only these two URLs ever need confirming.
2. **Hero illustrations are abstract geometric placeholders**, not the
   commissioned "diverse Kenyan and East African young adults" editorial
   illustrations the design brief calls for (`AFYAMKONONI_EMAIL_DESIGN_BRIEF.md`
   §4). They exist so the layout, color use and image-blocked fallback can
   be inspected now; real artwork should replace the files in `assets/`
   without any template code change (same filenames, same `<img>` contract).
3. **No logo image is used**, intentionally — the brief explicitly asks not
   to invent an official logo, so the header is a plain text wordmark.
4. **Footer structure was normalized across all 11 templates.** The
   approved content document's footers are inconsistent (templates 1–5
   include an "AfyaMkononi" brand line; 6–11 omit it). The design brief's
   master structure (§5) mandates the full footer on every email, so that
   structure was applied uniformly; every template's own approved
   contact-line label ("Need help?", "Questions?", "Professional support:",
   "Payment support:", "Security assistance:", "Need assistance?") is
   reproduced verbatim. No footer wording was invented.
5. **Illustration asset hosting is not configured.** `EmailAssets::none()`
   (no images at all) is the production-safe default used anywhere these
   templates might eventually be called from a real trigger, until a real
   `base_url` (CDN or equivalent) is approved and configured.
6. **Known email-client limitation:** inline/referenced SVG illustrations
   render reliably in every modern browser (confirmed by these previews)
   and in most mobile mail apps, but some desktop clients — notably Outlook
   on Windows, which uses Word's rendering engine — do not render SVG at
   all. No template's essential information depends on the hero image
   rendering (see `email_templates::tests::essential_content_survives_with_every_image_removed`
   and `no_hero_image_is_rendered_when_assets_are_unconfigured`), but the
   decorative illustration itself would simply not appear there. Before
   production activation, either export the final commissioned
   illustrations as PNG/JPEG (universally supported) or accept that Outlook
   desktop recipients see no hero image — a design decision, not a code
   change, once real artwork exists.

## Phase 2 additions (Secure Email OTP Password Recovery + MFA template coverage)

Four templates were added alongside the original eleven:

- **`password_reset_otp.html` — live, wired.** Sent by
  `PasswordResetService::request_reset` via
  `email_delivery::send_rendered_otp_email_tracked` (the same
  must-propagate-failure delivery contract as login OTP, not the
  fire-and-forget transactional path the other ten templates use). Content
  is the approved "reset mail" copy from `AFYAMKONONI_EMAIL_CONTENT_V1.md`.
- **`patient_login_otp.html`, `doctor_login_otp.html`, `admin_login_otp.html`
  — live, wired (branded login-OTP cutover).** Branded versions of the
  approved "patient auth mfa" / "B. Doctor authentication / MFA OTP" /
  "C. Admin authentication / MFA OTP" content. As of the explicit cutover
  authorization, `auth_service.rs`'s three login-OTP call sites
  (`request_patient_email_otp`, `doctor_login_step1`, `admin_login_step1`)
  render through these functions and send via
  `email_delivery::send_rendered_otp_email_tracked`, replacing the prior
  plain-text `email_delivery::otp_email_content` presentation. The cutover
  is presentation-only: OTP generation, challenge persistence, verification
  and expiration rules, rate limiting/lockout, MFA enforcement, and
  delivery-failure/retry/outbox semantics are byte-for-byte unchanged — only
  the subject/body (and now an HTML alternative) passed into the same
  tracked-send pipeline changed. The old plain-text `otp_email_content`
  function remains in `email_delivery.rs`, unused by any live call site, as
  a reference for the pre-cutover wording.

**MFA-enabled / MFA-disabled notification templates were not built.**
Neither Patient, Doctor, nor Admin has any self-service MFA
enrollment/toggle capability anywhere in this backend (Admin TOTP
self-enrollment was removed entirely — see
`migrations/20261202000002_remove_admin_totp` — and never existed for
Patient/Doctor; the emailed OTP second factor is mandatory and permanent
for all three roles, not something an account can turn on or off). Building
templates for an event that cannot structurally occur would be inventing a
feature rather than documenting one, so no action was taken for these two
notification types.

## Phase 1B additions (durable transactional email dispatch)

The remaining eight transactional templates (welcome, doctor application
received/approved/rejected, consultation confirmed/rescheduled, payment
confirmed, refund processed) are now wired to their real backend events via
a durable, asynchronous outbox/worker — not the synchronous, fire-and-forget
`send_templated_email` path these events used before. A *thin* intent row
(event type, recipient, dedup key, a reference to the business entity — never
the rendered content) is inserted atomically inside the same database
transaction as the business event itself (`email_outbox_repo::enqueue_transactional`);
a background worker (`services::email_dispatch_worker`, running as
`hellodoctor_system_worker`) resolves fresh display content and renders the
template at send time, with bounded retries, exponential backoff, and
stale-lease recovery mirroring the existing push-notification delivery
queue. This closes two real gaps the old synchronous path had: a crash
between the business transaction committing and the email send attempt
could silently lose the event entirely, and the triggering HTTP response
waited on the provider call (now it never does — see
`services/email_dispatch_worker.rs`'s own module doc).

**`consultation_cancelled.html` remains unwired** — there is no
authoritative cancel-appointment event anywhere in this backend (no route,
no state transition ever produces `AppointmentStatus::Cancelled`); building
a trigger for an event the product doesn't support yet would be inventing a
feature, not documenting one. The template and preview stay ready for when
that feature exists.

## Not activated

`consultation_cancelled.html` is the one remaining template with no live
trigger, for the reason above. `password_reset_otp.html` is wired to the real
`/auth/password-reset/request` endpoint (itself gated by
`PASSWORD_RESET_ENABLED`, off by default and refused outright in
staging/production — see `services/password_reset_service.rs`), and
`patient_login_otp.html`/`doctor_login_otp.html`/`admin_login_otp.html` are
wired to the live login/MFA endpoints as described above. Generating or
viewing these preview pages never sends an email or performs a
database/network operation regardless.
