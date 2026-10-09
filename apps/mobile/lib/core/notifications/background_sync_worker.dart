/// Layer 2 of the no-external-push notification architecture (see
/// `docs/DOCTOR_NOTIFICATION_DELIVERY_ARCHITECTURE.md`): OS-supported
/// background synchronization via the `workmanager` package.
///
/// This wakes on Android's own best-effort periodic schedule (NOT an exact
/// alarm — see [registerBackgroundSync]), runs an authenticated sync against
/// the backend using the app's existing session, and shows local device
/// notifications for anything newly discovered. It also feeds the doctor's
/// current appointment queue to Layer 3's
/// `ScheduleNotificationReconciler.reconcile` so the pre-scheduled daily
/// summary / T-15 reminders stay current even while the app process isn't
/// running.
///
/// Deliberately split into:
///  - pure decision logic ([computeNewNotifications],
///    [doctorIdentifierFromAccessToken]) with zero plugin/network/storage
///    dependency, unit-tested in `test/background_sync_worker_test.dart`;
///  - a thin impure shell ([runBackgroundSyncOnce], [callbackDispatcher])
///    that does the actual plugin/network/storage calls and cannot be run
///    under `flutter test` (no real Android platform channel in that host).
library;

import 'dart:convert';

import 'package:workmanager/workmanager.dart';

import '../network/api_client.dart';
import '../../features/doctor/doctor_repository.dart';
import '../../features/notifications/notification_models.dart';
import '../../features/notifications/notification_repository.dart';
import 'local_notification_service.dart';
import 'notification_ids.dart';
import 'schedule_notification_reconciler.dart';

/// Unique Android WorkManager task identifiers. [backgroundSyncUniqueName]
/// is the "only one of these may exist" key passed to
/// `registerPeriodicTask`/`cancelByUniqueName`; [backgroundSyncTaskName] is
/// the value handed back to [callbackDispatcher] in `taskName`.
const String backgroundSyncUniqueName = 'hellodoctor_doctor_background_sync_v1';
const String backgroundSyncTaskName = 'hellodoctor.doctor.background_sync';

/// Secure-storage key for this worker's own dedup cursor. Holds the newest
/// `NotificationDto.createdAt` (UTC, ISO-8601) seen on any previous run — NOT
/// a backend notification id, since the `/notifications` list has no numeric
/// ordinal to use as a cursor. ISO-8601 UTC timestamp strings compare
/// correctly with plain string ordering, so no parsing is required to decide
/// "is run N's item newer than the last run's newest item" at the storage
/// layer — only [computeNewNotifications] parses it, to compare `DateTime`
/// instants rather than strings (safe either way here, but clearer).
///
/// Named for what it actually stores (a timestamp, not an id) rather than
/// literally reusing the illustrative key name from the originating spec.
const String backgroundSyncLastSeenCreatedAtKey =
    'background_sync_last_seen_created_at';

// ---------------------------------------------------------------------
// Pure decision logic — zero plugin/network/storage dependency.
// ---------------------------------------------------------------------

/// One notification this run has newly discovered, plus the exact
/// [LocalNotificationService.showNow] arguments to use for it. Keeping this
/// as data (rather than calling the plugin directly) is what makes
/// [computeNewNotifications] unit-testable without a platform channel.
class NewNotificationDecision {
  const NewNotificationDecision({
    required this.id,
    required this.channel,
    required this.title,
    required this.body,
    required this.payload,
    required this.source,
  });

  final int id;
  final String channel;
  final String title;
  final String body;
  final String payload;
  final NotificationDto source;
}

/// The result of one reconciliation pass over the fetched notification list.
class SyncDecision {
  const SyncDecision({required this.newItems, required this.nextCursor});

  /// Notifications strictly newer than the persisted cursor, oldest first.
  final List<NewNotificationDecision> newItems;

  /// The cursor value to persist after this run (the newest `createdAt`
  /// across the whole fetched list, encoded as it should be written to
  /// storage). Never null as long as at least one notification was fetched
  /// or a cursor already existed; re-persisting an unchanged value when
  /// nothing qualifies as "new" is harmless (spec: "even if zero were new
  /// this run, re-persisting the same value is harmless").
  final String? nextCursor;
}

