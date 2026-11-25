import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/app_update_gate.dart';

void main() {
  test('only a known build older than the minimum must update', () {
    expect(appUpdateRequired(41, 42), isTrue);
    expect(appUpdateRequired(42, 42), isFalse);
    expect(appUpdateRequired(50, 42), isFalse);
    // No minimum set, or a build without a number (debug): never blocked.
    expect(appUpdateRequired(41, 0), isFalse);
    expect(appUpdateRequired(0, 42), isFalse);
  });

  testWidgets('the update dialog cannot be dismissed with Back',
      (tester) async {
    await tester.pumpWidget(MaterialApp(
        home: Builder(
            builder: (context) => TextButton(
                onPressed: () => showDialog<void>(
                    context: context,
                    barrierDismissible: false,
                    builder: (_) => const AppUpdateRequiredDialog()),
                child: const Text('open')))));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.text('Update required'), findsOneWidget);
    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(find.text('Update required'), findsOneWidget);
  });
}
