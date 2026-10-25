import 'dart:io' show Platform;
import 'dart:math';
import '../../core/network/api_client.dart';
import '../../core/network/client_error_reporter.dart';
import 'notification_models.dart';

/// Wraps the own-inbox notification endpoints confirmed live in
/// `services/backend/src/api/v1/notification_handlers.rs` — identical
/// contract to `apps/mobile`'s `NotificationRepository`, which this mirrors,
/// including real device-token registration/revocation backed by a real
/// FCM HTTP v1 transport and `firebase_messaging` client (see
/// `core/notifications/push_notification_service.dart`).
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
  /// be 'en' or 'sw' — the backend validates this itself (see
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
      final raw = _mapData(_data(await api.put('/notifications/preferences',
          data: {
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

  /// Best-effort backend sync of a language change: fetches the current
  /// preferences first and PUTs them back with only `language` changed,
  /// since the backend's `PUT` replaces the whole preference row — a naive
  /// PUT of just `{language}` with hardcoded defaults for the rest would
  /// silently reset quiet hours/category toggles the user had already set.
  /// Called from `LocaleController.setLocale` via its best-effort sync hook.
  Future<void> syncLanguage(String language) async {
    final current = await getPreferences();
    await updatePreferences(
      operationalEnabled: current.operationalEnabled,
      engagementEnabled: current.engagementEnabled,
      quietHoursEnabled: current.quietHoursEnabled,
      quietHoursStartLocal: current.quietHoursStartLocal,
      quietHoursEndLocal: current.quietHoursEndLocal,
      greetingNotificationsEnabled: current.greetingNotificationsEnabled,
      religiousGreetingsEnabled: current.religiousGreetingsEnabled,
      language: language,
    );
  }

  static String currentPlatformLabel() {
    if (Platform.isAndroid) return 'ANDROID';
    if (Platform.isIOS) return 'IOS';
    return 'WEB';
  }

  Future<String> deviceId() async {
    const key = 'notification_device_id';
    final existing = await _deviceIdStore.read(key);
    if (existing != null && existing.isNotEmpty) return existing;
    final rand = Random.secure();
    final bytes = List<int>.generate(16, (_) => rand.nextInt(256));
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    final hex = bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
    final generated = '${hex.substring(0, 8)}-${hex.substring(8, 12)}-'
        '${hex.substring(12, 16)}-${hex.substring(16, 20)}-${hex.substring(20)}';
    await _deviceIdStore.write(key, generated);
    return generated;
  }
}
