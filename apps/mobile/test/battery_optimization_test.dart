import 'package:flutter/material.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/notifications/battery_optimization.dart';

/// The once-per-install battery-optimisation guidance (2026-11-18).
void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final opened = <String>[];
  var ignoring = false;
  var manufacturer = 'TECNO';

  setUp(() {
    opened.clear();
    ignoring = false;
    manufacturer = 'TECNO';
    FlutterSecureStorage.setMockInitialValues({});
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(BatteryOptimization.channel, (call) async {
      switch (call.method) {
        case 'status':
          return {'ignoring': ignoring, 'manufacturer': manufacturer};
        case 'openSettings':
          opened.add('settings');
          return true;
      }
      return null;
    });
  });

  tearDown(() => TestDefaultBinaryMessengerBinding
      .instance.defaultBinaryMessenger
      .setMockMethodCallHandler(BatteryOptimization.channel, null));

  Future<BuildContext> host(WidgetTester tester) async {
    late BuildContext context;
    await tester.pumpWidget(MaterialApp(home: Builder(builder: (c) {
      context = c;
      return const Scaffold();
    })));
    return context;
  }

  test('phone brands map to their own steps', () {
    expect(phoneMakerOf('TECNO'), PhoneMaker.transsion);
    expect(phoneMakerOf('INFINIX MOBILITY LIMITED'), PhoneMaker.transsion);
    expect(phoneMakerOf('itel'), PhoneMaker.transsion);
    expect(phoneMakerOf('Xiaomi'), PhoneMaker.xiaomi);
    expect(phoneMakerOf('samsung'), PhoneMaker.samsung);
    expect(phoneMakerOf('OPPO'), PhoneMaker.oppo);
    expect(phoneMakerOf('realme'), PhoneMaker.oppo);
    expect(phoneMakerOf('vivo'), PhoneMaker.vivo);
    expect(phoneMakerOf('HONOR'), PhoneMaker.huawei);
    expect(phoneMakerOf('Google'), PhoneMaker.other);
    for (final maker in PhoneMaker.values) {
      expect(batteryStepsFor(null, maker), contains('Open settings'));
    }
  });

  testWidgets('a restricted phone sees the guidance once, with its brand steps',
      (tester) async {
    final context = await host(tester);
    final first = BatteryOptimization.maybePrompt(context);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('battery-optimization-dialog')),
        findsOneWidget);
    expect(find.textContaining('Phone Master'), findsOneWidget);
    await tester.tap(find.text('Open settings'));
    await tester.pumpAndSettle();
    await first;
    expect(opened, ['settings']);

    // Never again on this install, whatever the user chose.
    final again = BatteryOptimization.maybePrompt(context);
    await tester.pumpAndSettle();
    await again;
    expect(find.byKey(const ValueKey('battery-optimization-dialog')),
        findsNothing);
  });

  testWidgets('Not now closes it without opening settings', (tester) async {
    final context = await host(tester);
    final prompt = BatteryOptimization.maybePrompt(context);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Not now'));
    await tester.pumpAndSettle();
    await prompt;
    expect(opened, isEmpty);
  });

  testWidgets('an unrestricted phone is never asked', (tester) async {
    ignoring = true;
    final context = await host(tester);
    final prompt = BatteryOptimization.maybePrompt(context);
    await tester.pumpAndSettle();
    await prompt;
    expect(find.byKey(const ValueKey('battery-optimization-dialog')),
        findsNothing);
  });
}
