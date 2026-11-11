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
  Future<NotificationDto> detail(String id) async {
    final raw = _mapData(_data(await api.get('/notifications/$id')));
    return NotificationDto.fromJson(raw);
  }

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
  /// Backed by a real FCM HTTP v1 transport (`notification_transport.rs`'s
  /// `FcmTransport`) and a real `firebase_messaging` client
  /// (`core/notifications/push_notification_service.dart`), which calls this
  /// method with the device's actual FCM registration token on obtain/refresh.
  /// `register_device` in `notification_handlers.rs` reports
  /// `push_provider: "FCM_HTTP_V1"` once the backend's service-account
  /// credentials are configured, or `"NOOP_DEVELOPMENT"` in an environment
  /// where they aren't (development/test) — in that case this call still
  /// records the registration row, just without any provider actually
  /// delivering to it yet.
  Future<Json> registerDevice(
      {required String platform,
      required String pushToken,
      String? deviceId}) async {
    try {
      return _mapData(_data(await api.post('/notifications/devices', data: {
        'platform': platform,
        'push_token': pushToken,
        if (deviceId != null) 'device_id': deviceId,
      })));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Notifications', operation: 'register_device');
      rethrow;
    }
  }

  Future<void> revokeDevice({String? deviceId, String? pushToken}) async {
    try {
      await api.post('/notifications/devices/revoke', data: {
        if (deviceId != null) 'device_id': deviceId,
        if (pushToken != null) 'push_token': pushToken,
      });
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Notifications', operation: 'revoke_device');
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

  /// GET /notifications/preferences
  Future<NotificationPreferencesDto> getPreferences() async {
    try {
      final raw = _mapData(_data(await api.get('/notifications/preferences')));
      return NotificationPreferencesDto.fromJson(raw);
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'NotificationSettings', operation: 'load');
      rethrow;
    }
  }

  /// PUT /notifications/preferences. `language`, when passed, must already
  /// be 'en' or 'sw' — the backend itself validates this (see
  /// `NotificationService::update_preferences`), this is just the matching
  /// client-side contract.
  Future<NotificationPreferencesDto> updatePreferences({
    required bool operationalEnabled,
    required bool engagementEnabled,
    required bool quietHoursEnabled,
    required String quietHoursStartLocal,
    required String quietHoursEndLocal,
    required bool greetingNotificationsEnabled,
    required bool religiousGreetingsEnabled,
    String? language,
  }) async {
    try {
      final raw =
          _mapData(_data(await api.put('/notifications/preferences', data: {
        'operational_enabled': operationalEnabled,
        'engagement_enabled': engagementEnabled,
        'quiet_hours_enabled': quietHoursEnabled,
        'quiet_hours_start_local': quietHoursStartLocal,
        'quiet_hours_end_local': quietHoursEndLocal,
        'greeting_notifications_enabled': greetingNotificationsEnabled,
        'religious_greetings_enabled': religiousGreetingsEnabled,
        if (language != null) 'language': language,
      })));
      return NotificationPreferencesDto.fromJson(raw);
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'NotificationSettings', operation: 'update');
      rethrow;
    }
  }

  static const _deviceIdKey = 'notification_device_placeholder_id';

  Future<String> deviceId() async {
    final existing = await _deviceIdStore.read(_deviceIdKey);
    if (existing != null && existing.isNotEmpty) return existing;
    final rand = Random.secure();
    final bytes = List<int>.generate(16, (_) => rand.nextInt(256));
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    final hex = bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
    final generated = '${hex.substring(0, 8)}-${hex.substring(8, 12)}-'
        '${hex.substring(12, 16)}-${hex.substring(16, 20)}-${hex.substring(20)}';
    await _deviceIdStore.write(_deviceIdKey, generated);
    return generated;
  }
}
