# M8-G-30 / M8-G-31 — Single Live Consultation Per Participant: Design

Status: **Revision 2 — DESIGN ONLY, not implemented, not yet approved for
coding.** Approved in principle: a `consultation_live_reservations` table,
keyed by `user_id` alone (not `(user_id, role)` — product decision, see
"Resolved open question" at the end). This revision corrects six precision
issues raised against Revision 1. No code, migration, or test has been
written against the real codebase. Nothing here changes the 180-second
empty-room grace duration, the provider-signed-webhook design, scheduling
exclusivity, financial settlement, interruption handling, token renewal, or
reconnection behavior.

---

## 0. What this design does and does not guarantee (read this first)

This section exists because Revision 1 did not state its own limits clearly
enough, and the product owner correctly flagged that a short database lease
is not the same thing as one actual exclusive Agora call.

| Claim | True today (no Agora webhook configured) | True once the webhook is configured and tested |
|---|---|---|
| A user cannot **mint a second token** within ~2 minutes of minting a first, for a different appointment, unless the first is already known-terminal | **Yes** — this is a real, immediate, local guarantee | Yes, same mechanism, now also informed by confirmed presence |
| A user cannot mint a second token at any point **while provider-confirmed present** in a different, still-live session | **No** — nothing can confirm presence without the webhook | **Yes** — this is the actual requirement, fulfilled only here |
| A user **physically** cannot be connected to two Agora RTC channels at once | **Not enforced by this design at all** — see §6 | **Still not enforced by this design** — see §6; requires separate provider-side work |

**The honest summary: this design, as database-only engineering, prevents a
second *token from being issued* under conditions that are either (a)
provider-confirmed conflicting presence (once the webhook exists), or (b) a
short window immediately after a first, as-yet-unconfirmed mint (today,
before the webhook exists). It does not, and architecturally cannot, prevent
an already-admitted Agora RTC connection from continuing — Agora accepts a
connection before this backend ever hears about it. Treat the original
requirement ("must not participate in two live consultations simultaneously")
as only *partially* satisfied by this local-engineering work. Full
satisfaction requires the Agora webhook (external, already tracked as
blocked — M8-G-19/M8-G-07) and, for the physical-layer guarantee, a separate
provider-side control (§6) that is explicitly out of scope for this pass.
Production acceptance of "no simultaneous live participation" as a complete
guarantee must remain blocked until both exist and are tested together —
this local change alone does not close M8-G-30 to GREEN on its own.**

Given that, keeping M8-G-30 at P1 (per the reviewing recommendation) rather
than treating this design as a full fix is the correct call.

---

## 1. Confirmed failure path (unchanged from Revision 1, restated briefly)

`ConsultationService::mint_agora_token`
(`services/backend/src/services/consultation_service.rs:63-213`) checks
participant authorization, modality, appointment status, the temporal mint
window (`AGORA_TOKEN_WINDOW_BEFORE_MINS=15` / `AFTER_MINS=60`,
`config.rs:482-495`), and `recovery_state != NEEDS_REVIEW` — but nothing
checks whether `user_id` already holds a token or is already live elsewhere.
Two back-to-back appointments for one doctor have a structurally guaranteed
75-minute window (`slot_end - 15min` to `slot_end + 60min`) where both are
independently mintable, by construction of the default config, independent
of either call running long.

## 2. Token issuance vs. RTC participation (unchanged)

