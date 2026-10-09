import 'package:dio/dio.dart';
import 'dart:async';
import 'dart:math';
import 'package:flutter/foundation.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

/// Generates a token for the `Idempotency-Key` header (see
/// `services/backend/src/api/middleware/idempotency.rs`). Not an RFC 4122
/// UUID — the backend treats this header as an opaque string (it's SHA-256
/// hashed, never parsed), so a 128-bit cryptographically random hex string is
/// equally sufficient without adding a new package dependency just for this.
String _generateIdempotencyKey() {
  final rand = Random.secure();
  final bytes = List<int>.generate(16, (_) => rand.nextInt(256));
  return bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
}

abstract class TokenStore {
  Future<String?> read(String key);
  Future<void> write(String key, String value);
  Future<void> delete(String key);
  Future<void> clear();
}

class SecureTokenStore implements TokenStore {
  SecureTokenStore([FlutterSecureStorage? storage])
      : storage = storage ?? const FlutterSecureStorage();
  final FlutterSecureStorage storage;
  @override
  Future<String?> read(String key) => storage.read(key: key);
  @override
  Future<void> write(String key, String value) =>
      storage.write(key: key, value: value);
  @override
  Future<void> delete(String key) => storage.delete(key: key);
  @override
  Future<void> clear() => storage.deleteAll();
}

class ApiException implements Exception {
  const ApiException(this.status, this.message, {this.requestId});
  final int? status;
  final String message;
  final String? requestId;
  String get userMessage {
    if (status != null && status! >= 500) {
      return 'A temporary service error occurred. Please retry.';
    }
    switch (status) {
      case 401:
        return 'Your session expired. Please sign in again.';
      case 403:
        return 'You do not have permission to perform this action.';
      case 404:
        return 'This record is no longer available.';
      case 409:
        return message.isNotEmpty
            ? message
            : 'This action conflicts with the current server state.';
      case 413:
        return 'The selected file exceeds the upload size limit.';
      case 422:
        return message.isNotEmpty
            ? message
            : 'Please check the entered information.';
      case 429:
        return 'Too many attempts. Please try again shortly.';
      case null:
        return message.isNotEmpty
            ? message
            : 'Unable to reach HelloDoctor. Check your connection and retry.';
      default:
        return message;
    }
  }
}

/// True for loopback, the Android emulator's host alias, and RFC 1918
/// private ranges. These addresses qualify for an explicitly enabled local
/// development exception; private addressing itself does not encrypt traffic.
bool _isLanTestHost(String host) {
  if (host == 'localhost' || host == '127.0.0.1' || host == '::1') {
    return true;
  }
  final octets = host.split('.').map(int.tryParse).toList();
  if (octets.length != 4 || octets.any((o) => o == null)) return false;
  final a = octets[0]!, b = octets[1]!;
  return a == 10 || // 10.0.0.0/8 (also covers the 10.0.2.2 emulator alias)
      (a == 172 && b >= 16 && b <= 31) || // 172.16.0.0/12
      (a == 192 && b == 168); // 192.168.0.0/16
}

/// Whether a release build is allowed to talk to [baseUri] over plain HTTP.
///
/// Deliberately a plain top-level function (not private, not buried in the
/// constructor) so a unit test can pin this down directly. Getting this
/// wrong in either direction is a real-world incident: too strict and a
/// correctly configured production build white-screens for every patient and
/// doctor on launch; too loose and a production build could silently send
/// clinical data over an unencrypted public connection.
bool allowsInsecureHttp(
        {required bool allowInsecureHttpFlag,
        required Uri baseUri,
        required String environment}) =>
    baseUri.scheme == 'https' ||
    (baseUri.scheme == 'http' &&
        environment == 'development' &&
        allowInsecureHttpFlag &&
        _isLanTestHost(baseUri.host));

/// Independent client checks prevent a wrongly configured backend from enabling
/// development MFA behavior in a deployed app.
bool developmentOtpEnabled({required String environment, required bool flag}) =>
    environment == 'development' && flag;

