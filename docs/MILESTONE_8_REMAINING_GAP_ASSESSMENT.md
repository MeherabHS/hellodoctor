# HelloDoctor.Asia — Milestone 8 Remaining-Gap Assessment

Date: 2026-11-01
Author: evidence-based gap assessment, synthesized from six independent read-only
research passes against current repository state (docs + source code), cross-
checked against `docs/MILESTONE_8_FINAL_ACCEPTANCE.md` (2026-11-01, verdict
PARTIAL). No code was changed, no tests were run, no benchmark was initiated, no
git/database/Docker/Azure state was touched to produce this document.

This is a gap-finding document, not an authorization to begin fixes. Findings are
evidence-based and scoped strictly to the original Milestone 8 brief
(workstreams 8.0 capacity baseline, 8A architecture/bottleneck assessment, 8B
repository organization/maintainability, 8C measured optimization, 8D
regression/endurance, 8E pilot readiness/deployment/provider/device
acceptance). No new requirements outside that scope are introduced. Milestone 9
items (patient consultation history, chat UI modernization, general UI
corrections, notification appearance/UX polish) are explicitly excluded below
and should stay tracked separately.

---

## 1. Original acceptance criteria mapped to current evidence

The repository does not track workstreams under the literal labels "8A"/"8B" —
its own Milestone-8 document set uses a different internal numbering. This
mapping reconciles the two:

| Requested workstream | Repository's actual artifact(s) | Verdict on record |
|---|---|---|
| 8.0 — realistic 4-vCPU/8-GiB capacity baseline | `MILESTONE_8_0_PERFORMANCE_CHARACTERIZATION.md`, `MILESTONE_8_0_DB_PERFORMANCE_REMEDIATION.md`, `MILESTONE_8_0_BOOKING_FAIRNESS_REMEDIATION.md`, `MILESTONE_8_0R_ADMISSION_CONTROL_REMEDIATION.md` | Baseline established; DB JIT/RLS/index fixes landed (migrations `20261202000003-5`) |
| 8A — architecture/bottleneck assessment | **No standalone deliverable exists.** Bottleneck findings are scattered inside `CHECKPOINT.md` and the performance docs only | Never tracked as its own artifact — see §3, Gap M8-G-15 |
| 8B — repository organization/maintainability | **No standalone deliverable exists.** `GITHUB_CHECKPOINT_REVIEW.md` is a git-staging/secret-scan inventory, not a structure review | Never tracked as its own artifact — see §3, Gap M8-G-16 |
| 8C — measured CPU/memory/DB optimization | `MILESTONE_8_MEMORY_PROFILE.md`, `MILESTONE_8_CRITICAL_QUERY_INDEX_AUDIT.md` | GREEN for backend+Postgres+Redis only; Admin Web/Doctor Web/Caddy excluded (disclosed in the docs themselves) |
| 8D — regression/sustained-load/endurance | `MILESTONE_8_SUSTAINED_RELIABILITY.md`, backend 464/0/4 suite | GREEN for the 3-service core at 40 minutes; 60-minute original bar and full 6-service load remain open (see §3) |
| 8E — pilot readiness, cloud deployment, provider integration, physical-device acceptance | `MILESTONE_8_WORKSTREAM_4_FINAL_ENGINEERING.md`, `MILESTONE_8_FINAL_ACCEPTANCE.md`, `PILOT_READINESS_DECISION.md` | PARTIAL — local six-service packaging GREEN; cloud/provider acceptance BLOCKED pending Azure/Firebase/payment/email credentials |

No workstream from the original six was silently dropped. 8A and 8B are the two
that were never given a dedicated artifact — that absence is itself logged as a
gap below, not retroactively invented as new scope.

---

## 2. Verified completed work — respected, not repeated

The following is accepted as-is per the existing evidence and was **not**
re-run, re-audited, or second-guessed by this assessment:

- Backend serialized integration suite: 464 passed, 0 failed, 4 ignored.
- Admin Web: 98 + 28 passed. Doctor Web: 85 passed. Both production builds and
  Docker images GREEN.
- Six-service disposable Docker stack: all services healthy, migrations
  current, persistence survives Postgres/Redis restart.
- PostgreSQL pgBackRest local engineering verification (full backup,
  differential, WAL archiving, retention, encryption, scheduler, PITR, target
  startup/promotion, SQL-level restoration) — accepted as **GREEN**, not
  re-opened.
- 10,000-account/2,000-session load objective, 40-minute corrected run on the
  approved single-VM core topology: ~160 RPS, 0.3503% true error rate, zero
  HTTP 500s, zero OOM.
