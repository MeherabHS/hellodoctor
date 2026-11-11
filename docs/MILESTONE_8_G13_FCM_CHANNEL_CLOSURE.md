# M8-G-13 — FCM Android Notification Channel Routing: Closure

Date: 2026-11-01. Phase 1 of the three-phase Milestone 8 final local
engineering closure. Scope held to channel routing only — no change to
notification types, payload data, deep links, event semantics, or the
existing local-notification/polling delivery mechanism.

## Investigation findings (confirmed before any change)

- **The backend omitted `android.notification.channel_id` entirely.**
  `services/backend/src/services/notification_transport.rs`'s FCM message
  builder set `android.ttl`, `android.priority`, and an optional
  `android.collapse_key` — never `android.notification.channel_id`.
- **Both manifests default to the low-importance `GENERAL` channel.**
  `apps/mobile/android/app/src/main/AndroidManifest.xml:60-61` and
  `apps/mobile_patient/.../AndroidManifest.xml:60-61` both declare
  `com.google.firebase.messaging.default_notification_channel_id = GENERAL`.
  Confirmed unchanged by this pass — requirement 8 (preserve the existing
  `GENERAL` channel rather than touch an already-created channel's immutable
  importance).
- **The Doctor and Patient apps register *different* channel sets.** Doctor
  (`apps/mobile/lib/core/notifications/local_notification_service.dart`):
  `CONSULTATION_REMINDERS`/`MESSAGES` (high), `DAILY_SCHEDULE`/`ACCOUNT`/
  `FINANCIAL` (default), `GENERAL` (low). Patient
  (`apps/mobile_patient/.../local_notification_service.dart`):
  `CONSULTATION_REMINDERS`/`MESSAGES`/`APPOINTMENTS` (high), `GRIEVANCE`
  (default), `GENERAL` (low) — no `DAILY_SCHEDULE`/`ACCOUNT`/`FINANCIAL`
  channels exist there at all. This meant a single server-side channel
  mapping keyed only on `notification_type` could never be correct for both
  apps — it has to be keyed on the recipient's role too.
- **A real, independent initialization-ordering race existed in both apps.**
  Both `apps/mobile/lib/app.dart` and `apps/mobile_patient/lib/app.dart`
  fired channel creation (`LocalNotificationService.init()`) and push-service
  initialization as two independent `unawaited(...)` calls, with no ordering
  guarantee between them. On a genuinely first launch, a background push
  referencing a channel ID could theoretically arrive before that channel
  had been created — Android silently drops a notification whose
  `channel_id` does not yet exist, rather than falling back. Fixed by
  sequencing (not by adding a blocking `await` on the UI thread).
- The existing notification-type taxonomy already includes a
  `notification_type_registry` table with a `category` column
  (`TRANSACTIONAL`/`SECURITY`/`CLINICAL`/`OPERATIONAL`/`ENGAGEMENT`) — not
  reused directly here, since its categories don't line up 1:1 with the
  Flutter apps' actual channel names (e.g. `CONSULTATION_REMINDER_15M` and
  `NEW_CHAT_MESSAGE` are both `CLINICAL` in that registry but map to two
  different, both high-importance, Android channels). The new mapping
  function mirrors the Flutter `forType()` functions exactly instead.

## Implementation

1. **`services/backend/src/services/notification_transport.rs`**:
   - New `android_channel_id(notification_type, recipient_role) -> &'static str`
     — a pure function mirroring both Dart `NotificationChannels.forType()`
     implementations exactly, falling back to `"GENERAL"` for anything
     unmapped (same fallback semantics as the Dart source).
   - `PushOptions` gained one field, `recipient_role: String`.
   - The FCM message-building logic was extracted out of `send_push` into a
     new pure function, `build_fcm_message(...)`, specifically so it could be
     unit-tested without any network or Firebase credential — the request
     this inspection explicitly required. It now sets
     `android.notification.channel_id` to the mapped value.