void validateMobileDeployment(
    {required String environment,
    required bool release,
    required Uri baseUri,
    required bool allowHttp,
    required bool devOtp,
    required bool devPayoutBinding}) {
  if (!release && environment.isEmpty) return;
  if (!['development', 'test', 'staging', 'production'].contains(environment)) {
    throw StateError('An explicit valid ENVIRONMENT is required.');
  }
  if (environment != 'development' &&
      (allowHttp || devOtp || devPayoutBinding)) {
    throw StateError('Development flags are forbidden outside development.');
  }
  if (!allowsInsecureHttp(
      allowInsecureHttpFlag: allowHttp,
      baseUri: baseUri,
      environment: environment)) {
    throw StateError('API_BASE_URL requires HTTPS. LAN HTTP requires explicit '
        'ENVIRONMENT=development and ALLOW_INSECURE_HTTP=true.');
  }
}

class ApiClient {
  ApiClient(
      {String? baseUrl,
      Dio? dio,
      Dio? refreshDio,
      FlutterSecureStorage? storage,
      TokenStore? tokenStore})
      : _storage = tokenStore ?? SecureTokenStore(storage),
        _dio = dio ??
            Dio(BaseOptions(
                baseUrl: baseUrl ??
                    const String.fromEnvironment('API_BASE_URL',
                        defaultValue: 'http://10.0.2.2:8080/api/v1'),
                connectTimeout: const Duration(seconds: 15),
                sendTimeout: const Duration(seconds: 20),
                receiveTimeout: const Duration(seconds: 20))) {
    const allowInsecureHttp =
        bool.fromEnvironment('ALLOW_INSECURE_HTTP', defaultValue: false);
    final baseUri = Uri.parse(_dio.options.baseUrl);
    if (dio == null || kReleaseMode) {
      validateMobileDeployment(
        environment: const String.fromEnvironment('ENVIRONMENT'),
        release: true,
        baseUri: baseUri,
        allowHttp: allowInsecureHttp,
        devOtp: const bool.fromEnvironment('DEV_AUTH_OTP_ENABLED'),
        devPayoutBinding: const bool.fromEnvironment('DEV_PAYOUT_BINDING'),
      );
    }
    // API redirects must not downgrade transport or forward credentials to a
    // different origin. API endpoints return JSON directly.
    _dio.options.followRedirects = false;
    _refreshDio = refreshDio ?? Dio(_dio.options.copyWith());
    _refreshDio.options.followRedirects = false;
    _dio.interceptors
        .add(InterceptorsWrapper(onRequest: (options, handler) async {
      if (options.extra['client_incident'] == true) {
        if (options.extra['session_generation'] != _sessionGeneration) {
          return handler.reject(DioException(
              requestOptions: options, type: DioExceptionType.cancel));
        }
        return handler.next(options);
      }
      options.extra.putIfAbsent('session_generation', () => _sessionGeneration);
      if (!options.path.startsWith('/auth/')) {
        final token = await _storage.read('access_token');
        if (options.extra['session_generation'] != _sessionGeneration) {
          return handler.reject(DioException(
              requestOptions: options, type: DioExceptionType.cancel));
        }
        if (token != null) options.headers['Authorization'] = 'Bearer $token';
      }
      handler.next(options);
    }, onError: (error, handler) async {
      final options = error.requestOptions;
      if (options.extra['client_incident'] == true) return handler.next(error);
      if (options.extra['session_generation'] != _sessionGeneration) {
        return handler.next(error);
      }
      if (error.response?.statusCode != 401 ||
          options.path.startsWith('/auth/')) {
        return handler.next(error);
      }
      if (options.extra['retried'] == true) {
        await expireSession();
        return handler.next(error);
      }
      try {
        await refreshSession();
        final token = await read('access_token');
        if (options.extra['session_generation'] != _sessionGeneration) {
          return handler.next(error);
        }
        if (token == null) return handler.next(error);
        if (options.method != 'GET' && options.method != 'HEAD') {
          return handler.reject(DioException(
              requestOptions: options,
              type: DioExceptionType.badResponse,
              response:
                  Response(requestOptions: options, statusCode: 409, data: {
                'error': {
                  'message':
                      'Your session was renewed. Please retry this action.'
                }
              })));
        }
        options.headers['Authorization'] = 'Bearer $token';
        options.extra['retried'] = true;
        handler.resolve(await _dio.fetch<dynamic>(options));
      } catch (_) {
        handler.next(error);
      }
    }));
  }
  final Dio _dio;
  late final Dio _refreshDio;
  final TokenStore _storage;
  Uri resolveServerUrl(String value) =>
      Uri.parse(_dio.options.baseUrl).resolve(value);
  Future<void>? _refresh;
  int _sessionGeneration = 0;
  Future<void> _storageWrites = Future<void>.value();

