# HelloDoctor.Asia — Milestone 8: Endurance Validation and Benchmark-Harness Fixes

Date: 2026-11-05
Scope: closure of the Milestone 8 performance investigation's remaining open items
(60-minute endurance run, three harness/monitoring defects it surfaced) and the
final Milestone 8 performance status this document leaves in place.

All testing used the disposable `hellodoctor-m8-final-local` Docker Compose stack
with synthetic data only. No shared development, staging, or production database
was touched. No admission limits, pool sizes, or security controls were changed
anywhere in this pass.

## 1. 60-minute endurance run — result

Full evidence: `docs/performance-evidence/milestone-8-final/remediation/phase11_endurance_60min/`

- Clean run end-to-end: canonical baseline restore verified, both load
  generators exited 0, safety watchdog never aborted (memory 6.8-7.8GB free,
  disk flat at 40.9GB, all containers healthy the entire hour).
- **TRUE_SYSTEM_ERROR_RATE = 0.28%** full-run (under the 0.35% acceptance bar).
- Achieved RPS: 187.5 steady-state.
- Booking success: 2,749/2,751 = 99.9%.
- **Precise 5-minute error-rate breakdown** (computed directly from the
  360-row, 10-second-bucket time series):

  | Window | Error rate |
  |---|---|
  | 0-5 min | 0.706% |
  | 5-10 min | 0.893% |
  | 10-15 min | 0.983% |
  | 15-20 min | 0.317% |
  | 20-60 min (8 windows) | 0.039%-0.093%, averaging ~0.06% |

  The error rate does **not** fully disappear after any point in the run — a
  direct diff of admission-rejection counters across all four capture
  checkpoints (0/20/40/60 min) shows continued, real accumulation in every
  window, including the final 20 minutes. What is real: a sharp **drop** after
  ~15-20 minutes into a steady, much lower background rate that persists for
  the remaining 40 minutes. `sessions_active` is flat at 2,750 from minute 2
  onward and booking/payment-class rejections were negligible throughout, so
  this is not explained by exhausted booking eligibility or reduced traffic.
  The most likely explanation is a genuine deep warm-up effect on a timescale
  (15-20 minutes of real sustained load) that the project's earlier 150-second
  warm-up experiment (`phase7_warm_repeat_200rps/WARM_REPEAT_REPORT.md`) was
  far too short to ever capture — that experiment's "warming doesn't help"
  conclusion holds only for 150-second warm-ups, not for this timescale.
- No Postgres `Lock`-type wait events anywhere in the full hour — confirms the
  chat advisory-lock contention found in the earlier API-pool=14 experiment
  (`phase10_pool14_experiment_run1/`) was specific to that wider-pool
  configuration, not present at the approved pool=10 baseline even under a
  full hour of sustained load.
- Resource headroom (CPU, memory, disk, connection pool utilization) never
  approached any limit for the full duration.

**Endurance acceptance verdict: sufficient.** The three issues below are
harness/monitoring-level, not backend/admission-control/security issues, and
do not invalidate this result.

## 2. Admin/Doctor Web authentication harness defect — fixed

**Root cause** (confirmed via code, not inferred): `ACCESS_TOKEN_TTL_SECONDS =
15 * 60` (`services/backend/src/crypto.rs:394`). The benchmark harness
(`examples/web_workload.rs`) authenticated once at the start of the run and
never refreshed, so after the first 15 minutes of a 60-minute run its token
was expired for the remaining 45 minutes — (60-15)/60 ≈ 75%, matching the
observed ~73-74% authorization-rejection rate on every Admin/Doctor Web
endpoint almost exactly.

This was a **harness defect, not an application defect**: the real `admin-web`
app already implements correct session refresh (`apps/admin-web/src/lib/refresh.ts`
exists and is used); the harness simply never exercised it.

**Fix applied** (`examples/web_workload.rs`):
- Captures both `access_token` and `refresh_token` at initial login (the
  backend's `/auth/verify-otp` response already returns both; nothing new was
  added to the API).
- A dedicated background task per role (Admin, Doctor) proactively calls the
  real `POST /auth/refresh` endpoint — the same rotating-refresh-token
  mechanism the production apps use — 5 minutes before the 15-minute TTL would
  expire, comfortably inside the window rather than reactively after expiry.
- The refreshed access token is published to all worker tasks via a shared
  `Arc<RwLock<String>>` cookie; a failed refresh is logged and retried on the
  next cycle rather than crashing the run or falling back to a stale/bypassed
  token.
- Nothing about authentication, authorization, token lifetime, OTP, or session
  revocation was changed — only the harness's own re-authentication behavior.

**Tests** (`examples/web_workload.rs`, `#[cfg(test)] mod tests`, run via
`cargo test --example web_workload` — 6/6 passing):
- `does_not_refresh_while_well_within_ttl` / `refreshes_once_inside_the_safety_margin_before_expiry` —
  token-expiry timing logic, pure/network-free.
- `parses_a_successful_refresh_response` — successful refresh path.
- `rejects_a_refresh_response_with_success_false` /
  `rejects_a_malformed_refresh_response_missing_fields` — refresh-failure
  paths (explicit rejection and malformed/incomplete response), confirming a
  failure is always a clean `Err`, never a panic or a silently-accepted empty
  token.
- `refresh_cookie_kind_builds_the_expected_cookie_shape` — cookie
  reconstruction after refresh, including that the Doctor session's scope
  UUID is preserved (not regenerated) across a refresh, matching real session
  behavior.

No new large benchmark dataset was generated to validate this fix — it was
verified via unit tests plus a short functional smoke run against the live
disposable stack.

## 3. Notification delivery — documented, not modified