2. **`services/backend/src/services/notification_scheduler.rs::send_with_lease`**:
   resolves `recipient_role` via one additional indexed lookup,
   `SELECT role::text FROM users WHERE id=$1` (the exact `JOIN users` pattern
   already used elsewhere in this codebase for role/language resolution,
   e.g. `notification_service.rs::system_appointment_recipients`), and passes
   it into `PushOptions`. No new grant was needed —
   `hellodoctor_system_worker` already has `SELECT` on `users`
   (`migrations/20261020000001_doctor_engagement_notifications.up.sql:210`).
3. **`apps/mobile/lib/app.dart`** and **`apps/mobile_patient/lib/app.dart`**:
   the two independent `unawaited(...)` calls for channel creation and push
   initialization were replaced with one `unawaited(initA().then((_) => initB()))`
   chain each, so channel creation always completes first, without making
   either block the first frame.

No change was made to `AndroidManifest.xml` in either app (the `GENERAL`
fallback was already correct and is preserved), to any notification type,
payload field, deep link, or to the existing Layer 1 (foreground poll) /
Layer 2 (background `WorkManager`) local-notification delivery paths — this
closure is additive to the FCM-specific path only, so no duplicate delivery
path was introduced (requirement 10).

## Tests (all local, deterministic, no Firebase credentials)

New, in `services/backend/src/services/notification_transport.rs`
(`#[cfg(test)] mod m8_g13_channel_routing_tests`), run via `cargo test --lib`:

- `doctor_channel_mapping_matches_dart_source` — asserts every Doctor-app
  mapping against the Dart source, value for value.
- `patient_channel_mapping_matches_dart_source` — same for the Patient app,
  including the negative case that Doctor-only types resolve to `GENERAL`
  (not a Doctor-only channel ID) when the role is `PATIENT`.
- `unknown_role_falls_back_to_general_channel_set` — an unrecognized role
  (e.g. an admin variant) never panics and always resolves to a real,
  universally-present channel.
- `built_fcm_message_carries_the_mapped_channel_id` — a synthetic FCM HTTP v1
  payload is built (no network) and asserted to actually contain
  `android.notification.channel_id`, plus confirms no clinical content
  (title/body) leaks into the `data` map.
- `built_fcm_message_falls_back_to_general_for_unmapped_type_and_role` —
  the fallback path, asserted on the actual built payload.
- `every_known_notification_type_resolves_without_panicking` — all 41
  current values of the `notifications_type_check` CHECK constraint
  (`migrations/20261124000001_milestone4_5_engagement_and_holidays.up.sql`)
  are run through the mapping function for both roles.

Result: `cargo test --lib` — **6/6 new tests pass**; `cargo clippy
--all-targets -- -D warnings` clean; `cargo check --tests` clean.

**Regression check** (existing suite, run against a disposable, explicitly-
owned Redis fixture, never shared dev Redis): the full
`push_notification_delivery.rs` file — **5/5 passed**, unaffected by the new
`recipient_role` lookup or the payload change.

**Flutter regression check**, both apps, after the `app.dart` sequencing
change: `flutter analyze` clean on both; full `flutter test` —
**Doctor: 302/302 passed**, **Patient: 364/364 passed**.

## What remains explicitly unverified — real-device push acceptance

Per the product owner's own stated separation: this closes the **local code
completion** of channel routing. It does **not**, and cannot, establish that
a real backgrounded or terminated phone has actually received and displayed
a push on the correct channel with audible/heads-up behavior — that requires
a configured Firebase project and `google-services.json` on-device, both of
which remain blocked on **M8-G-01** (unchanged — no Firebase credentials
exist in this environment). No claim of real-device delivery is made here.

## Documentation / gap-assessment update

`docs/MILESTONE_8_REMAINING_GAP_ASSESSMENT.md`'s M8-G-13 entry is updated:
local-engineering scope (channel mapping, payload correctness, channel
creation ordering) closed to GREEN; real-device/provider acceptance remains
explicitly tracked under the existing M8-G-01 blocker, not reopened or
conflated with this closure.
