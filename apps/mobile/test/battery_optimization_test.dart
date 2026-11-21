import 'package:flutter/material.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/notifications/battery_optimization.dart';

/// Two taps (product owner, 2026-11-20): Android's notification dialog,
/// then Android's battery dialog, back to back. Nothing of the app's own,
/// and never a redirect to a Settings page.
void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final calls = <String>[];
  late Map<String, Object?> state;
  var permissionRequests = 0;
  var clock = DateTime.utc(2026, 11, 20, 9);

  setUp(() {
    calls.clear();
    permissionRequests = 0;
    clock = DateTime.utc(2026, 11, 20, 9);
    BatteryOptimization.now = () => clock;
    state = {
      'ignoring': false,
      'manufacturer': 'TECNO',
      'notificationsEnabled': false,
      'permissionGranted': false,
      'showRationale': false,
      'disabledChannels': <String>[],
      'sdk': 34,
    };
    FlutterSecureStorage.setMockInitialValues({});
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(BatteryOptimization.channel, (call) async {
      if (call.method == 'status') return state;
      calls.add(call.method);
      return true;
    });
  });

  tearDown(() => TestDefaultBinaryMessengerBinding
      .instance.defaultBinaryMessenger
      .setMockMethodCallHandler(BatteryOptimization.channel, null));

  Future<bool> grant() async {
    permissionRequests++;
    state['notificationsEnabled'] = true;
    state['permissionGranted'] = true;
    return true;
  }

  Future<BuildContext> host(WidgetTester tester) async {
    late BuildContext context;
    await tester.pumpWidget(MaterialApp(home: Builder(builder: (c) {
      context = c;
      return const Scaffold();
    })));
    return context;
  }

  test('phone brands are recognised', () {
    expect(phoneMakerOf('TECNO'), PhoneMaker.transsion);
    expect(phoneMakerOf('INFINIX MOBILITY LIMITED'), PhoneMaker.transsion);
    expect(phoneMakerOf('Xiaomi'), PhoneMaker.xiaomi);
    expect(phoneMakerOf('samsung'), PhoneMaker.samsung);
    expect(phoneMakerOf('Google'), PhoneMaker.other);
  });

  testWidgets('two system dialogs back to back, nothing of the app own',
      (tester) async {
    final context = await host(tester);
    final flow =
        BatteryOptimization.ensureReady(context, requestPermission: grant);
    await tester.pumpAndSettle();
    await flow;
    expect(find.byType(Dialog), findsNothing);
    expect(find.byType(AlertDialog), findsNothing);
    expect(permissionRequests, 1);
    // Only the in-place battery dialog; no Settings page.
    expect(calls, ['requestIgnore']);
  });

  testWidgets('asked again only after the interval', (tester) async {
    final context = await host(tester);
    Future<bool> refuse() async {
      permissionRequests++;
      state['showRationale'] = true; // refused once: Android asks again
      return false;
    }

    await BatteryOptimization.ensureReady(context, requestPermission: refuse);
    expect((permissionRequests, calls.length), (1, 1));

    clock = clock.add(const Duration(hours: 3));
    await BatteryOptimization.ensureReady(context, requestPermission: refuse);
    expect((permissionRequests, calls.length), (1, 1));

    clock = clock.add(const Duration(hours: 20));
    await BatteryOptimization.ensureReady(context, requestPermission: refuse);
    expect((permissionRequests, calls.length), (2, 2));
  });

  testWidgets(
      'once Android stops showing its notification dialog, only the battery '
      'dialog runs and nothing redirects to Settings', (tester) async {
    FlutterSecureStorage.setMockInitialValues(
        {BatteryOptimization.permissionAskedKey: '1'});
    final context = await host(tester);
    await BatteryOptimization.ensureReady(context, requestPermission: grant);
    expect(permissionRequests, 0);
    expect(calls, ['requestIgnore']);
  });

  testWidgets('a ready phone is never asked', (tester) async {
    state
      ..['ignoring'] = true
      ..['notificationsEnabled'] = true
      ..['permissionGranted'] = true;
    final context = await host(tester);
    await BatteryOptimization.ensureReady(context, requestPermission: grant);
    expect(permissionRequests, 0);
    expect(calls, isEmpty);
  });
}