  Future<void> _serializeSessionStorage(Future<void> Function() operation) {
    final result = Completer<void>();
    _storageWrites = _storageWrites.then((_) async {
      try {
        await operation();
        result.complete();
      } catch (error, stack) {
        result.completeError(error, stack);
      }
    });
    return result.future;
  }

  void Function()? onSessionExpired;
  void Function(Object, StackTrace)? onUnexpectedError;

  void _observeUnexpectedTransport(DioException error) {
    // A JSON decoder failure is a contract incident, not an offline response.
    // Reporting errors must never escape into this clinical transport.
    if (error.error is! FormatException ||
        error.requestOptions.extra['client_incident'] == true ||
        (error.response?.statusCode != null &&
            error.response!.statusCode! >= 400 &&
            error.response!.statusCode! < 500)) {
      return;
    }
    try {
      onUnexpectedError?.call(error.error!, error.stackTrace);
    } catch (_) {}
  }

  Future<void> refreshSession() async {
    final pending = _refresh;
    if (pending != null) return pending;
    final operation = _rotate();
    _refresh = operation;
    try {
      await operation;
    } finally {
      if (identical(_refresh, operation)) _refresh = null;
    }
  }

  Future<void> _rotate() async {
    final generation = _sessionGeneration;
    try {
      final refresh = await read('refresh_token');
      if (refresh == null) throw const ApiException(401, '');
      final response = await _refreshDio
          .post<dynamic>('/auth/refresh', data: {'refresh_token': refresh});
      final data = response.data['data'] as Map;
      await _serializeSessionStorage(() async {
        if (_sessionGeneration != generation) {
          throw const ApiException(401, 'The account session changed.');
        }
        await _storage.write('access_token', data['access_token'] as String);
        await _storage.write('refresh_token', data['refresh_token'] as String);
      });
    } on DioException catch (e) {
      if (generation == _sessionGeneration) _observeUnexpectedTransport(e);
      if (generation == _sessionGeneration &&
          (e.response?.statusCode == 401 || e.response?.statusCode == 403)) {
        await expireSession();
      }
      throw _mapError(e);
    } on ApiException {
      if (generation == _sessionGeneration) await expireSession();
      rethrow;
    }
  }

  Future<Response<dynamic>> get(String path, {Map<String, dynamic>? query}) =>
      _request((options) =>
          _dio.get(path, queryParameters: query, options: options));

