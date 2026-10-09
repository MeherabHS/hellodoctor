# Doctor Flutter Notification Delivery Architecture — No External Push

Covers the **client-side delivery architecture** (Flutter): how the Doctor app gets backend
notification truth onto the device as an in-app list and/or a native Android notification,
without Firebase Cloud Messaging, OneSignal, or any other external push provider. For the
**backend** notification system (schema, scheduler, event triggers, `PAYOUT_PAID` wiring, dedup
keys, and the full backend test matrix), see
[`docs/DOCTOR_MOBILE_PROFILE_NOTIFICATIONS_VERIFICATION.md`](DOCTOR_MOBILE_PROFILE_NOTIFICATIONS_VERIFICATION.md) —
this document assumes that backend system as a given and does not restate it.

## Product decision

No Firebase Cloud Messaging, FCM, OneSignal, Azure Notification Hubs, APNs server push, or any
other third-party push infrastructure is used or added anywhere in this codebase. The backend's
own `notifications` table remains the single source of truth; nothing described below invents a
second, competing notion of "what notifications exist." Three delivery layers work together:

- **Layer 1 — Real-time sync while the app process is alive.** No WebSocket/SSE client exists
  anywhere in this Flutter codebase (confirmed by repo-wide grep before writing any code), so this
  is a lifecycle-aware poll, the same shape already established by `app.dart`'s own 30s incident-retry
  timer and `doctor_pages.dart`'s `_ScheduleState` 15s schedule poll.
- **Layer 2 — OS-supported background synchronization** via `workmanager`, waking the app in a
  background isolate on Android's own best-effort periodic schedule to sync even when the app
  process isn't running.
- **Layer 3 — Pre-scheduled native local notifications** for predictable future events (the daily
  consultation summary, T-15 consultation reminders), registered ahead of time with the OS via
  `flutter_local_notifications`, so they can fire even if neither Layer 1 nor Layer 2 gets a chance
  to run beforehand.

## Layer 1 — realtime sync (`features/notifications/notification_cubit.dart`)

`NotificationCubit` already owned the in-app Notification Center's list/unread-count state
(`load`, `markRead`, `markAllRead`); this layer adds a 20-second `Timer.periodic` poll
(`startPolling()`), gated on `WidgetsBinding.instance.lifecycleState ==
AppLifecycleState.resumed` — it only actually fires its fetch while the app is genuinely in the
foreground, exactly mirroring the two existing timer precedents above. 20s sits deliberately
between the schedule poll's 15s (that query is heavier) and the incident-retry timer's 30s (that
one is a low-urgency background retry).

`startPolling()` is **not** called automatically from `load()` or the constructor — a `Cubit` has
no widget lifecycle hook of its own, and some call sites (short-lived cubits in widget tests)
construct one, call `load()` once, and tear it down without ever wanting a background timer left
running; an un-cancelled `Timer.periodic` would trip `flutter_test`'s "no pending timer after
dispose" check. It is instead started explicitly, once, at the real construction site in
`app.dart`'s `BlocProvider<NotificationCubit>`.

Each successful fetch (both the initial `load()` and every quiet poll tick) runs
`_processFetchedItems`: the first fetch only establishes a baseline (the newest `createdAt`
already present, so a doctor is never notified for everything that already existed before opening
the app); every fetch after that shows a local device notification
(`LocalNotificationService.showNow`) for any item strictly newer than the baseline, then advances
the baseline monotonically. The baseline is purely in-memory (reset on app restart) — Layer 2's
background worker independently maintains its own *persisted* cursor (see below) for exactly the
gap this leaves, so a notification created while the app process wasn't running at all is still
delivered.

A poll tick also triggers Layer 3 reconciliation (`_maybeReconcileSchedule`), fire-and-forgotten so
it never adds latency to the awaited fetch the Notification Center's pull-to-refresh depends on.

## Layer 2 — background worker (`core/notifications/background_sync_worker.dart`)

Uses `workmanager` (`registerBackgroundSync()`, called once per `DoctorShell` mount; harmless to
call repeatedly because of `ExistingWorkPolicy.keep`) to register a periodic task at the
**requested** interval of 15 minutes — Android's own documented minimum enforced periodic
interval; a shorter request would be silently clamped up to it by the OS, so there is no reason to
ask for less.

```
REQUESTED_INTERVAL    = 15 minutes
ACTUAL_OS_GUARANTEE   = BEST EFFORT
```

Doze mode, battery optimization, manufacturer-specific power management, and a low battery may all
defer or skip an individual run — this is documented Android OS behavior, not a HelloDoctor
defect, and is never represented anywhere in this codebase as an exact or guaranteed schedule.

