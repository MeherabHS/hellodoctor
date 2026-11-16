import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/legal/legal_links.dart';

void main() {
  test('every Care app registration document links to its website page', () {
    expect(legalWebPagesFor('PATIENT_TERMS').map((p) => p.path), ['/terms']);
    expect(legalWebPagesFor('PATIENT_PRIVACY_NOTICE').map((p) => p.path),
        ['/privacy', '/data-retention']);
    expect(legalWebPagesFor('PATIENT_DATA_SHARING_CONSENT').map((p) => p.path),
        ['/patient-consent']);
    expect(legalWebPagesFor('PATIENT_DATA_RIGHTS').map((p) => p.path),
        ['/account-deletion', '/data-retention']);
  });

  test('doctor and peer documents link to their website pages', () {
    expect(legalWebPagesFor('DOCTOR_TERMS').map((p) => p.path),
        ['/doctor-terms']);
    expect(legalWebPagesFor('PRIVACY_NOTICE').map((p) => p.path),
        ['/privacy', '/data-retention']);
    expect(
        legalWebPagesFor('PROFESSIONAL_VERIFICATION_DECLARATION')
            .map((p) => p.path),
        ['/doctor-terms#professional-declaration']);
    expect(legalWebPagesFor('TELEMEDICINE_PRACTITIONER_POLICY').map((p) => p.path),
        ['/doctor-terms#confidentiality']);
    expect(legalWebPagesFor('PEER_PRIVACY_NOTICE').map((p) => p.path),
        ['/peer-navigator-privacy', '/data-retention']);
  });

  test('documents without a public page get no link', () {
    expect(legalWebPagesFor('MARKETING_COMMUNICATIONS'), isEmpty);
    expect(legalWebPagesFor('PEER_TERMS'), isEmpty);
    expect(legalWebPagesFor('SOMETHING_NEW'), isEmpty);
  });

  test('links point at the public website over https', () {
    final page = legalWebPagesFor('PATIENT_PRIVACY_NOTICE').first;
    expect(page.uri.toString(), 'https://afyamkononi.app/privacy');
    expect(page.displayUrl, 'afyamkononi.app/privacy');
  });

  testWidgets('shows afyamkononi.app links and opens the tapped one',
      (tester) async {
    LegalWebPage? opened;
    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body: LegalWebLinks(
                pages: legalWebPagesFor('PATIENT_PRIVACY_NOTICE'),
                color: Colors.green,
                onOpen: (p) => opened = p))));
    expect(find.text('afyamkononi.app/privacy'), findsOneWidget);
    expect(find.text('afyamkononi.app/data-retention'), findsOneWidget);
    await tester.tap(find.text('afyamkononi.app/data-retention'));
    expect(opened?.path, '/data-retention');
  });
}
