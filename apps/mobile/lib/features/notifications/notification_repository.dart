import 'dart:io' show Platform;
import 'dart:math';
import '../../core/network/client_error_reporter.dart';
import '../../core/network/api_client.dart';
import 'notification_models.dart';

/// Wraps the own-inbox notification endpoints confirmed live in
/// `services/backend/src/api/v1/notification_handlers.rs`. Follows
/// `features/doctor/doctor_repository.dart`'s exact convention: every method
/// wraps errors through `ClientErrorReporter.observe` then rethrows, so a
/// failure is reported exactly once at the lowest layer that saw it.
class NotificationRepository {
  NotificationRepository(this.api, {TokenStore? deviceIdStore})
      : _deviceIdStore = deviceIdStore ?? SecureTokenStore();
  final ApiClient api;
  final TokenStore _deviceIdStore;

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
  /// mutation (same reasoning `ApiClient.post`'s own doc comment gives for
  /// duty/profile/payout-method calls), so no `Idempotency-Key` is attached.
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

  /// POST /notifications/devices.
  ///
  /// HONEST CAVEAT: the backend transport is a confirmed no-op stub today —
  /// `register_device` in `notification_handlers.rs` always answers
  /// `push_provider: "CONFIGURATION_REQUIRED"` — and this client intentionally
  /// has no push SDK dependency (no Firebase Cloud Messaging / APNs
  /// integration exists in `pubspec.yaml`, by design). [pushToken] is
  /// therefore NOT a real platform push token; pass [devicePlaceholderToken]'s
  /// result instead. This call only records a registration row for whenever a
  /// real push provider is wired up later.
  Future<Json> registerDevice(
      {required String platform, required String pushToken}) async {
    try {
      return _mapData(_data(await api.post('/notifications/devices',
          data: {'platform': platform, 'push_token': pushToken})));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Notifications', operation: 'register_device');
      rethrow;
    }
  }

  /// The backend only accepts `ANDROID`/`IOS`/`WEB` (see
  /// `RegisterDeviceRequest` validation in `notification_handlers.rs`).
  static String currentPlatformLabel() {
    if (Platform.isAndroid) return 'ANDROID';
    if (Platform.isIOS) return 'IOS';
    return 'WEB';
  }

  static const _deviceIdKey = 'notification_device_placeholder_id';

  /// A stable, locally-generated, non-cryptographic placeholder device
  /// identifier — explicitly NOT a real push token. The backend upserts a
  /// device row on `ON CONFLICT (user_id, push_token)` (see
  /// `notification_repo::device_tokens::upsert`), so this value is persisted
  /// (reusing the same secure-storage abstraction `ApiClient` uses for
  /// session tokens) and reused across app launches rather than regenerated
  /// each call — a fresh value every call would silently accumulate a new
  /// device row per launch instead of refreshing `last_seen_at` on one.
  Future<String> devicePlaceholderToken() async {
    final existing = await _deviceIdStore.read(_deviceIdKey);
    if (existing != null && existing.isNotEmpty) return existing;
    final rand = Random.secure();
    final bytes = List<int>.generate(16, (_) => rand.nextInt(256));
    final generated =
        'placeholder-${bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join()}';
    await _deviceIdStore.write(_deviceIdKey, generated);
    return generated;
  }
}