Each run (`callbackDispatcher` → `runBackgroundSyncOnce`) executes in its own background isolate,
so it constructs a fresh `ApiClient()`/`SecureTokenStore()` rather than reaching into the main
isolate's state — `ApiClient`'s default constructor transparently inherits the same persisted
session via the OS keystore, so no second token scheme was invented. It:

1. Fetches the notification inbox (`GET /notifications`) and unread count, matching the foreground
   inbox's own calls so auth/transport behavior is identical on both paths.
2. Computes which notifications are new via a **persisted cursor** (the newest `createdAt` seen on
   any previous run, stored under `background_sync_last_seen_created_at` in secure storage) —
   `computeNewNotifications`, a pure function, decides this without any plugin/network dependency,
   which is what makes it unit-testable under plain `flutter test` (no platform channel available
   in that host). Running the exact same fetched list twice in a row produces zero new decisions
   the second time, by construction (strict `isAfter` comparison against a monotonically-advancing
   cursor).
3. Shows a local device notification for each newly-discovered item.
4. Derives the signed-in doctor's stable identifier from the access-token JWT
   (`doctorIdentifierFromAccessToken` — mirrors `core/cache/doctor_cache_store.dart`'s existing
   private `_identitySeed`, since there is no `doctor_profiles.id` claim in the JWT today, only
   `sub`/`user_id`; this id is only ever used to namespace a local dedup key, never sent to the
   backend) and reconciles Layer 3's pre-scheduled reminders against a single-page (`limit: 100`)
   fetch of the doctor's appointment queue — a documented simplification (no pagination loop in
   this best-effort background pass; the foreground queue screen remains the fully-paginated
   authoritative view).
5. Always reports success to WorkManager, regardless of what happened (`runGracefully`) — an
   expired/revoked session, or any other unexpected failure, must stop quietly rather than
   triggering WorkManager's retry/backoff policy and risking an authentication storm. Nothing about
   the failure is logged.

Entry point requirements confirmed against the installed package source
(`workmanager-0.5.2/lib/src/workmanager.dart`): `callbackDispatcher` must be a top-level function
carrying `@pragma('vm:entry-point')` so it still resolves correctly in an obfuscated release build.

## Layer 3 — pre-scheduled local notifications