- Owner's physical device testing: real Patient–Doctor Agora calls, chat, and
  the consultation timer succeeded; a timer bug found during that testing was
  corrected (the exact commit/diff for this fix could not be located in the
  current checkout — logged as an evidence-traceability item, §3, not as a
  functional defect, since the owner's live confirmation is authoritative).
- **10-minute payment-hold lifecycle (corrects the original M8-G-12
  classification — see §3 note below): fully implemented, server-side, and
  tested end-to-end.** `hold_expires_at` is a DB column set at INSERT from
  `clock_timestamp() + 600s` (`BOOKING_HOLD_SECONDS = 600`,
  `services/backend/src/repository/appointment_repo.rs:31-43,195`), made
  immutable by trigger
  (`20261113000001_payment_hold_cleanup.up.sql::protect_payment_hold_deadline`).
  Flutter (`apps/mobile_patient/lib/features/patient/booking/booking_models.dart:58-59`)
  only parses `hold_expires_at`/`server_time` from the API response for
  display — it never invents its own deadline. A background sweep
  (`run_hold_expiry_sweep`, ticking in `notification_scheduler.rs:872-897`)
  expires abandoned holds and releases the slot independent of any client, and
  a lazy reclaim (`expire_abandoned_hold_for_slot`,
  `appointment_repo.rs:286-310`) additionally runs inside a new booking's own
  transaction so a sweep-cycle gap can never block a rebooker. Payment
  confirmation is an atomic compare-and-set under row lock
  (`confirm_payment_if_pending`, `appointment_repo.rs:170-185`), and the
  callback-vs-expiry race is transactional with a consistent lock order and
  `SKIP LOCKED`, directly verified by 10 concurrent trials asserting exactly
  one winner (`tests/cases/schedule_exclusivity.rs::test_payment_confirmation_races_hold_expiration_exactly_one_outcome`).
  A provider success callback that arrives *after* expiry is captured but not
  resurrected into a booking: the transaction is flagged
  `reconciliation_required=true`, an `LATE_PAYMENT_RECONCILIATION_REQUIRED`
  audit event is logged, and it surfaces in `/admin/finance/transactions` —
  verified end-to-end at the HTTP layer
  (`tests/cases/remediation_a.rs` reconciliation assertions,
  `tests/api_tests.rs:6217::test_late_webhook_preserves_expired_booking_and_reports_authoritative_state`).
  A `provider_request_state` provenance column (`NOT_STARTED`/`STARTED`/`UNKNOWN`)
  additionally refuses to auto-resolve any transaction the provider might
  still be mid-flight on, and hard-blocks monthly accounting-period close
  (409) until a human resolves it
  (`tests/cases/remediation_a.rs::a1_unknown_and_provider_started_attempts_keep_close_blocked`).
  `docs/PAYMENT_PRODUCTION_READINESS.md`'s prior `NOT_IMPLEMENTED` rows for
  "abandoned-payment expiry and slot release worker" and the bulk of "UNKNOWN
  payment status... reconciliation" predate this Stage A work
  (migrations `20261111000001`, `20261113000001`, `20261130000001`) and have
  been corrected in that document. The only piece that is genuinely still
  missing — an active worker that polls a live payment provider's API to
  resolve an ambiguous transaction — cannot exist usefully without a real
  payment provider integration, and is folded into M8-G-10's existing BLOCKED
  scope below, not treated as separate local engineering work.
- **Admin step-up re-authentication (M8-G-11): explicitly WAIVED by the
  product owner on 2026-11-01.** Existing Admin authentication and
  authorization (role/ABAC gating, session refresh/rotation/reuse-detection,
  logout revocation — all independently re-verified in this assessment, see
  above) remain intact and unchanged. TOTP is not being restored. This is a
  recorded risk-acceptance decision, not an engineering completion — see §3
  for the residual-risk note.
- **Backup operational safeguards (M8-G-05, M8-G-06, M8-G-17): closed
  2026-11-01.** Full detail, implementation, and disposable-receiver test
  evidence in `docs/MILESTONE_8_BACKUP_OPERATIONS_CLOSURE.md`. Summary:
  `deploy/single-vm/backup/alert-notify.sh` (new) dispatches alerts for all
  five required backup-health conditions, wired into `backup-runner.sh`/
  `backup-monitor.sh` and tested end-to-end against a disposable HTTP
  receiver — including catching and fixing a real bug where a forced backup
  failure was being reported as exit 0 (a bash `!`-negation pitfall) before
  this pass's own test caught it. `python3`, used but never installed for
  `backup-monitor.sh`'s staleness check, is now installed in
  `pgbackrest.Dockerfile` (another latent defect found during this pass).
  `docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md` (new) and the retention/
  immutability section added to `docs/BACKUP_RESTORE_RUNBOOK.md` close the
  other two items at the documentation level; enactment of anything
  Azure-dependent remains correctly BLOCKED on M8-G-04.
- Doctor approval/KMPDC verification, booking/slot-locking atomicity,
  prescription 40KB-per-image/20-image-per-consultation cap, consultation chat
  24-hour window, and session refresh/rotation/reuse-detection/logout
  revocation: all independently re-verified against **current** code (not just
  historical docs) and found intact, with no regression despite the large
  uncommitted working-tree diff.

---

## 3. Confirmed remaining gaps

Each gap below has an ID for cross-reference in §4–§6. Classification key:
**GREEN** verified / **PARTIAL** implemented with a limited defect or missing
test / **OPEN** missing / **BLOCKED** external dependency / **OUT OF SCOPE**
Milestone 9+.

### P0 — production-readiness blockers or undetected-risk items

| ID | Requirement | Class | Evidence | Impact | Local or blocked | Acceptance test for GREEN |
|---|---|---|---|---|---|---|
| M8-G-01 | Real FCM push delivery | OPEN | `apps/mobile/android/app/google-services.json` absent from disk; backend `.env` has no `FCM_PROJECT_ID`/`FCM_CLIENT_EMAIL`/`FCM_PRIVATE_KEY_B64` → `FcmTransport::from_config` returns `None` → `NoopTransport` always used (`services/backend/src/repository/mod.rs:191-196`, `notification_transport.rs:47-64`) | No real push reaches any device today, under any circumstance | **Blocked** — needs owner to create a Firebase project and supply `google-services.json` + service-account credentials (not Azure-gated, owner's own action) | `register_device` reports `push_provider: "FCM_HTTP_V1"`; real device receives a heads-up push while backgrounded/terminated |
| M8-G-02 | Resource/latency measurement across all six services | PARTIAL | `MILESTONE_8_MEMORY_PROFILE.md:58-62` discloses Admin Web/Doctor Web excluded from Config A's budget and traffic; Caddy appears only in a 30-request health check, never under sustained load; OS/Docker overhead is budget math, not live cgroup data | Every sustained-load CPU/memory number produced this milestone covers the 3-service core only | Local-actionable, but is itself a new multi-service benchmark — **requires explicit owner authorization before running**, per the assignment's own instruction not to initiate new load tests unilaterally | Full six-service CPU/memory table under sustained traffic, same rigor as `MILESTONE_8_SUSTAINED_RELIABILITY.md §10.5` |
| M8-G-03 | Full six-service sustained production-like load | BLOCKED (not demonstrated) | Only six-service run on record is the disposable-stack health/page-load check (30 requests/page, p50 36–59ms) — explicitly self-caveated as not certifying full-load behavior (`MILESTONE_8_FINAL_ACCEPTANCE.md:27-54`) | Production-shape capacity claims remain unproven beyond the 3-service core | Local-actionable, same authorization caveat as M8-G-02 | Sustained ≥40–60 min run with all six services live and reporting per-service metrics |
| M8-G-04 | Azure Blob off-VM backup repository + managed identity/RBAC | BLOCKED | `MILESTONE_8_POSTGRES_BACKUP_FINAL.md:121-128`; `AZURE_BLOB_SETUP.md:57-70` (RBAC steps documented, not executable) | A VM-loss event today loses all PostgreSQL backups — no off-host copy exists | **Blocked** — needs Azure subscription + Storage Account | Real `pgbackrest backup`/`restore` against an actual Azure Blob container via scoped managed identity |
| ~~M8-G-05~~ | ~~Production backup-failure alert routing~~ | **GREEN (closed 2026-11-01)** | `deploy/single-vm/backup/alert-notify.sh` implemented; wired into `backup-runner.sh`/`backup-monitor.sh` for all five required conditions; tested end-to-end against a disposable receiver, including a real forced-failure defect found and fixed (see `docs/MILESTONE_8_BACKUP_OPERATIONS_CLOSURE.md`) | N/A — closed | Remaining operator action: point it at a real production receiver (tracked as an operator decision, not an engineering gap) | Already met locally; see closure doc |
| ~~M8-G-06~~ | ~~Encryption-key custody/recovery runbook~~ | **GREEN (documentation closed 2026-11-01)** | `docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md` written in full; tabletop drill itself not run (requires naming real custodians/secret store — operator decision) | N/A — closed | See `docs/MILESTONE_8_BACKUP_OPERATIONS_CLOSURE.md` |
| M8-G-07 | Real HTTPS/TLS browser/device smoke test | OPEN | `MILESTONE_8_FINAL_ACCEPTANCE.md:47-54`: local Caddy `tls internal` attempt blocked by a Windows Docker certificate-install failure (environment limitation); `PRODUCTION_DEPLOYMENT.md:19` confirms external TLS infra not installed | Secure-cookie/production-browser behavior still unverified in a real browser | Partially **local-actionable** (retry the Caddy TLS-internal smoke test on a non-Windows Docker host); full closure is **blocked** on a real domain/VM | Real HTTPS session confirms Secure/SameSite cookie behavior end to end |
| M8-G-08 | Real Azure Blob clinical-document roundtrip | PARTIAL | Azurite-simulated roundtrip (conditional-create, isolation, missing-object handling) is GREEN (`MILESTONE_7B_AZURE_BLOB_REPORT.md:100-108`); real Azure cloud roundtrip explicitly NOT_RUN | Document storage correctness unproven against the real target provider | **Blocked** — needs Azure Storage Account + managed identity | Authenticated upload/download/delete cycle against a real Azure container |
| M8-G-09 | Real email OTP provider delivery | OPEN | `MILESTONE_8_FINAL_ACCEPTANCE.md:133`: dev OTP challenge still used; Azure Communication Services never configured | Patient/doctor login MFA via email is unverified against a real provider | **Blocked** — needs Azure Communication Services endpoint, key, verified sender | Real OTP email delivery and expiry exercised against a staging mailbox |
| M8-G-10 | Payment sandbox / authenticated callbacks | OPEN | `PAYMENT_PRODUCTION_READINESS.md`: sandbox E2E, refund execution, payout execution all `DEFERRED_EXTERNAL_DEPENDENCY`; `MILESTONE_8_FINAL_ACCEPTANCE.md:136` confirms no real gateway/credentials exercised, no money charged | Real-money payment path is entirely unverified | **Blocked** — needs a real payment-provider sandbox (e.g., M-Pesa Daraja) credentials | Authenticated sandbox webhook round-trips through the existing HMAC/idempotency/ledger path |

> **Note (2026-11-01 correction):** active UNKNOWN-provider-status polling
> (querying a live gateway's API to resolve an ambiguous transaction) is
> folded into this item's scope — it is blocked on the same real
> payment-provider integration as the rest of M8-G-10. The local-engineering
> half of what was previously tracked as M8-G-12 (hold expiry, slot release,
> race safety, late-payment reconciliation) is already GREEN — see §2.

### P1

| ID | Requirement | Class | Evidence | Local or blocked | Acceptance test for GREEN |
|---|---|---|---|---|---|
| ~~M8-G-11~~ | ~~Sensitive Admin step-up re-authentication~~ | **WAIVED (owner decision, 2026-11-01)** | Product owner explicitly accepted this risk; existing Admin auth/authz (role/ABAC, session rotation/reuse-detection/logout revocation) remains intact and independently re-verified; TOTP is not being restored | N/A — closed by owner decision, not engineering | None required; see §2 for the waiver record |
| ~~M8-G-12~~ | ~~Payment UNKNOWN-status reconciliation worker + abandoned-payment expiry sweep~~ | **GREEN (corrected, 2026-11-01)** | The hold-expiry/slot-release/race-safety/late-payment-reconciliation portion is fully implemented and tested — see §2 for full evidence. Only active live-provider-status polling remains, folded into M8-G-10 (blocked, not local) | N/A — closed | Already met; see §2 |
| ~~M8-G-13~~ | ~~FCM Android notification channel routing~~ | **GREEN (closed 2026-11-01)** | `android_channel_id()`/`build_fcm_message()` in `notification_transport.rs` now set `android.notification.channel_id`, keyed on both notification type and recipient role (the Doctor and Patient apps register different channel sets); 6 new unit tests plus the 5 pre-existing `push_notification_delivery.rs` tests pass; both Flutter apps' full suites pass (302/364). A real initialization-ordering race (channel creation vs. push-service init, both previously independent `unawaited` calls) was found and fixed in both apps. Full evidence: `docs/MILESTONE_8_G13_FCM_CHANNEL_CLOSURE.md` | N/A — closed for local code completion | Real-device/provider push acceptance remains explicitly BLOCKED on M8-G-01 (no Firebase credentials) — not claimed here | Already met for local scope; real-device acceptance tracked separately under M8-G-01 |
| M8-G-14 | Foreground/background notification delivery guarantee | OPEN | Layer 1 (20s poll) is foreground-only; Layer 2 (best-effort 15-minute `WorkManager`, OS-throttleable) is the only path while backgrounded/terminated (`DOCTOR_NOTIFICATION_DELIVERY_ARCHITECTURE.md:177-185`) | Resolved in substance by M8-G-01 (real FCM replaces this gap); no separate fix needed until then | Superseded once M8-G-01 closes |
| M8-G-15 | 60-minute sustained-run requirement vs. 40-minute corrected run | OPEN | Original brief required "at least a 60-minute sustained profile" (`MILESTONE_8_SUSTAINED_RELIABILITY.md:218-220`); a genuine 60-minute attempt exists only as an incomplete 8-line log; no document records an explicit owner decision lowering the bar to 40 minutes | **Local-actionable** — either obtain and record explicit owner sign-off accepting 40 minutes, or complete the remaining ~20 minutes (flagged: requires owner authorization, it is an extension of a prior load test) | Written owner approval in `OPEN_QUESTIONS.md`/acceptance doc, or a completed ≥60-minute run |
| M8-G-16 | 10,000-account/2,000-session objective's production-likeness | PARTIAL | Duration/topology for the 3-service core is confirmed correct; Admin Web/Doctor Web/Caddy were not part of that stack, so "tested on the approved topology" should not be read as validating the full deployed stack | Same remediation as M8-G-02/03 | Re-run with all six services live and measured |
| ~~M8-G-17~~ | ~~Retention/immutability backup policy (written)~~ | **GREEN (closed 2026-11-01)** | `docs/BACKUP_RESTORE_RUNBOOK.md`'s "Retention and immutability policy" section documents the schedule, verified line-by-line against `backup-entrypoint.sh`'s actual generated config, plus a written (not-enacted) Azure immutability proposal | N/A — closed | Enactment of the Azure half remains BLOCKED on Azure access (unchanged, see M8-G-04) |
| M8-G-18 | Replacement-VM restore with measured RPO/RTO | BLOCKED | `MILESTONE_8_POSTGRES_BACKUP_FINAL.md:128`; targets defined (VM RTO ≤1h, DB RTO ≤4h) but never measured against a real VM | **Blocked** — needs a real replacement Azure VM | Timed drill: new VM stood up, backup restored, app reconnected, elapsed time recorded against targets |
| M8-G-19 | Agora rejoin/mid-call token-renewal/empty-room grace recovery | PARTIAL — three-way split (corrected 2026-11-01 after focused inspection) | See detail below | Mixed — see detail | See detail below |

**M8-G-19 detail (2026-11-01 focused read-only inspection,
`services/backend/src/services/consultation_presence_service.rs`,
`consultation_service.rs::mint_agora_token`,
`tests/cases/consultation_recovery.rs`):**

- **GREEN — existing server-side grace state machine and its synthetic
  integration tests.** A 180-second (not 300-second) empty-room grace period
  is implemented (`consultation_presence_service.rs:14,190,197`) with
  sequence-ordered, race-safe, financially-safe state transitions, verified
  by 13 tests in `tests/cases/consultation_recovery.rs` driving the service
  function directly with provider-shaped synthetic events. This is real,
  solid engineering — not re-litigated or asked to be redone.
- **OWNER-VERIFIED (distinct from the above — does not exercise the grace
  mechanism at all).** The three physically-confirmed scenarios (Doctor
  solo-drop/rejoin, Patient solo-drop/rejoin, screen lock/unlock) are
  accepted as-is. Structurally, none of them can have exercised the
  both-absent grace timer: a solo drop leaves `recovery_state='NONE'` by
  design (`entering_grace_period` requires *both* sides absent — code and
  test both confirm this), and locking the screen never calls
  `leave()`/`engine.release()` at all (`call_controller.dart:315-324`
  `handleAppBackgrounded` only mutes the camera; the Agora channel stays
  joined). These are genuine, valuable confirmations of SDK-level
  reconnection and backgrounding behavior — just not evidence for the
  empty-room grace path specifically.
- **BLOCKED EXTERNAL — real Agora Notification Service webhook delivery.**
  The grace state machine is driven exclusively by a provider-signed webhook
  (`POST /webhooks/agora/presence`, Agora event types 107/108,
  `webhook_handlers.rs:89-148`); client SDK callbacks are explicitly not
  trusted. That endpoint fails closed (`503`) whenever
  `AGORA_WEBHOOK_HMAC_SECRET` is empty — confirmed empty in this
  environment's actual `.env` — and additionally requires a real, Agora-
  reachable public HTTPS endpoint, which does not yet exist (same blocker as
  M8-G-07). **No real Agora-originated event has ever exercised this code
  path; all existing evidence is synthetic.** Blocked on Agora cloud
  credential/webhook configuration and production HTTPS — not local
  engineering work.

**Recommended single-source configuration for the grace duration (not
implemented — the product owner has not approved changing 180s to 300s or
anything else).** The duration currently exists in three independent
hardcoded forms that happen to agree (`EMPTY_ROOM_GRACE` constant, two SQL
`interval '180 seconds'` literals, and `notification_service.rs:1027`'s
`.clamp(1, 180)`). A safe consolidation, modeled exactly on the existing
`agora_token_window_before_mins`/`after_mins` pattern (`config.rs:66-73`,
already config-driven for precisely this kind of product-decided duration):

1. Add one field, e.g. `consultation_empty_room_grace_secs: i64`, to
   `Config`, with an explicit default of `180` (preserving current behavior
   exactly — no silent change) and an `env::var("CONSULTATION_EMPTY_ROOM_GRACE_SECS")`
   override, matching the existing pattern's style.
2. In `apply_provider_event`'s two UPDATE queries (lines 190/197), replace
   the literal `interval '180 seconds'` with a bound parameter — e.g.
   `clock_timestamp() + make_interval(secs => $N)` — the exact style already
   used for the payment-hold deadline (`appointment_repo.rs::insert_pending_hold`),
   binding `state.config.consultation_empty_room_grace_secs`.
3. Remove the now-redundant `EMPTY_ROOM_GRACE` Rust constant (or derive it
   from config at startup) so the constant and the enforced value can never
   silently diverge again — this was already a latent inconsistency (the
   constant was defined but never actually consulted by the SQL).
4. Thread the same config value into `notify_consultation_rejoin`'s TTL
   clamp (currently `.clamp(1, 180)`, `notification_service.rs:1027`) instead
   of a second hardcoded `180`.
5. **Race-safety is preserved automatically, by the existing design, and
   needs no new work**: every comparison that matters
   (`clock_timestamp() >= empty_room_deadline` in both
   `apply_provider_event` and `expire_empty_rooms`) operates on the
   *already-computed, stored* `empty_room_deadline` timestamp, never on the
   duration constant at check-time — exactly the same principle already
   documented for the payment-hold window ("a later change to the configured
   window never alters an existing hold"). A future config change would only
   affect grace periods that start *after* the change; in-flight ones keep
   whatever deadline was already written. The sweep query doesn't reference
   the duration at all, so it needs no change.
6. Update the three existing hardcoded test assertions
   (`tests/cases/consultation_recovery.rs:130-131`'s `<=180`/`>=175`) to read
   the same config default instead of a literal, so the tests stay correct
   if the default is ever changed.

This is a mechanical, low-risk refactor with no behavior change at the
current default — it does not require, and should not be bundled with, an
actual decision to change 180 to 300.
| ~~M8-G-20~~ | ~~Admin-handler N+1 suspicion never verified~~ | **GREEN (closed 2026-11-01)** | Not classic N+1, but genuine whole-table-fetch-then-filter-in-Rust cost confirmed via `EXPLAIN (ANALYZE, BUFFERS)` + end-to-end timing against a 10,000-appointment/10,000-transaction/2,000-grievance fixture; fixed by pushing filters into SQL and reusing the existing `find_by_ids` scoping pattern — `GET /admin/doctors` ~1.8× faster, `?month=` filter ~4.5× faster, `GET /admin/grievances` ~1.7× faster; all existing regression tests pass. See `docs/MILESTONE_8_G20_ADMIN_QUERY_VERIFICATION.md` | N/A — closed | Already met; see verification doc |
| M8-G-21 | Monorepo four-app split (`apps/mobile`, `apps/mobile_patient`, `apps/admin-web`, `apps/doctor-web`) undocumented as a deviation from CLAUDE.md §3 | PARTIAL | No ADR covers the split; this ambiguity already caused one real misattribution (an earlier audit reported Doctor-app token-refresh coverage as Patient coverage, later corrected per `MILESTONE_8_FUNCTIONAL_DEPENDENCY_VERIFICATION.md §0`) | **Local-actionable** — write one ADR documenting the accepted structure | ADR exists naming all four app directories and their roles |

### P2 — evidence/traceability gaps and minor follow-ups (none block production readiness)

| ID | Requirement | Class | Evidence | Note |
|---|---|---|---|---|
| M8-G-22 | Doctor-session-past-expiry test case | OPEN (test gap only) | Could not be executed — placeholder password hashes in the test fixture (`MILESTONE_8_FUNCTIONAL_DEPENDENCY_VERIFICATION.md §5`, item 2) | Seed a real Doctor password hash and re-run; mechanism already proven via the equivalent Patient-side test |
| M8-G-23 | Consultation-timer-fix traceability | OPEN (evidence gap, not a defect) | No commit, diff, or doc in the current checkout matches "a timer bug found and fixed on physical device" — the only related change found is an unrelated video-telemetry reconnection-flag fix (commit `9dc0210`) | Does not contradict the owner's physical confirmation; just ask which file/line the fix landed in, or re-check after the next commit |
| M8-G-24 | Doctor Web visual-parity closure | OPEN (documentation gap) | An earlier `DOCTOR_WEB_FINAL_ACCEPTANCE.md` recorded "not accepted," 0/11 screens matched; no later document explicitly closes that specific verdict, even though functional correctness is since GREEN and `MILESTONE_8_FINAL_ACCEPTANCE.md` reports 85/85 passing | Needs a superseding doc or explicit re-scoping statement, not new engineering work |
| M8-G-25 | Duplicate-notification risk once FCM is activated | PARTIAL (theoretical) | Poll-driven local notifications and OS-auto-displayed FCM notifications use unrelated ID namespaces and cannot dedupe against each other | Unverifiable until M8-G-01 is resolved; flag for a single foreground observation test once FCM is live |
| M8-G-26 | No dedicated 8A (architecture/bottleneck) deliverable | OPEN | Confirmed via exhaustive grep across roadmap/audit/traceability/risk docs — zero matches | A 1-page consolidation of already-scattered bottleneck findings would close this without new investigation |
| M8-G-27 | No dedicated 8B (repository organization/maintainability) deliverable | OPEN | Same — confirmed absent | A short structural-audit doc (module boundaries, file-size outliers) would close this |
| M8-G-28 | `admin_handlers.rs` at 2,401 lines (largest handler file) | PARTIAL | Not a correctness defect; a maintainability hotspot | Optional split by admin sub-domain, mirroring existing `admin_article_handlers.rs` precedent |
| M8-G-29 | Doctor release-APK R8/resource-shrinking measurement never executed | OPEN | Investigation doc explicitly left this as a stated-but-undone follow-up; no minified Doctor arm64 artifact exists on disk to measure | Proguard rules already exist in the repo; likely only needs enabling + the three named smoke checks |
| ~~M8-G-30~~ | ~~No backend guard against one user holding two simultaneous live consultations~~ | **GREEN (implemented 2026-11-01, explicitly limited scope)** | `consultation_live_reservations` table + `mint_agora_token`/`apply_provider_event` wiring per `docs/MILESTONE_8_SINGLE_LIVE_CONSULTATION_DESIGN.md` (Revision 2, approved) — 12 regression tests passing, including the true zero-row concurrency race. **Full physical Agora-level exclusivity is explicitly NOT claimed** — see `docs/MILESTONE_8_G30_G31_IMPLEMENTATION.md` §3 for exactly what is and is not proven | N/A — closed for the approved, explicitly-scoped local-engineering guarantee | Physical-layer exclusivity remains BLOCKED on the real Agora webhook (M8-G-19/M8-G-07) and a separate, unscoped provider-side connection-management control | Already met for the approved scope; see implementation doc |
| ~~M8-G-31~~ | ~~`NEEDS_REVIEW` token-mint rejection exists but is untested~~ | **GREEN (closed 2026-11-01)** | `test_rtc_token_reissue_rejected_after_needs_review` now passes, asserting only that new issuance is rejected — not that an existing token is revoked (that remains a separate, documented, unimplemented limitation) | N/A — closed | See `docs/MILESTONE_8_G30_G31_IMPLEMENTATION.md` |

---

## 4. Local-actionable vs. Azure/provider-blocked

**Genuinely local-actionable now (no external credential needed to make progress):**
M8-G-15 (60-vs-40-minute waiver decision),
M8-G-21 (ADR), M8-G-22/23/24/26/27/28/29 (all
documentation/traceability/cleanup items).
(M8-G-20 is closed — GREEN — as of 2026-11-01; see
`docs/MILESTONE_8_G20_ADMIN_QUERY_VERIFICATION.md`.)
(M8-G-30 and M8-G-31 are closed — GREEN — as of 2026-11-01; see
`docs/MILESTONE_8_G30_G31_IMPLEMENTATION.md`.)
(M8-G-11 is closed by owner waiver; M8-G-05, M8-G-06, M8-G-12, and M8-G-17 are
closed — GREEN — see §2.)

**Local-actionable but is itself a new benchmark requiring explicit owner
authorization before running:** M8-G-02, M8-G-03, M8-G-16.

**Genuinely external-blocked:**
- Needs an Azure subscription: M8-G-04, M8-G-07 (partially), M8-G-08, M8-G-09,
  M8-G-18, and the enactment half of M8-G-17.
- Needs a Firebase project (owner action, not Azure-gated): M8-G-01.
- Needs real payment-provider sandbox credentials: M8-G-10.
- Needs Agora Notification Service credential/webhook configuration plus the
  same production HTTPS endpoint as M8-G-07: the BLOCKED-EXTERNAL third of
  M8-G-19 (real provider-webhook delivery for the empty-room grace
  mechanism).

No Azure resource provisioning, card use, or chargeable action is recommended
anywhere in this document, consistent with the owner's standing restriction.

---

## 5. Required targeted verification steps

In priority order, the minimum verification needed to close each local-
actionable gap (no new broad audits or redone benchmarks):

1. ~~Draft the backup alert-routing contract and encryption-key custody
   runbook (M8-G-05, M8-G-06)~~ — **done**, implemented/tested and
   documented, respectively; see `docs/MILESTONE_8_BACKUP_OPERATIONS_CLOSURE.md`.
2. Obtain and record an explicit owner decision on the 60-vs-40-minute
   sustained-run criterion (M8-G-15) before treating 8D as closed.
3. ~~Get product-owner sign-off on an Admin step-up policy...~~ — **done**:
   the owner waived M8-G-11 on 2026-11-01; no implementation follows.
4. ~~Implement and test the payment reconciliation worker...~~ — **done**:
   M8-G-12's local-engineering scope was already implemented and tested;
   corrected to GREEN on 2026-11-01 (see §2).
5. ~~Add `android.notification.channel_id` to the FCM payload (M8-G-13)~~ —
   **done**; real-device delivery still waits on M8-G-01's credentials.
6. ~~Run the admin-handler query audit (M8-G-20)~~ — **done**; closed to
   GREEN with measured 10k-row evidence (see
   `docs/MILESTONE_8_G20_ADMIN_QUERY_VERIFICATION.md`). Write the ADR for the
   four-app monorepo split (M8-G-21) remains open.
7. ~~Draft the retention/immutability policy document (M8-G-17)~~ — **done**;
   the two missing 8A/8B consolidation docs (M8-G-26, M8-G-27) remain open.
8. Once explicitly authorized by the owner: extend or re-run the sustained
   test to cover all six services (M8-G-02, M8-G-03, M8-G-16).
9. External, owner-dependent, no engineering action possible until credentials
   arrive: Firebase project (M8-G-01), Azure subscription (M8-G-04, M8-G-07,
   M8-G-08, M8-G-09, M8-G-18), payment sandbox (M8-G-10, now including the
   active UNKNOWN-provider-polling sliver of the former M8-G-12).

---

## 6. Recommended closure sequence (highest priority first)

1. ~~M8-G-05, M8-G-06~~ — closed: alerting implemented/tested, custody runbook
   written (`docs/MILESTONE_8_BACKUP_OPERATIONS_CLOSURE.md`).
2. M8-G-15 — get the 40-vs-60-minute decision on record (pure documentation
   action, resolves ambiguity about whether 8D is actually done).
3. ~~M8-G-11~~ — closed: owner waived Admin step-up on 2026-11-01.
4. ~~M8-G-12~~ — closed: payment hold/expiry/reconciliation already
   implemented and tested; corrected to GREEN.
5. ~~M8-G-13~~, ~~M8-G-20~~ — closed. M8-G-21 — cheap, local, no dependencies.
6. ~~M8-G-17~~ — closed: retention/immutability policy written and verified
   against actual config. M8-G-26, M8-G-27 (8A/8B consolidation docs) remain.
7. M8-G-02/03/16 — full six-service load test, only after explicit owner
   authorization to run it.
8. External-blocked items (M8-G-01, M8-G-04, M8-G-07, M8-G-08, M8-G-09,
   M8-G-10, M8-G-18) — proceed as soon as the owner provisions Firebase/Azure/
   payment-sandbox access; no engineering work is possible on these before
   then.

---

## 7. Milestone 8 local engineering verdict

**PARTIAL — unchanged from `MILESTONE_8_FINAL_ACCEPTANCE.md`.** The previously
accepted evidence (backend/web test suites, six-service disposable stack,
local PostgreSQL DR, the 3-service-core 40-minute load run, physical
Agora/chat/timer confirmation) remains valid and is not reopened. What keeps
the verdict at PARTIAL rather than GREEN, restricted to items that need **no**
external credential to resolve: the un-recorded 60-vs-40-minute decision
(M8-G-15), the untested full six-service load profile (M8-G-02/03/16, pending
authorization), and the two never-produced 8A/8B deliverables (M8-G-26/27).
None of these require Azure, Firebase, or payment-provider access — they are
the genuine remaining local-engineering work. The Admin step-up gap (M8-G-11)
is resolved by an explicit owner risk-acceptance waiver (2026-11-01), not by
implementation. The payment reconciliation/expiry item (M8-G-12) is resolved:
on re-inspection the 10-minute hold, auto-expiry, slot release, rebooking,
payment/expiry race safety, and late-payment reconciliation were already
implemented and tested; only the live-provider-polling sliver remains,
correctly folded into the external-blocked M8-G-10. The backup operational
safeguards (M8-G-05, M8-G-06, M8-G-17) were implemented, tested, and
documented on 2026-11-01 — see
`docs/MILESTONE_8_BACKUP_OPERATIONS_CLOSURE.md`.

## 8. Production-readiness verdict

**NOT DECLARED / NOT READY — unchanged.** Beyond the local-engineering items
above, production readiness additionally requires external dependencies that
cannot be resolved inside this environment: an Azure subscription (off-host
backup repository, managed identity/RBAC, real Blob roundtrip, Email OTP via
Azure Communication Services, replacement-VM RPO/RTO drill), a real HTTPS/DNS
production target, a Firebase project for real FCM delivery, and payment
provider sandbox credentials. The owner has not authorized spending to obtain
any of these, so they remain correctly classified as BLOCKED rather than as
engineering shortfalls.

---

## Executive summary

- **Confirmed P0 gaps:** 8 (M8-G-01 through M8-G-04, M8-G-07 through M8-G-10
  — M8-G-05 and M8-G-06 closed GREEN on 2026-11-01, moved to §2)
- **Confirmed P1 gaps:** 6 (M8-G-14 through M8-G-16, M8-G-18, M8-G-19, M8-G-21
  — M8-G-11 waived, M8-G-12/M8-G-17 corrected/closed to GREEN on 2026-11-01,
  M8-G-13 closed to GREEN the same day, M8-G-20 closed to GREEN the same day
  with measured evidence, all moved to §2)
- **Confirmed P2 gaps:** 8 (M8-G-22 through M8-G-29 — M8-G-30/31, found during
  the M8-G-19 focused inspection, were implemented and closed to GREEN the
  same day; see `docs/MILESTONE_8_G30_G31_IMPLEMENTATION.md`)
- **Evidence-only / documentation-traceability gaps (subset of the above, not
  separate defects):** 4 — M8-G-23 (timer-fix traceability), M8-G-24 (Doctor
  Web stale "not accepted" doc), M8-G-26 (no 8A doc), M8-G-27 (no 8B doc)
- **External blockers (cannot be resolved without owner-provisioned
  credentials/subscriptions):** 8 — M8-G-01 (Firebase), M8-G-04, M8-G-08,
  M8-G-09, M8-G-18 (Azure), M8-G-10 (payment sandbox, now including active
  UNKNOWN-provider-polling), the production half of M8-G-07 (real
  domain/TLS), and the BLOCKED-EXTERNAL third of M8-G-19 (real Agora
  provider-webhook delivery, added 2026-11-01)
- **Exact requirements still preventing full local engineering acceptance**
  (no external dependency, purely engineering/documentation work remaining):
  the 60-vs-40-minute sign-off (M8-G-15), full
  six-service load verification (M8-G-02/03/16, pending authorization), and
  the two missing 8A/8B workstream deliverables (M8-G-26/27).
- **2026-11-01 corrections:** M8-G-11 (Admin step-up) closed by explicit
  product-owner risk-acceptance waiver — not implemented, deliberately
  accepted. M8-G-12 (payment reconciliation/expiry) reclassified OPEN→GREEN:
  the local-engineering scope was already built and tested; only an
  external-blocked live-provider-polling sliver remains, folded into M8-G-10.
  M8-G-05 (backup alerting) and M8-G-06 (key-custody runbook) implemented/
  documented and closed GREEN; M8-G-17 (retention/immutability policy)
  written, verified against actual config, and closed GREEN (Azure-side
  enactment remains correctly blocked on M8-G-04). Full detail in
  `docs/MILESTONE_8_BACKUP_OPERATIONS_CLOSURE.md`.
- **2026-11-01 same-day follow-up (still GREEN, corrected/hardened — not
  reopened):** a targeted review found and fixed three precision issues in
  the above: (1) the key-custody runbook's rotation procedure was factually
  wrong (pgBackRest has no in-place passphrase rotation) and is now the
  correct new-repository migration path; (2) the cipher passphrase was
  confirmed exposed in plaintext via `docker inspect`/`docker exec env` (on a
  disposable test secret) — fixed with a new `PGBACKREST_REPO1_CIPHER_PASS_FILE`
  option, tested to eliminate that exposure while keeping the old env var
  working; (3) monitoring never checked PostgreSQL's own `pg_wal`
  accumulation or data-volume free space, only backup-repository disk growth
  — two new checks added and tested via a disposable size-limited volume.
  Full evidence in `docs/MILESTONE_8_BACKUP_OPERATIONS_CLOSURE.md`'s
  "2026-11-01 follow-up" section.
- **2026-11-01 M8-G-30/M8-G-31 implementation (approved design, now built and
  tested):** a `consultation_live_reservations` reservation table now
  enforces at most one live-consultation claim per authenticated `user_id`
  (Doctor/Patient/guardian uniformly), per
  `docs/MILESTONE_8_SINGLE_LIVE_CONSULTATION_DESIGN.md` Revision 2. 12
  regression tests pass, including the true zero-row concurrent-acquisition
  race; the 13 pre-existing `consultation_recovery.rs` tests re-run clean
  with no regression. **Full physical Agora-level exclusivity is explicitly
  not claimed** — only new-token-issuance conflicts are prevented, not an
  already-admitted RTC connection; that gap is folded into the existing
  M8-G-19/M8-G-07 external blocker. G-31's regression test now exists and
  passes, proving only that new issuance is rejected after `NEEDS_REVIEW`,
  not that an existing token is revoked. Full evidence in
  `docs/MILESTONE_8_G30_G31_IMPLEMENTATION.md`.

No application code was changed to produce this document itself. Prior
sections record read-only assessment findings; the one exception — the
M8-G-30/M8-G-31 implementation referenced immediately above — was a separate,
explicitly authorized coding pass, documented on its own in
`docs/MILESTONE_8_G30_G31_IMPLEMENTATION.md`, not performed as part of
producing this assessment document. All uncommitted modifications, the
shared development database, Docker volumes, running services, prior
evidence files, and existing APKs were left untouched throughout.