  /// [idempotent]: attaches a fresh `Idempotency-Key` header for this one
  /// logical call. Generated once per `post`/`put` invocation (not per raw
  /// HTTP attempt), so two separate user-initiated calls (e.g. two taps that
  /// slipped past a busy-guard) correctly get two different keys — only the
  /// *same* call's own internal 401-refresh-then-retry in the interceptor
  /// above reuses the same `Options`/headers, which is the one case where
  /// replaying the original response (rather than re-executing a
  /// non-idempotent mutation) is actually wanted. Pass this for genuinely
  /// consequential, not-already-structurally-safe mutations (schedule slot
  /// creation, chat send, prescription upload, consultation completion);
  /// skip it for naturally idempotent "set to this value" calls (duty,
  /// profile, payout method) and reads.
  Future<Response<dynamic>> post(String path,
          {Object? data,
          ProgressCallback? onSendProgress,
          bool idempotent = false}) =>
      _request(
          (options) => _dio.post(path,
              data: data, onSendProgress: onSendProgress, options: options),
          idempotencyKey: idempotent ? _generateIdempotencyKey() : null);
  Future<Response<dynamic>> put(String path,
          {Object? data, bool idempotent = false}) =>
      _request((options) => _dio.put(path, data: data, options: options),
          idempotencyKey: idempotent ? _generateIdempotencyKey() : null);
  Future<Response<dynamic>> patch(String path, {Object? data}) =>
      _request((options) => _dio.patch(path, data: data, options: options));
  Future<Response<dynamic>> delete(String path) =>
      _request((options) => _dio.delete(path, options: options));
  Future<Response<dynamic>> _request(
      Future<Response<dynamic>> Function(Options options) call,
      {String? idempotencyKey}) async {
    final generation = _sessionGeneration;
    try {
      final response = await call(Options(
          extra: {'session_generation': generation},
          headers: idempotencyKey != null
              ? {'Idempotency-Key': idempotencyKey}
              : null));
      if (_sessionGeneration != generation) {
        throw const ApiException(401, 'The account session changed.');
      }
      return response;
    } on DioException catch (e) {
      if (generation == _sessionGeneration) _observeUnexpectedTransport(e);
      throw _mapError(e);
    }
  }

  ApiException _mapError(DioException e) {
    final body = e.response?.data;
    final message = body is Map && body['error'] is Map
        ? (body['error']['message'] ?? 'Request failed').toString()
        : switch (e.type) {
            DioExceptionType.connectionTimeout ||
            DioExceptionType.receiveTimeout ||
            DioExceptionType.sendTimeout =>
              'The request timed out. Please retry.',
            DioExceptionType.connectionError =>
              'You appear to be offline. Check your connection and retry.',
            DioExceptionType.cancel => 'The request was cancelled.',
            _ => 'Unable to reach HelloDoctor. Check your connection and retry.'
          };
    final requestId = body is Map && body['meta'] is Map
        ? body['meta']['request_id']?.toString()
        : null;
    return ApiException(e.response?.statusCode, message, requestId: requestId);
  }

  Future<void> saveTokens(String access, String refresh) async {
    final generation = ++_sessionGeneration;
    await _serializeSessionStorage(() async {
      if (generation != _sessionGeneration) return;
      await _storage.write('access_token', access);
      await _storage.write('refresh_token', refresh);
    });
  }

  Future<String?> read(String key) => _storage.read(key);

  /// Uses the current session snapshot without refreshing, expiring it or
  /// replaying a mutation when best-effort diagnostics fail.
  Future<bool> sendClientIncident(Map<String, dynamic> payload) async {
    final generation = _sessionGeneration;
    try {
      final token = await read('access_token');
      if (token == null || generation != _sessionGeneration) return false;
      final result = await _dio.post('/client-errors',
          data: payload,
          options: Options(
              headers: {
                'Authorization': 'Bearer $token'
              },
              extra: {
                'client_incident': true,
                'session_generation': generation
              },
              sendTimeout: const Duration(seconds: 2),
              receiveTimeout: const Duration(seconds: 3)));
      return result.statusCode == 202;
    } catch (_) {
      return false;
    }
  }

  Future<void> clearSession() async {
    ++_sessionGeneration;
    _refresh = null;
    await _serializeSessionStorage(_storage.clear);
  }

  Future<void> expireSession() async {
    await clearSession();
    onSessionExpired?.call();
  }

  void dispose() {
    onUnexpectedError = null;
    _dio.close();
    _refreshDio.close();
  }
}
