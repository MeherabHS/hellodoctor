import 'dart:async';
import 'package:dio/dio.dart';
import 'package:fake_async/fake_async.dart';
import 'package:flutter/widgets.dart' show AppLifecycleState, WidgetsBinding;
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_cubit.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_models.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_repository.dart';

NotificationDto _n(String id, {bool read = false, DateTime? createdAt}) =>
    NotificationDto(
        id: id,
        userId: 'u1',
        type: 'EARNING_CREDITED',
        title: 'Title $id',
        body: 'Body $id',
        deliveryStatus: 'DELIVERED',
        createdAt: createdAt ?? DateTime.utc(2026, 10, 5, 10),
        readAt: read ? DateTime.utc(2026, 10, 5, 11) : null);

class StubNotificationRepository extends NotificationRepository {
  StubNotificationRepository() : super(ApiClient(dio: Dio()));
  List<NotificationDto> listResponse = [_n('a'), _n('b')];
  int unreadCountResponse = 2;
  bool failMarkRead = false;
  bool failMarkAllRead = false;
  final markedReadIds = <String>[];
  int markAllReadCalls = 0;
  int listCalls = 0;
  Completer<List<NotificationDto>>? listGate;

  @override
  Future<List<NotificationDto>> list({int? offset, int? limit}) async {
    listCalls++;
    return listGate == null ? listResponse : await listGate!.future;
  }

  @override
  Future<List<NotificationDto>> newerThan(NotificationDto cursor) async {
    final rows = (await list())
        .where((n) =>
            n.createdAt.isAfter(cursor.createdAt) ||
            (n.createdAt.isAtSameMomentAs(cursor.createdAt) &&
                n.id.compareTo(cursor.id) > 0))
        .toList();
    rows.sort((a, b) {
      final time = a.createdAt.compareTo(b.createdAt);
      return time == 0 ? a.id.compareTo(b.id) : time;
    });
    return rows;
  }

  @override
  Future<int> unreadCount() async => unreadCountResponse;

  @override
  Future<void> markRead(String id) async {
    markedReadIds.add(id);
    if (failMarkRead) throw const ApiException(409, 'Already read elsewhere');
  }

  @override
  Future<int> markAllRead() async {
    markAllReadCalls++;
    if (failMarkAllRead) throw const ApiException(409, 'Server error');
    return listResponse.length;
  }
}

/// Follows the same convention as `test/doctor_cubit_test.dart`'s own
/// `StubDoctorRepository` — extends the real repository and overrides only
/// what's needed (`queue()`, the schedule reconciler's appointment source).
class StubDoctorRepository extends DoctorRepository {
  StubDoctorRepository() : super(ApiClient(dio: Dio()));
  int queueCalls = 0;
  List<AppointmentDto> queueResponse = const [];

  @override
  Future<List<AppointmentDto>> queue({int limit = 25, int offset = 0}) async {
    queueCalls++;
    return queueResponse;
  }
}

