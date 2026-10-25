import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_repository.dart';

/// Focused coverage for the device-token registration lifecycle
/// (register/refresh/revoke) as part of the Firebase native-integration
/// milestone — mirrors `apps/mobile`'s equivalent assertions for the
/// identical contract. Unlike the Doctor app, this app previously had no
/// dedicated repository-layer test file at all.
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

void main() {
  test(
      'registerDevice() posts platform and push_token to /notifications/devices',
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
    final result = await repo.registerDevice(
        platform: 'ANDROID', pushToken: 'placeholder-abc', deviceId: 'd-1');
    expect(requests.single.method, 'POST');
    expect(requests.single.path, '/notifications/devices');
    expect(requests.single.data,
        {'platform': 'ANDROID', 'push_token': 'placeholder-abc', 'device_id': 'd-1'});
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
        'data': preferencesRow(quietHours: true, language: 'sw'),
      }));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = NotificationRepository(api);
    final prefs = await repo.getPreferences();
    expect(requests.single.method, 'GET');
    expect(requests.single.path, '/notifications/preferences');
    expect(prefs.quietHoursEnabled, isTrue);
    expect(prefs.preferredLanguage, 'sw');
    api.dispose();
  });

  test('updatePreferences() PUTs the full payload including language when given',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests.add(request);
      handler.resolve(Response(
          requestOptions: request,
          data: {'success': true, 'data': preferencesRow(language: 'sw')}));
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
      language: 'sw',
    );
    expect(requests.single.method, 'PUT');
    expect(requests.single.path, '/notifications/preferences');
    expect(requests.single.data['language'], 'sw');
    api.dispose();
  });

  /// `syncLanguage` is what `LocaleController`'s best-effort backend-sync
  /// hook actually calls — it must GET the current preferences first and
  /// PUT them back with only `language` changed, never clobbering quiet
  /// hours/category toggles the user already set with hardcoded defaults.
  test(
      'syncLanguage() fetches current preferences then PUTs them back with only language changed',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests.add(request);
      if (request.method == 'GET') {
        handler.resolve(Response(requestOptions: request, data: {
          'success': true,
          'data': preferencesRow(
              operational: false,
              engagement: true,
              quietHours: true,
              start: '21:00',
              end: '05:00',
              language: 'en'),
        }));
      } else {
        handler.resolve(Response(
            requestOptions: request,
            data: {'success': true, 'data': preferencesRow(language: 'sw')}));
      }
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final repo = NotificationRepository(api);
    await repo.syncLanguage('sw');
    expect(requests, hasLength(2));
    expect(requests[0].method, 'GET');
    expect(requests[1].method, 'PUT');
    expect(requests[1].data, {
      'operational_enabled': false,
      'engagement_enabled': true,
      'quiet_hours_enabled': true,
      'quiet_hours_start_local': '21:00',
      'quiet_hours_end_local': '05:00',
      'greeting_notifications_enabled': false,
      'religious_greetings_enabled': false,
      'language': 'sw',
    });
    api.dispose();
  });

  test('a transport failure on revokeDevice() is wrapped as ApiException',
      () async {
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
        repo.revokeDevice(deviceId: 'device-123'),
        throwsA(isA<ApiException>()
            .having((e) => e.status, 'status', 500)
            .having((e) => e.message, 'message', 'Internal error')));
    api.dispose();
  });
}
