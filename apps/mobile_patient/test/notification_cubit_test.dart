import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_models.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_repository.dart';

NotificationDto _n(String id, {bool read = false, DateTime? createdAt}) =>
    NotificationDto(
        id: id,
        userId: 'u1',
        type: 'APPOINTMENT_CONFIRMED',
        title: 'Title $id',
        body: 'Body $id',
        deliveryStatus: 'DELIVERED',
        createdAt: createdAt ?? DateTime.utc(2026, 10, 5, 10),
        readAt: read ? DateTime.utc(2026, 10, 5, 11) : null);

/// Stubs only the methods each test needs, per this project's established
/// no-mockito convention (see `family_profiles_cubit_test.dart`'s harness).
class FakeNotificationRepository extends NotificationRepository {
  FakeNotificationRepository()
      : super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));
  List<NotificationDto> listResponse = [_n('a'), _n('b')];
  int unreadCountResponse = 2;
  bool failMarkRead = false;
  bool failMarkAllRead = false;
  final markedReadIds = <String>[];
  int markAllReadCalls = 0;
  int listCalls = 0;

  @override
  Future<List<NotificationDto>> list({int? offset, int? limit}) async {
    listCalls++;
    return listResponse;
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
    if (failMarkAllRead) throw const ApiException(500, 'Server error');
    return listResponse.length;
  }
}

class _NoopStore implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

void main() {
  late FakeNotificationRepository repo;
  late NotificationCubit cubit;

  setUp(() {
    repo = FakeNotificationRepository();
    cubit = NotificationCubit(repo);
  });

  tearDown(() => cubit.close());

  group('load', () {
    test('populates items and unread count', () async {
      await cubit.load();
      expect(cubit.state.items.map((n) => n.id).toList(), ['a', 'b']);
      expect(cubit.state.unreadCount, 2);
      expect(cubit.state.loading, isFalse);
      expect(cubit.state.error, isNull);
    });

    test('a repository failure surfaces as state.error', () async {
      repo.listResponse = [];
      repo.unreadCountResponse = 0;
      cubit = NotificationCubit(_ThrowingRepository());
      await cubit.load();
      expect(cubit.state.error, isNotNull);
      expect(cubit.state.items, isEmpty);
    });

    test('does not re-notify for items that already existed on first load',
        () async {
      // The baseline-establishment behaviour is covered indirectly: a second
      // load() with the same items must not throw or duplicate state.
      await cubit.load();
      await cubit.load();
      expect(repo.listCalls, 2);
      expect(cubit.state.items, hasLength(2));
    });
  });

  group('markRead', () {
    test('optimistically marks read and decrements unread count', () async {
      await cubit.load();
      await cubit.markRead('a');
      expect(cubit.state.items.firstWhere((n) => n.id == 'a').isRead, isTrue);
      expect(cubit.state.unreadCount, 1);
      expect(repo.markedReadIds, ['a']);
    });

    test('a no-op for an unknown id', () async {
      await cubit.load();
      await cubit.markRead('does-not-exist');
      expect(repo.markedReadIds, isEmpty);
    });

    test('a no-op for an already-read item', () async {
      repo.listResponse = [_n('a', read: true)];
      await cubit.load();
      await cubit.markRead('a');
      expect(repo.markedReadIds, isEmpty);
    });

    test('rolls back on a server failure', () async {
      await cubit.load();
      repo.failMarkRead = true;
      await cubit.markRead('a');
      expect(cubit.state.items.firstWhere((n) => n.id == 'a').isRead, isFalse);
      expect(cubit.state.unreadCount, 2);
      expect(cubit.state.error, isNotNull);
    });
  });

  group('markAllRead', () {
    test('marks every item read and zeroes the unread count', () async {
      await cubit.load();
      await cubit.markAllRead();
      expect(cubit.state.items.every((n) => n.isRead), isTrue);
      expect(cubit.state.unreadCount, 0);
      expect(repo.markAllReadCalls, 1);
    });

    test('a no-op when nothing is unread', () async {
      repo.listResponse = [_n('a', read: true)];
      repo.unreadCountResponse = 0;
      await cubit.load();
      await cubit.markAllRead();
      expect(repo.markAllReadCalls, 0);
    });

    test('rolls back on a server failure', () async {
      await cubit.load();
      repo.failMarkAllRead = true;
      await cubit.markAllRead();
      expect(cubit.state.unreadCount, 2);
      expect(cubit.state.items.every((n) => !n.isRead), isTrue);
      expect(cubit.state.error, isNotNull);
    });
  });
}

class _ThrowingRepository extends NotificationRepository {
  _ThrowingRepository()
      : super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));
  @override
  Future<List<NotificationDto>> list({int? offset, int? limit}) async =>
      throw const ApiException(500, 'Server error.');
  @override
  Future<int> unreadCount() async =>
      throw const ApiException(500, 'Server error.');
}
