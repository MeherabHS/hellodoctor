// Explicit development-only harness. Not a production crash button and not
// part of the default test glob. Credentials arrive in process environment.
import 'dart:async';
import 'dart:io';
import 'dart:convert';
import 'package:dio/dio.dart';
import 'package:dio/io.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/network/client_error_reporter.dart';
import '../auth_security_test.dart' show MemoryStore;

// Flutter widget tests normally replace HttpClient with a fake returning 400.
// This explicitly invoked development harness needs the real loopback network.
class _LiveNetwork extends HttpOverrides {}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test(
      'real Flutter framework exception reaches existing development incident service',
      () async {
    final token = Platform.environment['HD_OBSERVABILITY_DOCTOR_TOKEN'];
    final output = Platform.environment['HD_OBSERVABILITY_RESULT'];
    expect(token, isNotNull);
    expect(output, isNotNull);
    final dio = Dio(BaseOptions(
        baseUrl: 'http://127.0.0.1:8080/api/v1',
        connectTimeout: const Duration(seconds: 2)));
    dio.httpClientAdapter = IOHttpClientAdapter(
        createHttpClient: () => _LiveNetwork().createHttpClient(null));
    final api = ApiClient(dio: dio, tokenStore: MemoryStore());
    await api.saveTokens(token!, 'unused-by-diagnostic-transport');
    final received = Completer<String>();
    dio.interceptors.add(InterceptorsWrapper(onResponse: (response, handler) {
      if (response.requestOptions.path == '/client-errors' &&
          response.statusCode == 202) {
        received.complete(response.data['data']['incident_id'] as String);
      }
      handler.next(response);
    }));
    ClientErrorReporter.current = ClientErrorReporter(api,
        authenticated: () => true, platform: 'windows');
    final old = FlutterError.onError;
    try {
      ClientErrorBoundaries.install();
      // Controlled development fixture: uses real FlutterError hook and real Dio.
      FlutterError.onError!(FlutterErrorDetails(
          exception: StateError(
              'Synthetic development exception; patient=Jane Test token=private'),
          stack: StackTrace.fromString(
              '#0 fixture (package:hellodoctor_mobile/main.dart:15:7)'),
          silent: true));
      final id = await received.future.timeout(const Duration(seconds: 8));
      File(output!).writeAsStringSync(jsonEncode({
        'incident_id': id,
        'framework_delivery': 'PASS',
        'device_delivery': 'NOT_LIVE_VERIFIED'
      }));
    } finally {
      FlutterError.onError = old;
      ClientErrorReporter.current = null;
      api.dispose();
    }
  });
}