/// Pure dedup decision: given the newest-first [fetched] notification list
/// and the previously persisted [cursorIso] (an ISO-8601 UTC timestamp
/// string, or null on the very first run), decides which notifications are
/// "newly discovered" and what the next cursor should be.
///
/// A notification counts as new only if its `createdAt` is strictly after
/// the cursor — this is what makes running the exact same fetched list
/// twice in a row produce zero new decisions the second time (spec section
/// 31, "worker executes twice -> no duplicate"): after the first run the
/// cursor advances to the newest `createdAt` in that list, so nothing in an
/// identical second fetch is still strictly newer than it.
SyncDecision computeNewNotifications({
  required List<NotificationDto> fetched,
  required String? cursorIso,
}) {
  final cursor =
      cursorIso == null ? null : DateTime.tryParse(cursorIso)?.toUtc();

  final newItems = <NewNotificationDecision>[];
  DateTime? newest = cursor;
  for (final dto in fetched) {
    final createdUtc = dto.createdAt.toUtc();
    if (newest == null || createdUtc.isAfter(newest)) newest = createdUtc;
    if (cursor == null || createdUtc.isAfter(cursor)) {
      final payload = remoteEventKey(dto.id);
      newItems.add(NewNotificationDecision(
        id: stableNotificationId(payload),
        channel: NotificationChannels.forType(dto.type).id,
        title: dto.title,
        body: dto.body,
        payload: payload,
        source: dto,
      ));
    }
  }

  return SyncDecision(
    newItems: newItems,
    nextCursor: newest?.toIso8601String(),
  );
}

/// Decodes the doctor's own stable identifier from an access token JWT's
/// payload, without any network/storage dependency (pure — takes the raw
/// token string, returns the claim value or null).
///
/// HONEST CAVEAT: the backend's `Claims` struct
/// (`services/backend/src/crypto.rs`) only carries `sub`/`user_id` (both set
/// to the authenticated `users.id`) — there is no `doctor_profiles.id` claim,
/// and no persisted `doctor_profiles.id` is reachable from a background
/// isolate (it exists only in in-memory `DoctorCubit`/`ProfileDto` state
/// built from an authenticated API call the main app isolate already made).
/// This mirrors the exact precedent already established in
/// `core/cache/doctor_cache_store.dart`'s private `_identitySeed` (same
/// claim-key fallback order, same "treat a malformed token as opaque, never
/// throw" behavior) rather than inventing a new identity derivation.
///
/// This is acceptable here because [ScheduleNotificationReconciler.reconcile]
/// only ever uses `doctorId` to namespace its own local, per-doctor dedup key
/// (`daily_schedule:<doctorId>:<date>`) — it is never sent to the backend or
/// compared against `doctor_profiles.id` anywhere. What matters is that the
/// same device/session consistently produces the same `doctorId` across
/// runs, which the JWT subject does.
String? doctorIdentifierFromAccessToken(String? jwt) {
  if (jwt == null || jwt.isEmpty) return null;
  final parts = jwt.split('.');
  if (parts.length != 3) return null;
  try {
    final normalized = base64Url.normalize(parts[1]);
    final payload = jsonDecode(utf8.decode(base64Url.decode(normalized)));
    if (payload is Map) {
      for (final key in const ['doctor_id', 'user_id', 'sub']) {
        final id = payload[key];
        if (id != null && id.toString().isNotEmpty) return id.toString();
      }
    }
  } catch (_) {
    // A malformed/unexpected token shape is treated as "no identifier
    // available" rather than crashing the background isolate.
  }
  return null;
}

// ---------------------------------------------------------------------
// Impure shell — the only part of this file touching the plugin, network,
// or secure storage. Not unit-testable under `flutter test` (no platform
// channel in that host); exercised manually/on-device only.
// ---------------------------------------------------------------------

/// Runs [body] and always resolves to `true`, regardless of what [body]
/// throws. This is the whole graceful-failure contract required by the
/// spec — a background sync failure (an expired/revoked session surfacing
/// as an [ApiException], or any other unexpected error) must stop quietly
/// rather than returning `false`/throwing, which would trigger WorkManager's
/// backoff-retry policy and risk an authentication-storm. Nothing about the
/// failure (message, stack) is logged: even an `ApiException.userMessage` is
/// operator-facing text this background path has no justified reason to
/// persist or print.
///
/// Kept as its own tiny function — separate from the plugin/network calls in
/// [runBackgroundSyncOnce] — specifically so this contract can be unit
/// tested in isolation, without a platform channel.
Future<bool> runGracefully(Future<void> Function() body) async {
  try {
    await body();
  } catch (_) {
    // Deliberately swallows every error type, not just ApiException: a
    // malformed response or any other unexpected failure must not crash the
    // background isolate or propagate into a WorkManager retry either.
  }
  return true;
}

