import '../../core/network/api_client.dart';
import '../../core/network/client_error_reporter.dart';
import 'notification_models.dart';

/// Wraps the own-inbox notification endpoints confirmed live in
/// `services/backend/src/api/v1/notification_handlers.rs` — identical
/// contract to `apps/mobile`'s `NotificationRepository`, which this mirrors.
/// No device-token registration here: the Patient app has no push SDK
/// dependency either (same honest "no real push provider configured" state
/// the Doctor app documents), and this pass is scoped to the in-app inbox.
class NotificationRepository {
  NotificationRepository(this.api);
  final ApiClient api;

  dynamic _data(dynamic r) {
    final body = r.data;
    if (body is Map && body.containsKey('data')) return body['data'];
    return body;
  }

  List<dynamic> _listData(dynamic value) {
    if (value is List) return value;
    if (value is Map && value['items'] is List) return value['items'] as List;
    throw const FormatException('Unexpected response shape.');
  }

  Map<String, dynamic> _mapData(dynamic value) => value is Map
      ? Map<String, dynamic>.from(value)
      : throw const FormatException('Unexpected response shape.');

  /// GET /notifications — paginated, newest rows first per the backend's
  /// `list_for_user` ordering.
  Future<List<NotificationDto>> list({int? offset, int? limit}) async {
    try {
      final raw = _listData(_data(await api.get('/notifications', query: {
        if (limit != null) 'limit': limit,
        if (offset != null) 'offset': offset,
      })));
      return raw
          .map((e) =>
              NotificationDto.fromJson(Map<String, dynamic>.from(e as Map)))
          .toList();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Notifications', operation: 'load');
      rethrow;
    }
  }

  Future<List<NotificationDto>> newerThan(NotificationDto cursor) =>
      _cursorPage(cursor, false);
  Future<List<NotificationDto>> olderThan(NotificationDto cursor) =>
      _cursorPage(cursor, true);
  Future<List<NotificationDto>> _cursorPage(
      NotificationDto cursor, bool before) async {
    final raw = _listData(_data(await api.get('/notifications', query: {
      'limit': 25,
      before ? 'before_time' : 'after_time':
          cursor.createdAt.toUtc().toIso8601String(),
      before ? 'before_id' : 'after_id': cursor.id,
    })));
    return raw
        .map((e) =>
            NotificationDto.fromJson(Map<String, dynamic>.from(e as Map)))
        .toList();
  }

  /// GET /notifications/unread-count
  Future<int> unreadCount() async {
    try {
      final raw = _mapData(_data(await api.get('/notifications/unread-count')));
      return (raw['unread_count'] as num?)?.toInt() ?? 0;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Notifications', operation: 'load');
      rethrow;
    }
  }

  /// POST /notifications/:id/read — a naturally idempotent "set to read"
  /// mutation, so no `Idempotency-Key` is attached.
  Future<void> markRead(String id) async {
    try {
      await api.post('/notifications/$id/read');
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Notifications', operation: 'update');
      rethrow;
    }
  }

  /// POST /notifications/read-all
  Future<int> markAllRead() async {
    try {
      final raw = _mapData(_data(await api.post('/notifications/read-all')));
      return (raw['marked_count'] as num?)?.toInt() ?? 0;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Notifications', operation: 'update');
      rethrow;
    }
  }
}
