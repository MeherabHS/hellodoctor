import 'dart:async';

import 'package:flutter/widgets.dart' show WidgetsBinding, AppLifecycleState;
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/network/api_client.dart';
import '../../core/notifications/local_notification_service.dart';
import '../../core/notifications/notification_ids.dart';
import 'notification_models.dart';
import 'notification_repository.dart';

const _unset = Object();

class NotificationState {
  const NotificationState({
    this.hasOlder = false,
    this.loadingOlder = false,
    this.loading = false,
    this.refreshing = false,
    this.busy = false,
    this.error,
    this.items = const [],
    this.unreadCount = 0,
    this.lastUpdatedAt,
  });
  final bool loading, refreshing, busy, hasOlder, loadingOlder;
  final String? error;
  final List<NotificationDto> items;
  final int unreadCount;
  final DateTime? lastUpdatedAt;

  NotificationState copyWith({
    bool? hasOlder,
    bool? loadingOlder,
    bool? loading,
    bool? refreshing,
    bool? busy,
    Object? error = _unset,
    List<NotificationDto>? items,
    int? unreadCount,
    DateTime? lastUpdatedAt,
  }) =>
      NotificationState(
        hasOlder: hasOlder ?? this.hasOlder,
        loadingOlder: loadingOlder ?? this.loadingOlder,
        loading: loading ?? this.loading,
        refreshing: refreshing ?? this.refreshing,
        busy: busy ?? this.busy,
        error: identical(error, _unset) ? this.error : error as String?,
        items: items ?? this.items,
        unreadCount: unreadCount ?? this.unreadCount,
        lastUpdatedAt: lastUpdatedAt ?? this.lastUpdatedAt,
      );
}

/// Same busy/optimistic-then-confirm/rollback-on-error shape as
/// `features/doctor/doctor_cubit.dart`'s `DoctorCubit.toggleDuty()`.
///
/// Also owns Layer 1 of the no-external-push notification delivery
/// architecture (docs/DOCTOR_NOTIFICATION_DELIVERY_ARCHITECTURE.md):
/// real-time synchronization while the Doctor app process is alive. There is
/// no WebSocket/SSE client anywhere in this codebase to subscribe to, so
/// this is a `Timer.periodic` poll — the same shape as
/// `features/doctor/doctor_pages.dart`'s `_ScheduleState` (15s) and
/// `app.dart`'s own simpler 30s timer: only actually fires its fetch while
/// `WidgetsBinding.instance.lifecycleState == AppLifecycleState.resumed`.
/// [startPolling]'s own default interval (20s) is a deliberate middle ground
/// between those two existing precedents, but the real production call site
/// (`app.dart`'s `BlocProvider<NotificationCubit>.create`) currently passes
/// an explicit 1-minute interval instead — a doctor should still see a
/// brand-new notification promptly, just not as tightly as this class's own
/// default. The poll itself is started by an explicit call to [startPolling]
/// (see its own doc comment for why this isn't automatic).
///
/// NOTE (notification-architecture consolidation): this cubit previously also
/// drove Layer 3's local `DAILY_CONSULTATION_SUMMARY` schedule reconciliation
/// via optional `doctorRepo`/`doctorId` constructor parameters. That local
/// scheduler was removed once the backend's own `DAILY_CONSULTATION_SUMMARY`
/// (`services/backend/src/services/notification_scheduler.rs::run_daily_summary_pass`,
/// covered by `services/backend/tests/api_tests.rs`) was confirmed as the
/// sole, reliable source — this cubit's own Layer 1 poll above already
/// surfaces that backend-created row through the ordinary dedup/bridging
/// path below, so no separate local reconciliation is needed.
class NotificationCubit extends Cubit<NotificationState> {
  NotificationCubit(this.repo, {DateTime Function()? now})
      : _now = now ?? DateTime.now,
        super(const NotificationState());
  final DateTime Function() _now;
  final NotificationRepository repo;

  bool _loadInFlight = false;

  Timer? _pollTimer;
  bool _pollingStarted = false;

  /// The newest backend `createdAt` this cubit has already either notified
  /// for or folded into its baseline. Null until the very first successful
  /// fetch sets it.
  DateTime? _lastNotifiedCreatedAt;

  /// True once the very first successful fetch has set the baseline above
  /// (distinct from `_lastNotifiedCreatedAt` itself being non-null, because
  /// an empty first fetch has nothing to derive a baseline `createdAt` from
  /// but must still not treat every later item as "new since baseline").
  bool _baselineEstablished = false;

