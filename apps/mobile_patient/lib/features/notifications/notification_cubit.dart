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

/// Mirrors `apps/mobile`'s `NotificationCubit` — same optimistic
/// mark-read/rollback shape, same Layer 1 realtime-poll-plus-local-notify
/// bridge from `docs/DOCTOR_NOTIFICATION_DELIVERY_ARCHITECTURE.md` (there is
/// no WebSocket/SSE client anywhere in this codebase to subscribe to
/// instead).
///
/// Per the final notification architecture decision, the backend/FCM is the
/// sole scheduler for every server-driven push, including the T-15
/// consultation reminder for both Doctor and Patient
/// (`notification_scheduler.rs::run_t15_reminder_pass`) — this cubit
/// deliberately does not run any local appointment-reminder scheduling of
/// its own. `LocalNotificationService.showNow` below is reactive display
/// only (an already-delivered backend event shown while the app is in the
/// foreground), never a locally-computed future reminder.
class NotificationCubit extends Cubit<NotificationState> {
  NotificationCubit(this.repo) : super(const NotificationState());
  final NotificationRepository repo;

  bool _loadInFlight = false;
  Timer? _pollTimer;
  bool _pollingStarted = false;

  /// The newest backend `createdAt` this cubit has already either notified
  /// for or folded into its baseline. Null until the very first successful
  /// fetch sets it.
  DateTime? _lastNotifiedCreatedAt;
  bool _baselineEstablished = false;

  /// Starts the Layer 1 realtime poll. Idempotent — safe to call more than
  /// once; only the first call has any effect. Deliberately not started
  /// automatically from [load] or the constructor — see `apps/mobile`'s
  /// identical method for the full rationale (a pending `Timer.periodic`
  /// surviving past a short-lived test's teardown trips `flutter_test`'s
  /// "no pending timer" check).
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
  /// to show yet; a subsequent call refreshes quietly in the background.
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
        lastUpdatedAt: DateTime.now().toUtc(),
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

  /// The quiet poll variant used by the 20s timer — never touches
  /// `loading`/`refreshing`/`busy`, and a failure is swallowed rather than
  /// surfaced, since the patient never asked for this particular fetch.
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

  /// Dedup + remote-event-to-local-notification bridging, identical logic to
  /// `apps/mobile`'s cubit: establishes a baseline on the first fetch (so the
  /// patient is never notified for everything that predates opening the
  /// app), then notifies only for items strictly newer than that baseline on
  /// every fetch after.
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
  /// `flutter test` host) can never crash fetch or poll handling.
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

  @override
  Future<void> close() {
    _pollTimer?.cancel();
    return super.close();
  }
}