`core/notifications/local_notification_service.dart` is the single wrapper around
`flutter_local_notifications`/`timezone` — every other module depends on this one seam rather
than the plugin APIs directly. It fixes the `timezone` package's local location to `Africa/Nairobi`
(Kenya has no DST, matching the backend scheduler's own fixed `UTC+3` arithmetic exactly), so a
doctor whose phone happens to be set to a different timezone (e.g. while traveling) still gets the
daily summary at 7am Nairobi time, not 7am device-local time. Schedules use
`AndroidScheduleMode.inexactAllowWhileIdle` deliberately, never an exact alarm — an exact alarm on
Android 12+ requires the special `SCHEDULE_EXACT_ALARM`/`USE_EXACT_ALARM` access this architecture
deliberately avoids needing, and the spec itself requires treating timing as best-effort.

`core/notifications/schedule_notification_reconciler.dart` is split into a pure decision function
(`computePlan`) and a thin apply step — mirroring the backend scheduler's own
daily-summary-pass/T-15-pass split, but computed entirely from an already-fetched appointment list
(it never fetches anything itself). Given the doctor's id, the current appointment list, "now," and
the set of currently-pending local notifications (by payload, not just hashed id — a hashed id
can't be reversed back into the key that produced it, so key-based lookup is what lets the
reconciler find and cancel a *specific* stale entry like a cancelled appointment's old T-15 request
rather than only ones it already knows the id for), it decides:

- **Daily summary**: counts today's (Nairobi-calendar-date) non-`CANCELLED`/`EXPIRED`
  appointments. Zero means no summary at all (and cancels any stale pending one); otherwise
  schedules one `"Good morning" / "You have N consultation(s) scheduled today."` notification at
  the configured cutoff (07:00 Nairobi, matching the backend's own default — Flutter has no access
  to the backend's configurable `DOCTOR_DAILY_SUMMARY_TIME_LOCAL`, so this local layer always uses
  that same default), keyed `daily_schedule:<doctorId>:<yyyy-MM-dd>` so a repeated reconciliation
  pass harmlessly re-schedules the identical request rather than duplicating it (the plugin
  replaces an existing request sharing the same id in place).
- **T-15 reminders**: for every `CONFIRMED`/`WAITING` appointment still in the future, schedules
  `"Consultation starting soon" / "Your consultation starts at HH:MM."` at `start - 15m` (clamped
  to "now" if that would already be in the past — an appointment booked with under 15 minutes'
  notice still gets a reminder, just an immediate one), keyed `consultation_t15:<appointmentId>`.
- **Cleanup**: any pending local notification whose key belongs to this reconciler's two managed
  prefixes but no longer corresponds to a should-exist entry (appointment cancelled, no longer
  eligible, or a stale prior day's summary) is cancelled. A pending entry under any other payload
  prefix is left untouched — not this reconciler's to manage.

All notification ids are derived deterministically from a stable string key via FNV-1a hashing
(`core/notifications/notification_ids.dart`'s `stableNotificationId`) — never `Object.hashCode`,
which Dart explicitly does not guarantee stable across isolates/runs. The same logical event
always produces the same platform id, which is the actual mechanism behind every "repeated
reconciliation must not duplicate it" requirement throughout this architecture.

## Reconciliation algorithm (summary)

| Appointment transition | Reconciler action |
|---|---|
| New, future, `CONFIRMED`/`WAITING` | Schedule (or re-schedule in place) its T-15 reminder |
| Unchanged | Re-scheduling the identical request is a harmless no-op (same id, same content) |
| Rescheduled (same id, new `start`) | Re-schedule at the new `start - 15m`; the old pending entry is replaced in place, never duplicated |
| Cancelled / no longer eligible | Cancel its pending T-15 reminder |
| Already started/passed | No reminder; cancel any stale pending one |
| Daily count becomes 0 | Cancel any pending daily-summary entry for that date |

## Deduplication

| Source | Dedup key |
|---|---|
| Layer 1 remote-event bridging | `notification_event:<backend_notification_id>` |
| Layer 2 background remote-event bridging | Same `notification_event:<backend_notification_id>`, via a persisted `createdAt` cursor rather than a per-id set (compact; correct because the inbox list is already newest-first and a cursor only ever advances) |
| Layer 3 daily summary | `daily_schedule:<doctor_id>:<yyyy-MM-dd>` |
| Layer 3 T-15 reminder | `consultation_t15:<appointment_id>` |

Every path converges on `LocalNotificationService`'s deterministic id derivation as defense in
depth: even if dedup logic somehow fired twice for the same logical event, the plugin call itself
would *replace*, never duplicate, the OS-level notification.

## App lifecycle behavior

| App state | What happens |
|---|---|
| Foreground | Layer 1's 20s poll runs; a new item updates the in-app Notification Center immediately and shows a device notification |
| Backgrounded, process alive | Layer 1's poll pauses (lifecycle check fails), but nothing is lost — Layer 2 can still wake independently |
| Terminated | Layer 3's already-registered OS-level alarms (daily summary, T-15) still fire, subject to OS behavior; Layer 2 may wake the process in a background isolate if/when Android grants it execution; a new event that arrives with neither having run is discovered on the next app open (`load()`'s own baseline catch-up) |
| Force-stopped by the user | Android may block alarms/background workers entirely until the user manually relaunches the app — documented OS behavior, not a HelloDoctor defect, and never claimed otherwise anywhere in this codebase |
| Device reboot | `RECEIVE_BOOT_COMPLETED` is granted so WorkManager's own periodic-task registration survives a reboot (AndroidX WorkManager's manifest-merge handles the receiver; nothing custom was added) |

## Android platform specifics

- **Permission**: `POST_NOTIFICATIONS` (API 33+) is requested once, from `DoctorShell`'s startup
  path (`LocalNotificationService.requestPermission()`), not on first launch before the doctor has
  seen anything of the app. A denial never crashes or blocks the shell — the in-app Notification
  Center keeps working regardless of whether the OS also shows a tray notification.
- **Channels**: six fixed, meaningful channels (`CONSULTATION_REMINDERS`, `DAILY_SCHEDULE`,
  `MESSAGES`, `ACCOUNT`, `FINANCIAL`, `GENERAL`) — never one generic channel, and only
  `CONSULTATION_REMINDERS`/chat-adjacent channels use `Importance.high`; routine account/financial
  events use default/low importance.
- **Desugaring**: `flutter_local_notifications` 18.x uses `java.time`, which needs
  `isCoreLibraryDesugaringEnabled = true` + `coreLibraryDesugaring("com.android.tools:desugar_jdk_libs:2.1.4")`
  below API 26 — added to `android/app/build.gradle.kts`; without it, a release build fails with a
  desugaring error, not an app-level bug.
- **No foreground service.** Nothing in this architecture keeps an always-running
  "HelloDoctor is running" notification permanently visible, and no continuous battery-consuming
  background execution was added — only the three layers above (ordinary foreground lifecycle,
  WorkManager's own best-effort periodic scheduling, and the OS's native alarm scheduler).

## Privacy

Notification text is always short and non-clinical — "Consultation in 15 minutes," "You have N
consultations scheduled today," "New message," never a diagnosis, prescription detail, chief
complaint, or patient-identifying content. `schedule_notification_reconciler_test.dart` includes a
dedicated test asserting this directly: it builds an `AppointmentDto` with deliberately
identifiable `patientName`/`complaint` fields set, and asserts neither ever appears in any
generated notification title or body. Only safe synchronization state (a timestamp cursor,
scheduled-notification ids) is persisted locally; no PHI-rich payload, signed URL, or prescription
content is ever written to local storage by any of these three layers.

## Notification event matrix

| Event | Backend persisted | Realtime (L1) | Background sync (L2) | Local scheduled (L3) | In-app | OS notification | Tap destination |
|---|---|---|---|---|---|---|---|
| `DAILY_CONSULTATION_SUMMARY` | Yes | — | — | Yes (registered ahead of time from the fetched queue) | Yes (via inbox) | Yes | Notification Center (no deep schedule link — see Known limitations) |
| `CONSULTATION_REMINDER_15M` | Yes | — | — | Yes | Yes (via inbox) | Yes | Notification Center |
| `EARNING_CREDITED` | Yes | Yes | Yes | — | Yes | Yes (on discovery) | Notification Center |
| `PAYOUT_PAID` | Yes | Yes | Yes | — | Yes | Yes (on discovery) | Notification Center |
| `REGISTRATION_VERIFIED` / `REGISTRATION_REJECTED` | Yes | Yes | Yes | — | Yes | Yes (on discovery) | Notification Center |
| `ARTICLE_STATUS_CHANGED` | Yes | Yes | Yes | — | Yes | Yes (on discovery) | Notification Center |
| `NEW_CHAT_MESSAGE` | N/A today (no chat-originated notification kind exists in the backend inventory above; `NotificationChannels.forType` maps it defensively in case one is added later) | Yes (if it existed) | Yes (if it existed) | — | Yes (if it existed) | Yes (if it existed) | — |

Real push delivery remains `PUSH_PROVIDER_CONFIGURATION_REQUIRED` for every row — `NotificationTransport`'s
only implementation, `NoopTransport`, always returns `PushOutcome::NoProviderConfigured` (see the
backend verification doc). What this document's three layers deliver is the device-local
notification shown *after the Flutter app itself discovers* a row via the in-app inbox endpoints
— never a server-pushed wake-up.

## Tests

- `test/schedule_notification_reconciler_test.dart` — 13 tests: 6-appointment daily count, zero
  appointments (cancels stale pending), `CANCELLED` exclusion, Nairobi date-boundary correctness
  (a 23:30 UTC appointment counting toward the *next* Nairobi day), T-15 scheduling, cancellation,
  rescheduling (no duplicate), 20 appointments → 20 distinct non-duplicated decisions, an
  already-past appointment producing no reminder (and cancelling any stale one), a sub-15-minute
  booking firing immediately rather than in the past, an unrelated payload prefix left untouched,
  and the dedicated PHI-leak test. All 13 pass.
- `test/background_sync_worker_test.dart` — 10 tests: new-vs-older dedup against a cursor, nothing
  newer leaves the cursor unchanged, the same list run twice produces zero duplicates the second
  time, an empty fetch leaves the cursor unchanged, `runGracefully` swallows both `ApiException`
  and any other error while still reporting success, and `doctorIdentifierFromAccessToken`'s claim
  precedence/malformed-token handling. All 10 pass.
- `test/notification_cubit_test.dart` — extended with 7 new tests covering `startPolling()`'s
  lifecycle-gated firing (via `fakeAsync` + `TestWidgetsFlutterBinding`), pause/resume behavior,
  first-fetch baseline establishment, duplicate-discovery immunity across repeated loads, and
  schedule-reconciliation being skipped/invoked correctly depending on whether `doctorRepo`/
  `doctorId` are supplied. All (15 total in that file) pass.
- Full project suite (`flutter test`): **170 passed, 2 failed** — the 2 failures are
  `notification_center_page_test.dart`'s pre-existing golden-image flakiness (a ~0.03-0.04% pixel
  diff that reproduces only inside the full suite, never in isolation, and predates this
  architecture entirely — documented earlier in this same project during the Kenyan phone
  validation work). Not caused by, and not fixed by, this pass.