  /// Starts the Layer 1 realtime poll described in the class doc. Idempotent
  /// — safe to call more than once; only the first call has any effect.
  ///
  /// Deliberately NOT started automatically from [load] or the constructor:
  /// a `Cubit` has no `initState`/widget-lifecycle hook of its own, and this
  /// cubit is also constructed in contexts (e.g. widget tests that build a
  /// `NotificationCenterPage` directly around a short-lived cubit and call
  /// `load()` without ever calling `startPolling()`) that must not have a
  /// background timer silently begin running just because a fetch
  /// succeeded — a `Timer.periodic` left pending past such a test's own
  /// teardown trips `flutter_test`'s "no pending timer" invariant check
  /// before `addTearDown(cubit.close)` has had a chance to run. Call this
  /// explicitly, once, from wherever the cubit is wired up for real
  /// (a future app-bootstrap step) after construction.
  void startPolling({Duration interval = const Duration(seconds: 20)}) {
    if (_pollingStarted || isClosed) return;
    _pollingStarted = true;
    _pollTimer = Timer.periodic(interval, (_) {
      if (WidgetsBinding.instance.lifecycleState == AppLifecycleState.resumed) {
        unawaited(_poll());
      }
    });
  }

  /// Initial fetch — shows a full-page spinner only while there is nothing
  /// to show yet; a subsequent call (e.g. on reopening the page) refreshes
  /// quietly in the background instead of blanking an already-populated list.
  bool _olderLoaded = false;
  Future<void> load() async {
    if (_loadInFlight) return;
    _loadInFlight = true;
    final hadItems = state.items.isNotEmpty;
    emit(state.copyWith(loading: !hadItems, refreshing: hadItems, error: null));
    try {
      final results = await Future.wait([repo.list(), repo.unreadCount()]);
      if (isClosed) return;
      final items = results[0] as List<NotificationDto>;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        error: null,
        items: _olderLoaded ? _mergeItems(state.items, items) : items,
        hasOlder: _olderLoaded ? state.hasOlder : items.length == 25,
        unreadCount: results[1] as int,
        lastUpdatedAt: _now().toUtc(),
      ));
      _processFetchedItems(items);
    } catch (e) {
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        error: _message(e, 'Unable to load notifications.'),
      ));
    } finally {
      _loadInFlight = false;
    }
  }

  List<NotificationDto> _mergeItems(
      List<NotificationDto> old, List<NotificationDto> next) {
    final items = {
      for (final item in old) item.id: item,
      for (final item in next) item.id: item
    }.values.toList();
    items.sort((a, b) {
      final time = b.createdAt.compareTo(a.createdAt);
      return time == 0 ? b.id.compareTo(a.id) : time;
    });
    return items;
  }

  Future<List<NotificationDto>> _delta() async {
    var cursor = state.items.isNotEmpty
        ? _mergeItems([], state.items).first
        : NotificationDto(
            id: '00000000-0000-0000-0000-000000000000',
            userId: '',
            type: '',
            title: '',
            body: '',
            createdAt: DateTime.fromMillisecondsSinceEpoch(0, isUtc: true),
            deliveryStatus: '');
    final collected = <NotificationDto>[];
    for (var pageNumber = 0; pageNumber < 4; pageNumber++) {
      final page = await repo.newerThan(cursor);
      collected.addAll(page);
      if (page.length < 25) break;
      cursor = page.last;
    }
    return collected;
  }

  bool _olderBusy = false;
  Future<void> loadOlder() async {
    if (_olderBusy || isClosed || !state.hasOlder || state.items.isEmpty) {
      return;
    }
    _olderBusy = true;
    emit(state.copyWith(loadingOlder: true));
    try {
      final older = await repo.olderThan(state.items.last);
      if (!isClosed) {
        _olderLoaded = true;
        emit(state.copyWith(
            items: _mergeItems(state.items, older),
            hasOlder: older.length == 25));
      }
    } catch (e) {
      if (!isClosed) {
        emit(state.copyWith(
            error: _message(e, 'Unable to load earlier notifications.')));
      }
    } finally {
      _olderBusy = false;
      if (!isClosed) emit(state.copyWith(loadingOlder: false));
    }
  }

  Future<void> refresh() => load();

  /// The quiet poll variant used by the 20s timer. Reuses `load()`'s exact
  /// `Future.wait([repo.list(), repo.unreadCount()])` fetch, but — unlike
  /// `load()` — never touches `loading`/`refreshing`/`busy` at all, so a
  /// silent background poll can never flash a spinner or progress
  /// affordance into view while the doctor is looking at something else.
  /// `load()` itself isn't reused unmodified here because even its
  /// already-quieter "has items, so just `refreshing: true`" path still
  /// emits that flag, which the notification-center page reads to show a
  /// thin progress indicator — appropriate for an explicit reload the
  /// doctor asked for, not for an automatic 20s tick.
  ///
  /// A poll failure (network blip, etc.) is deliberately swallowed rather
  /// than surfaced as `state.error`: the doctor never asked for this fetch,
  /// so it should fail invisibly; an explicit `load()`/pull-to-refresh still
  /// surfaces errors exactly as before.
  Future<void> _poll() async {
    if (_loadInFlight || isClosed) return;
    _loadInFlight = true;
    try {
      final results = await Future.wait([_delta(), repo.unreadCount()]);
      if (isClosed) return;
      final items = results[0] as List<NotificationDto>;
      emit(state.copyWith(
        error: null,
        items: _mergeItems(state.items, items),
        unreadCount: results[1] as int,
        lastUpdatedAt: DateTime.now().toUtc(),
      ));
      _processFetchedItems(items);
    } catch (_) {
      // Swallowed — see doc comment above.
    } finally {
      _loadInFlight = false;
    }
  }

  /// Dedup + remote-event-to-local-notification bridging. Called after every
  /// successful fetch (both `load()` and each `_poll()` tick) with the
  /// freshly-fetched item list.
  ///
  /// On the very first successful fetch this only establishes the baseline
  /// (the newest `createdAt` already present) so the doctor is never
  /// notified for everything that already existed before the app was
  /// opened. Every fetch after that notifies for items strictly newer than
  /// the current baseline, then advances the baseline to the newest
  /// `createdAt` across the *whole* fetched list — even items that weren't
  /// newer than the old baseline — which keeps it simple and monotonic for
  /// this small list.
  ///
  /// Because the baseline only ever advances forward and the comparison is
  /// strict (`isAfter`), the same item surfacing again in a later poll is
  /// never re-notified: duplicate discovery of an already-seen backend
  /// event cannot produce a duplicate local notification. As defense in
  /// depth, `LocalNotificationService.showNow`'s own deterministic id
  /// (`stableNotificationId(remoteEventKey(dto.id))`) would also naturally
  /// *replace*, not duplicate, the OS notification even if this logic
  /// somehow fired twice for the same id.
  void _processFetchedItems(List<NotificationDto> items) {
    if (items.isEmpty) {
      _baselineEstablished = true;
      return;
    }
    final newest =
        items.map((n) => n.createdAt).reduce((a, b) => a.isAfter(b) ? a : b);
    if (!_baselineEstablished) {
      _lastNotifiedCreatedAt = newest;
      _baselineEstablished = true;
      return;
    }
    final baseline = _lastNotifiedCreatedAt;
    for (final dto in items) {
      if (baseline == null || dto.createdAt.isAfter(baseline)) {
        unawaited(_showLocalNotification(dto));
      }
    }
    if (baseline == null || newest.isAfter(baseline)) {
      _lastNotifiedCreatedAt = newest;
    }
  }

  /// Isolated so a platform-channel failure (e.g. no
  /// `flutter_local_notifications` plugin registered, as in a plain
  /// `flutter test` host with no platform bindings) can never crash fetch or
  /// poll handling — this is a best-effort local notification, not a
  /// correctness-critical write.
  Future<void> _showLocalNotification(NotificationDto dto) async {
    try {
      await LocalNotificationService.showNow(
        id: stableNotificationId(remoteEventKey(dto.id)),
        channel: NotificationChannels.forType(dto.type),
        title: dto.title,
        body: dto.body,
        payload: remoteEventKey(dto.id),
      );
    } catch (_) {
      // Best-effort — see doc comment above.
    }
  }

  /// Marks a single notification read: optimistic local update, then
  /// confirms with the backend; rolls back and surfaces an error if the
  /// confirmation fails. A no-op for an unknown id or an already-read row.
  Future<void> markRead(String id) async {
    if (isClosed) return;
    final index = state.items.indexWhere((n) => n.id == id);
    if (index == -1 || state.items[index].isRead) return;
    final previousItems = state.items;
    final previousUnread = state.unreadCount;
    final updated = [...state.items];
    updated[index] = updated[index].copyWith(readAt: DateTime.now().toUtc());
    emit(state.copyWith(
        items: updated,
        unreadCount: previousUnread > 0 ? previousUnread - 1 : 0,
        error: null));
    try {
      await repo.markRead(id);
    } catch (e) {
      if (isClosed) return;
      emit(state.copyWith(
        items: previousItems,
        unreadCount: previousUnread,
        error: _message(e, 'This notification could not be marked as read.'),
      ));
    }
  }

  Future<void> markAllRead() async {
    if (isClosed || state.busy || state.unreadCount == 0) return;
    final previousItems = state.items;
    final previousUnread = state.unreadCount;
    final now = DateTime.now().toUtc();
    emit(state.copyWith(
        busy: true,
        error: null,
        items: [
          for (final n in state.items) n.isRead ? n : n.copyWith(readAt: now)
        ],
        unreadCount: 0));
    try {
      await repo.markAllRead();
      if (isClosed) return;
      emit(state.copyWith(busy: false));
    } catch (e) {
      if (isClosed) return;
      emit(state.copyWith(
        busy: false,
        items: previousItems,
        unreadCount: previousUnread,
        error: _message(e, 'Could not mark all notifications as read.'),
      ));
    }
  }

  static String _message(Object error, String fallback) {
    if (error is ApiException) return error.userMessage;
    return fallback;
  }

  /// Cancels the Layer 1 poll timer so it can never fire after this cubit is
  /// disposed — mirrors `_ScheduleState.dispose()`'s `_syncTimer?.cancel()`.
  @override
  Future<void> close() {
    _pollTimer?.cancel();
    return super.close();
  }
}
