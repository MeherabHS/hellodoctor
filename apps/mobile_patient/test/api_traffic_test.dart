import 'dart:async';
import 'dart:math' as math;

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/network/screen_refresh.dart';
import 'package:hellodoctor_mobile_patient/core/network/connectivity_service.dart';

class MemoryTokens implements TokenStore {
  final values = <String, String>{};
  @override
  Future<String?> read(String key) async => values[key];
  @override
  Future<void> write(String key, String value) async {
    values[key] = value;
  }

  @override
  Future<void> delete(String key) async {
    values.remove(key);
  }

  @override
  Future<void> clear() async {
    values.clear();
  }
}

class RefreshHost extends StatefulWidget {
  const RefreshHost(this.refresh, {super.key});
  final ScreenRefresh refresh;
  @override
  State<RefreshHost> createState() => _RefreshHostState();
}

class _RefreshHostState extends State<RefreshHost> {
  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    widget.refresh.attach(context);
  }

  @override
  void dispose() {
    widget.refresh.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
        body: TextButton(
          onPressed: () => Navigator.push(
            context,
            MaterialPageRoute<void>(
              builder: (_) => const Scaffold(body: Text('Covered')),
            ),
          ),
          child: const Text('Cover'),
        ),
      );
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test(
    'equivalent GETs share only the in-flight transport, then refetch',
    () async {
      final dio = Dio();
      final gate = Completer<void>();
      final started = Completer<void>();
      var calls = 0;
      dio.interceptors.add(
        InterceptorsWrapper(
          onRequest: (o, h) async {
            calls++;
            if (!started.isCompleted) started.complete();
            await gate.future;
            h.resolve(Response(requestOptions: o, data: {'value': calls}));
          },
        ),
      );
      final api = ApiClient(dio: dio, tokenStore: MemoryTokens());
      final a = api.get('/protected', query: {'offset': 0, 'limit': 25});
      final b = api.get('/protected', query: {'limit': 25, 'offset': 0});
      expect(identical(a, b), isTrue);
      await started.future;
      expect(calls, 1);
      gate.complete();
      expect(identical(await a, await b), isTrue);
      await api.get('/protected', query: {'limit': 25, 'offset': 0});
      expect(calls, 2);
      api.dispose();
    },
  );
  test('different resources and queries are not coalesced', () async {
    final dio = Dio();
    var calls = 0;
    dio.interceptors.add(
      InterceptorsWrapper(
        onRequest: (o, h) {
          calls++;
          h.resolve(Response(requestOptions: o, data: {}));
        },
      ),
    );
    final api = ApiClient(dio: dio, tokenStore: MemoryTokens());
    await Future.wait([
      api.get('/a', query: {'offset': 0}),
      api.get('/a', query: {'offset': 25}),
      api.get('/b'),
    ]);
    expect(calls, 3);
    api.dispose();
  });
  test(
    'failed GET is not cached and a later authorized retry reaches server',
    () async {
      final dio = Dio();
      var calls = 0;
      dio.interceptors.add(
        InterceptorsWrapper(
          onRequest: (o, h) {
            calls++;
            if (calls == 1) {
              h.reject(
                DioException(
                  requestOptions: o,
                  type: DioExceptionType.connectionError,
                ),
              );
            } else {
              h.resolve(Response(requestOptions: o, data: {}));
            }
          },
        ),
      );
      final api = ApiClient(dio: dio, tokenStore: MemoryTokens());
      await expectLater(api.get('/a'), throwsA(isA<ApiException>()));
      await api.get('/a');
      expect(calls, 2);
      api.dispose();
    },
  );
  test('account change never joins a previous account in-flight GET', () async {
    final dio = Dio();
    final gate = Completer<void>();
    final started = Completer<void>();
    var calls = 0;
    dio.interceptors.add(
      InterceptorsWrapper(
        onRequest: (o, h) async {
          calls++;
          if (!started.isCompleted) started.complete();
          if (calls == 1) await gate.future;
          h.resolve(Response(requestOptions: o, data: {}));
        },
      ),
    );
    final api = ApiClient(dio: dio, tokenStore: MemoryTokens());
    final old = api.get('/protected');
    final denied = expectLater(old, throwsA(isA<ApiException>()));
    await started.future;
    await api.clearSession();
    await api.get('/protected');
    expect(calls, 2);
    gate.complete();
    await denied;
    api.dispose();
  });
  testWidgets(
    'screen timer stops under another route and in background, resumes once',
    (tester) async {
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
      var calls = 0;
      final refresh = ScreenRefresh(() async {
        calls++;
      }, interval: const Duration(seconds: 10));
      await tester.pumpWidget(
        MaterialApp(
          navigatorObservers: [screenRouteObserver],
          home: RefreshHost(refresh),
        ),
      );
      await tester.pump(const Duration(seconds: 30));
      expect(calls, 3);
      await tester.tap(find.text('Cover'));
      await tester.pumpAndSettle();
      final covered = calls;
      await tester.pump(const Duration(seconds: 30));
      expect(calls, covered);
      tester.state<NavigatorState>(find.byType(Navigator)).pop();
      await tester.pumpAndSettle();
      await tester.pump(const Duration(seconds: 2));
      expect(calls, covered + 1);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
      await tester.pump(const Duration(seconds: 60));
      expect(calls, covered + 1);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
      await tester.pump();
      await tester.pump(const Duration(seconds: 2));
      expect(calls, covered + 2);
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump(const Duration(seconds: 60));
      expect(calls, covered + 2);
    },
  );
  testWidgets('repeated hasError polls back off, jitter stays bounded', (
    tester,
  ) async {
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    var calls = 0;
    var failing = true;
    final refresh = ScreenRefresh(
      () async {
        calls++;
      },
      interval: const Duration(seconds: 10),
      hasError: () => failing,
      random: math.Random(1),
    );
    await tester.pumpWidget(
      MaterialApp(
        navigatorObservers: [screenRouteObserver],
        home: RefreshHost(refresh),
      ),
    );
    // First tick at 10s fails (hasError==true) -> backoff doubles each time
    // instead of ticking again at a fixed 10s cadence.
    await tester.pump(const Duration(seconds: 10));
    expect(calls, 1);
    // Fixed-interval behavior would have produced a 2nd call by t=20s;
    // backoff (>=20s, capped at maxInterval=80s, plus <=20% jitter) must not.
    await tester.pump(const Duration(seconds: 9));
    expect(calls, 1);
    expect(refresh.currentInterval.inSeconds, greaterThanOrEqualTo(20));
    expect(
      refresh.currentInterval,
      lessThanOrEqualTo(const Duration(seconds: 80 + 16)),
    );
    // Let the backoff elapse, then report success -> interval resets to 10s.
    await tester.pump(const Duration(seconds: 80));
    expect(calls, greaterThanOrEqualTo(2));
    failing = false;
    await tester.pump(const Duration(seconds: 90));
    final afterReset = calls;
    expect(refresh.currentInterval, const Duration(seconds: 10));
    await tester.pump(const Duration(seconds: 10));
    expect(calls, afterReset + 1);
  });
  test(
    'concurrent timer, reconnect and explicit refresh share one read',
    () async {
      WidgetsBinding.instance.handleAppLifecycleStateChanged(
        AppLifecycleState.resumed,
      );
      final network = StreamController<NetworkStatus>();
      final gate = Completer<void>();
      var calls = 0;
      final refresh = ScreenRefresh(
        () async {
          calls++;
          await gate.future;
        },
        interval: const Duration(milliseconds: 20),
        networkChanges: network.stream,
      );
      final a = refresh.run(), b = refresh.run();
      network.add(NetworkStatus.online);
      await Future<void>.delayed(const Duration(milliseconds: 70));
      expect(calls, 1);
      expect(identical(a, b), isTrue);
      gate.complete();
      await Future.wait([a, b]);
      await refresh.run();
      refresh.dispose();
      expect(calls, 2);
      await network.close();
    },
  );
}
