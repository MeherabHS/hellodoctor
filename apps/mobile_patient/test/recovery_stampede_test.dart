import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/connectivity_service.dart';
import 'package:hellodoctor_mobile_patient/core/network/screen_refresh.dart';

class RecoveryRandom implements math.Random {
  RecoveryRandom(this.value);
  final int value;
  @override
  int nextInt(int max) => value % max;
  @override
  double nextDouble() => value / 1751;
  @override
  bool nextBool() => value.isEven;
}

void main() {
  testWidgets('1000 clients spread recovery and coalesce event bursts', (
    tester,
  ) async {
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    final network = StreamController<NetworkStatus>.broadcast();
    final counts = List<int>.filled(1000, 0);
    final clients = List.generate(
      1000,
      (index) => ScreenRefresh(
        () {
          counts[index]++;
        },
        interval: const Duration(minutes: 1),
        random: RecoveryRandom(index),
        networkChanges: network.stream,
      ),
    );
    for (final client in clients) {
      client.didChangeAppLifecycleState(AppLifecycleState.resumed);
      client.didPopNext();
    }
    network.add(NetworkStatus.online);
    await tester.pump();
    int total() => counts.fold(0, (sum, value) => sum + value);
    expect(total(), 0);
    await tester.pump(const Duration(milliseconds: 249));
    expect(total(), 0);
    await tester.pump(const Duration(milliseconds: 1));
    expect(total(), 1);
    await tester.pump(const Duration(milliseconds: 500));
    expect(total(), 501);
    await tester.pump(const Duration(milliseconds: 500));
    expect(total(), 1000);
    expect(counts.every((value) => value == 1), isTrue);
    for (final client in clients) {
      client.dispose();
    }
    network.add(NetworkStatus.online);
    await tester.pump(const Duration(minutes: 2));
    expect(total(), 1000);
    await network.close();
  });

  testWidgets('manual refresh stays immediate and recovery cannot overlap it', (
    tester,
  ) async {
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    final network = StreamController<NetworkStatus>.broadcast();
    final gate = Completer<void>();
    var calls = 0;
    final refresh = ScreenRefresh(
      () async {
        calls++;
        await gate.future;
      },
      interval: const Duration(seconds: 10),
      random: RecoveryRandom(0),
      networkChanges: network.stream,
    );
    final first = refresh.run(), second = refresh.run();
    expect(identical(first, second), isTrue);
    expect(calls, 1);
    network.add(NetworkStatus.online);
    await tester.pump(const Duration(seconds: 2));
    expect(calls, 1);
    refresh.dispose();
    gate.complete();
    await Future.wait([first, second]);
    await tester.pump(const Duration(minutes: 2));
    expect(calls, 1);
    await network.close();
  });

  testWidgets(
    'recovery preserves failure history and disposal cancels recovery',
    (tester) async {
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
      final network = StreamController<NetworkStatus>.broadcast();
      var calls = 0;
      var failing = true;
      final refresh = ScreenRefresh(
        () {
          calls++;
        },
        interval: const Duration(seconds: 10),
        hasError: () => failing,
        random: RecoveryRandom(0),
        networkChanges: network.stream,
      );
      await refresh.run();
      expect(calls, 1);
      expect(refresh.currentInterval, const Duration(seconds: 20));
      network.add(NetworkStatus.online);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 250));
      expect(calls, 2);
      expect(refresh.currentInterval, const Duration(seconds: 40));
      failing = false;
      await refresh.run();
      expect(calls, 3);
      expect(refresh.currentInterval, const Duration(seconds: 10));
      network.add(NetworkStatus.online);
      await tester.pump();
      refresh.dispose();
      await tester.pump(const Duration(seconds: 20));
      expect(calls, 3);
      await network.close();
    },
  );
}