- `flutter analyze`: clean except one expected **info**-level lint
  (`fake_async isn't a dependency of the importing package` in `notification_cubit_test.dart`,
  used transitively via `flutter_test`'s own dependency rather than adding a new direct pubspec
  entry for a test-only transitive package).
- `dart format --set-exit-if-changed lib test`: clean.

## What could not be exercised (physical-device verification)

No physical Android device was connected during this work, and per this architecture's own
constraint, no emulator was started to fake one. Consequently, the following are implemented and
unit/widget-tested at the logic layer, but their actual on-device platform behavior is
**unverified**:

- Whether `LocalNotificationService.showNow`/`scheduleAt` actually displays/schedules a visible
  system notification (the test host has no platform channel; both show up only as "didn't throw,"
  never as "did the right platform call").
- Whether `workmanager`'s periodic task genuinely wakes the app in the background under real
  Android Doze/battery-optimization conditions.
- Whether a scheduled T-15/daily-summary alarm actually fires at the expected wall-clock time on a
  real device, surviving app close and device reboot.
- Whether the `POST_NOTIFICATIONS` permission prompt renders and behaves as expected on a real
  Android 13+ device.

These require the physical-device test matrix (`TEST A`–`TEST E`) this architecture's own
originating spec describes; none of that evidence exists yet, and nothing in this document or
elsewhere claims otherwise.