Issuance (a token was returned) and participation (the client actually
joined Agora's RTC channel) are different facts. Only a provider-signed
webhook event confirms the latter
(`consultation_presence_service::apply_provider_event`). Client SDK state
and app telemetry are not trusted for this and are not used to acquire,
confirm, or extend any reservation anywhere in this design.

---

## 3. Proposed invariant and table (revised)

```sql
CREATE TABLE consultation_live_reservations (
    user_id        UUID PRIMARY KEY,
    appointment_id UUID NOT NULL REFERENCES appointments(id),
    session_id     UUID NOT NULL REFERENCES consultation_sessions(id),
    state          TEXT NOT NULL CHECK (state IN ('PENDING','CONFIRMED')),
    acquired_at    TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    expires_at     TIMESTAMPTZ NOT NULL,
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
```

Unchanged from Revision 1: `user_id` as the primary key makes "at most one
live reservation per user" database-enforced. **Resolved per product
decision: this is per-`user_id` only, covering Doctor, Patient, and
guardian-of-a-dependent uniformly** — the real authenticated human, never a
per-role split.

Two states, as before: `PENDING` (mint-time, local-only, §5) and `CONFIRMED`
(webhook-upgraded, §7).

---

## 4. Issue 3 fix — atomic first acquisition, deterministic 409, no DB errors

**Problem identified:** `SELECT ... FOR UPDATE` against a `WHERE user_id =
$1` that matches zero rows locks nothing. Two concurrent `mint_agora_token`
calls for the same user, both seeing "no row," would both proceed to
`INSERT`, and the second would hit the `user_id` primary-key violation as a
raw database error — not a clean `409`.

**Fix:** never `SELECT` first. Acquire with a single atomic
`INSERT ... ON CONFLICT (user_id) DO UPDATE ... WHERE <admit-condition>
RETURNING user_id`, exactly the same idiom this codebase already uses for
conditional compare-and-set (`appointment_repo::confirm_payment_if_pending`
is a conditional `UPDATE` that returns 0 rows on failure, not an error; this
is the `INSERT`-side equivalent). Postgres resolves `ON CONFLICT` races by
serializing on the conflicting key internally — the second of two concurrent
inserts for the same `user_id` waits for the first to commit, then evaluates
its own `WHERE` against the now-current row. If `WHERE` doesn't match, zero
rows are returned — never an exception.

```sql
-- Mint-time acquire/refresh. $1=user_id $2=appointment_id $3=session_id
-- $4=RESERVATION_PENDING_GRACE_SECS (proposed 120)
INSERT INTO consultation_live_reservations
       (user_id, appointment_id, session_id, state, acquired_at, expires_at, updated_at)
VALUES ($1, $2, $3, 'PENDING', clock_timestamp(),
        clock_timestamp() + make_interval(secs => $4), clock_timestamp())
ON CONFLICT (user_id) DO UPDATE SET
    appointment_id = EXCLUDED.appointment_id,
    session_id     = EXCLUDED.session_id,
    -- Issue 2 fix: never downgrade an existing CONFIRMED same-appointment
    -- row's state or shrink its expiry to the short PENDING window. A
    -- same-appointment re-mint (token renewal, reconnect) must be a no-op
    -- against an already-CONFIRMED reservation.
    state      = CASE WHEN consultation_live_reservations.appointment_id = EXCLUDED.appointment_id
                        AND consultation_live_reservations.state = 'CONFIRMED'
                   THEN consultation_live_reservations.state
                   ELSE 'PENDING' END,
    expires_at = CASE WHEN consultation_live_reservations.appointment_id = EXCLUDED.appointment_id
                        AND consultation_live_reservations.state = 'CONFIRMED'
                   THEN consultation_live_reservations.expires_at
                   ELSE EXCLUDED.expires_at END,
    updated_at = clock_timestamp()
WHERE consultation_live_reservations.appointment_id = EXCLUDED.appointment_id         -- same appointment: always admit (refresh or no-op)
   OR (consultation_live_reservations.state = 'PENDING'
       AND consultation_live_reservations.expires_at <= clock_timestamp())            -- stale, never-confirmed claim: reclaim
   OR (consultation_live_reservations.state = 'CONFIRMED'
       AND consultation_live_reservations.expires_at <= clock_timestamp())            -- CONFIRMED backstop elapsed (§7): reclaim
   OR EXISTS (SELECT 1 FROM consultation_sessions cs
               WHERE cs.id = consultation_live_reservations.session_id
                 AND cs.status = 'COMPLETED')                                         -- other session genuinely finished: reclaim
   OR EXISTS (SELECT 1 FROM appointments a
               WHERE a.id = consultation_live_reservations.appointment_id
                 AND a.status IN ('CANCELLED','EXPIRED'))                             -- other appointment genuinely dead: reclaim
RETURNING user_id;
```

**Rust-level handling** (in `mint_agora_token`, same transaction, same style
as the rest of the function): `fetch_optional` this query. `Some(_)` →
proceed to mint. `None` → `return Err(AppError::Conflict("You are already
live in another consultation.".into()))`. **No code path relies on catching
a unique-violation error** — the `WHERE` clause is what produces the
deterministic zero-or-one-row outcome, which is the entire fix for issue 3.

## 5. Issue 1 fix — precisely scoped claim for the PENDING (local-only) layer

`RESERVATION_PENDING_GRACE_SECS` is proposed at **120 seconds** — deliberately
**far shorter** than `DEFAULT_TOKEN_EXPIRE_SECS = 3600`
(`agora_token_service.rs:27`), the token's own cryptographic validity.
Reusing the token's long TTL here was considered and rejected in Revision 1
already; this revision makes the consequence explicit rather than implied:

**What 120 seconds actually buys, precisely:** it prevents a user from
minting a second token for a *different* appointment within roughly two
minutes of minting a first one that has not yet been confirmed. It is aimed
specifically at the accidental-double-tap / two-tabs-open bug class.

**What it does not buy, stated plainly per the review's instruction:** if a
user mints token A, genuinely joins, and is still on a real call 10 minutes
later, the `PENDING` row for A has already self-expired at the 2-minute mark
(nothing renews a `PENDING` row except a same-appointment re-mint — see the
`WHERE` clause above) and is fully reclaimable. **In an environment where
the Agora webhook is not configured, there is no mechanism that upgrades A
to `CONFIRMED`, so from minute 2 onward this design provides zero protection
against minting B.** This is not a bug to silently patch with a longer
timer (that would reintroduce exactly the "blocks someone who never joined,
for up to an hour" failure mode already rejected) — it is the honest, fixed
shape of what a short, self-expiring, telemetry-free local lease can
achieve. Closing this gap requires §7 (the webhook) or §6's provider-side
control, not a bigger local number.

## 6. Issue 6 fix — provider admission ordering and conflict remediation

**The ordering problem, stated precisely:** Agora's RTC servers accept a
`joinChannel` call and begin carrying real audio/video *before* Agora's
Notification Service asynchronously delivers the corresponding webhook to
`POST /webhooks/agora/presence`. By the time this backend's database makes
any decision at all about a Joined event, the physical connection the event
describes has already existed for some (unbounded, provider-controlled)
interval. **No database-level check — ours or any other application's — can
retroactively prevent a connection Agora already admitted.** A mint-time
`409` (§4) is genuinely preventative, because it runs *before* the client
can call `joinChannel` at all; nothing after that point is.

**Required for an actual physical-layer guarantee (not part of this
pass):** Agora's RESTful channel-management API (a separate credential from
the RTC certificate already configured here) exposes server-to-server
connection control (kicking a UID from a channel). Using it to forcibly
terminate a conflicting second connection the moment a conflicting Joined
webhook is recognized is the only way to make the physical guarantee real,
and it is explicitly **out of scope for this design** — a new external
dependency, a new credential, a new failure-handling policy, needing its own
review. **Production acceptance of full physical-layer exclusivity stays
blocked until that is built, configured, and tested separately — this
local-engineering pass does not and cannot deliver it.**

**What this design does instead, once the webhook exists (conflict
remediation, not conflict prevention):** when a trusted Joined event is
applied for `user_id` in session S, and that user already holds an
unexpired `CONFIRMED` reservation for a *different*, non-releasable session
— i.e., real evidence that the mint-time guard was bypassed (two tokens
existed before the guard closed the gap, a webhook was delayed past the
mint-time check, or exactly the physical-admission-ordering gap above —
record it, do not try to silently resolve it, and do not corrupt either
session's own bookkeeping:

1. **Session S's own presence/grace/billing state updates exactly as it
   already would, unaffected.** The join is physically real; refusing to
   record it would risk spuriously starting an empty-room grace timer on a
   session someone is actually, physically present in — a worse outcome
   than the reservation conflict itself.
2. **The reservation row is left untouched** (neither session's claim is
   silently preferred over the other).
3. **One new audit event is written** —
   `CONCURRENT_LIVE_PARTICIPATION_DETECTED`, same `AuditService::log_in_tx`
   mechanism already used for `LATE_PAYMENT_RECONCILIATION_REQUIRED` — naming
   `user_id`, both `session_id`s, both `appointment_id`s — for a human to
   review. This mirrors the existing "when the system cannot safely resolve
   an ambiguity automatically, flag it for admin review rather than
   guessing" philosophy already used for late payments and `NEEDS_REVIEW`.

This is honestly a detection-and-flag mechanism, not a prevention mechanism,
for this specific residual case — stated as such, not oversold.

## 7. Issue 4 fix — safe CONFIRMED backstop, no premature release of a live call

**Problem identified:** Revision 1 proposed capping a `CONFIRMED`
reservation's backstop at the appointment's own mint-window ceiling
(`slot.end_time + AGORA_TOKEN_WINDOW_AFTER_MINS`). That ceiling governs
whether a *new* token may be minted — it says nothing about how long an
*already-issued*, still-cryptographically-valid token (up to
`DEFAULT_TOKEN_EXPIRE_SECS = 3600` seconds from when it was minted) might
legitimately still be in active use. Using it as the reservation's backstop
risked releasing a genuinely still-active consultation's reservation while
the participant remained connected — exactly the premature-release failure
the review flagged.

**Fix — two independent safeguards, neither derived from the mint window:**

1. **A separate, long, explicitly product-decided ceiling constant**,
   proposed as `consultation_live_reservation_max_secs`, default
   **6 hours**, following the exact `PENDING_PRODUCT_DECISION` convention
   already used in this codebase for values that are a policy choice, not an
   architectural fact (`config.rs`'s `payment_reservation_window_secs`
   comment: *"this default is a `PENDING_PRODUCT_DECISION` placeholder, not
   a specified requirement"*). Six hours is chosen only as a value no
   realistic single telehealth consultation should ever approach, so under
   normal operation this ceiling **never fires during a genuinely ongoing
   call** — it exists purely to bound the worst case of a permanently lost
   Left webhook to something finite and known, not to model expected call
   length. **This number needs explicit product-owner approval before
   implementation**, same as the 180-vs-300-second grace question already
   deferred.
2. **The backstop is refreshed, not fixed at first confirmation**: every
   time a *new, accepted* (non-stale-sequence) trusted Joined event is
   processed for a user already in `CONFIRMED` state for that same session
   (an ordinary reconnect mid-call), `expires_at` is pushed forward to
   `clock_timestamp() + consultation_live_reservation_max_secs` again. A
   call with normal reconnect activity keeps re-arming its own backstop and
   essentially never reaches it; only a session with **zero** further
   trusted events for six hours — i.e., exactly the "Left webhook was lost
   and nothing else ever arrived for this user" case — relies on the
   ceiling at all.
3. **A manual admin-release path, for recovery sooner than six hours**:
   extend the existing admin consultation-recovery resolution tooling
   (`admin_handlers.rs`, the same family as the existing `NEEDS_REVIEW`
   resolution endpoint exercised by
   `test_admin_monitor_and_resolution_preserve_interrupted_financial_state`)
   with a narrow action that deletes a specific stuck
   `consultation_live_reservations` row by `user_id`, audited the same way.
   This gives an operator a way to unblock a user immediately upon noticing
   a stuck reservation, rather than waiting out the ceiling — consistent
   with this codebase's existing pattern of deferring ambiguous states to
   admin judgment rather than only automated timers.

Nothing here touches the 180-second empty-room grace period, which remains
entirely about *per-session* both-absent detection — this backstop is a
*different*, cross-session, much longer-lived safety net with a different
job (bounding a stuck cross-user lock, not detecting an abandoned call).

## 8. Issue 5 fix — multiple devices, out-of-order events, identity mapping

**Identity**: the release/confirm hooks below always use whichever of
`patient_user_id` / `guardian_user_id` / `doctor_user_id` the *existing*
`role` match at the top of `apply_provider_event`
(`consultation_presence_service.rs:77-94`) already resolved — never a
re-derivation. This is the same authenticated identity already verified
against `derive_uid` for authorization purposes.

**Ordering**: both the `CONFIRMED` upgrade (§7) and the release-on-Left hook
below are placed in `apply_provider_event` **after** its existing
sequence-number check (`event.provider_sequence <= current_sequence` → event
discarded, lines 165-173) already accepts or rejects the event. A stale,
out-of-order Left arriving after a newer Joined has already been discarded
by that existing check and never reaches either hook — this design adds no
new ordering logic, it inherits the one already proven by
`test_provider_presence_recovery_grace_rejoin_and_expiry`'s stale-leave
assertion.

**Multiple devices, same role, same session**: `derive_uid(channel_name,
user_id)` (`agora_token_service.rs:107-120`) is a pure function of channel
and user — **not** device-salted — confirmed by its own unit tests
(`derive_uid_is_deterministic`). Two devices authenticated as the same user
joining the same channel therefore present Agora with the **identical**
UID. Agora's RTC platform does not admit two simultaneous connections under
one UID in one channel — a second connection with the same UID causes Agora
to resolve the conflict at its own layer (reject the newcomer or displace
the incumbent), independent of anything this codebase does. Consequently
"device 2 is still active while device 1's Left arrives" cannot occur as a
*distinct, simultaneously-live* condition for the same `(channel, uid)` pair
in the first place — what actually happens is a single evolving occupancy
for that UID, already correctly modeled by the existing one-boolean-per-role
presence columns and their sequence-number ordering. The release hook
therefore only ever fires on a genuinely accepted "this role is no longer
the one connected" event, never while a second, distinguishable device
connection for the same role still exists at the provider layer.

**Release-on-Left** (inside `apply_provider_event`, after the existing
sequence check accepts a `Left` event for role `R`, same transaction):

```sql
DELETE FROM consultation_live_reservations
 WHERE user_id = $resolved_user_id
   AND session_id = $this_session_id;
```

Scoped to `session_id = $this_session_id` so it can never delete a
reservation that — per §6's conflict-remediation path — legitimately points
at a *different* session for this user.

---

## 9. Reconnection compatibility (unchanged conclusion, now precisely justified)

A reconnect to the **same** appointment never reaches the "different
appointment" branch of either the mint-time acquire (§4's `WHERE
...appointment_id = EXCLUDED.appointment_id` branch, always admits) or the
webhook upgrade (§7, always refreshes). This covers the owner's already-
confirmed solo-drop/rejoin scenarios, token-renewal re-mints
(`call_controller.dart`'s `_tokenRenewalGuard`), and reconnect-after-network-
loss to the same appointment — none of which are new to this design; they
are simply never gated by it.

## 10. G-31 (separately approved, unchanged scope)

Re-confirmed in Revision 1 and not reopened: `mint_agora_token:187-191`
already rejects new token issuance with `409 Conflict` once
`session.recovery_state == 'NEEDS_REVIEW'`. The one approved regression test:

`test_rtc_token_reissue_rejected_after_needs_review` — force a session to
`NEEDS_REVIEW` (existing test setup pattern), call
`POST /api/v1/consultations/{appointment_id}/rtc-token` as both Doctor and
Patient, assert `409` for both. **The test and this document assert only
that new issuance is rejected. Neither claims, nor can claim, that a token
already issued before the `NEEDS_REVIEW` transition stops working** — that
remains the separate, unimplemented, provider-side-only limitation described
in the "required for an actual physical-layer guarantee" discussion in §6
(the same class of gap, for the same underlying reason: this backend has no
mechanism to revoke an already-issued, already-in-use Agora token or
connection).

## 11. Required tests (revised and expanded)

1. `test_mint_rejected_for_overlapping_appointment_same_doctor` — §4's core
   case, unchanged from Revision 1.
2. `test_mint_allowed_for_same_appointment_reconnect` — unchanged.
3. `test_pending_reservation_self_expires_allows_new_mint` — unchanged,
   now explicitly bounded to the 120s window (§5).
4. `test_confirmed_reservation_not_downgraded_by_same_appointment_remint`
   — **new, closes issue 2**: confirm presence for A (webhook path),
   re-mint a token for A, assert the reservation row's `state` is still
   `CONFIRMED` and `expires_at` is unchanged (not reset to a ~120s PENDING
   deadline).
5. `test_concurrent_first_acquisition_exactly_one_succeeds_no_db_error` —
   **new, closes issue 3**: `tokio::join!` two mint calls for two different
   appointments, same user, with **no prior row at all** (the true
   zero-row-race case Revision 1 missed), assert exactly one `200`/one
   `409`, and assert no test failure from an unhandled constraint-violation
   error surfacing anywhere in the stack.
6. `test_confirmed_reservation_survives_past_appointment_window_during_live_call`
   — **new, closes issue 4**: confirm presence for A, advance the clock past
   A's own `window_end` (mint-window ceiling) without sending a Left event
   or reaching the 6-hour backstop, attempt to mint for a different
   appointment B, assert `409` — proving the appointment-window ceiling is
   *not* used as the CONFIRMED backstop.
7. `test_confirmed_reservation_releases_after_long_backstop_with_no_further_events`
   — advance the clock past `consultation_live_reservation_max_secs` with no
   further trusted events for that user, assert the reservation is now
   reclaimable.
8. `test_confirmed_backstop_refreshes_on_reconnect_during_long_call` — confirm
   presence for A, advance partway toward the backstop, send another
   accepted trusted Joined for A (ordinary reconnect), advance again such
   that *only* the refreshed ceiling (not the original) would have expired,
   assert the reservation is still held.
9. `test_stale_left_cannot_release_reservation_after_newer_join` — **new,
   closes issue 5**: apply Joined (seq 2), then a stale Left (seq 1) for the
   same role/session, assert the reservation is untouched (inherits the
   existing sequence-rejection test's guarantee, asserted here against the
   new table specifically).
10. `test_concurrent_confirmed_join_elsewhere_flags_audit_without_disrupting_either_session`
    — **new, closes issue 6**: confirm presence for A, then apply a trusted
    Joined for the same user in a *different*, unrelated session B, assert
    (a) B's own presence/status fields update normally, (b) A's reservation
    is untouched, (c) exactly one `CONCURRENT_LIVE_PARTICIPATION_DETECTED`
    audit row is written referencing both sessions.
11. `test_rtc_token_reissue_rejected_after_needs_review` — G-31, §10.
12. `test_reservation_logic_does_not_affect_financial_or_interruption_state`
    — unchanged regression guard from Revision 1.

## 12. External Agora dependencies (unchanged conclusion)

The `PENDING` layer (§5) ships today, fully local, with the precisely scoped
claim stated there. The `CONFIRMED` layer and §6's conflict-remediation
audit activate automatically once the webhook is configured — no new local
dependency. The physical-layer kick/terminate control described in §6
remains a distinct, unscoped, future external dependency (separate Agora
RESTful credential), explicitly not part of this proposal.

## 13. Files that would need modification (unchanged list, updated detail)

- New migration: `consultation_live_reservations` (§3), plus FORCE RLS
  policies for `hellodoctor_api` (mint-time writes) and
  `hellodoctor_system_worker` (webhook-path writes), following the existing
  per-role-capability convention.
- `consultation_service.rs::mint_agora_token` — the §4 acquire query,
  inserted between the existing `NEEDS_REVIEW` check (line 187-191) and
  `tx.commit()` (193).
- `consultation_presence_service.rs::apply_provider_event` — the §7 upgrade
  query and §6 conflict-audit branch (on accepted Joined), and the §8
  release query (on accepted Left), both placed after the existing
  sequence-ordering check.
- `config.rs` — `RESERVATION_PENDING_GRACE_SECS` (§5, proposed as a plain
  constant) and `consultation_live_reservation_max_secs` (§7, proposed as a
  configured, `PENDING_PRODUCT_DECISION`-style value needing explicit
  approval of the 6-hour default).
- `admin_handlers.rs` — one narrow manual-release action (§7.3), alongside
  the existing consultation-recovery resolution endpoint.
- `services/backend/tests/cases/` — the 12 tests in §11, likely a new
  `consultation_live_reservation.rs` case file given the count.

No Flutter change required — both apps already handle a non-200 response
from the existing `rtc-token` endpoint.

## Resolved open question

Revision 1 asked whether reservations should be `user_id`-only or
`(user_id, role)`-scoped. **Resolved by product decision: `user_id` only.**
One authenticated person must not simultaneously participate in separate
consultations regardless of Doctor, Patient, or guardian role — the design
above (§3 onward) reflects this throughout; no further change needed on
this point.

---

**Nothing in this document has been implemented.** This revision addresses
all six correctness issues raised against Revision 1. Awaiting product-owner
sign-off specifically on: the 120-second `RESERVATION_PENDING_GRACE_SECS`
value (§5), the 6-hour `consultation_live_reservation_max_secs` default
(§7), and explicit acknowledgment that physical-layer exclusivity (§6)
remains blocked on separate, future provider-side work before any migration
or application code is written.
