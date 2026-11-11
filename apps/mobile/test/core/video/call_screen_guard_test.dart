import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/video/call_screen_guard.dart';

void main() {
  testWidgets('a rapid double-tap pushes the guarded screen only once',
      (tester) async {
    final guard = CallScreenGuard();
    var buildCount = 0;

    await tester.pumpWidget(MaterialApp(
      home: Builder(
        builder: (context) => Scaffold(
          body: ElevatedButton(
            onPressed: () => guard.push(context,
                (_) => Scaffold(body: Text('call screen ${++buildCount}'))),
            child: const Text('Join'),
          ),
        ),
      ),
    ));

    // Two taps back to back, before the first push's transition settles —
    // exactly the double-tap window the real bug lived in.
    await tester.tap(find.text('Join'));
    await tester.tap(find.text('Join'));
    await tester.pumpAndSettle();

    expect(buildCount, 1);
    expect(find.textContaining('call screen'), findsOneWidget);
  });

  testWidgets('popping the pushed screen allows the next join to go through',
      (tester) async {
    final guard = CallScreenGuard();
    var buildCount = 0;

    await tester.pumpWidget(MaterialApp(
      home: Builder(
        builder: (context) => Scaffold(
          body: ElevatedButton(
            onPressed: () => guard.push(
                context,
                (context) => Scaffold(
                        body: Column(children: [
                      Text('call screen ${++buildCount}'),
                      ElevatedButton(
                          onPressed: () => Navigator.pop(context),
                          child: const Text('Leave')),
                    ]))),
            child: const Text('Join'),
          ),
        ),
      ),
    ));

    await tester.tap(find.text('Join'));
    await tester.pumpAndSettle();
    expect(buildCount, 1);
    expect(guard.busy, isTrue);

    await tester.tap(find.text('Leave'));
    await tester.pumpAndSettle();
    expect(guard.busy, isFalse);

    await tester.tap(find.text('Join'));
    await tester.pumpAndSettle();
    expect(buildCount, 2);
  });
}
