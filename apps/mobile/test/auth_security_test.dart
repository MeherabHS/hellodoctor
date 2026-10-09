import 'package:dio/dio.dart';
import 'dart:async';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/auth/auth_cubit.dart';

class MemoryStore implements TokenStore {
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
  test('refresh never automatically replays a clinical mutation', () async {
    final store = MemoryStore();
    final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    final refreshDio = Dio(dio.options.copyWith());
    final api = ApiClient(dio: dio, refreshDio: refreshDio, tokenStore: store);
    await api.saveTokens('doctor-A', 'refresh-A');
    var sends = 0;
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      sends++;
      handler.reject(
          DioException(
              requestOptions: request,
              response: Response(requestOptions: request, statusCode: 401),
              type: DioExceptionType.badResponse),
          true);
    }));
    refreshDio.interceptors
        .add(InterceptorsWrapper(onRequest: (request, handler) {
      handler.resolve(Response(requestOptions: request, data: {
        'data': {'access_token': 'renewed', 'refresh_token': 'renewed-refresh'}
      }));
    }));
    await expectLater(api.post('/appointments/a/complete'),
        throwsA(isA<ApiException>().having((e) => e.status, 'status', 409)));
    expect(sends, 1);
    expect(await api.read('access_token'), 'renewed');
    api.dispose();
  });

  test('a rejected old refresh does not sign out the new Doctor', () async {
    final store = MemoryStore();
    final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    final refreshDio = Dio(dio.options.copyWith());
    final api = ApiClient(dio: dio, refreshDio: refreshDio, tokenStore: store);
    var expired = false;
    api.onSessionExpired = () => expired = true;
    await api.saveTokens('doctor-A', 'refresh-A');
    final started = Completer<void>();
    final returned = Completer<void>();
    refreshDio.interceptors
        .add(InterceptorsWrapper(onRequest: (request, handler) async {
      started.complete();
      await returned.future;
      handler.reject(DioException(
          requestOptions: request,
          response: Response(requestOptions: request, statusCode: 401),
          type: DioExceptionType.badResponse));
    }));
    final refresh = api.refreshSession();
    await started.future;
    await api.clearSession();
    await api.saveTokens('doctor-B', 'refresh-B');
    returned.complete();
    await expectLater(refresh, throwsA(isA<ApiException>()));
    expect(await api.read('access_token'), 'doctor-B');
    expect(expired, isFalse);
    api.dispose();
  });

  test('late protected response cannot reach cache after account switch',
      () async {
    final store = MemoryStore();
    final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    final api = ApiClient(dio: dio, tokenStore: store);
    await api.saveTokens('doctor-A', 'refresh-A');
    final started = Completer<void>();
    final returned = Completer<void>();
    dio.interceptors
        .add(InterceptorsWrapper(onRequest: (request, handler) async {
      started.complete();
      await returned.future;
      handler.resolve(Response(
          requestOptions: request, data: {'data': 'Doctor A financial data'}));
    }));
    final request = api.get('/doctor/wallet/summary');
    await started.future;
    await api.clearSession();
    await api.saveTokens('doctor-B', 'refresh-B');
    returned.complete();
    await expectLater(request,
        throwsA(isA<ApiException>().having((e) => e.status, 'status', 401)));
    expect(await api.read('access_token'), 'doctor-B');
    api.dispose();
  });

  test(
      'late refresh cannot restore a logged-out account or expire a new account',
      () async {
    final store = MemoryStore();
    final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    final refreshDio = Dio(dio.options.copyWith());
    final api = ApiClient(dio: dio, refreshDio: refreshDio, tokenStore: store);
    await api.saveTokens('doctor-A', 'refresh-A');
    final started = Completer<void>();
    final returned = Completer<void>();
    refreshDio.interceptors
        .add(InterceptorsWrapper(onRequest: (request, handler) async {
      started.complete();
      await returned.future;
      handler.resolve(Response(requestOptions: request, data: {
        'data': {
          'access_token': 'rotated-A',
          'refresh_token': 'rotated-refresh-A'
        }
      }));
    }));
    final refresh = api.refreshSession();
    await started.future;
    await api.clearSession();
    await api.saveTokens('doctor-B', 'refresh-B');
    returned.complete();
    await expectLater(refresh, throwsA(isA<ApiException>()));
    expect(await api.read('access_token'), 'doctor-B');
    expect(await api.read('refresh_token'), 'refresh-B');
    api.dispose();
  });

  test('tokens are persisted and clearSession removes both credentials',
      () async {
    final store = MemoryStore();
    final api = ApiClient(dio: Dio(), tokenStore: store);
    await api.saveTokens('access', 'refresh');
    expect(await api.read('access_token'), 'access');
    expect(await api.read('refresh_token'), 'refresh');
    await api.clearSession();
    expect(await api.read('access_token'), isNull);
    expect(await api.read('refresh_token'), isNull);
  });

  test('unexpected backend debug OTP does not bypass the manual MFA screen',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'https://example.test/api/v1'));
    var requests = 0;
    dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
      requests++;
      handler.resolve(Response(requestOptions: request, data: {
        'data': {'session_token': 'challenge', 'debug_otp_code': '123456'}
      }));
    }));
    final api = ApiClient(dio: dio, tokenStore: MemoryStore());
    final auth = AuthCubit(api);
    await auth.login('synthetic@example.test', 'synthetic-password');
    expect(auth.state, isA<MfaRequired>());
    expect(requests, 1);
    await auth.close();
    api.dispose();
  });

  test('logout clears local credentials even when the server is offline',
      () async {
    final store = MemoryStore();
    final dio = Dio(BaseOptions(baseUrl: 'http://127.0.0.1:1'));
    final api = ApiClient(dio: dio, tokenStore: store);
    await api.saveTokens('access', 'refresh');
    final auth = AuthCubit(api);
    await auth.logout();
    expect(auth.state, isA<SignedOut>());
    expect((auth.state as SignedOut).notice,
        contains('revocation could not be confirmed'));
    expect(await api.read('access_token'), isNull);
    expect(await api.read('refresh_token'), isNull);
    await auth.close();
  });

  test('offline startup keeps a secure session for cached read-only data',
      () async {
    final store = MemoryStore();
    final dio = Dio(BaseOptions(baseUrl: 'http://127.0.0.1:1'));
    final api = ApiClient(dio: dio, tokenStore: store);
    await api.saveTokens('access', 'refresh');
    final auth = AuthCubit(api);
    await auth.restore();
    expect(auth.state, isA<SignedIn>());
    await auth.close();
  });
}