void main() {
  test('load() populates items and unread count', () async {
    final repo = StubNotificationRepository();
    final cubit = NotificationCubit(repo);
    await cubit.load();
    expect(cubit.state.loading, isFalse);
    expect(cubit.state.error, isNull);
    expect(cubit.state.items.map((n) => n.id).toList(), ['a', 'b']);
    expect(cubit.state.unreadCount, 2);
    await cubit.close();
  });

  test('load() surfaces a user-facing error on failure', () async {
    final repo = StubNotificationRepository()
      ..listGate = (Completer<List<NotificationDto>>()
        ..completeError(const ApiException(409, 'boom')));
    final cubit = NotificationCubit(repo);
    await cubit.load();
    expect(cubit.state.error, 'boom');
    expect(cubit.state.items, isEmpty);
    await cubit.close();
  });

  test('cached items are kept visible (as "refreshing") while a reload runs',
      () async {
    final repo = StubNotificationRepository();
    final cubit = NotificationCubit(repo);
    await cubit.load();
    expect(cubit.state.items, isNotEmpty);
    repo.listGate = Completer<List<NotificationDto>>();
    final states = <NotificationState>[];
    final sub = cubit.stream.listen(states.add);
    final pending = cubit.load();
    await Future<void>.delayed(Duration.zero);
    expect(states.first.refreshing, isTrue);
    expect(states.first.loading, isFalse);
    expect(states.first.items, isNotEmpty); // old data stays visible
    repo.listGate!.complete([_n('c')]);
    await pending;
    await sub.cancel();
    await cubit.close();
  });

  test('markRead() is optimistic, decrements unread count, then confirms',
      () async {
    final repo = StubNotificationRepository();
    final cubit = NotificationCubit(repo);
    await cubit.load();
    expect(cubit.state.unreadCount, 2);
    await cubit.markRead('a');
    expect(cubit.state.items.firstWhere((n) => n.id == 'a').isRead, isTrue);
    expect(cubit.state.unreadCount, 1);
    expect(repo.markedReadIds, ['a']);
    // Marking an already-read notification again is a no-op.
    await cubit.markRead('a');
    expect(repo.markedReadIds, ['a']);
    await cubit.close();
  });

  test('markRead() rolls back the optimistic update on a server failure',
      () async {
    final repo = StubNotificationRepository()..failMarkRead = true;
    final cubit = NotificationCubit(repo);
    await cubit.load();
    await cubit.markRead('a');
    expect(cubit.state.items.firstWhere((n) => n.id == 'a').isRead, isFalse);
    expect(cubit.state.unreadCount, 2);
    expect(cubit.state.error, contains('Already read elsewhere'));
    await cubit.close();
  });

  test('markAllRead() marks every item read and zeroes the unread count',
      () async {
    final repo = StubNotificationRepository();
    final cubit = NotificationCubit(repo);
    await cubit.load();
    await cubit.markAllRead();
    expect(cubit.state.items.every((n) => n.isRead), isTrue);
    expect(cubit.state.unreadCount, 0);
    expect(cubit.state.busy, isFalse);
    expect(repo.markAllReadCalls, 1);
    await cubit.close();
  });

  test('markAllRead() is a no-op when nothing is unread', () async {
    final repo = StubNotificationRepository()
      ..listResponse = [_n('a', read: true)]
      ..unreadCountResponse = 0;
    final cubit = NotificationCubit(repo);
    await cubit.load();
    await cubit.markAllRead();
    expect(repo.markAllReadCalls, 0);
    await cubit.close();
  });

  test('markAllRead() rolls back on failure and surfaces an error', () async {
    final repo = StubNotificationRepository()..failMarkAllRead = true;
    final cubit = NotificationCubit(repo);
    await cubit.load();
    await cubit.markAllRead();
    expect(cubit.state.items.every((n) => !n.isRead), isTrue);
    expect(cubit.state.unreadCount, 2);
    expect(cubit.state.busy, isFalse);
    expect(cubit.state.error, contains('Server error'));
    await cubit.close();
  });

  // --- Layer 1 realtime polling -------------------------------------------
  //
  // `WidgetsBinding.instance.lifecycleState` requires a Flutter binding to
  // be initialized, which a plain `test()` (as opposed to `testWidgets()`)
  // does not do automatically; `TestWidgetsFlutterBinding.ensureInitialized()`
  // sets one up explicitly. `fakeAsync` lets the 20s `Timer.periodic` be
  // advanced instantly instead of the test actually waiting 20 real seconds.

  test(
      'startPolling() (called explicitly once) polls every 20s while resumed, and stops after close()',
      () {
    TestWidgetsFlutterBinding.ensureInitialized();
    WidgetsBinding.instance
        .handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    fakeAsync((async) {
      final repo = StubNotificationRepository();
      final cubit = NotificationCubit(repo);
      cubit.load();
      cubit.startPolling();
      cubit.startPolling(); // idempotent — a second call must be a no-op.
      async.flushMicrotasks();
      expect(repo.listCalls, 1, reason: 'initial load() fetch');

      async.elapse(const Duration(seconds: 20));
      expect(repo.listCalls, 2, reason: 'first 20s poll tick');

      async.elapse(const Duration(seconds: 20));
      expect(repo.listCalls, 3, reason: 'second 20s poll tick');

      cubit.close();
      async.elapse(const Duration(seconds: 60));
      expect(repo.listCalls, 3,
          reason: 'no further poll fires once the cubit is closed');
    });
  });

  test('a poll tick is skipped while the app is not resumed', () {
    TestWidgetsFlutterBinding.ensureInitialized();
    fakeAsync((async) {
      WidgetsBinding.instance
          .handleAppLifecycleStateChanged(AppLifecycleState.resumed);
      final repo = StubNotificationRepository();
      final cubit = NotificationCubit(repo);
      cubit.load();
      cubit.startPolling();
      async.flushMicrotasks();
      final callsAfterLoad = repo.listCalls;

      WidgetsBinding.instance
          .handleAppLifecycleStateChanged(AppLifecycleState.paused);
      async.elapse(const Duration(seconds: 20));
      expect(repo.listCalls, callsAfterLoad,
          reason: 'paused -> the 20s tick must not fetch');

      WidgetsBinding.instance
          .handleAppLifecycleStateChanged(AppLifecycleState.resumed);
      async.elapse(const Duration(seconds: 20));
      expect(repo.listCalls, callsAfterLoad + 1,
          reason: 'resumed again -> the next 20s tick fetches');

      cubit.close();
    });
  });

  // --- Dedup / remote-event-to-local-notification bridging ---------------
  //
  // `LocalNotificationService.showNow` is a static wrapper around
  // `flutter_local_notifications`'s platform channel with no injectable
  // fake available in this codebase (no mocking library, no seam). These
  // tests therefore cannot assert that a device notification was actually
  // shown; what they DO verify is that the dedup/baseline bookkeeping that
  // decides WHETHER to call it runs correctly end-to-end — including
  // tolerating `showNow` throwing in this plugin-less test host — by
  // checking there is no crash and that `state` ends up correct, exactly as
  // the task's test guidance allows ("scope your tests to avoid actually
  // exercising that call path directly" / "catch gracefully").

  test(
      'first load() only establishes the createdAt baseline (no backlog renotification), and a later strictly-newer item is processed without throwing',
      () async {
    final repo = StubNotificationRepository()
      ..listResponse = [_n('a', createdAt: DateTime.utc(2026, 10, 5, 10))]
      ..unreadCountResponse = 1;
    final cubit = NotificationCubit(repo);
    await cubit.load(); // baseline established from 'a', nothing "new" yet.
    expect(cubit.state.items.map((n) => n.id).toList(), ['a']);

    // A genuinely newer item appears on the next fetch.
    repo.listResponse = [
      _n('a', createdAt: DateTime.utc(2026, 10, 5, 10)),
      _n('b', createdAt: DateTime.utc(2026, 10, 5, 11)),
    ];
    repo.unreadCountResponse = 2;
    await cubit.load(); // must not throw even though no notification plugin
    // is registered in this test host (showNow's failure is caught
    // internally — see NotificationCubit._showLocalNotification).
    expect(cubit.state.items.map((n) => n.id).toList(), ['a', 'b']);
    await cubit.close();
  });

  test(
      'duplicate discovery of an already-seen item across repeated fetches does not change unreadCount unexpectedly',
      () async {
    final repo = StubNotificationRepository()
      ..listResponse = [_n('a', createdAt: DateTime.utc(2026, 10, 5, 10))]
      ..unreadCountResponse = 1;
    final cubit = NotificationCubit(repo);
    await cubit.load();
    await cubit.load(); // same item again — baseline already covers it.
    await cubit.load(); // and again.
    expect(cubit.state.items.map((n) => n.id).toList(), ['a']);
    expect(cubit.state.unreadCount, 1);
    await cubit.close();
  });

  // --- Schedule reconciliation (Layer 3) hookup ---------------------------

  test(
      'schedule reconciliation is skipped (no crash, no queue() call) when doctorRepo/doctorId are not supplied — keeps the app.dart single-argument call site working',
      () {
    fakeAsync((async) {
      final repo = StubNotificationRepository();
      final cubit = NotificationCubit(repo); // no doctorRepo, no doctorId
      cubit.load();
      async.flushMicrotasks();
      expect(cubit.state.items, isNotEmpty);
      cubit.close();
    });
  });

  test(
      'schedule reconciliation fetches doctorRepo.queue() once both doctorRepo and doctorId are supplied',
      () {
    fakeAsync((async) {
      final repo = StubNotificationRepository();
      final doctorRepo = StubDoctorRepository();
      final cubit =
          NotificationCubit(repo, doctorRepo: doctorRepo, doctorId: 'doc-1');
      cubit.load();
      async.flushMicrotasks();
      expect(doctorRepo.queueCalls, greaterThanOrEqualTo(1));
      cubit.close();
    });
  });
}
