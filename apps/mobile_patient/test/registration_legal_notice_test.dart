import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/legal/legal_links.dart';

void main() {
  testWidgets(
      'patientRegistrationPages: registration notice shows the afyamkononi.app links and opens them',
      (tester) async {
    final opened = <String>[];
    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body: RegistrationLegalNotice(
                text: 'By registering you agree.',
                pages: patientRegistrationPages,
                textColor: Colors.black,
                linkColor: Colors.green,
                onOpen: (page) => opened.add(page.path)))));
    expect(find.text('By registering you agree.'), findsOneWidget);
    for (final url in const [
      'afyamkononi.app/terms',
      'afyamkononi.app/privacy'
    ]) {
      expect(find.text(url), findsOneWidget);
    }
    await tester.tap(find.text(
        const ['afyamkononi.app/terms', 'afyamkononi.app/privacy'].first));
    expect(opened, ['/terms']);
  });
}
