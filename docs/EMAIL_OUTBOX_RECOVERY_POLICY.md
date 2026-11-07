# Email outbox recovery policy (post-PITR restore)

> Scope: `email_outbox` (Phase 0, OTP only — `services/backend/src/email_delivery.rs`,
> `services/backend/src/repository/email_outbox_repo.rs`). Cross-reference:
> [`RECOVERY_SECURITY_MODEL.md`](RECOVERY_SECURITY_MODEL.md), which already
> establishes the governing principle this document applies to email
> specifically: *"Notification state is reconciled using delivery claim
> tokens and uncertainty markers; no entire restored outbox is resent."*

## The hazard this document addresses

After a PostgreSQL point-in-time (PITR) restore, a row that reflects "email
not yet sent" (`CREATED`) may in fact correspond to an email that **was**
already accepted by Azure Communication Services before the restore point —
the database's record of that fact was rolled back, but the external
provider's action was not. Any mechanism that naively resumes processing
every `CREATED`/non-terminal outbox row after a restore risks re-sending
content the recipient already received, or — worse, for a future
non-idempotent trigger — re-running a side effect twice.

## Policy, by category

### 1. OTP (Patient/Doctor/Admin login and registration — the only live case today)

**Safe by construction; no special recovery procedure is required.**

- `email_outbox` has **no resend path**. The only code that ever calls
  `EmailTransport::send_email` is the inline, synchronous retry loop inside
  `send_otp_email_tracked`, which runs once per original HTTP request and
  then is gone. The scheduler-driven sweep
  (`email_delivery::run_email_outbox_expiry_sweep`) only ever transitions a
  row to `EXPIRED`; it never calls the transport. A restored `CREATED` or
  `FAILED` row is therefore never automatically resent by anything, restore
  or not.
- **The outbox is not the authority on whether a code is still valid.**
  That authority is the challenge table the OTP's own flow already owns
  (`patient_auth_challenges` / the shared pending-MFA-challenge table via
  `session_repo`), each row carrying its own `expires_at` and single-use
  row-lock semantics, verified independently on every `POST
  /auth/.../verify-otp` call. A restored `email_outbox` row's status has
  **zero effect** on whether a given code can still be used to log in —
  only the challenge table's own (also-restored, but independently correct)
  expiry and consumption state does.
- Net effect of a restore: some `email_outbox` rows may show `CREATED`
  indefinitely in the restored snapshot's "current" view. This is purely an
  observability/audit artifact — the next scheduler tick's expiry sweep
  closes them out via `expires_at` (a wall-clock value, unaffected by the
  restore) as soon as that time has passed, which — given how long a
  restore takes in practice — it almost always already has by the time the
  restored system is serving traffic again.
- **Operator action required: none.** If an operator wants immediate
  cleanliness rather than waiting for the next sweep tick, a manual
  `UPDATE email_outbox SET delivery_status='EXPIRED' WHERE delivery_status='CREATED' AND expires_at <= now()`
  is equivalent to what the sweep would do — never a resend, never a
  change to any other table.

### 2. Password-recovery tokens (not implemented — design requirement for when they are)

Password reset remains deliberately disabled (`PASSWORD_RESET_ENABLED`
fails closed at startup; see `config.rs`). When this is eventually built,
it **must** follow the same two rules already true of OTP:

- The reset flow's own token table (not any outbox) must be the sole
  authority on token validity, with its own `expires_at`/single-use
  semantics, checked independently of outbox/delivery state.
- Whatever durably tracks a reset email's send attempt must have no
  automatic resend path, for the same reason as OTP: a resend after a
  restore would deliver a **second, independently valid** reset link for an
  action the user may not have re-requested, which is a materially larger
  risk than an OTP's harmless duplicate (see §3's distinction).

### 3. Appointment confirmation / booking events (push only today — email not yet built)

Out of scope for this document's immediate concern: these events are
**not** sent by email in Phase 0 — booking confirmation/reschedule/
cancellation remain push/in-app only via the existing `notifications`
table (see `services/email_templates.rs`'s module doc: templates for these
events exist but are wired to no trigger). That table
**does** have a resend path (the claim/lease scheduler in
`notification_scheduler.rs`), and its post-restore behavior is already
governed by `RECOVERY_SECURITY_MODEL.md`'s existing policy (delivery claim
tokens + the `delivery_uncertain` marker; no blanket outbox replay) —
unaffected by this Phase 0 change, since Phase 0 added no new trigger here.

**Forward requirement:** if/when a future phase adds an *email* version of
a booking event, it must be built on the same claim/lease-protected,
non-replay-after-restore design already proven for push — not on
`email_outbox`'s simpler "no resend" shape, since a booking confirmation
*may* legitimately need an at-least-once redelivery story in a way a
six-digit OTP does not.

### 4. Financial notifications (push only today — email not yet built)

Same analysis and same forward requirement as §3: not sent by email today;
governed by the existing push outbox's documented recovery policy; a future
email version must inherit it, and — because financial notifications are
the single highest-consequence category raised in the review that prompted
this document — should additionally be cross-checked against
`RECOVERY_SECURITY_MODEL.md`'s financial-reconciliation principle
("Financial state is reconciled against provider-side events using
transaction and idempotency identities. No collection, refund, settlement
or disbursement is automatically replayed.") before any such trigger is
activated, not merely against this document's email-specific reasoning
alone.

## Why `dedup_key` alone is not a sufficient answer (acknowledging the review's point directly)

`email_outbox.dedup_key`'s uniqueness constraint prevents a *second logical
send attempt within the current, live database* from creating a second
row. It says nothing about — and cannot prevent — a provider-side send that
already happened before a restore point the database was then rolled back
to, which is exactly why §1 above relies on "no resend path exists" (a
structural guarantee) rather than on `dedup_key` (a live-database
guarantee) to close this specific hazard for OTP. Any future category that
*does* need a resend path (§3, §4, and any future non-OTP email) must
therefore be designed with this distinction in mind from the start, not
retrofitted onto `dedup_key` alone.
