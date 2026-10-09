import 'dart:convert';
import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/services.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/network/client_error_reporter.dart';
import 'package:hellodoctor_mobile/core/network/startup_incident_queue.dart';
import 'auth_security_test.dart' show MemoryStore;

class FailingStartupStore extends MemoryStore {
  @override
  Future<String?> read(String key) async =>
      throw StateError('keystore unavailable');
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final stack = StackTrace.fromString('patient=private JWT=secret\n'
      '#0 boot (package:hellodoctor_mobile/app.dart:80:7)');

  test('unavailable native keystore never makes startup reporting throw',
      () async {
    final queue = StartupIncidentQueue(store: FailingStartupStore());
    final payload = clientIncidentPayload(StateError('fixture'), stack,
        screen: 'Application', operation: 'startup', startup: true);
    await expectLater(queue.enqueue(payload), completes);
    await expectLater(
        queue.flush(authenticated: () => true, send: (_) async => true),
        completes);
  });

  test(
      'failure before API construction persists sanitized reference across restart',
      () async {
    final store = MemoryStore();
    final previous = ClientErrorReporter.startupQueue;
    ClientErrorReporter.startupQueue = StartupIncidentQueue(store: store);
    addTearDown(() => ClientErrorReporter.startupQueue = previous);
    ClientErrorReporter.current = null;
    const channel = MethodChannel('hellodoctor/build_metadata');
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel,
            (_) async => {'app_version': '2.1.0', 'build_number': '17'});
    addTearDown(() => TestDefaultBinaryMessengerBinding
        .instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null));
    await ClientErrorReporter.captureStartup(
        StateError('password=private patient=Jane JWT=secret'), stack);
    final raw = store.values[StartupIncidentQueue.storageKey]!;
    expect(raw, isNot(contains('private')));
    expect(raw, isNot(contains('secret')));
    expect(raw, isNot(contains('Jane')));
    final sent = <Map<String, dynamic>>[];
    final restarted = StartupIncidentQueue(store: store);
    await restarted.flush(
        authenticated: () => true,
        send: (payload) async {
          sent.add(payload);
          return true;
        });
    expect(sent.single['startup_reference'], '784C89E0');
    expect(sent.single['app_version'], '2.1.0');
    expect(sent.single['build_number'], '17');
    expect(sent.single['operation'], 'startup');
    expect(sent.single['stack_trace'], contains('app.dart:80:7'));
    expect(store.values[StartupIncidentQueue.storageKey], isNull);
  });

  test(
      'pre-auth and failed deliveries remain queued until authenticated acceptance',
      () async {
    final store = MemoryStore();
    final queue = StartupIncidentQueue(store: store);
    await queue.enqueue(clientIncidentPayload(StateError('fixture'), stack,
        screen: 'Application', operation: 'startup', startup: true));
    var attempts = 0;
    var signedIn = false;
    var accept = false;
    Future<bool> send(Map<String, dynamic> _) async {
      attempts++;
      return accept;
    }

    await queue.flush(authenticated: () => signedIn, send: send);
    expect(attempts, 0);
    signedIn = true;
    await queue.flush(authenticated: () => signedIn, send: send);
    expect(store.values[StartupIncidentQueue.storageKey], isNotNull);
    accept = true;
    await queue.flush(authenticated: () => signedIn, send: send);
    await queue.flush(authenticated: () => signedIn, send: send);
    expect(attempts, 2);
    expect(store.values[StartupIncidentQueue.storageKey], isNull);
  });

  test('queue bounds, retry deduplication and expiry are enforced', () async {
    final store = MemoryStore();
    var now = DateTime.now().toUtc().add(const Duration(minutes: 1));
    final queue = StartupIncidentQueue(store: store, now: () => now);
    for (var i = 0; i < 12; i++) {
      final payload = clientIncidentPayload(StateError('fixture'), stack,
          screen: 'Application',
          operation: 'startup',
          startup: true,
          buildNumber: '$i');
      await queue.enqueue(payload);
      await queue.enqueue(payload);
    }
    expect(
        (jsonDecode(store.values[StartupIncidentQueue.storageKey]!) as List)
            .length,
        StartupIncidentQueue.maxEntries);
    now = now.add(const Duration(days: 8));
    var sent = false;
    await queue.flush(
        authenticated: () => true,
        send: (_) async {
          sent = true;
          return true;
        });
    expect(sent, isFalse);
    expect(store.values[StartupIncidentQueue.storageKey], isNull);
  });

  test(
      'queued startup uses real session transport without refresh on reporting failure',
      () async {
    final store = MemoryStore();
    final queue = StartupIncidentQueue(store: MemoryStore());
    await queue.enqueue(clientIncidentPayload(StateError('fixture'), stack,
        screen: 'Application', operation: 'startup', startup: true));
    final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    final api = ApiClient(dio: dio, tokenStore: store);
    addTearDown(api.dispose);
    await api.saveTokens('current-doctor', 'refresh');
    var requests = 0;
    var accept = false;
    dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
      requests++;
      expect(r.path, '/client-errors');
      expect(r.headers['Authorization'], 'Bearer current-doctor');
      expect((r.data as Map).containsKey('impacted_user_id'), isFalse);
      h.resolve(Response(requestOptions: r, statusCode: accept ? 202 : 503));
    }));
    await queue.flush(authenticated: () => true, send: api.sendClientIncident);
    expect(await api.read('access_token'), 'current-doctor');
    accept = true;
    await Future.wait(List.generate(
        3,
        (_) => queue.flush(
            authenticated: () => true, send: api.sendClientIncident)));
    expect(requests, 2);
  });
}