/// Runs one background sync pass: fetches the doctor's notification inbox
/// and appointment queue with a fresh [api], shows local notifications for
/// anything newly discovered since [cursorStore]'s persisted cursor, and
/// reconciles Layer 3's pre-scheduled daily-summary/T-15 reminders.
///
/// Always returns `true` — see [runGracefully].
Future<bool> runBackgroundSyncOnce({
  required ApiClient api,
  required TokenStore cursorStore,
}) =>
    runGracefully(() async {
      // Separate isolate from the main app — its own fresh plugin state, so
      // this must be (re-)initialized here. Idempotent: safe every run.
      await LocalNotificationService.init();

      final notificationRepo = NotificationRepository(api);
      final fetched = await notificationRepo.list(limit: 25);
      // Exercised per spec so the background path's auth/transport behavior
      // matches the foreground inbox screen's; this pass has no badge/counter
      // surface to update with the result, so the value itself is discarded.
      await notificationRepo.unreadCount();

      final cursor = await cursorStore.read(backgroundSyncLastSeenCreatedAtKey);
      final decision =
          computeNewNotifications(fetched: fetched, cursorIso: cursor);

      for (final item in decision.newItems) {
        await LocalNotificationService.showNow(
          id: item.id,
          channel: NotificationChannels.forType(item.source.type),
          title: item.title,
          body: item.body,
          payload: item.payload,
        );
      }
      if (decision.nextCursor != null) {
        await cursorStore.write(
            backgroundSyncLastSeenCreatedAtKey, decision.nextCursor!);
      }

      // --- Layer 3 reconciliation ------------------------------------
      // KNOWN LIMITATION (documented, not silent): a single-page fetch of
      // up to 100 appointments, with no delta sync and no pagination loop.
      // A doctor with a queue deeper than 100 entries will have reminders
      // reconciled only for the first page returned by the backend's
      // default ordering. Acceptable for this best-effort background pass;
      // the foreground app's own queue screen remains the authoritative,
      // fully paginated view.
      final doctorId =
          doctorIdentifierFromAccessToken(await api.read('access_token'));
      if (doctorId != null) {
        final appointments = await DoctorRepository(api).queue(limit: 100);
        await ScheduleNotificationReconciler.reconcile(
          doctorId: doctorId,
          appointments: appointments,
          nowUtc: DateTime.now().toUtc(),
        );
      }
    });

/// Top-level background entry point required by `workmanager` ^0.5.2's
/// documented API (`Workmanager().executeTask((taskName, inputData) async
/// {...})`, confirmed against the installed package source at
/// `workmanager-0.5.2/lib/src/workmanager.dart`). Must stay a top-level (or
/// static) function, never a class instance method, and keep the
/// `@pragma('vm:entry-point')` annotation — mandatory for Flutter 3.1+/an
/// obfuscated build to still be able to resolve this as a background
/// isolate entry point.
@pragma('vm:entry-point')
void callbackDispatcher() {
  Workmanager().executeTask((taskName, inputData) async {
    if (taskName != backgroundSyncTaskName) {
      // Not a task this worker recognizes (e.g. a future, differently-named
      // task sharing the same callback dispatcher) — nothing to do, but
      // still answer "success" rather than failing a task that isn't ours.
      return true;
    }
    // A background isolate has no access to whatever `ApiClient`/repository
    // instances the main app isolate is holding — each one must be freshly
    // constructed here. `ApiClient()`'s default constructor needs no
    // arguments and internally builds its own `SecureTokenStore()` reading
    // from the OS keystore, so this fresh instance transparently picks up
    // the same persisted session the main app isolate is using.
    final api = ApiClient();
    final cursorStore = SecureTokenStore();
    return runBackgroundSyncOnce(api: api, cursorStore: cursorStore);
  });
}

/// Registers the periodic background sync. Call once from the main app's
/// startup (NOT called from this file itself — wiring this into `app.dart`
/// is a separate step owned elsewhere).
///
/// `REQUESTED_INTERVAL = 15 minutes` — Android's documented minimum enforced
/// periodic interval; a shorter value would be silently clamped up to 15 by
/// the OS, so there is no point requesting less.
/// `ACTUAL_OS_GUARANTEE = BEST EFFORT`. Doze mode, battery optimization, and
/// manufacturer-specific power management may defer or skip individual
/// runs — this is documented Android OS behavior, not a HelloDoctor defect,
/// and must never be represented anywhere as an exact/guaranteed execution
/// schedule.
///
/// Uses `ExistingWorkPolicy.keep`: if this is called again on a later app
/// launch while the periodic task is already registered, the existing
/// schedule is left alone rather than being torn down and restarted (which
/// would otherwise reset the OS's own internal backoff/timing state on every
/// single app open).
Future<void> registerBackgroundSync() async {
  await Workmanager().initialize(callbackDispatcher);
  await Workmanager().registerPeriodicTask(
    backgroundSyncUniqueName,
    backgroundSyncTaskName,
    frequency: const Duration(minutes: 15),
    constraints: Constraints(networkType: NetworkType.connected),
    existingWorkPolicy: ExistingWorkPolicy.keep,
  );
}
