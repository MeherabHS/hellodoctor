import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_models.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_repository.dart';

class _Store implements TokenStore {
  final values = <String, String>{};
  @override
  Future<String?> read(String key) async => values[key];
  @override
  Future<void> write(String key, String value) async => values[key] = value;
  @override
  Future<void> delete(String key) async => values.remove(key);
  @override
  Future<void> clear() async => values.clear();
}

Map<String, dynamic> row(
        {String id = 'n1',
        String type = 'EARNING_CREDITED',
        bool read = false}) =>
    {
      'id': id,
      'user_id': 'u1',
      'notification_type': type,
      'title': 'Consultation earning credited',
      'body': 'Consultation earning credited: KES 80.',
      'related_entity_type': 'APPOINTMENT',
      'related_entity_id': 'appt-1',
      'scheduled_for': null,
      'created_at': '2026-10-05T10:00:00Z',
      'delivered_at': '2026-10-05T10:00:01Z',
      'read_at': read ? '2026-10-05T10:05:00Z' : null,
      'delivery_status': 'DELIVERED',
      'metadata': null,
    };

void main() {
  test('NotificationDto reads notification_type, not serde-absent "type"', () {
    final n = NotificationDto.fromJson(row());
    expect(n.id, 'n1');
    expect(n.type, 'EARNING_CREDITED');
    expect(n.relatedEntityType, 'APPOINTMENT');
    expect(n.relatedEntityId, 'appt-1');
    expect(n.isRead, isFalse);
    final readN = NotificationDto.fromJson(row(id: 'n2', read: true));
    expect(readN.isRead, isTrue);
  });

  test('list() requests GET /notifications with limit/offset and parses rows',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests.add(request);
      handler.resolve(Response(requestOptions: request, data: {
        'success': true,
        'data': [row(id: 'a'), row(id: 'b', read: true)]
      }));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = NotificationRepository(api);
    final items = await repo.list(limit: 10, offset: 5);
    expect(requests.single.path, '/notifications');
    expect(requests.single.queryParameters, {'limit': 10, 'offset': 5});
    expect(items.map((n) => n.id).toList(), ['a', 'b']);
    api.dispose();
  });

  test('unreadCount() requests GET /notifications/unread-count', () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests.add(request);
      handler.resolve(Response(requestOptions: request, data: {
        'success': true,
        'data': {'unread_count': 7}
      }));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = NotificationRepository(api);
    expect(await repo.unreadCount(), 7);
    expect(requests.single.path, '/notifications/unread-count');
    api.dispose();
  });

  test('markRead() posts to /notifications/:id/read', () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests.add(request);
      handler.resolve(Response(requestOptions: request, data: {
        'success': true,
        'data': {'id': 'n1', 'marked': true}
      }));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = NotificationRepository(api);
    await repo.markRead('n1');
    expect(requests.single.method, 'POST');
    expect(requests.single.path, '/notifications/n1/read');
    api.dispose();
  });

  test('markAllRead() posts to /notifications/read-all and returns the count',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests.add(request);
      handler.resolve(Response(requestOptions: request, data: {
        'success': true,
        'data': {'marked_count': 3}
      }));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = NotificationRepository(api);
    expect(await repo.markAllRead(), 3);
    expect(requests.single.path, '/notifications/read-all');
    api.dispose();
  });

  test(
      'registerDevice() posts platform and push_token to /notifications/devices',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests.add(request);
      handler.resolve(Response(requestOptions: request, data: {
        'success': true,
        'data': {
          'status': 'REGISTERED',
          'push_provider': 'NOOP_DEVELOPMENT'
        }
      }));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = NotificationRepository(api);
    final result = await repo.registerDevice(
        platform: 'ANDROID', pushToken: 'placeholder-abc');
    expect(requests.single.method, 'POST');
    expect(requests.single.path, '/notifications/devices');
    expect(requests.single.data,
        {'platform': 'ANDROID', 'push_token': 'placeholder-abc'});
    // Passes through whatever the server reports — FCM_HTTP_V1 once
    // configured, NOOP_DEVELOPMENT (as mocked here) when it isn't.
    expect(result['push_provider'], 'NOOP_DEVELOPMENT');
    api.dispose();
  });

  test(
      'a token-refresh re-registration posts the new push_token with the same device_id',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests.add(request);
      handler.resolve(Response(requestOptions: request, data: {
        'success': true,
        'data': {'status': 'REGISTERED', 'push_provider': 'NOOP_DEVELOPMENT'}
      }));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = NotificationRepository(api);
    await repo.registerDevice(
        platform: 'ANDROID', pushToken: 'token-v1', deviceId: 'stable-device');
    await repo.registerDevice(
        platform: 'ANDROID', pushToken: 'token-v2', deviceId: 'stable-device');
    expect(requests, hasLength(2));
    expect(requests[0].data['push_token'], 'token-v1');
    expect(requests[1].data['push_token'], 'token-v2');
    expect(requests[0].data['device_id'], requests[1].data['device_id']);
    api.dispose();
  });

  test('revokeDevice() posts device_id to /notifications/devices/revoke',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests.add(request);
      handler.resolve(Response(requestOptions: request, data: {
        'success': true,
        'data': {'revoked': 1}
      }));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = NotificationRepository(api);
    await repo.revokeDevice(deviceId: 'device-123');
    expect(requests.single.method, 'POST');
    expect(requests.single.path, '/notifications/devices/revoke');
    expect(requests.single.data, {'device_id': 'device-123'});
    api.dispose();
  });

  test(
      'deviceId() is generated once and persisted across calls (never a fresh value per app launch)',
      () async {
    final store = _Store();
    final api = ApiClient(dio: Dio(), tokenStore: store);
    final repo = NotificationRepository(api, deviceIdStore: store);
    final first = await repo.deviceId();
    final second = await repo.deviceId();
    expect(first, second);
    expect(first, isNotEmpty);
    // A second repository instance (simulating a fresh app launch reading
    // the same persisted storage) must reuse the same value, not mint a new
    // device row every time (the backend upserts on (user_id, push_token)).
    final relaunchedRepo = NotificationRepository(api, deviceIdStore: store);
    expect(await relaunchedRepo.deviceId(), first);
    api.dispose();
  });

  Map<String, dynamic> preferencesRow({
    bool operational = true,
    bool engagement = true,
    bool quietHours = false,
    String start = '22:00',
    String end = '07:00',
    bool greetings = false,
    bool religious = false,
    String language = 'en',
  }) =>
      {
        'operational_enabled': operational,
        'engagement_enabled': engagement,
        'quiet_hours_enabled': quietHours,
        'quiet_hours_start_local': start,
        'quiet_hours_end_local': end,
        'greeting_notifications_enabled': greetings,
        'religious_greetings_enabled': religious,
        'preferred_language': language,
      };

  test('getPreferences() requests GET /notifications/preferences and parses fields',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests.add(request);
      handler.resolve(Response(requestOptions: request, data: {
        'success': true,
        'data': preferencesRow(quietHours: true, start: '21:30', end: '06:15'),
      }));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = NotificationRepository(api);
    final prefs = await repo.getPreferences();
    expect(requests.single.method, 'GET');
    expect(requests.single.path, '/notifications/preferences');
    expect(prefs.operationalEnabled, isTrue);
    expect(prefs.engagementEnabled, isTrue);
    expect(prefs.quietHoursEnabled, isTrue);
    expect(prefs.quietHoursStartLocal, '21:30');
    expect(prefs.quietHoursEndLocal, '06:15');
    expect(prefs.preferredLanguage, 'en');
    api.dispose();
  });

  test('updatePreferences() PUTs the full payload including language when given',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests.add(request);
      handler.resolve(Response(requestOptions: request, data: {
        'success': true,
        'data': preferencesRow(
            operational: false,
            engagement: false,
            quietHours: true,
            start: '21:30',
            end: '06:15',
            language: 'sw'),
      }));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = NotificationRepository(api);
    final saved = await repo.updatePreferences(
      operationalEnabled: false,
      engagementEnabled: false,
      quietHoursEnabled: true,
      quietHoursStartLocal: '21:30',
      quietHoursEndLocal: '06:15',
      greetingNotificationsEnabled: true,
      religiousGreetingsEnabled: false,
      language: 'sw',
    );
    expect(requests.single.method, 'PUT');
    expect(requests.single.path, '/notifications/preferences');
    expect(requests.single.data, {
      'operational_enabled': false,
      'engagement_enabled': false,
      'quiet_hours_enabled': true,
      'quiet_hours_start_local': '21:30',
      'quiet_hours_end_local': '06:15',
      'greeting_notifications_enabled': true,
      'religious_greetings_enabled': false,
      'language': 'sw',
    });
    expect(saved.preferredLanguage, 'sw');
    api.dispose();
  });

  test('updatePreferences() omits language from the request when not given',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests.add(request);
      handler.resolve(Response(
          requestOptions: request,
          data: {'success': true, 'data': preferencesRow()}));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = NotificationRepository(api);
    await repo.updatePreferences(
      operationalEnabled: true,
      engagementEnabled: true,
      quietHoursEnabled: false,
      quietHoursStartLocal: '22:00',
      quietHoursEndLocal: '07:00',
      greetingNotificationsEnabled: false,
      religiousGreetingsEnabled: false,
    );
    expect(requests.single.data.containsKey('language'), isFalse);
    api.dispose();
  });

  test('a transport failure is wrapped as ApiException and rethrown', () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      handler.reject(DioException(
          requestOptions: request,
          type: DioExceptionType.badResponse,
          response: Response(requestOptions: request, statusCode: 500, data: {
            'error': {'message': 'Internal error'}
          })));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = NotificationRepository(api);
    await expectLater(
        repo.list(),
        throwsA(isA<ApiException>()
            .having((e) => e.status, 'status', 500)
            .having((e) => e.message, 'message', 'Internal error')));
    api.dispose();
  });
}