## Known limitations (stated plainly)

- The daily-summary notification's tap destination is the Notification Center, not a deep link
  into the schedule itself — no such deep-link route was requested or built in this pass.
- Layer 2's schedule reconciliation uses a single `limit: 100` fetch with no pagination; a doctor
  with a queue deeper than 100 entries has reminders reconciled only for the backend's
  default-ordered first page in that specific background pass. The foreground queue screen is
  unaffected and remains fully paginated.
- `doctorIdentifierFromAccessToken` falls back through `doctor_id` → `user_id` → `sub`, but the
  current backend `Claims` struct only ever sets `sub`/`user_id` (no `doctor_id` claim exists) —
  acceptable because this id is only used to namespace a *local* dedup key, never compared against
  `doctor_profiles.id` or sent to the backend anywhere.
- Push delivery (a real FCM/APNs/other provider) remains entirely unimplemented, by design — see
  "No product decision" above and the backend verification doc's own transport section.

## Final status

- `DOCTOR_IN_APP_NOTIFICATIONS_VERIFIED` — pre-existing, re-confirmed: Notification Center list/
  unread-count/mark-read/mark-all-read/pagination/empty/loading/error states all pass their
  existing tests (`notification_center_page_test.dart`, `notification_repository_test.dart`,
  `notification_cubit_test.dart`), modulo the 2 pre-existing unrelated golden-image flakes noted
  above.
- `DOCTOR_REALTIME_NOTIFICATION_SYNC_VERIFIED` — proven by `notification_cubit_test.dart`'s
  `startPolling` lifecycle-gated tests and the dedup/baseline tests.
- `DOCTOR_BACKGROUND_NOTIFICATION_SYNC_IMPLEMENTED` — implemented and unit-tested at the pure-logic
  layer (`background_sync_worker_test.dart`); **not** physically verified on a device (see above),
  so stated as implemented, not as device-verified.
- `DOCTOR_LOCAL_NOTIFICATION_SCHEDULER_IMPLEMENTED` — implemented and unit-tested
  (`schedule_notification_reconciler_test.dart`); same physical-verification caveat as above.
- `DOCTOR_DAILY_SUMMARY_LOCAL_NOTIFICATION_VERIFIED` — **not claimed**: "verified" would require
  the physical-device `TEST D` evidence this pass does not have. The logic is implemented and
  unit-tested only; use `DOCTOR_DAILY_SUMMARY_LOCAL_NOTIFICATION_IMPLEMENTED` as the accurate label
  for this pass.
- `DOCTOR_T15_LOCAL_REMINDER_VERIFIED` — **not claimed**, same reasoning; implemented and
  unit-tested only (`DOCTOR_T15_LOCAL_REMINDER_IMPLEMENTED`).
- `DOCTOR_REMOTE_EVENT_LOCAL_NOTIFICATION_VERIFIED` — **not claimed** at the device level for the
  same reason; the dedup/bridging logic is implemented and unit-tested
  (`DOCTOR_REMOTE_EVENT_LOCAL_NOTIFICATION_IMPLEMENTED`).
- `DOCTOR_PHYSICAL_DEVICE_NOTIFICATION_VERIFIED` — **not applicable / not done.** No physical
  Android device was available in this environment, and per this architecture's own constraint, no
  emulator was substituted for one. Stated explicitly rather than fabricated.
