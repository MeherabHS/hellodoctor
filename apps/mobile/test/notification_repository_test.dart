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
          'push_provider': 'CONFIGURATION_REQUIRED'
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
    // Honest confirmation: the backend transport remains a no-op stub.
    expect(result['push_provider'], 'CONFIGURATION_REQUIRED');
    api.dispose();
  });

  test(
      'devicePlaceholderToken() is generated once and persisted across calls (never a fresh value per app launch)',
      () async {
    final store = _Store();
    final api = ApiClient(dio: Dio(), tokenStore: store);
    final repo = NotificationRepository(api, deviceIdStore: store);
    final first = await repo.devicePlaceholderToken();
    final second = await repo.devicePlaceholderToken();
    expect(first, second);
    expect(first, isNotEmpty);
    // A second repository instance (simulating a fresh app launch reading
    // the same persisted storage) must reuse the same value, not mint a new
    // device row every time (the backend upserts on (user_id, push_token)).
    final relaunchedRepo = NotificationRepository(api, deviceIdStore: store);
    expect(await relaunchedRepo.devicePlaceholderToken(), first);
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
