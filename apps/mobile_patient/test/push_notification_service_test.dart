import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/network/client_error_reporter.dart';
import 'package:hellodoctor_mobile_patient/core/notifications/push_notification_service.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_repository.dart';

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
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
      'a Firebase.initializeApp() failure (no native config — the state of '
      'this repository today, per docs/FCM_SETUP.md) is captured through '
      'sanitized client diagnostics rather than silently swallowed, and '
      'never blocks initialize() or calls onNotificationReceived', () async {
    final sent = <RequestOptions>[];
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      sent.add(request);
      handler.resolve(Response(
          requestOptions: request,
          statusCode: 202,
          data: {'success': true, 'data': {}}));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    await api.saveTokens('patient-token', 'refresh-token');
    final reporter = ClientErrorReporter(api, authenticated: () => true);
    ClientErrorReporter.current = reporter;

    var notified = 0;
    final service = PushNotificationService(
      repository: NotificationRepository(api, deviceIdStore: _Store()),
      onNotificationReceived: () async {
        notified++;
      },
    );

    await service.initialize();
    // `ClientErrorReporter.observe` reports via an unawaited future.
    await Future<void>.delayed(const Duration(milliseconds: 20));

    expect(sent, isNotEmpty,
        reason: 'Firebase.initializeApp() has no native config in this test '
            'host (and none in this repository today) and must throw — the '
            'failure must reach sanitized diagnostics, not disappear '
            'silently.');
    expect(sent.every((r) => (r.data as Map)['operation'] == 'push_init'),
        isTrue);
    expect(sent.every((r) => (r.data as Map)['screen'] == 'Notifications'),
        isTrue);
    // The in-app inbox refresh path is never invoked just because
    // initialize() ran — only a real incoming/opened FCM message does that,
    // neither of which happened here.
    expect(notified, 0);

    ClientErrorReporter.current = null;
    api.dispose();
  });

  test('initialize() is idempotent — a second call is a no-op', () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    var requestCount = 0;
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requestCount++;
      handler.resolve(Response(
          requestOptions: request,
          statusCode: 202,
          data: {'success': true, 'data': {}}));
    }));
    final api = ApiClient(dio: dio, tokenStore: _Store());
    final service = PushNotificationService(
      repository: NotificationRepository(api, deviceIdStore: _Store()),
      onNotificationReceived: () async {},
    );
    await service.initialize();
    final afterFirst = requestCount;
    await service.initialize();
    expect(requestCount, afterFirst,
        reason: 'a second initialize() call must not re-run setup');
    api.dispose();
  });
}
