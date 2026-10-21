import 'dart:async';
import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/network/client_error_reporter.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/profile_repository.dart';
import 'auth_security_test.dart' show MemoryStore;
import 'package:hellodoctor_mobile_patient/core/network/startup_incident_queue.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  late ApiClient api;
  late Dio dio;
  late ClientErrorReporter reporter;
  late List<RequestOptions> sent;
  late bool signedIn;
  late MemoryStore startupStore;
  late StartupIncidentQueue previousQueue;
  final stack = StackTrace.fromString(
      '#0 load (package:hellodoctor_mobile_patient/features/patient/profile/profile_repository.dart:33:7)');
  setUp(() async {
    sent = [];
    previousQueue = ClientErrorReporter.startupQueue;
    startupStore = MemoryStore();
    ClientErrorReporter.startupQueue =
        StartupIncidentQueue(store: startupStore);
    signedIn = true;
    dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    api = ApiClient(dio: dio, tokenStore: MemoryStore());
    await api.saveTokens('current-patient-token', 'current-refresh');
    dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
      sent.add(r);
      h.resolve(Response(requestOptions: r, statusCode: 202, data: {
        'success': true,
        'data': {'incident_id': 'fixture'}
      }));
    }));
    reporter = ClientErrorReporter(api,
        authenticated: () => signedIn, platform: 'android');
    ClientErrorReporter.current = reporter;
  });
  tearDown(() {
    ClientErrorReporter.current = null;
    ClientErrorReporter.startupQueue = previousQueue;
    api.dispose();
  });

  test('global FlutterError hook invokes reporter once', () async {
    final oldFlutter = FlutterError.onError;
    final oldPlatform = PlatformDispatcher.instance.onError;
    ClientErrorBoundaries.install();
    final error = StateError('synthetic render failure');
    FlutterError.onError!(
        FlutterErrorDetails(exception: error, stack: stack, silent: true));
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(sent.length, 1);
    expect((sent.single.data as Map)['operation'], 'render');
    await reporter.capture(error, stack);
    expect(sent.length, 1);
    FlutterError.onError = oldFlutter;
    PlatformDispatcher.instance.onError = oldPlatform;
  });
  test('zone unhandled async hook uploads', () async {
    final done = Completer<void>();
    runZonedGuarded(() {
      Future<void>.microtask(() => throw StateError('synthetic async'));
    }, (error, trace) {
      ClientErrorBoundaries.asynchronous(error, trace);
      done.complete();
    });
    await done.future;
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(sent.length, 1);
    expect((sent.single.data as Map)['operation'], 'unhandled_async');
  });
  test('PlatformDispatcher hook invokes reporter', () async {
    expect(
        ClientErrorBoundaries.platform(StateError('synthetic'), stack), isTrue);
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(sent.length, 1);
  });
  test('uses current authenticated session without client user identity',
      () async {
    await api.saveTokens('patient-B-token', 'refresh-B');
    expect(await reporter.capture(StateError('synthetic'), stack), isTrue);
    expect(sent.single.headers['Authorization'], 'Bearer patient-B-token');
    expect((sent.single.data as Map).containsKey('impacted_user_id'), isFalse);
  });
  test('pre-auth errors are journaled and cleared sessions cannot upload',
      () async {
    signedIn = false;
    expect(await reporter.capture(StateError('synthetic'), stack), isFalse);
    expect(startupStore.values[StartupIncidentQueue.storageKey], isNotNull);
    signedIn = true;
    await api.clearSession();
    expect(await reporter.capture(StateError('synthetic'), stack), isFalse);
    expect(sent, isEmpty);
  });
  test('normal 409 is not a crash', () async {
    expect(
        await reporter.capture(const ApiException(409, 'slot conflict'), stack),
        isFalse);
    expect(sent, isEmpty);
  });
  test('normal 422 is not a crash', () async {
    expect(
        await reporter.capture(const ApiException(422, 'invalid input'), stack),
        isFalse);
    expect(sent, isEmpty);
  });
  test('offline timeout/cancellation is not a crash', () async {
    for (final error in <Object>[
      const ApiException(null, 'offline'),
      TimeoutException('offline'),
      DioException(
          requestOptions: RequestOptions(path: '/doctor/slots'),
          type: DioExceptionType.connectionError)
    ]) {
      expect(await reporter.capture(error, stack), isFalse);
    }
    expect(sent, isEmpty);
  });
  test('report failure does not recurse, refresh or expire session', () async {
    var expired = false;
    api.onSessionExpired = () => expired = true;
    dio.interceptors.clear();
    dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
      sent.add(r);
      // A reentrant report must not clear the active sender guard.
      unawaited(reporter.capture(StateError('reporter failure'), stack));
      h.reject(DioException(
          requestOptions: r,
          type: DioExceptionType.badResponse,
          response: Response(requestOptions: r, statusCode: 401)));
    }));
    expect(await reporter.capture(StateError('synthetic'), stack), isFalse);
    expect(sent.length, 1);
    expect(expired, isFalse);
    expect(await api.read('access_token'), 'current-patient-token');
  });
  test('secret and PHI prose is omitted before transmission', () async {
    final error = StateError(
        'JWT=secret refresh_token=private OTP=123456 password=private Agora=private patient=Jane Test diagnosis=asthma chat=private account=123456 +254712345678 https://private.test?token=secret');
    final unsafeStack = StackTrace.fromString(
        '$stack\npatient=Jane Test diagnosis=asthma Authorization: Bearer private');
    expect(
        await reporter.capture(error, unsafeStack,
            screen: 'Jane Test', operation: 'diagnosis_asthma'),
        isTrue);
    final payload = sent.single.data.toString();
    for (final value in [
      'private',
      'secret',
      '123456',
      'Jane Test',
      'asthma',
      '+254712345678',
      'https://'
    ]) {
      expect(payload.contains(value), isFalse);
    }
    expect((sent.single.data as Map)['screen'], 'Application');
    expect((sent.single.data as Map)['operation'], 'unhandled_async');
  });
  test('stack is bounded and preserves Dart source frames', () async {
    final raw = List.filled(1000, stack.toString()).join('\n');
    await reporter.capture(StateError('synthetic'), StackTrace.fromString(raw));
    final safe = (sent.single.data as Map)['stack_trace'] as String;
    expect(safe.length, lessThanOrEqualTo(6000));
    expect(safe.split('\n').length, lessThanOrEqualTo(24));
    expect(safe, contains('profile_repository.dart:33:7'));
  });
  test('native installed app version and build are correlated', () async {
    const channel = MethodChannel('hellodoctor_patient/build_metadata');
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel,
            (_) async => {'app_version': '2.3.4', 'build_number': '76'});
    await reporter.loadBuildMetadata();
    await reporter.capture(StateError('synthetic'), stack);
    expect((sent.single.data as Map)['app_version'], '2.3.4');
    expect((sent.single.data as Map)['build_number'], '76');
    expect((sent.single.data as Map)['platform'], 'android');
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });
  test('caught unexpected repository parsing failure is reported', () async {
    dio.interceptors.clear();
    dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
      if (r.path == '/client-errors') {
        sent.add(r);
        h.resolve(Response(requestOptions: r, statusCode: 202));
      } else {
        h.resolve(Response(requestOptions: r, statusCode: 200, data: {
          'data': [42]
        }));
      }
    }));
    await expectLater(
        ProfileRepository(api).profile(), throwsA(isA<FormatException>()));
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(sent.length, 1);
    expect((sent.single.data as Map)['screen'], 'Profile');
  });
  test('Dio JSON decoding failure reports once without leaking response data',
      () async {
    dio.interceptors.clear();
    dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
      if (r.path == '/client-errors') {
        sent.add(r);
        h.resolve(Response(requestOptions: r, statusCode: 202));
      } else {
        h.reject(DioException(
            requestOptions: r,
            error: const FormatException('patient=Jane Test token=private'),
            stackTrace: stack));
      }
    }));
    await expectLater(
        ProfileRepository(api).profile(), throwsA(isA<ApiException>()));
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(sent.length, 1);
    expect((sent.single.data as Map)['operation'], 'response_parse');
    expect(sent.single.data.toString(), isNot(contains('Jane Test')));
    expect(sent.single.data.toString(), isNot(contains('private')));
  });
}
