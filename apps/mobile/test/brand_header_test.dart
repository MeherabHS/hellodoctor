import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/widgets/afya_auth_widgets.dart';

void main() {
  testWidgets('the splash brand reads "AfyaMkononi Care", not "Doctor"',
      (tester) async {
    await tester.pumpWidget(const MaterialApp(
        home: Scaffold(body: Center(child: AfyaBrandHeader()))));
    expect(find.text('AfyaMkononi'), findsOneWidget);
    expect(find.text('Care'), findsOneWidget);
    expect(find.textContaining('Doctor'), findsNothing);
  });
}