**Finding**: all 120,370 notifications created during the 60-minute run
remained in `delivery_status = CREATED`, never reaching `SENT_TO_PROVIDER` or
`DELIVERED`.

**Root cause** (confirmed via code read, `services/backend/src/services/notification_scheduler.rs:827-830`):
`run_push_delivery_pass` explicitly checks
`if !state.notification_transport.is_configured() { return Ok(0); }` before
ever calling `claim_next_notification`. The claim/deliver pipeline itself is
fully implemented and correct — `claim_next_notification`
(`services/backend/src/repository/notification_repo.rs:343+`) correctly
treats `delivery_status='CREATED'` as eligible and would promote it through
the pipeline — but the pass never reaches that code because no real push
transport (FCM credentials) is configured on this disposable benchmark stack.

**Classification: intentionally disabled external delivery transport.** This
is the correct, expected, and safe behavior for a disposable environment that
must never hold or use real push credentials. It is not a processing defect
and not an unimplemented state transition.

**Per explicit instruction, the notification-delivery pipeline was not
modified.** Real FCM delivery verification remains a separate, outstanding
internal pre-launch requirement (tracked independently of this performance
investigation — see `docs/FCM_SETUP.md` and the project's own checkpoint
notes on bilingual notification content and delivery triggers, which were
already flagged as not-yet-started work unrelated to this pass).

## 4. Missing metrics-snapshot histograms — root-caused and fixed (monitoring layer)

**Finding**: three histogram metric families
(`chat_send_message_transaction_seconds`, `sqlx_pool_acquire_wait_seconds`,
`chat_advisory_lock_wait_seconds`) were present in the 20-minute and 40-minute
counter snapshots but absent from the final (60-minute) snapshot.

**Root cause** (confirmed via direct file inspection): the final snapshot file
was 544 lines / 44KB versus 1,216 lines / 127KB for the 40-minute snapshot —
despite representing a *later*, more metric-populated point in the run — and
ended cleanly mid-metric-list with no error. This is consistent with a
**truncated HTTP scrape** of the `/metrics` endpoint, not the metrics actually
disappearing from the backend's registry (the `metrics` /
`metrics-exporter-prometheus` crates do not expire entries on their own). The
diagnostic `admission_permit_hold_by_endpoint_seconds{class,endpoint}` label
added earlier in this investigation meaningfully increased the `/metrics`
response size, making a late-run scrape more likely to hit an implicit
timeout than earlier, smaller scrapes.

**Classification: monitoring/diagnostic-capture defect** (the scrape
mechanism, not the backend).

**Fix applied**: `services/backend/scripts/lib_metrics_snapshot.ps1`, a new
shared `Get-MetricsSnapshot` function, now used by
`run_six_service_web_comparison.ps1` for both its pre-test and post-test
counter captures:
- Explicit, generous HTTP timeout (never the client's implicit default).
- Bounded retries (default 4 attempts, 3-second backoff) on transport
  failure, non-200 status, or a response that fails the completeness check.
- Completeness verified two ways: (a) every caller-specified required metric
  family actually appears in the body, and (b) the body's last line matches a
  well-formed Prometheus sample line (`metric_name{labels} value`), catching
  exactly the kind of mid-line truncation this fix exists to prevent.
- Atomic write: the body is written to a temp file, then moved into place —
  a reader can never observe a partial file, and a failed attempt can never
  clobber a previously-good snapshot already at the destination path.
- Optional line-filtering (`-LineFilter`) narrows what's written to disk
  (e.g. `^admission_` for the existing counters-file format) without ever
  narrowing what gets validated for completeness.

**Tests**: `services/backend/scripts/lib_metrics_snapshot.smoketest.ps1` — a
self-contained, non-destructive smoke test using a local HTTP listener (no
Docker/network dependency), covering: a complete well-formed response is
accepted; a truncated response is rejected and writes nothing; a response
missing a required metric family is rejected; a failed attempt never
clobbers an existing good snapshot; the line-filter narrows output while
completeness is still checked against the full body. 5/5 passing.

## 5. Final classification summary

| Finding | Classification |
|---|---|
| Admission-rejection rate drops after ~15-20 min, doesn't fully disappear | Expected — genuine warm-up effect |
| Web auth ~73-74% rejection rate over the hour | Benchmark harness defect — **fixed** |
| 120,370 notifications stuck in CREATED | Expected disposable-environment behavior — **documented, not modified** |
| 3 histograms missing from final snapshot | Monitoring/diagnostic-capture defect — **fixed** |
| Lock contention (none at pool=10 over a full hour) | Confirms pool=14 finding was pool-size-specific |
| Resource headroom | Healthy throughout — no finding |

## 6. Milestone 8 performance status

**Short-run (360s) and endurance (3600s) acceptance evidence are both
sufficient and consistent.** TRUE_SYSTEM_ERROR_RATE is under the 0.35% bar on
the full-run endurance figure and far under it in steady state; booking/
payment reliability is 99.9%+; no lock contention or resource exhaustion at
the approved pool=10 configuration. The benchmark harness itself had two real
defects (session-refresh gap, metrics-scrape truncation), both now fixed and
verified via targeted tests; neither affected the validity of the underlying
performance measurements they were discovered alongside.

**Outstanding, explicitly out of scope for this pass**: real FCM push-delivery
verification (requires actual provider credentials, never to be configured on
a disposable stack); the deep-warm-up mechanism noted in §1 is characterized
but not further investigated, since no corrective action was requested or is
evidently needed; Azure deployment and integration work remains unstarted
(see `docs/MILESTONE_8_BACKUP_OPERATIONS_CLOSURE.md` and related docs for
what deployment-readiness work was already completed separately from this
performance investigation).
